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
    spawn_fake_ollama_with(FAKE_BODY)
}

/// Serves `body` as the `/api/chat` response, so a test can make the model
/// answer out of format (W2-fix PR-F2).
fn spawn_fake_ollama_with(body: &str) -> (String, std::thread::JoinHandle<Option<String>>) {
    let body = body.to_string();
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
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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

/// Fake Foundry (OpenAI-compatible chat completions) on an ephemeral port.
fn spawn_fake_foundry(body: &str) -> (String, std::thread::JoinHandle<Option<String>>) {
    spawn_fake_ollama_with(body)
}

/// Fake Foundry whose accept loop gives up after a deadline: `None` proves
/// NOTHING ever connected (the no-network-call contract of the missing key).
fn spawn_deadline_foundry() -> (String, std::thread::JoinHandle<Option<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    let handle = std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1000);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => return None,
            }
        };
        let _ = listener.set_nonblocking(false);
        let _ = stream.set_nonblocking(false);
        let request = read_http_request(&mut stream);
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
    // O HTML cru do work item não pode chegar ao modelo. A checagem é escopada à
    // parte do work item: o pack de contexto pode trazer uma linha de repositório
    // que legitimamente contém `<div>` (fixture, template) — são camadas
    // diferentes, e a limpeza de HTML vale para o item.
    let work_item_part = request
        .split("Contexto do projeto")
        .next()
        .unwrap_or(&request);
    assert!(
        !work_item_part.contains("<div>"),
        "prompt não pode conter HTML cru do item: {work_item_part}"
    );
    assert!(
        !work_item_part.contains("&amp;"),
        "prompt não pode conter entidade crua do item: {work_item_part}"
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
        Self::with_body(tag, FAKE_BODY)
    }

    /// Sandbox whose fake model answers with `body` instead of the in-format
    /// answer — used to prove the out-of-format path fails loudly (PR-F2).
    fn with_body(tag: &str, body: &str) -> Self {
        let (base_url, server) = spawn_fake_ollama_with(body);
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
            .env("KIRA_SECRET_STORE", "off")
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

    /// Runs the CLI with explicit env overrides/removes — the Foundry tests
    /// need `KIRA_FOUNDRY_*` and a deliberately dead Ollama port.
    fn run_with(
        &self,
        args: &[&str],
        envs: &[(&str, String)],
        removes: &[&str],
    ) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
        command
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env_remove("AZURE_DEVOPS_PAT")
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_LLM")
            .env_remove("KIRA_FOUNDRY_URL")
            .env_remove("KIRA_FOUNDRY_AUTH")
            .env_remove("KIRA_FOUNDRY_BODY_MODE");
        for (name, value) in envs {
            command.env(name, value);
        }
        for name in removes {
            command.env_remove(name);
        }
        let output = command
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
            .join(".kira")
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

/// PR30: the triage command leaves an episode in .kira/wiki without asking
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

/// W2-fix PR-F3 — o defeito real do bug 529511: as três seções vieram TODAS
/// vazias (só o placeholder `—`), o triage saiu com exit 0 e a wiki registrou o
/// episódio vazio como se fosse conhecimento. Agora o episódio vazio NÃO grava
/// arquivo: a falha fica só na trilha (`kira history`).
#[test]
fn triage_empty_sections_never_write_a_wiki_page() {
    const EMPTY_SECTIONS_BODY: &str = r#"{"model":"qwen3.5:4b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n"},"done":true}"#;
    let sandbox = Sandbox::with_body("empty-sections", EMPTY_SECTIONS_BODY);

    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "formato ok com seções vazias: stderr: {stderr}");
    assert!(
        stdout.contains("Causa raiz provável:") && stdout.contains("—"),
        "stdout segue com os placeholders do formato: {stdout}"
    );
    assert!(
        stderr.contains(
            "aviso: wiki não registrou o episódio do item 4821 (triage sem conteúdo — falha registrada na trilha)"
        ),
        "aviso acionável no stderr: {stderr}"
    );
    assert!(
        !stderr.contains("episódio do item 4821 registrado"),
        "nenhum note de sucesso para episódio vazio: {stderr}"
    );
    assert!(
        !sandbox.cwd.join(".kira").join("wiki").exists(),
        "episódio vazio NÃO pode criar .kira/wiki"
    );
    assert!(
        !sandbox.page().exists(),
        "episódio vazio NÃO pode gravar arquivo na wiki"
    );

    // A falha fica registrada na trilha da sessão (`kira history`).
    let (code, stdout, stderr) = sandbox.run(&["history"]);
    assert_eq!(code, 0, "history: stderr: {stderr}");
    assert!(
        stdout.contains("wikipage: episódio failed (triage sem conteúdo nas 3 seções)"),
        "a trilha deve mostrar a falha do episódio: {stdout}"
    );
}

