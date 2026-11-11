//! End-to-end test of the offline triage path: fixture file + fake Ollama.
//!
//! No real Ollama, no PAT, no external network: a std `TcpListener` serves the
//! canned `/api/chat` response on an ephemeral port.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use kira::triage;

const FIXTURE_PATH: &str = "tests/fixtures/work_item_4821.json";

const FAKE_BODY: &str = r#"{"model":"qwen3.5:4b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia"},"done":true}"#;

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

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

fn spawn_fake_ollama() -> (String, std::thread::JoinHandle<Option<String>>) {
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
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FAKE_BODY}",
            FAKE_BODY.len()
        );
        if stream.write_all(response.as_bytes()).is_err() {
            return None;
        }
        let _ = stream.flush();
        Some(request)
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn triage_file_prints_the_three_sections_with_the_model_text() {
    let (base_url, server) = spawn_fake_ollama();
    let output = triage::triage_file(&base_url, "qwen3.5:4b", Path::new(FIXTURE_PATH))
        .await
        .unwrap_or_else(|err| panic!("triage --file deve funcionar offline: {err}"));
    assert_eq!(output.item_id, 4821, "o id acompanha o texto (PR30)");
    let output = output.output;
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.first(), Some(&"Causa raiz provável:"), "{output}");
    assert!(
        output.contains("\nTTL do token muito curto."),
        "texto do modelo deve aparecer dentro da seção: {output}"
    );
    assert!(lines.contains(&"Arquivos prováveis:"), "{output}");
    assert!(output.contains("\nsrc/auth/session.rs"), "{output}");
    assert!(lines.contains(&"Confiança:"), "{output}");
    assert!(output.contains("\nmédia"), "{output}");
    // The prompt sent to the (fake) model carries the fixture data, cleaned.
    let request = server
        .join()
        .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"));
    let Some(request) = request else {
        panic!("servidor falso deve capturar a requisição");
    };
    assert!(request.contains("POST /api/chat"), "{request}");
    assert!(
        request.contains("Sessão expirada com token válido"),
        "prompt deve conter o título da fixture: {request}"
    );
    assert!(
        !request.contains("<div>"),
        "prompt não pode conter HTML cru: {request}"
    );
}

/// Sandbox of the CLI path: a temp project + isolated store, no Azure env.
struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
    base_url: String,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let (base_url, server) = spawn_fake_ollama();
        // O handle fica solto de propósito: a thread do servidor falso acaba
        // quando o processo termina, e um `join` poderia esperar para sempre.
        drop(server);
        let base = std::env::temp_dir().join(format!(
            "kira-triage-e2e-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        Self {
            cwd,
            store,
            base_url,
        }
    }

    fn fixture(&self) -> String {
        format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE_PATH)
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_OLLAMA_URL", &self.base_url)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn page(&self) -> PathBuf {
        self.cwd
            .join("docs")
            .join("wiki")
            .join("bugs")
            .join("4821.md")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Some(base) = self.cwd.parent() {
            let _ = std::fs::remove_dir_all(base);
        }
    }
}

/// PR30: the triage command leaves an episode in docs/wiki without asking
/// anything (ADR-0013), and the printed answer does not change.
#[test]
fn triage_file_captures_an_episode_in_the_project_wiki() {
    let sandbox = Sandbox::new("capture");
    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.starts_with("Causa raiz provável:"),
        "saída do triage intacta: {stdout}"
    );
    assert!(
        stderr.contains("episódio do item 4821"),
        "o aviso vai para stderr: {stderr}"
    );

    let page = std::fs::read_to_string(sandbox.page())
        .unwrap_or_else(|err| panic!("ler a página do episódio: {err}"));
    assert!(page.contains("# Bug 4821"), "{page}");
    assert!(
        page.contains("- Causa raiz prevista: TTL do token muito curto."),
        "causa raiz prevista gravada: {page}"
    );
    assert!(
        page.contains("- Arquivos prováveis: src/auth/session.rs"),
        "{page}"
    );
    assert!(page.contains("- Confiança: média"), "{page}");
}

/// Fail-open: a wiki que não pode ser escrita vira um `aviso:` em stderr e nada
/// mais — a saída e o código de saída do triage continuam os mesmos.
#[test]
fn a_broken_wiki_never_breaks_the_triage_command() {
    let sandbox = Sandbox::new("fail-open");
    // `docs` como arquivo: criar `docs/wiki/bugs` é impossível.
    std::fs::write(sandbox.cwd.join("docs"), "não sou um diretório")
        .unwrap_or_else(|err| panic!("criar docs: {err}"));

    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "fail-open: stderr: {stderr}");
    assert!(
        stdout.contains("Causa raiz provável:"),
        "saída do triage preservada: {stdout}"
    );
    assert!(
        stderr.contains("aviso: wiki não registrou o episódio"),
        "aviso acionável no stderr: {stderr}"
    );
    assert!(!sandbox.page().exists(), "nada pode ser gravado");
}

/// Constantes do teste do context packer: um achado `high` apontando para o
/// arquivo que existe no sandbox.
const FINDINGS_JSON: &str = r#"{"findings":[{"tool":"dotnet","rule":"vulnerable","package":"Newtonsoft.Json","version":"12.0.1","severity":"High","path":"src/App.csproj","message":"advisory","fingerprint":"0000000000000001"}],"stacks":[]}"#;

/// PR5 (ADR-0015): o `kira triage` do CLI monta o pack com o repositório real —
/// trechos `caminho:linha` e achados do scan — e diz no stderr o que enviou.
#[test]
fn triage_command_reports_the_context_it_sent() {
    let sandbox = Sandbox::new("context");
    let src = sandbox.cwd.join("src");
    std::fs::create_dir_all(src.join("auth")).unwrap_or_else(|err| panic!("criar src/auth: {err}"));
    std::fs::write(
        src.join("auth").join("session.rs"),
        "pub fn ttl() {\n    let token = refresh_token();\n}\n",
    )
    .unwrap_or_else(|err| panic!("escrever session.rs: {err}"));
    std::fs::write(src.join("App.csproj"), "<Project />\n")
        .unwrap_or_else(|err| panic!("escrever App.csproj: {err}"));
    let kira_dir = sandbox.cwd.join(".kira");
    std::fs::create_dir_all(&kira_dir).unwrap_or_else(|err| panic!("criar .kira: {err}"));
    std::fs::write(kira_dir.join("findings.json"), FINDINGS_JSON)
        .unwrap_or_else(|err| panic!("escrever findings.json: {err}"));

    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.starts_with("Causa raiz provável:"),
        "saída do triage intacta: {stdout}"
    );
    assert!(
        stderr.contains("contexto: "),
        "o stderr diz o que foi enviado: {stderr}"
    );
    assert!(
        stderr.contains("trecho(s) de repositório"),
        "o grep do repositório entrou no pack: {stderr}"
    );
    assert!(
        stderr.contains("findings do scan"),
        "os achados entraram no pack: {stderr}"
    );
}

/// PR7: a geração do prompt aparece no stderr, com versão e impressão digital —
/// é o que permite dizer se duas saídas do modelo são comparáveis.
#[test]
fn triage_command_reports_the_prompt_generation() {
    let sandbox = Sandbox::new("prompt-id");
    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("fnv1a"),
        "a impressão digital não contamina a saída pipeável: {stdout}"
    );
    assert!(
        stderr.contains("prompt: triage v1 (fnv1a "),
        "o stderr identifica a geração do prompt: {stderr}"
    );
}
