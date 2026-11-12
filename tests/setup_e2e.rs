//! End-to-end tests of `kira setup` (fluxo LOCAL-FIRST, ADR-0030) and of
//! the `BoardProvider` seam.
//!
//! Local-first em uma frase: o setup pergunta nome → projeto (raiz
//! detectada) → backend de IA (4 opções, com "4) nenhuma por agora" como
//! default quando o Ollama não está instalado) → resumo, e NUNCA pede
//! credencial — PAT só entra como atalho explícito `--pat`, validado na hora
//! contra um servidor ADO falso. Nada é conectado implicitamente: env/keyring
//! com PAT NÃO são usados pelo setup (decisão 6 do ADR-0030).
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
use kira::capabilities::{Capability, require_connected};
use kira::credentials::{self, CRED_KEY_PAT, CredStore};
use kira::error::Error;
use kira::setup::{self, ConnectAzureOptions, FoundryChoice, IaHooks, SetupOptions};
use kira::store::Store;

/// Two visible projects for the happy path (usado só pelos testes de `--pat`).
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

/// Raiz de projeto FIXA para os testes (o motor só imprime fora de TTY; o
/// valor não precisa existir a menos que o teste confirme digitando).
fn fake_root(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("kira-root-{name}-{}", std::process::id()))
        .display()
        .to_string()
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
    project_root: Option<String>,
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
        project_root,
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
// Fluxo interativo (TTY): nome → projeto (raiz detectada + confirmar) →
// backend de IA (4 opções) → resumo. NENHUMA pergunta de credencial.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_interactive_local_first_asks_name_project_and_ia_without_pat() {
    let store = credentials::InMemory::new();
    let config = temp_config("local-first-interactive");
    let ia = IaStub::without_ollama();
    let root = fake_root("interativo");

    // Enter no nome (padrão Joao), Enter na confirmação da raiz, "4" = nenhuma.
    let (code, out) = run_setup(
        "\n\n4\n",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        true, /* interactive */
        None, /* name */
        false,
        None, /* backend */
        &[],
        Some(root.clone()),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Nome: perguntado com o padrão do SO.
    assert!(
        out.contains("Como devo te chamar? (Enter = Joao): "),
        "deve perguntar o nome em TTY: {out}"
    );
    // Projeto: raiz DETECTADA mostrada e confirmada.
    assert!(
        out.contains(&format!("Projeto: {root}")),
        "deve mostrar a raiz detectada: {out}"
    );
    assert!(out.contains("É este? (Enter = sim): "), "{out}");
    // IA: as QUATRO opções, e a escolha da 4ª cai em "nenhuma".
    assert!(out.contains("1) Ollama local"), "{out}");
    assert!(out.contains("2) Copilot CLI"), "{out}");
    assert!(out.contains("3) Microsoft Foundry"), "{out}");
    assert!(out.contains("4) nenhuma por agora"), "{out}");
    assert!(
        out.contains(
            "ia: nenhuma por agora — scan, wiki, context, pr draft e dashboard funcionam sem IA."
        ),
        "{out}"
    );
    // Resumo com os três blocos do MVP-2.
    assert!(out.contains("pronto:"), "resumo: {out}");
    assert!(
        out.contains("funciona agora:  scan · wiki · context · pr draft · dashboard"),
        "{out}"
    );
    assert!(
        out.contains("desativado:      judge de item do board · read (board) · publicar PR"),
        "{out}"
    );
    assert!(
        out.contains("ativar depois:   kira connect azure   (leitura de work items)"),
        "{out}"
    );
    assert!(out.contains("próximo passo:   kira scan"), "{out}");
    // O setup local-first NUNCA fala de PAT (nem pergunta, nem ensina).
    assert!(
        !out.contains("PAT"),
        "a palavra PAT não pode aparecer no fluxo interativo: {out}"
    );
    assert!(
        !out.contains("senha descartável"),
        "a aula do PAT saiu do setup: {out}"
    );
    // Nada de credencial no store; o nome vai para o config.
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None, "{out}");
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs.iter().any(|(k, v)| k == "dev_name" && v == "Joao"),
        "{pairs:?}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_typed_project_path_replaces_the_detected_root() {
    let store = credentials::InMemory::new();
    let config = temp_config("raiz-digitada");
    let ia = IaStub::without_ollama();
    let detected = fake_root("detectada");
    let typed = std::env::temp_dir().join(format!("kira-root-existe-{}", std::process::id()));
    std::fs::create_dir_all(&typed).unwrap_or_else(|err| panic!("criar {typed:?}: {err}"));

    // Nome (Enter), raiz DIGITADA que existe, IA "4".
    let stdin = format!("\n{}\n4\n", typed.display());
    let (code, out) = run_setup(
        &stdin,
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        Some(detected),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains(&format!("projeto: {}", typed.display())),
        "a raiz digitada e existente vence: {out}"
    );

    // Caminho digitado que NÃO existe: aviso e mantém a raiz detectada —
    // nunca inventa raiz. Config NOVO (com pergunta de nome): a ordem do
    // stdin é nome → raiz → IA.
    let missing = std::env::temp_dir()
        .join(format!("kira-root-inexistente-{}", std::process::id()))
        .display()
        .to_string();
    let stdin = format!("\n{missing}\n4\n");
    // Config NOVO: a pergunta de nome volta (ordem nome → raiz → IA).
    let config2 = temp_config("raiz-digitada-2");
    let (code, out) = run_setup(
        &stdin,
        None,
        None,
        &[],
        None,
        Some(&config2),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        Some(fake_root("detectada-2")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains(&format!("caminho '{missing}' não existe")),
        "deve avisar: {out}"
    );
    assert!(
        out.contains(&format!("projeto: {}", fake_root("detectada-2"))),
        "mantém a raiz detectada: {out}"
    );
    let _ = std::fs::remove_file(&config2);
    let _ = std::fs::remove_dir_all(&typed);
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_invalid_ia_answers_fall_back_to_the_menu_default() {
    let store = credentials::InMemory::new();
    let config = temp_config("ia-invalid");
    let ia = IaStub::new(); // Ollama PRESENTE ⇒ default do menu é "1".

    // 'x' duas vezes: aviso em cada resposta inválida e o default '1' é
    // assumido — o fluxo segue até o resumo.
    let (code, out) = run_setup(
        "\n\nx\nx\n",
        None,
        None,
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
        Some(fake_root("invalid")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("resposta inválida ('x') — use 1, 2, 3, 4"),
        "deve avisar a resposta inválida: {out}"
    );
    assert!(
        out.contains("assumindo '1' (padrão)"),
        "deve assumir o default: {out}"
    );
    assert!(
        out.contains("✓ ia: ollama (qwen3.5:4b)"),
        "default '1' é o Ollama quando ele existe: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_without_ollama_defaults_to_none() {
    let store = credentials::InMemory::new();
    let config = temp_config("ia-default-none");
    let ia = IaStub::without_ollama();

    // Enter no menu de IA com Ollama ausente: default "4" — o default é o
    // que a máquina TEM, nunca um pré-requisito inexistente.
    let (code, out) = run_setup(
        "\n\n\n",
        None,
        None,
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
        Some(fake_root("default-none")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("ollama não encontrado nesta máquina"),
        "a causa do 'nenhuma' aparece: {out}"
    );
    assert!(out.contains("ia: nenhuma por agora"), "{out}");
    assert!(!out.contains("✓ ia: ollama"), "{out}");
    // Resumo marca ia: nenhuma e traz o comando de ativação.
    assert!(out.contains("  ia:  nenhuma"), "{out}");
    assert!(
        out.contains("                 kira setup --backend ollama"),
        "{out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_interactive_explicit_four_beats_an_ollama_that_is_present() {
    let store = credentials::InMemory::new();
    let config = temp_config("ia-four-explicit");
    let ia = IaStub::new(); // ollama presente, mas o dev escolhe 4.

    let (code, out) = run_setup(
        "\n\n4\n",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        true,
        None,
        false,
        None,
        &[],
        Some(fake_root("four")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Escolha explícita: SEM a dica de "não encontrado" (a causa só vem
    // quando "nenhuma" decorre da ausência do Ollama).
    assert!(!out.contains("ollama não encontrado"), "{out}");
    assert!(out.contains("ia: nenhuma por agora"), "{out}");
    assert!(!out.contains("✓ ia: ollama"), "{out}");
    let _ = std::fs::remove_file(&config);
}

// ---------------------------------------------------------------------------
// Não-interativo (--yes): nunca bloqueia; sem Ollama escolhe "nenhuma" e
// imprime o pulado; NUNCA usa credencial de env/keyring (nada implícito).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_yes_clean_env_prints_the_three_blocks_and_never_mentions_pat() {
    let store = credentials::InMemory::new();
    let config = temp_config("yes-limpo");
    let ia = IaStub::without_ollama();
    let root = fake_root("yes");

    // Env limpo: sem Ollama, sem PAT, sem org/project.
    let (code, out) = run_setup(
        "",
        None,
        None,
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
        Some(root.clone()),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Bloco 1 — pronto.
    assert!(
        out.contains("pronto:\n  dev: Joao\n  ia:  nenhuma\n"),
        "{out}"
    );
    assert!(out.contains("  store: "), "{out}");
    // Projeto: a raiz detectada, impressa e confirmada sem pergunta.
    assert!(
        out.contains(&format!("projeto: {root} (raiz detectada)")),
        "{out}"
    );
    // Bloco 2 — funciona agora / desativado / ativar depois.
    assert!(
        out.contains("funciona agora:  scan · wiki · context · pr draft · dashboard"),
        "{out}"
    );
    assert!(
        out.contains("desativado:      judge de item do board · read (board) · publicar PR"),
        "{out}"
    );
    assert!(
        out.contains("ativar depois:   kira connect azure   (leitura de work items)"),
        "{out}"
    );
    assert!(
        out.contains("                 kira setup --backend ollama"),
        "sem IA, o ativar depois traz o backend: {out}"
    );
    // Bloco 3 — próximo passo.
    assert!(out.contains("próximo passo:   kira scan"), "{out}");
    // O pulado é impresso com a causa (nunca silencioso).
    assert!(out.contains("ollama não encontrado nesta máquina"), "{out}");
    assert!(out.contains("ia: nenhuma por agora"), "{out}");
    // CONTRATO CENTRAL do MVP-2: a palavra PAT NÃO aparece na saída do
    // setup não-interativo.
    assert!(
        !out.contains("PAT"),
        "a palavra PAT não pode aparecer no setup --yes: {out}"
    );
    assert!(
        !out.contains("pat:"),
        "não há mais linha de pat no resumo: {out}"
    );
    // Sem TTY o setup não pergunta nome; e nada de credencial é salvo.
    assert!(!out.contains("Como devo te chamar?"), "{out}");
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_yes_never_uses_a_pat_from_env_or_keyring() {
    // ADR-0030 #6: nada é conectado implicitamente — presença de segredo na
    // env/keyring NÃO é consentimento. Se o board for consultado, o factory
    // abaixo PANICA (nenhum endpoint foi passado): o teste prova que o setup
    // não toca na rede com credencial implícita.
    let config = temp_config("yes-implicito");
    let ia = IaStub::new();

    // (a) PAT na env: ignorado.
    let store = credentials::InMemory::new();
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"), ("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("implicito-env")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("PAT"),
        "env com PAT não é usada nem mencionada pelo setup: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "nada pode ser salvo implicitamente"
    );

    // (b) PAT no keyring de um setup anterior: também ignorado.
    let store = credentials::InMemory::new();
    store
        .set(CRED_KEY_PAT, "Aa2xTOKENCOMPLETO")
        .unwrap_or_else(|err| panic!("set: {err}"));
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("implicito-keyring")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("PAT"),
        "keyring não é usado implicitamente: {out}"
    );
    assert!(!out.contains("Aa2xTOKENCOMPLETO"), "{out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_yes_non_interactive_defaults_to_ollama_and_never_to_a_costly_backend() {
    let store = credentials::InMemory::new();
    let config = temp_config("yes-ia-default");
    let ia = IaStub::new(); // Ollama com o modelo ⇒ default não-interativo.

    // --yes sem --backend: o default é o barato (Ollama); copilot/foundry
    // nunca são escolhidos por omissão.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("yes-ollama")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("✓ ia: ollama (qwen3.5:4b)"),
        "default de IA deve ser o ollama: {out}"
    );
    assert!(!out.contains("foundry"), "foundry nunca é default: {out}");
    // Com IA escolhida, o "ativar depois" fica só com o board.
    assert!(
        !out.contains("kira setup --backend ollama"),
        "a linha do backend só existe quando a IA ficou nenhuma: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_skip_ia_reports_the_skipped_step() {
    let store = credentials::InMemory::new();
    let config = temp_config("skip-ia");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &["ia"],
        Some(fake_root("skip")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(out.contains("IA pulada (--skip)"), "{out}");
    assert!(out.contains("  ia:  nenhuma"), "{out}");
    assert!(ia.calls() == 0, "nenhuma chamada do Foundry: {out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_backend_nenhuma_flag_lands_on_none_without_noise() {
    let store = credentials::InMemory::new();
    let config = temp_config("backend-nenhuma");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("nenhuma".to_string()),
        &[],
        Some(fake_root("nenhuma")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Escolha explícita: SEM a causa de ausência (o dev já disse o que quer).
    assert!(!out.contains("ollama não encontrado"), "{out}");
    assert!(out.contains("ia: nenhuma por agora"), "{out}");
    assert!(out.contains("  ia:  nenhuma"), "{out}");
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
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("foundry".to_string()),
        &[],
        Some(fake_root("foundry-no-endpoint")),
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
    // reportado com a causa; a IA fica em "nenhuma" (nunca fallback
    // silencioso) e a chave nunca aparece em claro.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[
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
        Some(fake_root("foundry-401")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("chave recusada pelo Foundry (HTTP 401)"),
        "deve reportar o 401: {out}"
    );
    assert!(
        out.contains("kira setup --backend ollama"),
        "IA fica em nenhuma, com comando de ativação: {out}"
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
        None,
        None,
        &[
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
        Some(fake_root("foundry-ok")),
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
/// TTY, sem env e sem rede).
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
    .unwrap_or_else(|err| panic!("pré-gravar config: {err}"));
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

// ---------------------------------------------------------------------------
// Atalho EXPLÍCITO `--pat` (ADR-0030 #4): valida na hora, salva, nunca ecoa.
// O resto do fluxo segue local-first — sem aula, sem menu, sem prompt.
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
        Some(fake_root("pat-flag")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("Como devo te chamar?"),
        "fora de TTY não pergunta nome: {out}"
    );
    assert!(
        !out.contains("Organização do Azure DevOps (ex."),
        "não deve pedir org: {out}"
    );
    assert!(
        !out.contains("Como criar (2 minutos)"),
        "--pat não imprime a aula: {out}"
    );
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "valida na hora: {out}"
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
async fn setup_pat_flag_without_org_or_project_fails_with_the_fix() {
    let store = credentials::InMemory::new();
    let config = temp_config("pat-sem-org");
    let ia = IaStub::without_ollama();

    // `--pat` é um atalho que PRECISA do contexto de board: sem org, erro
    // acionável e nada consultado.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        Some(fake_root("pat-sem-org")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("--pat precisa da organização"),
        "erro acionável: {out}"
    );
    assert!(!out.contains("pronto:"), "{out}");

    // E sem projeto: mesma regra.
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        None,
        &[],
        None,
        Some(&config),
        &store,
        Some("Aa2xTOKENCOMPLETO".to_string()),
        false,
        None,
        false,
        None,
        &[],
        Some(fake_root("pat-sem-proj")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("--pat precisa do projeto"),
        "erro acionável: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_pat_flag_with_401_does_not_save_or_echo_the_token() {
    let (endpoint, _server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#"{"message":"TF400813"}"#);
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
        Some(fake_root("pat-401")),
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
// Validação imediata do atalho `--pat`: as 4 mensagens exatas (200 / 401 /
// 403 / sem rede).
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
        Some(fake_root("unauthorized")),
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
        Some(fake_root("forbidden")),
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
    // aparece, o token É salvo (spec) e o fluxo segue até o resumo.
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
        Some(fake_root("no-network")),
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
        out.contains("validação pendente"),
        "o setup marca a validação pendente: {out}"
    );
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    // O fluxo NÃO morre na falha de rede: o resumo local-first aparece.
    assert!(out.contains("próximo passo:   kira scan"), "{out}");
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
        Some(fake_root("wrong")),
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
// Pré-requisitos de IA: ollama sem o modelo, copilot ausente, valores
// desconhecidos — reportados, nunca em fallback silencioso.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_ollama_without_the_model_shows_the_pull_command_and_lands_on_none() {
    let store = credentials::InMemory::new();
    let config = temp_config("ollama-no-model");
    let ia = IaStub::new();
    ia.set_ollama(Some("name\noutro-modelo:1b\n".to_string()));

    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("no-model")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        out.contains("qwen3.5:4b não está no ollama — rode: ollama pull qwen3.5:4b (3,3 GB)"),
        "pré-requisito com o comando exato: {out}"
    );
    assert!(out.contains("Por agora, ia: nenhuma"), "{out}");
    assert!(out.contains("  ia:  nenhuma"), "{out}");
    assert!(
        out.contains("kira setup --backend ollama"),
        "ativação do backend no ativar depois: {out}"
    );
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_copilot_missing_from_path_is_reported_and_skipped() {
    let store = credentials::InMemory::new();
    let config = temp_config("copilot-missing");
    let ia = IaStub::new();
    ia.set_copilot(None);

    // Escolha EXPLÍCITA (flag) do copilot: o pré-requisito é verificado e a
    // ausência é reportada — nunca fallback silencioso para o ollama.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        Some("copilot".to_string()),
        &[],
        Some(fake_root("copilot")),
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
    assert!(out.contains("  ia:  nenhuma"), "{out}");
    let _ = std::fs::remove_file(&config);
}

#[tokio::test]
async fn setup_unknown_backend_fails_without_silently_defaulting() {
    let store = credentials::InMemory::new();
    let config = temp_config("bad-backend");
    let ia = IaStub::new();

    let (code, out) = run_setup(
        "",
        None,
        None,
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
        Some(fake_root("bad-backend")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("backend desconhecido 'gpt-pro'"),
        "erro acionável: {out}"
    );
    assert!(
        out.contains("use ollama, copilot, foundry ou nenhuma"),
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
        None,
        None,
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
        Some(fake_root("bad-skip")),
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
    .unwrap_or_else(|err| panic!("pré-gravar config: {err}"));
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
/// The Azure env vars are removed first; `extra_envs` repõe as variáveis que
/// o teste quiser (ex.: `AZURE_DEVOPS_PAT` para a máscara).
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
    let store = credentials::InMemory::new();
    let config = temp_config("dev-name-default");
    let ia = IaStub::new();

    // Sem TTY, sem --name e sem dev_name salvo: o padrão vem do SO via a env
    // injetada (USER=sergin ⇒ "Sergin") — NUNCA há prompt esperando input.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "sergin")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("dev-default")),
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
async fn setup_then_doctor_shows_capabilities_line_without_the_pat_lesson() {
    // MVP-2 no doctor: linha de capacidades (modo local) e NADA de aula do
    // PAT — a credencial só é assunto de `kira connect`.
    let store = credentials::InMemory::new();
    let config = temp_config("doctor-capacidades");
    let ia = IaStub::without_ollama();

    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("doctor-cap")),
        &ia,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    let doctor_out = run_doctor(&config, &[]);
    assert!(
        doctor_out.contains("capacidades: nenhuma conectada (modo local)"),
        "o doctor deve mostrar o modo local: {doctor_out}"
    );
    assert!(
        !doctor_out.contains("senha descartável"),
        "o doctor NÃO ensina mais o PAT: {doctor_out}"
    );
    assert!(
        !doctor_out.contains("Work Items → Read"),
        "a aula do PAT saiu do doctor: {doctor_out}"
    );
    assert!(
        doctor_out.contains("kira connect azure"),
        "a dica do PAT aponta para o connect: {doctor_out}"
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
        Some(fake_root("doctor-masked")),
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
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        Some("Sergin".to_string()),
        true,
        None,
        &[],
        Some(fake_root("name-flag")),
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
    credentials::write_config_file(&config, &[("dev_name", "Velho")])
        .unwrap_or_else(|err| panic!("pre-gravar: {err}"));
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[],
        None,
        Some(&config),
        &store,
        None,
        false,
        Some("Sergin".to_string()),
        true,
        None,
        &[],
        Some(fake_root("name-flag-2")),
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
    let store = credentials::InMemory::new();
    let config = temp_config("dev-name-saved");
    let ia = IaStub::new();
    credentials::write_config_file(&config, &[("dev_name", "Sergin")])
        .unwrap_or_else(|err| panic!("pre-gravar: {err}"));

    // Nome já existe: pergunta não aparece (nem em TTY, nem pipado) — só a
    // saudação; troca fica para o `--name`.
    let (code, out) = run_setup(
        "",
        None,
        None,
        &[("USER", "joao")],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("greet")),
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
    let ia = IaStub::new(); // ollama presente: o default do menu é "1".

    // TTY: nome por extenso, Enter na raiz, Enter no menu de IA (default 1).
    let config = temp_config("dev-name-interactive");
    let (code, out) = run_setup(
        "Serginho\n\n\n",
        None,
        None,
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
        Some(fake_root("name-tty")),
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
    assert!(
        out.contains("✓ ia: ollama (qwen3.5:4b)"),
        "Enter no menu assume o default barato: {out}"
    );
    let _ = std::fs::remove_file(&config);

    // TTY com resposta inválida (41 chars > limite de 40): repete a pergunta
    // (sem erro fatal) e o valor seguinte é aceito. O nome é o PRIMEIRO
    // passo do fluxo local-first — o stdin começa por ele.
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let config = temp_config("dev-name-interactive-invalid");
    let stdin = format!("{}\nSerginho\n\n4\n", "x".repeat(41));
    let (code, out) = run_setup(
        &stdin,
        None,
        None,
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
        Some(fake_root("name-invalid")),
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
        "\n\n4\n",
        None,
        None,
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
        Some(fake_root("name-default")),
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
        "",
        None,
        None,
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
        Some(fake_root("name-eof")),
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

// ---------------------------------------------------------------------------
// Flags/env/config viram config.toml sem NENHUMA pergunta (org/projeto não
// são mais pedidos — ADR-0030 #3).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn setup_uses_flags_and_env_before_prompting() {
    let store = credentials::InMemory::new();
    let config = temp_config("flags");
    let ia = IaStub::new();

    // org comes from --org, project from env: zero prompts (o PAT da env é
    // IGNORADO — nada é conectado implicitamente).
    let (code, out) = run_setup(
        "",
        Some("minha-org".to_string()),
        None,
        &[
            ("AZURE_DEVOPS_PROJECT", "Kira"),
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
        ],
        None,
        Some(&config),
        &store,
        None,
        false,
        None,
        true,
        None,
        &[],
        Some(fake_root("flags")),
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
    assert!(
        !out.contains("PAT"),
        "o PAT da env não é usado nem mencionado: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "nada é salvo implicitamente"
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

// ---------------------------------------------------------------------------
// `kira connect azure` (MVP-3): TTY (aula + menu 1/2/3 + segredo sem eco),
// não-interativo (só a env) e as 4 mensagens exatas — com servidor ADO falso,
// store InMemory e store de capacidades isolado. O keyring do OS nunca é
// tocado; o binário real é coberto por tests/capabilities_e2e.rs.
// ---------------------------------------------------------------------------

/// Roda `setup::connect_azure` com tudo falso; devolve (exit, saída, store de
/// capacidades, raiz do projeto) para o teste conferir o estado gravado.
#[allow(clippy::too_many_arguments)]
async fn run_connect(
    stdin_text: &str,
    interactive: bool,
    env_values: &[(&str, &str)],
    endpoint: Option<String>,
    config_path: Option<&Path>,
    secret_store: &dyn CredStore,
) -> (ExitCode, String, Store, PathBuf) {
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
    // Store de capacidades + raiz ÚNICOS por chamada: o estado é POR PROJETO.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("kira-connect-db-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = dir.join("repo");
    std::fs::create_dir_all(&root).unwrap_or_else(|err| panic!("criar {root:?}: {err}"));
    let state_store = Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}"));
    let mut input = BufReader::new(Cursor::new(stdin_text.to_string()));
    let mut output: Vec<u8> = Vec::new();
    let options = ConnectAzureOptions {
        interactive,
        hidden_input: false,
        secret_store,
        config_path,
        env: &env_lookup,
        make_board: &make_board,
        project_root: &root,
        state_store: &state_store,
    };
    let code = setup::connect_azure(&options, &mut input, &mut output).await;
    let root_cloned = root.clone();
    (
        code,
        String::from_utf8_lossy(&output).to_string(),
        state_store,
        root_cloned,
    )
}

/// Limpa o diretório temporário do store de capacidades do run_connect.
fn drop_connect_dir(root: &Path) {
    if let Some(dir) = root.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tokio::test]
async fn connect_azure_tty_imprime_a_aula_e_o_menu_e_conecta_sem_eco() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("connect-tty");
    let (code, out, state, root) = run_connect(
        "1\nAa2xTOKENCOMPLETO\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some(endpoint),
        Some(&config),
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    // Aula (fonte única) com a org vinda da env no link.
    assert!(out.contains("Como criar (2 minutos)"), "aula: {out}");
    assert!(
        out.contains("https://dev.azure.com/minha-org/_usersSettings/tokens"),
        "{out}"
    );
    // Menu exato do escopo do MVP-3.
    assert!(out.contains("1) colar PAT"), "{out}");
    assert!(
        out.contains("2) usar AZURE_DEVOPS_PAT do ambiente"),
        "{out}"
    );
    assert!(out.contains("3) cancelar"), "{out}");
    assert!(out.contains("Cole o PAT"), "{out}");
    // Validação na hora + gravação + máscara (o valor completo NUNCA sai).
    assert!(
        out.contains("PAT ok — lê Work Items de minha-org/Kira"),
        "{out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "PAT colado vai para o keyring (store)"
    );
    assert!(out.contains("Aa2x****"), "confirmação mascarada: {out}");
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT completo nunca pode aparecer na saída: {out}"
    );
    assert!(out.contains("board.azure conectada"), "{out}");
    assert!(
        out.contains("kira triage <work-item-id>"),
        "próximo passo: {out}"
    );
    // Capacidade conectada PARA ESTE PROJETO.
    require_connected(&state, &root, Capability::BoardAzure)
        .unwrap_or_else(|err| panic!("board.azure deveria estar conectada: {err}"));
    // org/projeto (públicos) foram ao config.toml no sucesso.
    let pairs = credentials::read_config_file(&config).unwrap_or_default();
    assert!(
        pairs
            .iter()
            .any(|(k, v)| k == "organization" && v == "minha-org")
    );
    assert!(pairs.iter().any(|(k, v)| k == "project" && v == "Kira"));
    let _ = std::fs::remove_file(&config);
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_sem_org_mostra_o_marcador_e_falha_acionavel_sem_tocar_rede() {
    let store = credentials::InMemory::new();
    // Endpoint morto: se o código tentasse validar, o erro viria da rede —
    // o teste espera o erro acionável de org ANTES de qualquer conexão.
    let (code, out, state, root) = run_connect(
        "1\nAa2xTOKENCOMPLETO\n",
        true,
        &[],
        Some("http://127.0.0.1:1".to_string()),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("<sua-org>"),
        "sem org a aula usa o marcador: {out}"
    );
    assert!(
        out.contains("organização do Azure DevOps não configurada"),
        "erro acionável com o comando que resolve: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "sem org NADA é gravado"
    );
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "capacidade continua desconectada"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_cancelar_nada_grava_e_exit_nao_zero() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "3\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some(endpoint),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "cancelar sai ≠ 0: {out}");
    assert!(out.contains("connect cancelado"), "{out}");
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "cancelar não grava PAT"
    );
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "cancelar não conecta a capacidade"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_opcao_2_usa_a_env_sem_eco() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "2\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
        ],
        Some(endpoint),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "PAT da env vai para o keyring (store)"
    );
    assert!(out.contains("Aa2x****"), "{out}");
    assert!(
        !out.contains("Aa2xTOKENCOMPLETO"),
        "o PAT da env nunca pode ser ecoado: {out}"
    );
    require_connected(&state, &root, Capability::BoardAzure)
        .unwrap_or_else(|err| panic!("conectada: {err}"));
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_opcao_2_sem_env_falha_acionavel_sem_perguntar() {
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "2\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some("http://127.0.0.1:1".to_string()),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("AZURE_DEVOPS_PAT não está definida"),
        "erro acionável: {out}"
    );
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "nada conectado"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_401_mostra_a_mensagem_exata_e_nao_salva() {
    let (endpoint, _server) =
        spawn_fake_ado("HTTP/1.1 401 Unauthorized", r#"{"message":"TF400813"}"#);
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "1\nPATRUIMSEGREDO\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some(endpoint),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains(
            "PAT não autenticou (401). O token pode ter expirado ou foi copiado incompleto — crie outro em https://dev.azure.com/minha-org/_usersSettings/tokens"
        ),
        "mensagem exata da spec: {out}"
    );
    assert!(
        !out.contains("PATRUIMSEGREDO"),
        "o PAT nunca pode ser ecoado: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "PAT rejeitado NÃO é salvo"
    );
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "401 não conecta a capacidade"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_403_mostra_a_mensagem_exata_e_nao_salva() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 403 Forbidden", r#"{}"#);
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "1\nPATSEMSCOPO\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some(endpoint),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains(
            "PAT autenticou, mas sem escopo Work Items (403). Edite o token e marque Work Items → Read"
        ),
        "mensagem exata da spec: {out}"
    );
    assert!(!out.contains("PATSEMSCOPO"), "{out}");
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "403 não conecta a capacidade"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_sem_rede_salva_o_pat_mas_nao_conecta_a_capacidade() {
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "1\nAa2xTOKENCOMPLETO\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some("http://127.0.0.1:1".to_string()),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains(
            "não foi possível validar agora (sem rede?). O token foi salvo; rode kira doctor depois"
        ),
        "mensagem exata da spec (origem colada): {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO"),
        "sem rede o token É salvo (spec) — é o que torna a mensagem verdadeira"
    );
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "sem rede a capacidade NÃO é marcada"
    );
    assert!(
        out.contains("board.azure continua desconectada"),
        "o próximo passo fica claro: {out}"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_nao_interativo_com_env_conecta_sem_aula_nem_menu() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let config = temp_config("connect-nao-interativo");
    let (code, out, state, root) = run_connect(
        // stdin VAZIO: o modo não-interativo NUNCA o lê.
        "",
        false,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
            ("AZURE_DEVOPS_PAT", "Aa2xTOKENCOMPLETO"),
        ],
        Some(endpoint),
        Some(&config),
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::SUCCESS, "saída: {out}");
    assert!(
        !out.contains("Como criar (2 minutos)"),
        "fora de TTY não imprime a aula: {out}"
    );
    assert!(
        !out.contains("1) colar PAT"),
        "fora de TTY não oferece o menu: {out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten().as_deref(),
        Some("Aa2xTOKENCOMPLETO")
    );
    assert!(out.contains("board.azure conectada"), "{out}");
    require_connected(&state, &root, Capability::BoardAzure)
        .unwrap_or_else(|err| panic!("conectada: {err}"));
    let _ = std::fs::remove_file(&config);
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_nao_interativo_sem_env_falha_acionavel_sem_ler_stdin() {
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        // stdin VAZIO: se o modo lesse o stdin, cairia no default do menu
        // (cancelar) — o teste distingue isso do erro acionável esperado.
        "",
        false,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "Kira"),
        ],
        Some("http://127.0.0.1:1".to_string()),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("precisa de AZURE_DEVOPS_PAT"),
        "erro acionável aponta a env: {out}"
    );
    assert!(
        !out.contains("cancelado"),
        "o stdin NUNCA é lido fora de TTY: {out}"
    );
    assert_eq!(store.get(CRED_KEY_PAT).ok().flatten(), None);
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "nada conectado"
    );
    drop_connect_dir(&root);
}

#[tokio::test]
async fn connect_azure_projeto_incorreto_nao_salva_nada() {
    let (endpoint, _server) = spawn_fake_ado("HTTP/1.1 200 OK", PROJECTS_BODY);
    let store = credentials::InMemory::new();
    let (code, out, state, root) = run_connect(
        "1\nAa2xTOKENCOMPLETO\n",
        true,
        &[
            ("AZURE_DEVOPS_ORG", "minha-org"),
            ("AZURE_DEVOPS_PROJECT", "ProjetoQueNaoExiste"),
        ],
        Some(endpoint),
        None,
        &store,
    )
    .await;

    assert_eq!(code, ExitCode::FAILURE, "saída: {out}");
    assert!(
        out.contains("projeto incorreto — o token NÃO foi salvo"),
        "{out}"
    );
    assert_eq!(
        store.get(CRED_KEY_PAT).ok().flatten(),
        None,
        "projeto errado: nada é salvo"
    );
    assert!(
        require_connected(&state, &root, Capability::BoardAzure).is_err(),
        "projeto errado não conecta"
    );
    drop_connect_dir(&root);
}