/// W2-fix PR-F3 — o path exibido no note de sucesso nunca sai com o prefixo
/// cru `\\?\` do Windows (mesmo bug de display já corrigido no `kira profile`).
#[test]
fn triage_success_note_shows_the_path_without_the_windows_prefix() {
    let sandbox = Sandbox::new("display-path");
    let (code, _, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stderr.contains("wiki: episódio do item 4821 registrado em "),
        "note de sucesso presente: {stderr}"
    );
    assert!(
        !stderr.contains("\\\\?\\"),
        "o path exibido não pode sair com o prefixo cru do Windows: {stderr}"
    );
}

/// W2-fix PR-F2 — o defeito real: modelo respondendo fora do formato das três
/// seções produzia três `—` com exit 0 e a wiki gravava o episódio vazio.
/// Agora o comando falha com erro acionável, não imprime as seções e não grava
/// nada.
#[test]
fn triage_answer_out_of_format_fails_loudly() {
    const OUT_OF_FORMAT_BODY: &str = r#"{"model":"qwen3.5:4b","created_at":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":"O token expira rápido demais. Provavelmente o TTL está curto."},"done":true}"#;
    let sandbox = Sandbox::with_body("out-of-format", OUT_OF_FORMAT_BODY);

    let (code, stdout, stderr) = sandbox.run(&[
        "triage",
        "--file",
        &sandbox.fixture(),
        "--model",
        "qwen3.5:4b",
    ]);

    assert_eq!(
        code, 1,
        "resposta fora do formato não pode sair com 0 — stdout: {stdout} / stderr: {stderr}"
    );
    assert!(
        !stdout.contains("Causa raiz provável:"),
        "não pode imprimir o template com três —: {stdout}"
    );
    assert!(
        stdout.trim().is_empty(),
        "stdout deve ficar vazio: {stdout}"
    );
    assert!(
        stderr.contains("fora do formato das 3 seções"),
        "erro precisa dizer que o modelo saiu do formato: {stderr}"
    );
    assert!(
        stderr.contains("--model") && stderr.contains("ollama ps"),
        "erro precisa dizer o que fazer: {stderr}"
    );
    assert!(
        !stderr.contains("episódio do item"),
        "sem formato não há episódio: {stderr}"
    );
    assert!(!sandbox.page().exists(), "nada pode ser gravado na wiki");
}

/// Fail-open: a wiki que não pode ser escrita vira um `aviso:` em stderr e nada
/// mais — a saída e o código de saída do triage continuam os mesmos.
#[test]
fn a_broken_wiki_never_breaks_the_triage_command() {
    let sandbox = Sandbox::new("fail-open");
    // `.kira` como arquivo: criar `.kira/wiki/bugs` é impossível.
    std::fs::write(sandbox.cwd.join(".kira"), "não sou um diretório")
        .unwrap_or_else(|err| panic!("criar .kira: {err}"));

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
        stderr.contains("prompt: triage v2 (fnv1a "),
        "o stderr identifica a geração do prompt: {stderr}"
    );
}

// ===========================================================================
// PR11 (ADR-0019): the Copilot backend — offline e2e with a fake `copilot`
// CLI shim (`KIRA_COPILOT_BIN`, same contract as `KIRA_SCAN_<TOOL>_BIN`) and
// a DEAD Ollama port, proving the explicit flag runs the CLI and never the
// default backend. No network, no real login.
// ===========================================================================

