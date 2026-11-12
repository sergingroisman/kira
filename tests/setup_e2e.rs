//! End-to-end tests of `kira setup` (ADR-0025: o wizard que ENSINA) and of
//! the `BoardProvider` seam.
//!
//! No real Azure DevOps, no OS keyring: a std `TcpListener` serves canned
//! Azure DevOps REST responses on 127.0.0.1 (same pattern as the fake Ollama
//! server), the credential store is `credentials::InMemory`, stdin is a fake
//! buffer and the hooks de IA (ollama/copilot/foundry) são esboços — nenhuma
//! chamada de rede ou processo real acontece nos testes.

use std::future::Future;
use std::io::{BufReader, Cursor, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Command, ExitCode};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use kira::board::azure::AdoBoard;
use kira::board::{self, BoardProvider};
use kira::credentials::{self, CRED_KEY_PAT, CredStore};
use kira::error::Error;
use kira::setup::{self, FoundryChoice, IaHooks, SetupOptions};

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

/// Esboço dos hooks de IA: resultados determinísticos + contador de chamadas
/// do Foundry (o teste verifica "sem chamada" quando deve). As células de
/// estado vivem em `Arc<Mutex<_>>` e os hooks guardam clones desses `Arc`s,
/// porque `IaHooks` guarda REFERÊNCIAS — os closures têm que viver tanto
/// quanto o esboço (por isso são campos, não temporários).
struct IaStub {
    /// Saída simulada de `ollama list` (`None` = ollama ausente).
    ollama: Arc<Mutex<Option<String>>>,
    /// Caminho simulado do `copilot` no PATH (`None` = ausente).
    copilot: Arc<Mutex<Option<String>>>,
    /// Resultado simulado da chamada de teste do Foundry.
    foundry: Arc<Mutex<FoundryStub>>,
    foundry_calls: Arc<Mutex<usize>>,
    ollama_hook: Box<dyn Fn() -> Option<String> + Send + Sync>,
    copilot_hook: Box<dyn Fn() -> Option<String> + Send + Sync>,
    foundry_hook: Box<FoundryStubCallFn>,
}

/// Tipo do hook do Foundry no esboço (verboso por causa do `Future` em caixa
/// — o alias mantém `clippy::type_complexity` longe).
type FoundryStubCallFn =
    dyn Fn(&FoundryChoice) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send>> + Send + Sync;

/// Resultado simulado da chamada de teste do Foundry (`Error` não é `Clone`,
/// então o esboço guarda a variante e reconstrói o erro no hook).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FoundryStub {
    Ok,
    Auth401,
}

impl IaStub {
    fn build(ollama: Option<String>, copilot: Option<String>, foundry: FoundryStub) -> Self {
        let ollama_cell = Arc::new(Mutex::new(ollama));
        let ollama_hook = {
            let cell = Arc::clone(&ollama_cell);
            Box::new(move || {
                cell.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone()
            }) as Box<dyn Fn() -> Option<String> + Send + Sync>
        };
        let copilot_cell = Arc::new(Mutex::new(copilot));
        let copilot_hook = {
            let cell = Arc::clone(&copilot_cell);
            Box::new(move || {
                cell.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone()
            }) as Box<dyn Fn() -> Option<String> + Send + Sync>
        };
        let foundry_cell = Arc::new(Mutex::new(foundry));
        let foundry_calls = Arc::new(Mutex::new(0));
        let foundry_hook = {
            let cell = Arc::clone(&foundry_cell);
            let calls = Arc::clone(&foundry_calls);
            Box::new(move |_choice: &FoundryChoice| {
                *calls
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
                let stub = *cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let result = match stub {
                    FoundryStub::Ok => Ok(()),
                    FoundryStub::Auth401 => Err(Error::FoundryAuth { status: 401 }),
                };
                Box::pin(async move { result })
                    as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>
            }) as Box<FoundryStubCallFn>
        };
        Self {
            ollama: ollama_cell,
            copilot: copilot_cell,
            foundry: foundry_cell,
            foundry_calls,
            ollama_hook,
            copilot_hook,
            foundry_hook,
        }
    }

    fn new() -> Self {
        Self::build(
            Some("name\nqwen3.5:4b:1.2 GB\n".to_string()),
            Some("/usr/bin/copilot".to_string()),
            FoundryStub::Ok,
        )
    }

    fn without_ollama() -> Self {
        Self::build(None, None, FoundryStub::Ok)
    }

    fn set_ollama(&self, value: Option<String>) {
        *self
            .ollama
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
    }

    fn set_copilot(&self, value: Option<String>) {
        *self
            .copilot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
    }

    fn set_foundry(&self, value: FoundryStub) {
        *self
            .foundry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
    }

