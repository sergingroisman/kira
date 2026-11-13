//! e2e da ingestão no onboarding (M6): o BINÁRIO real, com store, config e
//! keyring isolados — o aceite do BUILD-PROGRAM:
//! - `setup --yes` cria `.kira/digest.md` + inventário, SEM rede (scan fica
//!   fora do setup — contrato do M1) e sem exigir trust;
//! - `setup --yes --skip ingest` não cria nada e diz como ativar depois;
//! - `--lang` do setup chega ao `kira digest` via `config.toml` (ADR-0038 D4:
//!   flag > KIRA_LANG > config > en).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FAKE_BODY: &str = "{\"model\":\"fake\",\"message\":{\"role\":\"assistant\",\"content\":\"Corpo do post.\\n\"},\"done\":true}";

fn spawn_fake_ollama() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FAKE_BODY}",
                    FAKE_BODY.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{addr}")
}

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
    config: PathBuf,
    ollama: String,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "kira-setup-ingest-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(cwd.join("src")).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::write(cwd.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("manifesto");
        std::fs::write(cwd.join("src/main.rs"), "fn main() {}\n").expect("main");
        let config = base.join("config.toml");
        Self {
            base,
            cwd,
            store,
            config,
            ollama: spawn_fake_ollama(),
        }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            // Hermético (a mesma doutrina dos IaHooks: "testes nunca tocam
            // processo real, PATH ou rede"): sem PATH, `ollama list` não é
            // encontrado e o passo de IA cai em "nenhuma" na hora. Com o
            // PATH da máquina, um daemon ocupado pode travar o spawn sem
            // timeout — achado registrado no PR, issue própria.
            .env("PATH", "")
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", &self.config)
            .env("KIRA_SECRET_STORE", "off")
            .env("KIRA_OLLAMA_URL", &self.ollama)
            .env("KIRA_MODEL", "fake")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_BOARD")
            .env_remove("KIRA_LANG")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    fn exists(&self, rel: &str) -> bool {
        self.cwd.join(rel).exists()
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.cwd.join(rel)).unwrap_or_default()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn setup_yes_creates_the_digest_without_network_or_trust() {
    let sandbox = Sandbox::new("sim");
    let (code, stdout, stderr) = sandbox.run(&["setup", "--yes"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    // O passo executa e ecoa os números reais da caminhada.
    assert!(stdout.contains("ingerido:"), "stdout: {stdout}");
    assert!(
        stdout
            .contains("ingestão: 2 arquivo(s), sem scan (rode kira scan), digest .kira/digest.md"),
        "o resumo ecoa a ingestão (sem scan, com a causa): {stdout}"
    );
    assert!(sandbox.exists(".kira/digest.md"), "digest criado");
    assert!(
        sandbox.exists(".kira/ingest/inventory.json"),
        "inventário criado"
    );
    assert!(
        sandbox.exists(".kira/.gitignore"),
        "o marcador local (.kira nunca vai para o git) é escrito junto"
    );
    // M6: o scan saiu do próximo passo.
    assert!(
        stdout.contains("próximo passo:   kira connect azure"),
        "stdout: {stdout}"
    );
}

#[test]
fn setup_skip_ingest_creates_nothing() {
    let sandbox = Sandbox::new("nao");
    let (code, stdout, stderr) = sandbox.run(&["setup", "--yes", "--skip", "ingest"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("ingestão pulada"), "stdout: {stdout}");
    assert!(
        stdout.contains("ingestão: pulada — ativar depois: kira ingest"),
        "stdout: {stdout}"
    );
    assert!(
        !sandbox.cwd.join(".kira").exists(),
        "--skip ingest não pode criar nada"
    );
}

#[test]
fn setup_lang_reaches_the_digest_posts_through_the_config() {
    // ADR-0038 D4 de ponta a ponta: o idioma escolhido no setup (flag) é
    // salvo em config.toml e o `kira digest` o lê SEM nova flag — a cadeia
    // flag > KIRA_LANG > config > en.
    let sandbox = Sandbox::new("lang");
    let (code, stdout, stderr) = sandbox.run(&["setup", "--yes", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stdout.contains("idioma: pt-BR (posts, cards e PR)"),
        "stdout: {stdout}"
    );
    let config = std::fs::read_to_string(&sandbox.config).unwrap_or_default();
    assert!(
        config.contains("output_lang = \"pt-BR\""),
        "config.toml: {config}"
    );

    // Trust (o digest manda código ao modelo — exige o aceite, D2) e digest
    // SEM --lang: vem do config.
    let (code, stdout, stderr) = sandbox.run(&["trust"]);
    assert_eq!(code, 0, "trust: {stdout}\n{stderr}");
    let (code, stdout, stderr) = sandbox.run(&["digest"]);
    assert_eq!(code, 0, "digest: {stdout}\n{stderr}");
    let intro = sandbox.read(".kira/digest/intro.md");
    assert!(
        intro.contains("lang: pt-BR"),
        "o rascunho nasce no idioma salvo: {intro}"
    );
}