/// Unique scratch dir for shims/captures.
fn copilot_dir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "kira-triage-e2e-copilot-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0)
    ));
    std::fs::remove_dir_all(&base).unwrap_or(());
    std::fs::create_dir_all(&base).unwrap_or_else(|err| panic!("criar {base:?}: {err}"));
    base
}

/// Writes a `copilot` shim (`.cmd` on Windows, shebang script elsewhere) and
/// returns its path (to be set as `KIRA_COPILOT_BIN`).
fn write_copilot_shim(dir: &Path, windows_script: &str, unix_script: &str) -> PathBuf {
    let (name, script) = if cfg!(windows) {
        (String::from("copilot.cmd"), windows_script)
    } else {
        (String::from("copilot"), unix_script)
    };
    let path = dir.join(&name);
    std::fs::write(&path, script).unwrap_or_else(|err| panic!("escrever shim {path:?}: {err}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|err| panic!("chmod shim {path:?}: {err}"));
    }
    path
}

/// The success shim: captures its argv (proof the prompt went to the CLI,
/// never to a fallback), prints an ANSI-styled banner and the clean report.
fn success_shim(dir: &Path, capture: &Path) -> PathBuf {
    let cap = capture.to_string_lossy().into_owned();
    let unix = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{cap}'\nprintf '\\033[1m\\033[36mCopilot CLI v0.9\\033[0m\\n'\nprintf '\\033[32mCopilot: connected\\033[0m\\n'\nprintf 'Causa raiz provável:\\nTTL do token muito curto.\\nArquivos prováveis:\\nsrc/auth/session.rs\\nConfiança:\\nmédia\\n'\n"
    );
    let windows = format!(
        "@echo %*>\"{cap}\"\r\n@echo Causa raiz provável:\r\n@echo TTL do token muito curto.\r\n@echo Arquivos prováveis:\r\n@echo src/auth/session.rs\r\n@echo Confiança:\r\n@echo media\r\n"
    );
    write_copilot_shim(dir, &windows, &unix)
}

/// The SAML shim: exit 1, harmless traceback + SAML/SSO text on stderr.
fn saml_shim(dir: &Path) -> PathBuf {
    let windows = "@echo Traceback (most recent call last): 1>&2\r\n@echo SAML SSO assertion required 1>&2\r\n@exit /b 1\r\n";
    let unix = "#!/bin/sh\necho 'Traceback (most recent call last):' 1>&2\necho 'SAML SSO assertion required' 1>&2\nexit 1\n";
    write_copilot_shim(dir, windows, unix)
}

/// Runs the built `kira` triage with explicit env pairs (the Ollama URL is
/// ALWAYS the caller's — dead port for the Copilot tests).
fn run_triage_with(
    cwd: &Path,
    store: &Path,
    envs: &[(&str, Option<&Path>)],
    args: &[&str],
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(cwd)
        .env("KIRA_STORE_DIR", store)
        .env("KIRA_CONFIG", store.join("absent-config.toml"))
        .env_remove("KIRA_LLM")
        .env_remove("KIRA_COPILOT_BIN")
        .env_remove("KIRA_COPILOT_TIMEOUT")
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("KIRA_MODEL");
    for (name, value) in envs {
        match value {
            Some(path) => command.env(name, path),
            None => command.env_remove(name),
        };
    }
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A port that will never answer: bound once and released.
fn dead_ollama_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|err| panic!("bind na porta efêmera: {err}"));
    let addr = listener
        .local_addr()
        .unwrap_or_else(|err| panic!("endereço local: {err}"));
    drop(listener);
    format!("http://{addr}")
}

/// ADR-0019 e2e: `--llm copilot` with a fake CLI and a DEAD Ollama port —
/// exit 0 with the report proves the CLI answered and Ollama was NEVER used
/// (a fallback to the dead port would have failed loudly).
#[test]
fn copilot_flag_runs_the_fake_cli_and_never_the_ollama_port() {
    let sandbox = Sandbox::new("copilot-success");
    let dir = copilot_dir("success");
    let capture = dir.join("argv.txt");
    let shim = success_shim(&dir, &capture);
    let (code, stdout, stderr) = run_triage_with(
        &sandbox.cwd,
        &sandbox.store,
        &[
            ("KIRA_COPILOT_BIN", Some(shim.as_path())),
            ("KIRA_OLLAMA_URL", Some(Path::new(&dead_ollama_url()))),
        ],
        &["triage", "--file", &sandbox.fixture(), "--llm", "copilot"],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("Causa raiz provável:"),
        "relatório do CLI na saída: {stdout}"
    );
    assert!(stdout.contains("Confiança:"), "{stdout}");
    assert!(
        !stdout.contains('\u{1b}'),
        "ANSI do CLI não sobrevive: {stdout}"
    );
    assert!(
        !stderr.contains("Ollama não está rodando"),
        "nenhuma tentativa de fallback ao Ollama: {stderr}"
    );
    // The CLI really received the full prompt (`-p <system>\n\n<prompt> -s`).
    assert!(capture.exists(), "shim deve capturar o argv");
    #[cfg(unix)]
    {
        let argv = std::fs::read_to_string(&capture)
            .unwrap_or_else(|err| panic!("ler argv capturado: {err}"));
        assert!(argv.starts_with("-p\n"), "o prompt vai via -p: {argv}");
        assert!(
            argv.trim_end().ends_with("\n-s"),
            "a flag silenciosa acompanha: {argv}"
        );
        assert!(
            argv.contains("Sessão expirada com token válido"),
            "o prompt real (título do item) chegou ao CLI: {argv}"
        );
        assert!(
            argv.contains("TRIAGE") || argv.lines().count() >= 3,
            "o system prompt acompanha o texto: {argv}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The no-copilot case: `KIRA_LLM=copilot` (env wiring) with NO binary
/// anywhere — the run FAILS LOUDLY with the actionable CopilotMissing
/// message; there is NO silent fallback to Ollama.
#[test]
fn copilot_env_without_any_binary_fails_loudly() {
    let sandbox = Sandbox::new("copilot-missing");
    let dir = copilot_dir("missing");
    let empty_path = dir.join("path");
    std::fs::create_dir_all(&empty_path).unwrap_or_else(|err| panic!("criar path vazio: {err}"));
    let (code, _stdout, stderr) = run_triage_with(
        &sandbox.cwd,
        &sandbox.store,
        &[
            ("KIRA_COPILOT_BIN", None),
            ("KIRA_LLM", Some(Path::new("copilot"))),
            ("KIRA_OLLAMA_URL", Some(Path::new(&dead_ollama_url()))),
            ("PATH", Some(empty_path.as_path())),
        ],
        &["triage", "--file", &sandbox.fixture()],
    );
    assert_ne!(code, 0, "sem CLI e sem override, deve falhar: {stderr}");
    assert!(
        stderr.contains("instale o Copilot CLI"),
        "mensagem deve dizer como instalar: {stderr}"
    );
    assert!(
        stderr.contains("copilot auth login"),
        "mensagem deve apontar o login: {stderr}"
    );
    assert!(
        stderr.contains("--llm ollama"),
        "mensagem deve lembrar o default: {stderr}"
    );
    assert!(
        stderr.contains("KIRA_COPILOT_BIN"),
        "mensagem deve citar o override de testes: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The SAML case: the corporate CLI answers exit 1 with an SSO traceback —
/// the message is the EXACT four-step script and never echoes the raw stderr.
#[test]
fn copilot_saml_stderr_becomes_the_four_step_message() {
    let sandbox = Sandbox::new("copilot-saml");
    let dir = copilot_dir("saml");
    let shim = saml_shim(&dir);
    let (code, _stdout, stderr) = run_triage_with(
        &sandbox.cwd,
        &sandbox.store,
        &[
            ("KIRA_COPILOT_BIN", Some(shim.as_path())),
            ("KIRA_OLLAMA_URL", Some(Path::new(&dead_ollama_url()))),
        ],
        &["triage", "--file", &sandbox.fixture(), "--llm", "copilot"],
    );
    assert_ne!(code, 0, "SAML deve falhar: {stderr}");
    assert!(stderr.contains("copilot auth login"), "passo 1: {stderr}");
    assert!(
        stderr.contains("https://github.com/settings/connections/applications"),
        "passo 2: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("grant") || stderr.to_lowercase().contains("authorize"),
        "passo 2 (botão): {stderr}"
    );
    assert!(stderr.contains("TI"), "passo 3: {stderr}");
    assert!(stderr.contains("--llm ollama"), "passo 4: {stderr}");
    assert!(
        !stderr.contains("Traceback"),
        "o stderr cru do CLI nunca é ecoado: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `KIRA_LLM` validates values outside clap: an unknown backend fails with
/// the known-options message (never a silent default).
#[test]
fn unknown_kira_llm_value_fails_with_the_valid_options() {
    let sandbox = Sandbox::new("copilot-unknown");
    let (code, _stdout, stderr) = run_triage_with(
        &sandbox.cwd,
        &sandbox.store,
        &[
            ("KIRA_LLM", Some(Path::new("gpt"))),
            ("KIRA_OLLAMA_URL", Some(Path::new(&dead_ollama_url()))),
        ],
        &["triage", "--file", &sandbox.fixture()],
    );
    assert_ne!(code, 0, "valor desconhecido deve falhar: {stderr}");
    assert!(stderr.contains("desconhecido"), "mensagem: {stderr}");
    assert!(
        stderr.contains("ollama") && stderr.contains("copilot"),
        "o erro lista as opções válidas: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// P0 (auditoria F2) — o teto diário de chamadas de LLM é enforcement real
// ---------------------------------------------------------------------------

/// Duas chamadas com teto 1: a primeira passa e entra no ledger; a segunda é
/// recusada ANTES de qualquer conexão com o modelo, com o conserto na mensagem.
/// Dano se regredir: o budget do mandato voltaria a ser promessa de doc.
#[test]
fn daily_llm_budget_blocks_the_second_call_with_an_actionable_error() {
    let sandbox = Sandbox::new("budget");
    let fixture = sandbox.fixture();
    let envs = [
        // O fake do sandbox atende UMA conexão: consumida pela 1ª chamada.
        ("KIRA_OLLAMA_URL", sandbox.base_url.clone()),
        ("KIRA_MAX_LLM_CALLS_PER_DAY", "1".to_string()),
    ];

    let (code, stdout, stderr) = sandbox.run_with(&["triage", "--file", &fixture], &envs, &[]);
    assert_eq!(
        code, 0,
        "primeira chamada sob o teto deve passar: {stdout} / {stderr}"
    );
    assert!(stdout.starts_with("Causa raiz provável:"), "{stdout}");

    // A chamada boa entrou no ledger: uma linha em `llm_calls` para hoje.
    let store = kira::store::Store::open_at(&sandbox.store.join("store.db"))
        .unwrap_or_else(|err| panic!("abrir store do sandbox: {err}"));
    assert_eq!(
        store
            .llm_calls_today()
            .unwrap_or_else(|err| panic!("ler ledger: {err}")),
        1,
        "o triage precisa registrar a chamada no contador"
    );

    let (code, stdout, stderr) = sandbox.run_with(&["triage", "--file", &fixture], &envs, &[]);
    assert_ne!(code, 0, "teto atingido precisa falhar: {stdout}");
    assert!(
        stderr.contains("limite diário"),
        "erro acionável no stderr: {stderr}"
    );
    assert!(
        stderr.contains("max_llm_calls_per_day"),
        "a mensagem precisa dizer o conserto: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// PR11b / ADR-0020 — backend foundry (`--llm foundry`, envs KIRA_FOUNDRY_*)
// ---------------------------------------------------------------------------

/// Canonical OpenAI-compatible answer of the fake Foundry (GPT-5 shape).
const FOUNDRY_SUCCESS_BODY: &str = r#"{"id":"chatcmpl-1","object":"chat.completion","model":"gpt-5","choices":[{"index":0,"message":{"role":"assistant","content":"Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia"},"finish_reason":"stop"}]}"#;

/// `--llm foundry` with the v1 endpoint: exit 0 with the three sections —
/// with KIRA_OLLAMA_URL on a dead port, proving Ollama was never used.
#[test]
fn triage_with_llm_foundry_prints_the_three_sections_and_never_touches_ollama() {
    let (foundry_url, server) = spawn_fake_foundry(FOUNDRY_SUCCESS_BODY);
    let sandbox = Sandbox::new("foundry-success");
    let fixture = sandbox.fixture();

    let (code, stdout, stderr) = sandbox.run_with(
        &["triage", "--file", &fixture, "--llm", "foundry"],
        &[
            ("KIRA_FOUNDRY_ENDPOINT", format!("{foundry_url}/v1")),
            ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-5".to_string()),
            ("KIRA_FOUNDRY_API_KEY", "kira-e2e-key".to_string()),
            // Porta morta de propósito: se o KIRA tentasse o Ollama, o
            // comando falharia com conexão recusada.
            ("KIRA_OLLAMA_URL", "http://127.0.0.1:1".to_string()),
        ],
        &[],
    );

    assert_eq!(
        code, 0,
        "triage foundry deve sair com 0 — stdout: {stdout} / stderr: {stderr}"
    );
    assert!(
        stdout.starts_with("Causa raiz provável:"),
        "saída em 3 seções preservada: {stdout}"
    );

    let request = server
        .join()
        .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"))
        .unwrap_or_else(|| panic!("servidor falso deve capturar a requisição"));
    assert!(
        request.starts_with("POST /v1/chat/completions HTTP/1.1"),
        "a superfície v1 envia {{endpoint}}/chat/completions: {request}"
    );
    assert!(
        !request.contains("api-version"),
        "a superfície v1 não anexa api-version: {request}"
    );
    assert!(
        request.contains("\"model\":\"gpt-5\""),
        "o deployment vai no campo model do corpo: {request}"
    );
    assert!(
        !request.contains("\"temperature\":"),
        "gpt-5 (família reasoning) não recebe temperature: {request}"
    );
    assert!(
        request.contains("\"max_completion_tokens\":"),
        "gpt-5 exige max_completion_tokens: {request}"
    );
    assert!(
        !request.contains("\"max_tokens\":"),
        "gpt-5 não recebe max_tokens: {request}"
    );
    assert!(
        request.contains("api-key: kira-e2e-key"),
        "o header api-key deve viajar: {request}"
    );
}

/// Missing key fails with an actionable error BEFORE any network call — the
/// deadline server proves nothing ever connected.
#[test]
fn triage_foundry_without_key_fails_before_any_request() {
    let (foundry_url, server) = spawn_deadline_foundry();
    let sandbox = Sandbox::new("foundry-missing-key");
    let fixture = sandbox.fixture();

    let (code, stdout, stderr) = sandbox.run_with(
        &["triage", "--file", &fixture, "--llm", "foundry"],
        &[
            ("KIRA_FOUNDRY_ENDPOINT", format!("{foundry_url}/v1")),
            ("KIRA_FOUNDRY_DEPLOYMENT", "gpt-5".to_string()),
            ("KIRA_OLLAMA_URL", "http://127.0.0.1:1".to_string()),
        ],
        &["KIRA_FOUNDRY_API_KEY"],
    );

    assert_eq!(
        code, 1,
        "chave ausente deve falhar — stdout: {stdout} / stderr: {stderr}"
    );
    assert!(
        stdout.trim().is_empty(),
        "stdout deve ficar vazio: {stdout}"
    );
    assert!(
        stderr.contains("KIRA_FOUNDRY_API_KEY") && stderr.contains("defina"),
        "o erro deve dizer que env definir: {stderr}"
    );

    let captured = server
        .join()
        .unwrap_or_else(|_| panic!("thread do servidor falso entrou em pânico"));
    assert!(
        captured.is_none(),
        "sem chave, NENHUM byte pode chegar ao servidor: {captured:?}"
    );
}