    fn calls(&self) -> usize {
        *self
            .foundry_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn hooks(&self) -> IaHooks<'_> {
        IaHooks {
            ollama_models: self.ollama_hook.as_ref(),
            copilot_bin: self.copilot_hook.as_ref(),
            foundry_call: self.foundry_hook.as_ref(),
        }
    }
}

/// Runs `kira setup` with all inputs faked; returns (exit code, stdout).
#[allow(clippy::too_many_arguments)]
async fn run_setup(
    stdin_text: &str,
    org_flag: Option<String>,
    project_flag: Option<String>,
    env_values: &[(&str, &str)],
    endpoint: Option<String>,
    config_path: Option<&Path>,
    store: &dyn CredStore,
    pat_flag: Option<String>,
    interactive: bool,
    name_flag: Option<String>,
    yes: bool,
    backend: Option<String>,
    skip: &[&str],
    ia: &IaStub,
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
        pat: pat_flag,
        interactive,
        name: name_flag,
        yes,
        backend,
        skip: skip.iter().map(|item| (*item).to_string()).collect(),
        foundry: None,
        hidden_input: false,
    };
    let code = setup::run(
        &options,
        store,
        config_path,
        &env_lookup,
        &mut input,
        &mut output,
        &make_board,
        &ia.hooks(),
    )
    .await;
    (code, String::from_utf8_lossy(&output).to_string())
}

// ---------------------------------------------------------------------------
// Fluxo interativo (TTY): aula do PAT → menu (1 colar / 2 env / 3 pular) →
// validação na hora → nome → backend de IA → resumo final.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_interactive_teaches_menu_validates_and_saves_the_pat() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("interactive-ok");
    let ia = IaStub::new();

    // Menu 1 (colar) + PAT + Enter no nome + Enter no menu de IA.
    let (code, out) = run_setup(
        "1\nAa2xTOKENCOMPLETO\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        true, /* interactive */
        None, /* name */
        false,
        None, /* backend */
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // A aula vem ANTES do menu: o que é, como criar, escopo e valor único.
    assert!(
        out.contains("senha descartável"),
        "deve ensinar o que é um PAT: {out}"
    );
    assert!(
        out.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
        "deve mostrar a URL do PAT: {out}"
    );
    assert!(
        out.contains("Work Items → Read"),
        "deve citar o escopo: {out}"
    );
    assert!(out.contains("30 dias"), "deve citar a validade: {out}");
    assert!(
        out.contains("ele aparece uma única vez"),
        "deve avisar o valor único: {out}"
    );
    // Menu obrigatório com as 3 opções.
    assert!(
        out.contains("Como o KIRA vai falar com o Azure DevOps?"),
        "deve mostrar o menu: {out}"
    );
    assert!(out.contains("1) Colar um PAT agora"), "{out}");
    assert!(
        out.contains("2) Usar AZURE_DEVOPS_PAT do ambiente"),
        "{out}"
    );
    assert!(out.contains("3) Pular por agora"), "{out}");
    // Validação na hora com a mensagem exata de 200.
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "deve validar com a mensagem da spec: {out}"
    );
    // PAT salvo e mascarado; o valor completo NUNCA aparece na saída.
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "PAT deve estar no store"
    );
    assert!(out.contains("Aa2x****"), "confirmação deve mascarar: {out}");
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    // Backend de IA: pré-requisito verificado e custo explicado.
    assert!(
        out.contains("✓ ia: ollama (qwen3.5:4b)"),
        "deve verificar o ollama: {out}"
    );
    assert!(
        out.contains("0 custo, o código não sai da sua máquina"),
        "deve explicar custo/dados: {out}"
    );
    // Resumo final no formato da spec.
    assert!(out.contains("pronto:"), "resumo: {out}");
    assert!(out.contains("próximo passo:  kira scan"), "{out}");
    assert!(
        !out.contains("configuração concluída"),
        "nunca termina com 'configuração concluída': {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_invalid_menu_answers_fall_back_to_the_default() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("menu-invalid");
    let ia = IaStub::new();

    // 'x' duas vezes: aviso em cada resposta inválida e o default '1'
    // (colar) é assumido — o fluxo segue até o prompt de colar.
    let (code, out) = run_setup(
        "x\nx\nAa2xTOKENCOMPLETO\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("resposta inválida ('x')"),
        "deve avisar a resposta inválida: {out}"
    );
    assert!(
        out.contains("assumindo '1' (padrão)"),
        "deve assumir o default: {out}"
    );
    assert!(
        out.contains("Cole o PAT e pressione Enter"),
        "default '1' deve levar ao prompt de colar: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO")
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_menu_option_2_uses_the_env_pat() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("menu-env");
    let ia = IaStub::new();

    // Opção 2: token já está na env — validado, NÃO copiado para o keyring.
    let (code, out) = run_setup(
        "2\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("usando AZURE_DEVOPS_PAT do ambiente"),
        "deve dizer que usou a env: {out}"
    );
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "deve validar: {out}"
    );
    assert!(
        out.contains("da env AZURE_DEVOPS_PAT"),
        "resumo deve apontar a origem: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "token da env não deve ser copiado para o keyring"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_validates_the_pat_already_in_the_keyring_instead_of_skipping() {
    // Wizard manteve o PAT de um setup anterior: o motor tem que VALIDAR o
    // token do keyring, não anunciar "PAT pulado".
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    store
        .set(CRED_KEY_PAT, "Aa2xTOKENCOMPLETO")
        .unwrap_or_else(|err| panic!("set: {err}"));
    let config = temp_config("pat-keyring");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("usando o PAT do keyring do OS"),
        "deve dizer a origem: {out}"
    );
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "o token salvo é validado: {out}"
    );
    assert!(
        out.contains("do keyring do OS — lê Work Items"),
        "resumo com a origem certa: {out}"
    );
    assert!(
        !out.contains("PAT pulado"),
        "não é pulado: existe token salvo: {out}"
    );
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o token nunca aparece: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_menu_skip_leaves_local_mode_and_doctor_teaches_again() {
    let store = credentials::InMemory::new();
    let config = temp_config("menu-skip");
    let ia = IaStub::without_ollama();

    // Opção 3 (pular): sem servidor ADO — nada é validado, nada é salvo;
    // nome e IA seguem com os defaults (nunca bloqueia além do menu).
    let (code, out) = run_setup(
        "3\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("PAT pulado — o KIRA funciona em modo local (kira triage --file)."),
        "deve dizer o que acontece ao pular: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "nenhum PAT deve ser salvo ao pular"
    );
    assert!(
        out.contains("pat: não configurado (modo local)"),
        "resumo deve registrar o modo local: {out}"
    );
    assert!(
        out.contains("pulado (dá para fazer depois):"),
        "resumo deve listar o pulado: {out}"
    );
    assert!(out.contains("pat — kira setup --pat"), "{out}");
    // org/projeto/nome vão para o config mesmo com o PAT pulado — a
    // retomada (`kira setup --pat`) já tem contexto.
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "organization" && v == "minha-org"),
        "config deve ser escrito: {pairs:?}"
    );
    assert!(pairs.iter().any(|(k, v)| k == "dev_name" && v == "Joao"));

    // O `kira doctor` ensina de novo quando o PAT está ausente (binário
    // real, offline — a seção 1 imprime a explicação curta antes do corte).
    let doctor_out = run_doctor(&config, &[]);
    assert!(
        doctor_out.contains("senha descartável"),
        "doctor deve repetir a explicação curta: {doctor_out}"
    );
    assert!(
        doctor_out.contains("Work Items → Read"),
        "doctor deve apontar o escopo: {doctor_out}"
    );
    let _ = std::fs::remove_file(&config);
}

