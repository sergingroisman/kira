//! End-to-end tests of `kira setup` and of the `BoardProvider` seam.
//!
//! No real Azure DevOps, no OS keyring: a std `TcpListener` serves canned
//! Azure DevOps REST responses on 127.0.0.1 (same pattern as the fake Ollama
//! server), and the credential store is `credentials::InMemory`. Stdin is a
//! fake buffer and the browser opener just records the URL.

use std::io::{BufReader, Cursor, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use kira::board::azure::AdoBoard;
use kira::board::{self, BoardProvider};
use kira::credentials::{self, CRED_KEY_PAT, CredStore};
use kira::error::Error;
use kira::setup::{self, SetupOptions};

/// Two visible projects for the happy path.
const PROJECTS_BODY: &str = r#"{"count":2,"value":[{"id":"a","name":"Kira","state":"wellFormed","visibility":"private"},{"id":"b","name":"Outro","state":"wellFormed","visibility":"private"}]}"#;

/// Canned work item in the shape of the Azure DevOps REST API.
const WORK_ITEM_BODY: &str = r#"{"id":4821,"url":"https://dev.azure.com/minha-org/_apis/wit/workItems/4821","fields":{"System.Title":"Sessão expirada com token válido","System.State":"Active","System.WorkItemType":"Bug","System.AreaPath":"Kira\\Backend","System.AssignedTo":{"displayName":"Maria Souza"},"System.Description":"<div>Usuário relata sessão expirada após 5 minutos.</div>"}}"#;

/// Fake Azure DevOps: answers every request with `status_line` + `body`.
///
/// Same pattern as the fake Ollama server: one request served by a std
/// thread on an ephemeral port; returns the base URL plus a handle resolving
/// to the captured raw HTTP request.
fn spawn_fake_ado(
    status_line: &'static str,
    body: &'static str,
) -> (String, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return None;
        };
        let request = read_http_request(&mut stream);
        let response = format!(
            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return None;
        }
        let _ = stream.flush();
        Some(request)
    });
    (format!("http://{addr}"), handle)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Reads until the full HTTP request (headers + content-length body) is in.
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..read]);
        if let Some(headers_end) = find_subsequence(&raw, b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&raw[..headers_end]).to_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if raw.len() >= headers_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&raw).to_string()
}

/// Unique temp config path per test run.
fn temp_config(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("kira-setup-{}-{name}-{nanos}", std::process::id()))
}

/// Runs `kira setup` with all inputs faked; returns (exit code, stdout).
#[allow(clippy::too_many_arguments)]
async fn run_setup(
    stdin_text: &str,
    org_flag: Option<String>,
    project_flag: Option<String>,
    env_values: &[(&str, &str)],
    endpoint: Option<String>,
    config_path: Option<&std::path::Path>,
    store: &dyn CredStore,
    opened: &std::sync::Mutex<Vec<String>>,
) -> (ExitCode, String) {
    let make_board =
        move |pat: &str, org: &str, project: &str| -> Result<Box<dyn BoardProvider>, Error> {
            match &endpoint {
                Some(endpoint) => Ok(Box::new(AdoBoard::with_endpoint(
                    endpoint, pat, org, project,
                ))),
                None => Ok(Box::new(AdoBoard::new(pat, org, project))),
            }
        };
    let open_url = |url: &str| -> Result<(), std::io::Error> {
        opened.lock().expect("lock aberto").push(url.to_string());
        Ok(())
    };
    let env_lookup = move |name: &str| -> Option<String> {
        env_values
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    };
    let mut input = BufReader::new(Cursor::new(stdin_text.to_string()));
    let mut output: Vec<u8> = Vec::new();
    let options = SetupOptions {
        org: org_flag,
        project: project_flag,
        reset: false,
    };
    let code = setup::run(
        &options,
        store,
        config_path,
        &env_lookup,
        &mut input,
        &mut output,
        &make_board,
        &open_url,
    )
    .await;
    (code, String::from_utf8_lossy(&output).to_string())
}

