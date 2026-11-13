//! End-to-end de `kira publish` (ADR-0038, M5).
//!
//! Contratos verificados no binário real, com Ollama falso em `127.0.0.1`:
//! - publica cada rascunho em `.kira/wiki/<slug>.md` com front-matter e gera o
//!   `index.md` DERIVADO;
//! - rodar de novo sem mudança é idempotente (`0 publicado`);
//! - post editado à mão é **recusado** e o conteúdo do dev é preservado;
//! - `--force` é a única porta para sobrescrever.

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
            "kira-publish-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(cwd.join("src")).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::write(cwd.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("manifesto");
        std::fs::write(cwd.join("src/main.rs"), "fn main() {}\n").expect("main");
        Self {
            base,
            cwd,
            store,
            ollama: spawn_fake_ollama(),
        }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env("KIRA_SECRET_STORE", "off")
            .env("KIRA_OLLAMA_URL", &self.ollama)
            .env("KIRA_MODEL", "fake")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_BOARD")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// ingest → trust → digest: deixa os rascunhos prontos para o publish.
    fn prepare(&self) {
        for args in [
            &["ingest", "--skip-scan"][..],
            &["trust"][..],
            &["digest", "--lang", "pt-BR"][..],
        ] {
            let (code, stdout, stderr) = self.run(args);
            assert_eq!(code, 0, "{args:?}: {stdout}\n{stderr}");
        }
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.cwd.join(rel)).unwrap_or_default()
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.cwd.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, body).expect("write");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn publish_writes_the_posts_and_the_derived_index() {
    let sandbox = Sandbox::new("publica");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("2 publicado(s)"), "stdout: {stdout}");

    let intro = sandbox.read(".kira/wiki/intro.md");
    assert!(intro.starts_with("---\n"), "sem front-matter: {intro}");
    for key in [
        "slug: intro",
        "kind: intro",
        "lang: pt-BR",
        "published_hash:",
        "source_hash:",
    ] {
        assert!(intro.contains(key), "front-matter sem {key}: {intro}");
    }
    assert!(
        intro.contains("Corpo do post."),
        "corpo do rascunho ausente: {intro}"
    );

    let index = sandbox.read(".kira/wiki/index.md");
    assert!(
        index.contains("[intro](intro.md)"),
        "índice sem o intro: {index}"
    );
    assert!(
        index.contains("[c4-container](c4-container.md)"),
        "índice sem o c4: {index}"
    );
}

#[test]
fn publishing_twice_is_idempotent() {
    let sandbox = Sandbox::new("idem");
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "primeiro publish: {stderr}");
    let before = sandbox.read(".kira/wiki/intro.md");

    let (code, stdout, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "segundo publish: {stderr}");
    assert!(stdout.contains("0 publicado(s)"), "stdout: {stdout}");
    assert!(stdout.contains("2 sem mudança"), "stdout: {stdout}");
    assert_eq!(
        before,
        sandbox.read(".kira/wiki/intro.md"),
        "conteúdo mudou sem motivo"
    );
}

/// O caso que o ADR-0038 exige: wiki escrito pelo dev nunca é sobrescrito sozinho.
#[test]
fn a_post_edited_by_hand_is_refused_and_preserved() {
    let sandbox = Sandbox::new("editado");
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "publish inicial: {stderr}");

    let edited = "---\nslug: intro\npublished_hash: x\n---\n\n## Minha edição\nnão mexa\n";
    sandbox.write(".kira/wiki/intro.md", edited);

    let (code, stdout, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "recusa não é erro do comando: {stderr}");
    assert!(
        stdout.contains("recusado(s) por edição manual"),
        "a recusa precisa aparecer: {stdout}"
    );
    assert!(
        stdout.contains("intro"),
        "o arquivo recusado precisa ser nomeado: {stdout}"
    );
    assert_eq!(
        sandbox.read(".kira/wiki/intro.md"),
        edited,
        "o texto do dev foi sobrescrito sem --force"
    );
}

#[test]
fn force_overwrites_a_hand_edited_post() {
    let sandbox = Sandbox::new("force");
    sandbox.prepare();
    let (code, _, stderr) = sandbox.run(&["publish"]);
    assert_eq!(code, 0, "publish inicial: {stderr}");
    sandbox.write(
        ".kira/wiki/intro.md",
        "---\nslug: intro\n---\n\ntexto do dev\n",
    );

    let (code, stdout, stderr) = sandbox.run(&["publish", "--force"]);
    assert_eq!(code, 0, "force: {stderr}");
    assert!(stdout.contains("1 publicado(s)"), "stdout: {stdout}");
    let published = sandbox.read(".kira/wiki/intro.md");
    assert!(
        !published.contains("texto do dev"),
        "--force deveria ter sobrescrito: {published}"
    );
}