// ---------------------------------------------------------------------------
// Validação imediata: as 4 mensagens exatas (200 / 401 / 403 / sem rede).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_with_401_shows_the_exact_message_with_the_link_and_does_not_save() {
    let (endpoint, _server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#"{"message":"TF400813"}"#);
    let store = credentials::InMemory::new();
    let config = temp_config("unauthorized");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("PATRUIM".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains(
            "PAT não autenticou (401). O token pode ter expirado ou foi copiado incompleto — crie outro em https://dev.azure.com/minha-org/_usersSettings/tokens"
        ),
        "mensagem exata da spec com o link: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "PAT rejeitado NÃO pode ser salvo"
    );
    assert!(!out.contains("pronto:"), "não pode concluir: {out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_with_403_shows_the_exact_message_and_does_not_save() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 403 Forbidden", r#"{}"#);
    let store = credentials::InMemory::new();
    let config = temp_config("forbidden");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("PATSEMESCOPO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains(
            "PAT autenticou, mas sem escopo Work Items (403). Edite o token e marque Work Items → Read"
        ),
        "mensagem exata da spec: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "PAT sem escopo NÃO pode ser salvo"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_network_failure_saves_the_pat_and_continues() {
    // Endpoint recusado (conexão recusada = sem rede): a mensagem de rede
    // aparece, o token É salvo (spec) e o fluxo termina com o resumo.
    let store = credentials::InMemory::new();
    let config = temp_config("no-network");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some("http://127.0.0.1:1".to_string()),
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains(
            "não foi possível validar agora (sem rede?). O token foi salvo; rode kira doctor depois"
        ),
        "mensagem exata da spec: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "sem rede o token é salvo (spec)"
    );
    assert!(
        out.contains("validação pendente — rode kira doctor"),
        "resumo marca a validação pendente: {out}"
    );
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_with_wrong_project_name_does_not_save() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("wrong-project");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Projeto Errado".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
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

// ---------------------------------------------------------------------------
// Não-interativo (--yes / sem TTY): usa a env se existir, senão pula — e
// imprime o pulado. Nunca bloqueia; o caro nunca é default.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_yes_without_env_skips_and_prints_the_skipped() {
    let store = credentials::InMemory::new();
    let config = temp_config("yes-skip");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true, /* --yes */
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("PAT pulado — o KIRA funciona em modo local"),
        "deve imprimir o pulado: {out}"
    );
    assert!(
        out.contains("pat: não configurado (modo local)"),
        "resumo: {out}"
    );
    assert!(out.contains("pat — kira setup --pat"), "{out}");
    assert!(
        !out.contains("Como devo te chamar?"),
        "sem TTY o setup não pode perguntar o nome: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "nada pode ser salvo ao pular"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_yes_uses_the_env_pat_and_validates() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("yes-env");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("usando AZURE_DEVOPS_PAT do ambiente"),
        "deve usar a env: {out}"
    );
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "deve validar na hora: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "token da env não é copiado para o keyring"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_yes_non_interactive_defaults_to_ollama_and_never_to_a_costly_backend() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("yes-ia-default");
    let ia = IaStub::new();

    // --yes sem --backend: o default é o barato (Ollama); copilot/foundry
    // nunca são escolhidos por omissão.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("✓ ia: ollama (qwen3.5:4b)"),
        "default de IA deve ser o ollama: {out}"
    );
    assert!(!out.contains("foundry"), "foundry nunca é default: {out}");
    let _ = std::fs::remove_file(&config);
}