#[tokio::test]
async fn setup_prompts_org_project_validates_and_saves_the_pat() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("ok");
    let opened = std::sync::Mutex::new(Vec::new());

    // Piped stdin: org and project answered as prompts, then the PAT line.
    let stdin_text = "minha-org\nKira\nAa2xTOKENCOMPLETO\n";
    let (code, out) = run_setup(
        stdin_text,
        None,
        None,
        &[],
        Some(endpoint.clone()),
        Some(&config),
        &store,
        &opened,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Prompts were shown, the PAT URL was printed and opened in the browser.
    assert!(
        out.contains("Organização do Azure DevOps"),
        "deve pedir a organização: {out}"
    );
    assert!(
        out.contains("Projeto do Azure DevOps"),
        "deve pedir o projeto: {out}"
    );
    assert!(
        out.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
        "deve mostrar a URL do PAT: {out}"
    );
    assert_eq!(
        *opened
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        vec!["https://dev.azure.com/minha-org/_usersSettings/tokens"]
    );
    assert!(
        out.contains("Work Items (Read)"),
        "deve citar os escopos: {out}"
    );
    // Validation happened against the fake server and the PAT was saved.
    assert!(
        out.contains("Validando"),
        "deve validar antes de salvar: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "PAT deve estar no store"
    );
    // Confirmation masks the secret.
    assert!(
        out.contains("Aa2x****"),
        "confirmação deve mascarar o PAT: {out}"
    );
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    // org/project went to config.toml.
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    let org = pairs
        .iter()
        .find(|(k, _)| k == "organization")
        .map(|(_, v)| v.clone());
    let project = pairs
        .iter()
        .find(|(k, _)| k == "project")
        .map(|(_, v)| v.clone());
    assert_eq!(org.as_deref(), Some("minha-org"));
    assert_eq!(project.as_deref(), Some("Kira"));
    assert!(
        out.contains("setup concluído — rode kira triage <id>"),
        "mensagem final: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_with_401_shows_what_to_review_and_does_not_save() {
    let (endpoint, _server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#"{"message":"TF400813"}"#);
    let store = credentials::InMemory::new();
    let config = temp_config("unauthorized");
    let opened = std::sync::Mutex::new(Vec::new());

    let (code, out) = run_setup(
        "minha-org\nMeuProjeto\nPATRUIM\n",
        None,
        None,
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        &opened,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("PAT não aceito (401)"),
        "deve explicar o 401: {out}"
    );
    assert!(
        out.contains("Work Items (Read)"),
        "deve dizer o que revisar: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "PAT rejeitado NÃO pode ser salvo"
    );
    assert!(!out.contains("setup concluído"), "não pode concluir: {out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_with_wrong_project_name_does_not_save() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("wrong-project");
    let opened = std::sync::Mutex::new(Vec::new());

    let (code, out) = run_setup(
        "minha-org\nProjeto Errado\nAa2xTOKENCOMPLETO\n",
        None,
        None,
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        &opened,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("não encontrado"),
        "deve listar os projetos existentes: {out}"
    );
    assert!(
        out.contains("NÃO foi salvo"),
        "deve dizer que o token não foi salvo: {out}"
    );
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_uses_flags_and_env_before_prompting() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("flags");
    let opened = std::sync::Mutex::new(Vec::new());

    // org comes from --org, project from env; only the PAT is read from stdin.
    let (code, out) = run_setup(
        "Aa2xTOKENCOMPLETO\n",
        Some("minha-org".to_string()),
        None,
        &[("AZURE_DEVOPS_PROJECT", "Kira")],
        Some(endpoint),
        Some(&config),
        &store,
        &opened,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("Organização do Azure DevOps (ex."),
        "não deve pedir org quando veio por flag: {out}"
    );
    assert!(
        !out.contains("Projeto do Azure DevOps (ex."),
        "não deve pedir projeto quando veio do env: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    let org = pairs
        .iter()
        .find(|(k, _)| k == "organization")
        .map(|(_, v)| v.clone());
    assert_eq!(org.as_deref(), Some("minha-org"));
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_reset_removes_the_pat_from_the_store() {
    let store = credentials::InMemory::new();
    store
        .set(CRED_KEY_PAT, "Aa2xTOKENCOMPLETO")
        .unwrap_or_else(|err| panic!("set no store em memória: {err}"));
    let mut output: Vec<u8> = Vec::new();
    let mut input = BufReader::new(Cursor::new(String::new()));
    let options = SetupOptions {
        reset: true,
        ..SetupOptions::default()
    };
    let opened = |_url: &str| Ok::<(), std::io::Error>(());
    let make_board =
        |_pat: &str, _org: &str, _project: &str| -> Result<Box<dyn BoardProvider>, Error> {
            panic!("--reset não deve consultar o board");
        };
    let code = setup::run(
        &options,
        &store,
        None,
        &|_| None,
        &mut input,
        &mut output,
        &make_board,
        &opened,
    )
    .await;
    let out = String::from_utf8_lossy(&output).to_string();
    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(out.contains("PAT removido"), "saída: {out}");
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
}

#[tokio::test]
async fn ado_board_fetches_work_item_and_projects_through_the_trait() {
    let (work_item_endpoint, work_item_server) = spawn_fake_ado("HTTP/1.1 200 OK", WORK_ITEM_BODY);
    let board = AdoBoard::with_endpoint(
        &work_item_endpoint,
        "Aa2xTOKENCOMPLETO",
        "minha-org",
        "Kira",
    );
    let item = board
        .work_item(4821)
        .await
        .unwrap_or_else(|err| panic!("work item via trait contra servidor falso: {err}"));
    assert_eq!(item.id, 4821);
    assert_eq!(item.title, "Sessão expirada com token válido");
    assert_eq!(item.work_item_type, "Bug");
    assert_eq!(item.assigned_to.as_deref(), Some("Maria Souza"));

    let (projects_endpoint, projects_server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let board =
        AdoBoard::with_endpoint(&projects_endpoint, "Aa2xTOKENCOMPLETO", "minha-org", "Kira");
    let names = board
        .project_names()
        .await
        .unwrap_or_else(|err| panic!("project_names via trait contra servidor falso: {err}"));
    assert_eq!(names, vec!["Kira".to_string(), "Outro".to_string()]);
    let _ = work_item_server.join();
    let _ = projects_server.join();
}

#[tokio::test]
async fn ado_board_maps_401_to_an_actionable_error() {
    let (endpoint, server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#"{"message":"TF400813"}"#);
    let board = AdoBoard::with_endpoint(&endpoint, "PATRUIM", "minha-org", "Kira");
    let Err(err) = board.project_names().await else {
        panic!("401 deve virar erro");
    };
    let _ = server.join();
    let message = err.to_string();
    assert!(
        message.contains("PAT inválido"),
        "erro deve ser acionável: {message}"
    );
    assert!(
        message.contains("_usersSettings/tokens"),
        "erro deve apontar a página do token: {message}"
    );
}

#[test]
fn factory_selects_azure_by_default() {
    // No network here: only checks the name → impl selection.
    let board = board::for_board("azure", "pat", "org", "proj");
    assert!(board.is_ok(), "azure deve ser aceito");
}