// ---------------------------------------------------------------------------
// `--pat` por CLI: zero aula, zero menu, zero prompt; o valor nunca é ecoado.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_pat_flag_is_fully_non_interactive_and_never_echoes_the_token() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("pat-flag");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("Organização do Azure DevOps (ex."),
        "não deve pedir org: {out}"
    );
    assert!(
        !out.contains("Projeto do Azure DevOps (ex."),
        "não deve pedir projeto: {out}"
    );
    assert!(
        !out.contains("Como o KIRA vai falar"),
        "--pat não mostra o menu: {out}"
    );
    assert!(
        !out.contains("Como criar (2 minutos)"),
        "--pat não imprime a aula: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "PAT da CLI deve estar no store"
    );
    assert!(out.contains("Aa2x****"), "confirmação deve mascarar: {out}");
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "organization" && v == "minha-org"),
        "config.toml deve ser escrito: {pairs:?}"
    );
    assert!(pairs.iter().any(|(k, v)| k == "project" && v == "Kira"));
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_pat_flag_with_401_does_not_save_or_echo_the_token() {
    let (endpoint, _server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#""{"message":"TF400813"}""#);
    let store = credentials::InMemory::new();
    let config = temp_config("pat-flag-401");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("PATRUIMSEGREDO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("PAT não autenticou (401)"),
        "deve explicar o 401: {out}"
    );
    assert!(
        !out.contains("PATRUIMSEGREDO"),
        "o PAT passado por CLI nunca pode aparecer na saída: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "PAT rejeitado NÃO pode ser salvo"
    );
    let _ = std::fs::remove_file(&config);
}

// ---------------------------------------------------------------------------
// Passo de backend de IA: pré-requisito verificado; foundry sem endpoint
// falha SEM chamada; o caro nunca é default.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_foundry_without_endpoint_fails_without_calling() {
    let store = credentials::InMemory::new();
    let config = temp_config("foundry-no-endpoint");
    let ia = IaStub::new();

    // --yes --backend foundry sem as envs do Foundry: erro acionável e
    // ZERO chamadas (a chave/endpoint nunca são perguntados fora de TTY).
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("foundry".to_string()),
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("endpoint do Foundry não informado"),
        "erro acionável: {out}"
    );
    assert!(
        out.contains("KIRA_FOUNDRY_ENDPOINT"),
        "deve apontar a env: {out}"
    );
    assert_eq!(ia.calls(), 0, "sem endpoint nenhuma chamada pode acontecer");
    assert!(!out.contains("pronto:"), "{out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_foundry_reports_401_and_keeps_the_key_hidden() {
    let store = credentials::InMemory::new();
    let config = temp_config("foundry-401");
    let ia = IaStub::new();
    ia.set_foundry(FoundryStub::Auth401);

    // Com endpoint/deployment/chave na env, UMA chamada acontece e o 401 é
    // reportado com a causa; a IA fica pulada (nunca fallback silencioso) e
    // a chave nunca aparece em claro.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
            (
                "KIRA_FOUNDRY_ENDPOINT",
                "https://recurso.openai.azure.com/openai/v1",
            ),
            ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-5"),
            ("KIRA_FOUNDRY_API_KEY", "chave-secreta-foundry"),
        ],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("foundry".to_string()),
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("chave recusada pelo Foundry (HTTP 401)"),
        "deve reportar o 401: {out}"
    );
    assert!(
        out.contains("ia — kira setup --backend ollama"),
        "IA fica pulada com comando de retomada: {out}"
    );
    assert!(
        !out.contains("chave-secreta-foundry"),
        "a chave nunca pode aparecer em claro: {out}"
    );
    assert_eq!(ia.calls(), 1, "exatamente UMA chamada de teste barata");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_foundry_ok_reports_the_deployment_and_reminds_the_env() {
    let store = credentials::InMemory::new();
    let config = temp_config("foundry-ok");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
            (
                "KIRA_FOUNDRY_ENDPOINT",
                "https://recurso.openai.azure.com/openai/v1",
            ),
            ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-5"),
            ("KIRA_FOUNDRY_API_KEY", "chave-secreta-foundry"),
        ],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("foundry".to_string()),
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("✓ ia: foundry (gpt-5)"),
        "deve confirmar o backend: {out}"
    );
    assert!(
        out.contains("custo por token"),
        "deve explicar o custo: {out}"
    );
    assert!(
        out.contains("KIRA_FOUNDRY_API_KEY"),
        "lembrete das envs: {out}"
    );
    assert!(out.contains("  ia:  foundry (gpt-5)"), "resumo: {out}");
    let _ = std::fs::remove_file(&config);
}

/// `kira setup` com os três valores do Foundry já coletados pelo wizard (sem
/// TTY, sem env e sem rede: o passo do PAT é pulado).
fn foundry_form_options(choice: FoundryChoice) -> SetupOptions {
    SetupOptions {
        org: Some("minha-org".to_string()),
        project: Some("Kira".to_string()),
        interactive: false,
        backend: Some("foundry".to_string()),
        foundry: Some(choice),
        ..SetupOptions::default()
    }
}

/// Roda o motor com `SetupOptions` prontos (sem PAT, sem rede).
async fn run_options(
    options: &SetupOptions,
    config_path: &Path,
    store: &dyn CredStore,
    ia: &IaStub,
) -> (ExitCode, String) {
    let make_board = |_: &str, _: &str, _: &str| -> Result<Box<dyn BoardProvider>, Error> {
        panic!("sem PAT nenhum board é consultado")
    };
    let mut input = BufReader::new(Cursor::new(String::new()));
    let mut output: Vec<u8> = Vec::new();
    let code = setup::run(
        options,
        store,
        Some(config_path),
        &|_| None,
        &mut input,
        &mut output,
        &make_board,
        &ia.hooks(),
    )
    .await;
    (code, String::from_utf8_lossy(&output).to_string())
}

fn form_choice() -> FoundryChoice {
    FoundryChoice {
        endpoint: "https://recurso.openai.azure.com/openai/v1".to_string(),
        deployment: "gpt-5".to_string(),
        api_key: "chave-secreta-foundry".to_string(),
    }
}

#[tokio::test]
async fn setup_foundry_form_is_tested_then_saved_to_config_and_keyring() {
    let store = credentials::InMemory::new();
    let config = temp_config("foundry-form-ok");
    let ia = IaStub::new();
    let options = foundry_form_options(form_choice());

    let (code, out) = run_options(&options, &config, &store, &ia).await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert_eq!(ia.calls(), 1, "exatamente UMA chamada de teste barata");
    assert!(
        out.contains("✓ ia: foundry (gpt-5)"),
        "confirma o backend: {out}"
    );
    assert!(
        out.contains("foundry gravado"),
        "diz o que foi gravado: {out}"
    );
    assert!(
        !out.contains("chave-secreta-foundry"),
        "a chave nunca aparece: {out}"
    );

    // Endpoint/deployment são públicos e vão ao config; a chave vai ao keyring.
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "foundry_endpoint"
                && v == "https://recurso.openai.azure.com/openai/v1"),
        "{pairs:?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "foundry_deployment" && v == "gpt-5"),
        "{pairs:?}"
    );
    assert_eq!(
        store
            .get(credentials::CRED_KEY_FOUNDRY_API_KEY)
            .ok()
            .flatten()
            .as_deref(),
        Some("chave-secreta-foundry"),
        "a chave fica no keyring"
    );
    let raw = std::fs::read_to_string(&config).unwrap_or_default();
    assert!(
        !raw.contains("chave-secreta-foundry"),
        "a chave NUNCA vai para o config.toml: {raw}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_foundry_form_with_401_saves_nothing() {
    let store = credentials::InMemory::new();
    let config = temp_config("foundry-form-401");
    let ia = IaStub::new();
    ia.set_foundry(FoundryStub::Auth401);
    let options = foundry_form_options(form_choice());

    let (code, out) = run_options(&options, &config, &store, &ia).await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("chave recusada pelo Foundry (HTTP 401)"),
        "deve reportar o 401: {out}"
    );
    assert_eq!(
        store
            .get(credentials::CRED_KEY_FOUNDRY_API_KEY)
            .ok()
            .flatten(),
        None,
        "chave recusada NÃO pode ser gravada"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        !pairs.iter().any(|(k, _)| k == "foundry_endpoint"),
        "endpoint sem teste aprovado não é gravado: {pairs:?}"
    );
    assert!(
        !out.contains("chave-secreta-foundry"),
        "a chave nunca aparece: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_foundry_backend_uses_config_and_keyring_without_any_env() {
    // Depois do wizard: endpoint/deployment no config, chave no keyring. Um
    // `kira setup --backend foundry` no modo texto tem que achar os três —
    // senão o que o wizard gravou não serve para nada.
    let store = credentials::InMemory::new();
    store
        .set(credentials::CRED_KEY_FOUNDRY_API_KEY, "chave-do-keyring")
        .unwrap_or_else(|err| panic!("set: {err}"));
    let config = temp_config("foundry-saved");
    credentials::write_config_file(
        &config,
        &[
            ("organization", "minha-org"),
            ("project", "Kira"),
            (
                "foundry_endpoint",
                "https://recurso.openai.azure.com/openai/v1",
            ),
            ("foundry_deployment", "gpt-5"),
        ],
    )
    .expect("pré-gravar config");
    let ia = IaStub::new();
    let options = SetupOptions {
        backend: Some("foundry".to_string()),
        interactive: false,
        ..SetupOptions::default()
    };

    let (code, out) = run_options(&options, &config, &store, &ia).await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert_eq!(ia.calls(), 1, "uma chamada de teste com o que está salvo");
    assert!(out.contains("✓ ia: foundry (gpt-5)"), "{out}");
    assert!(
        !out.contains("chave-do-keyring"),
        "a chave do keyring nunca aparece: {out}"
    );
    let raw = std::fs::read_to_string(&config).unwrap_or_default();
    assert!(
        !raw.contains("chave-secreta") && !raw.contains("chave-do-keyring"),
        "config.toml continua sem segredo: {raw}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_reset_removes_the_foundry_key_and_its_public_config() {
    let store = credentials::InMemory::new();
    store
        .set(CRED_KEY_PAT, "Aa2xTOKENCOMPLETO")
        .unwrap_or_else(|err| panic!("set pat: {err}"));
    store
        .set(credentials::CRED_KEY_FOUNDRY_API_KEY, "chave-do-foundry")
        .unwrap_or_else(|err| panic!("set foundry: {err}"));
    let config = temp_config("reset-full");
    credentials::write_config_file(
        &config,
        &[
            ("organization", "minha-org"),
            (
                "foundry_endpoint",
                "https://recurso.openai.azure.com/openai/v1",
            ),
            ("foundry_deployment", "gpt-5"),
        ],
    )
    .expect("pré-gravar config");
    let ia = IaStub::without_ollama();
    let options = SetupOptions {
        reset: true,
        ..SetupOptions::default()
    };
    let make_board =
        |_pat: &str, _org: &str, _project: &str| -> Result<Box<dyn BoardProvider>, Error> {
            panic!("--reset não consulta o board");
        };
    let mut input = BufReader::new(Cursor::new(String::new()));
    let mut output: Vec<u8> = Vec::new();
    let code = setup::run(
        &options,
        &store,
        Some(&config),
        &|_| None,
        &mut input,
        &mut output,
        &make_board,
        &ia.hooks(),
    )
    .await;
    let out = String::from_utf8_lossy(&output).to_string();

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Simetria com o que o wizard grava: PAT, chave do Foundry e o par público.
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    assert_eq!(
        store
            .get(credentials::CRED_KEY_FOUNDRY_API_KEY)
            .ok()
            .flatten(),
        None,
        "a chave do Foundry sai junto"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        !pairs.iter().any(|(k, _)| k == "foundry_endpoint"),
        "{pairs:?}"
    );
    assert!(
        !pairs.iter().any(|(k, _)| k == "foundry_deployment"),
        "{pairs:?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "organization" && v == "minha-org"),
        "org/projeto/dev são identidade do projeto, não credencial: {pairs:?}"
    );
    assert!(out.contains("PAT removido"), "{out}");
    assert!(out.contains("chave do Foundry removida"), "{out}");
    assert!(
        out.contains("endpoint/deployment do Foundry removidos"),
        "{out}"
    );
    assert!(!out.contains("chave-do-foundry"), "nunca o valor: {out}");

    // Idempotente: rodar de novo não é erro e diz que não havia nada.
    let mut output: Vec<u8> = Vec::new();
    let code = setup::run(
        &options,
        &store,
        Some(&config),
        &|_| None,
        &mut input,
        &mut output,
        &make_board,
        &ia.hooks(),
    )
    .await;
    let out = String::from_utf8_lossy(&output).to_string();
    assert_eq!(code, ExitCode::SUCCESS, "{out}");
    assert!(out.contains("não havia chave do Foundry"), "{out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_ollama_without_the_model_shows_the_pull_command_and_skips() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("ollama-no-model");
    let ia = IaStub::new();
    ia.set_ollama(Some("name\noutro-modelo:1b\n".to_string()));

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("qwen3.5:4b não está no ollama — rode: ollama pull qwen3.5:4b (3,3 GB)"),
        "pré-requisito com o comando exato: {out}"
    );
    assert!(
        out.contains("ia — kira setup --backend ollama"),
        "IA fica pulada com retomada: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_copilot_missing_from_path_is_reported_and_skipped() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("copilot-missing");
    let ia = IaStub::new();
    ia.set_copilot(None);

    // Escolha EXPLÍCITA (flag) do copilot: o pré-requisito é verificado e a
    // ausência é reportada — nunca fallback silencioso para o ollama.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("copilot".to_string()),
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("copilot não está no PATH"),
        "deve reportar a ausência: {out}"
    );
    assert!(
        out.contains("política da sua empresa"),
        "deve citar o pré-requisito de política: {out}"
    );
    assert!(
        !out.contains("✓ ia: ollama"),
        "sem fallback silencioso para o ollama: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_unknown_backend_fails_without_silently_defaulting() {
    let store = credentials::InMemory::new();
    let config = temp_config("bad-backend");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("gpt-pro".to_string()),
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("backend desconhecido 'gpt-pro'"),
        "erro acionável: {out}"
    );
    assert!(
        out.contains("use ollama, copilot ou foundry"),
        "deve listar as opções: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_unknown_skip_step_fails_with_actionable_error() {
    let store = credentials::InMemory::new();
    let config = temp_config("bad-skip");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &["fonte"],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("--skip aceita 'pat' ou 'ia'"),
        "erro acionável: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

// ---------------------------------------------------------------------------
// `--reset` (comportamento existente, preservado).
// ---------------------------------------------------------------------------

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
    let ia = IaStub::without_ollama();
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
        &ia.hooks(),
    )
    .await;
    let out = String::from_utf8_lossy(&output).to_string();
    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(out.contains("PAT removido"), "saída: {out}");
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
}

// ---------------------------------------------------------------------------
// Nome do dev (`--name` / pergunta em TTY / padrão do SO) salvo como
// `dev_name` em config.toml e mostrado pelo `kira doctor`.
// ---------------------------------------------------------------------------

/// Runs the REAL binary `kira doctor` with `KIRA_CONFIG` pointing at `config`.
/// The Azure env vars are removed first (o doctor corta offline no PAT
/// ausente, imprimindo a explicação curta — ADR-0025); `extra_envs` repõe as
/// variáveis que o teste quiser (ex.: `AZURE_DEVOPS_PAT` para a máscara).
fn run_doctor(config: &Path, extra_envs: &[(&str, &str)]) -> String {
    // Sandbox próprio: o doctor grava o resumo no store de sessões, e o
    // KIRA_CONFIG aponta para o config gravado pelo setup.
    let sandbox = std::env::temp_dir().join(format!(
        "kira-doctor-e2e-{}-{}",
        std::process::id(),
        config
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let _ = std::fs::create_dir_all(&sandbox);
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .arg("doctor")
        .env("KIRA_CONFIG", config)
        .env("KIRA_STORE_DIR", &sandbox)
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT");
    for (name, value) in extra_envs {
        command.env(name, value);
    }
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar kira doctor: {err}"));
    let _ = std::fs::remove_dir_all(&sandbox);
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[tokio::test]
async fn setup_non_interactive_uses_the_default_dev_name_and_doctor_shows_it() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("dev-name-default");
    let ia = IaStub::new();

    // Sem TTY, sem --name e sem dev_name salvo: o padrão vem do SO via a env
    // injetada (USER=sergin ⇒ "Sergin") — NUNCA há prompt esperando input.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
            ("USER", "sergin"),
        ],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("Como devo te chamar?"),
        "sem TTY o setup não pode perguntar o nome: {out}"
    );
    assert!(
        out.contains("dev: Sergin"),
        "deve registrar o nome derivado do SO: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Sergin"),
        "dev_name deve ir ao config.toml: {pairs:?}"
    );

    let doctor_out = run_doctor(&config, &[]);
    assert!(
        doctor_out.contains("dev: Sergin"),
        "kira doctor deve mostrar o nome salvo: {doctor_out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_valid_pat_shows_masked_in_doctor() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("doctor-masked");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[],
        Some(endpoint),
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    // O doctor do binário real resolve o PAT da env (o keyring do teste não
    // é tocado) e mostra SÓ a máscara.
    assert_eq!(code, ExitCode::SUCCESS, "setup deve salvar o PAT: {out}");
    let doctor_out = run_doctor(&config, &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")]);
    assert!(
        doctor_out.contains("Aa2x****"),
        "doctor deve mostrar o PAT mascarado: {doctor_out}"
    );
    assert!(
        !doctor_out.contains("Aa2xTOKENCOMPLETO"),
        "doctor nunca pode mostrar o PAT completo: {doctor_out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_name_flag_wins_over_the_default_and_the_saved_name() {
    let store = credentials::InMemory::new();
    let config = temp_config("dev-name-flag");
    let ia = IaStub::new();

    // Parte 1: --name vence o padrão do SO (USER=joao ⇒ "Joao" não aparece).
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"), ("USER", "joao")],
        Some(endpoint.clone()),
        Some(&config),
        &store,
        None,
        false,
        Some("Sergin".to_string()),
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(out.contains("dev: Sergin"), "saída: {out}");
    assert!(
        !out.contains("Joao"),
        "o padrão do SO não pode vencer o --name: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Sergin"),
        "config deve gravar exatamente 'Sergin': {pairs:?}"
    );

    // Parte 2: --name também troca um nome JÁ salvo (sem "Olá de novo").
    credentials::write_config_file(&config, &[("dev_name", "Velho")]).expect("pre-gravar");
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        Some("Sergin".to_string()),
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(out.contains("dev: Sergin"), "saída: {out}");
    assert!(
        !out.contains("Olá de novo"),
        "com --name não deve usar o nome salvo: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Sergin"),
        "o nome salvo deve ter sido trocado: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_greets_when_a_name_is_already_saved() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("dev-name-saved");
    let ia = IaStub::new();
    credentials::write_config_file(&config, &[("dev_name", "Sergin")]).expect("pre-gravar");

    // Nome já existe: pergunta não aparece (nem em TTY, nem pipado) — só a
    // saudação; troca fica para o `--name`.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"), ("USER", "joao")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("Olá de novo, Sergin."),
        "deve cumprimentar pelo nome salvo: {out}"
    );
    assert!(
        !out.contains("Como devo te chamar?"),
        "com nome salvo a pergunta não aparece: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Sergin"),
        "o nome salvo deve permanecer: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_prompts_for_the_name_with_repeat_and_eof_fallbacks() {
    let store = credentials::InMemory::new();
    let ia = IaStub::new();

    // TTY: menu 1, colar o PAT, responder o nome por extenso, Enter na IA.
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let config = temp_config("dev-name-interactive");
    let (code, out) = run_setup(
        "1\nAa2xTOKENCOMPLETO\nSerginho\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        Some(endpoint.clone()),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("Como devo te chamar? (Enter = Joao): "),
        "deve perguntar o nome em TTY com o padrão do SO: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "dev_name" && v == "Serginho"),
        "nome digitado deve ser salvo: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);

    // TTY com resposta inválida (41 chars > limite de 40): repete a pergunta
    // (sem erro fatal) e o valor seguinte é aceito.
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let config = temp_config("dev-name-interactive-invalid");
    let (code, out) = run_setup(
        &format!("1\nAa2xTOKENCOMPLETO\n{}\nSerginho\n\n", "x".repeat(41)),
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        Some(endpoint.clone()),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("nome inválido"),
        "deve avisar e repetir a pergunta: {out}"
    );
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "dev_name" && v == "Serginho"),
        "depois da repetição o nome válido entra: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);

    // TTY com Enter na pergunta: padrão do SO; e stdin que fecha (EOF) também
    // cai no padrão — nunca erro fatal.
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let config = temp_config("dev-name-interactive-default");
    let (_code, _out) = run_setup(
        "1\nAa2xTOKENCOMPLETO\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        Some(endpoint.clone()),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Joao"),
        "Enter deve assumir o padrão do SO: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);

    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let config = temp_config("dev-name-interactive-eof");
    let (_code, _out) = run_setup(
        "1\nAa2xTOKENCOMPLETO\n\n\n",
        Some("minha-org".to_string()),
        Some("Kira".to_string()),
        &[("USER", "joao")],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        &ia,
    )
    .await;
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Joao"),
        "EOF deve assumir o padrão do SO: {pairs:?}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_uses_flags_and_env_before_prompting() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("flags");
    let ia = IaStub::new();

    // org comes from --org, project from env, PAT from env: zero prompts.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        None,
        &[
            ("AZURE_DEVOPS_PROJECT", "Kira"),
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
        ],
        Some(endpoint),
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        &ia,
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

// ---------------------------------------------------------------------------
// BoardProvider seam (sem mudanças de comportamento).
// ---------------------------------------------------------------------------

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
