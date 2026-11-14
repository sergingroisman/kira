//! End-to-end test of `kira loop` (PR15) — the binary's turn: one tick is
//! ingest → digest (when trusted) → publish, all LOCAL.
//!
//! No real Ollama and no network: a fake server answers the digest calls on an
//! ephemeral port, the store is isolated with `KIRA_STORE_DIR`, and the repo is
//! a temp fixture. The centerpiece is the anti-inócuo proof (D2): two ticks
//! without change leave the work state byte-identical.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;

const FAKE_BODY: &str = "{\"model\":\"fake\",\"message\":{\"role\":\"assistant\",\"content\":\"What this is\\nKIRA e um CLI.\\n\"},\"done\":true}";

/// Fake Ollama: one canned answer per connection, as many as needed.
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
                let _ = stream.flush();
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
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "kira-loop-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::create_dir_all(cwd.join("src")).expect("src");
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
            .env_remove("KIRA_LANG")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// Starting state: facts ingested and repository accepted (the loop's
    /// digest only runs on a trusted repo — ADR-0038).
    fn prepare(&self) {
        let (code, stdout, stderr) = self.run(&["ingest", "--skip-scan"]);
        assert_eq!(code, 0, "ingest: {stdout}\n{stderr}");
        let (code, stdout, stderr) = self.run(&["trust"]);
        assert_eq!(code, 0, "trust: {stdout}\n{stderr}");
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.cwd.join(rel)).unwrap_or_default()
    }

    /// Every file under a `.kira` subdir as (relative name, bytes), sorted —
    /// the byte-identical comparison of the anti-inócuo test.
    fn snapshot_dir(&self, rel: &str) -> Vec<(String, Vec<u8>)> {
        let dir = self.cwd.join(rel);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut files: Vec<(String, Vec<u8>)> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let bytes = std::fs::read(entry.path()).ok()?;
                Some((name, bytes))
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    /// Work state of the `posts` queue (the trail in `sessions` is excluded on
    /// purpose: every run records a session, that is the audit log, not the
    /// cycle's work state).
    fn posts_state(&self) -> Vec<(String, String, String, String, Option<String>)> {
        let conn = rusqlite::Connection::open(self.store.join("store.db"))
            .unwrap_or_else(|err| panic!("abrir store: {err}"));
        let mut statement = conn
            .prepare("SELECT slug, kind, state, source_hash, path FROM posts ORDER BY slug")
            .unwrap_or_else(|err| panic!("preparar query: {err}"));
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .unwrap_or_else(|err| panic!("query posts: {err}"));
        rows.filter_map(|row| row.ok()).collect()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// `kira loop --once` runs the full tick: facts ingested, drafts generated by
/// the digest, wiki published — all in one command, all local.
#[test]
fn loop_once_runs_ingest_digest_and_publish() {
    let sandbox = Sandbox::new("tick-completo");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--skip-scan", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "tick completo sai 0: {stdout}\n{stderr}");
    assert!(stdout.contains("tick 1:"), "a linha do tick: {stdout}");
    assert!(
        stdout.contains("2 novo(s)"),
        "o digest gerou os dois posts no tick: {stdout}"
    );
    assert!(
        stdout.contains("2 publicado(s)"),
        "o publish publicou os entregues: {stdout}"
    );
    assert!(
        !sandbox.read(".kira/drafts/intro.md").is_empty(),
        "rascunho gerado"
    );
    assert!(
        !sandbox.read(".kira/wiki/intro.md").is_empty(),
        "wiki publicada pelo ciclo"
    );
}

/// THE anti-inócuo proof (D2): a second tick over an unchanged repo leaves the
/// work state byte-identical — facts, drafts, wiki and the posts queue. The
/// second run says so in plain text ("facts iguais", zero new, zero published).
#[test]
fn two_ticks_without_change_leave_the_state_identical() {
    let sandbox = Sandbox::new("estavel");
    sandbox.prepare();

    let (code, first_stdout, stderr) =
        sandbox.run(&["loop", "--once", "--skip-scan", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "primeiro tick: {first_stdout}\n{stderr}");
    let facts_1 = sandbox.read(".kira/facts.md");
    let drafts_1 = sandbox.snapshot_dir(".kira/drafts");
    let wiki_1 = sandbox.snapshot_dir(".kira/wiki");
    let posts_1 = sandbox.posts_state();
    assert!(!posts_1.is_empty(), "o ciclo enfileirou posts");

    let (code, second_stdout, stderr) =
        sandbox.run(&["loop", "--once", "--skip-scan", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "segundo tick: {second_stdout}\n{stderr}");
    assert!(
        second_stdout.contains("facts iguais"),
        "ingest não mudou nada: {second_stdout}"
    );
    assert!(
        second_stdout.contains("0 novo(s)"),
        "digest tudo em cache: {second_stdout}"
    );
    assert!(
        second_stdout.contains("0 publicado(s)"),
        "publish sem novidade: {second_stdout}"
    );

    // Estado de trabalho idêntico byte a byte (a trilha de sessões cresce de
    // propósito — é o log de auditoria, não o estado do ciclo).
    assert_eq!(
        facts_1,
        sandbox.read(".kira/facts.md"),
        "facts.md byte-idêntico entre ticks"
    );
    assert_eq!(
        drafts_1,
        sandbox.snapshot_dir(".kira/drafts"),
        "drafts idênticos"
    );
    assert_eq!(wiki_1, sandbox.snapshot_dir(".kira/wiki"), "wiki idêntica");
    assert_eq!(posts_1, sandbox.posts_state(), "fila de posts idêntica");
}

/// Without `--once` the loop runs ticks until the state stops moving: tick 1
/// generates, tick 2 finds nothing to do and the run says "estável".
#[test]
fn loop_without_once_stops_at_the_fixed_point() {
    let sandbox = Sandbox::new("ponto-fixo");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["loop", "--skip-scan", "--lang", "pt-BR"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("tick 1:"), "primeiro tick: {stdout}");
    assert!(stdout.contains("tick 2:"), "segundo tick: {stdout}");
    assert!(
        stdout.contains("estável: nada a fazer"),
        "o ciclo para no ponto fixo: {stdout}"
    );
    assert!(
        !stdout.contains("tick 3:"),
        "nenhum tick extra depois de estabilizar: {stdout}"
    );
}

/// Without trust the digest is skipped with an actionable note — the cycle
/// still runs (ingest + publish) and exits 0: local knowledge never requires
/// the owner's acceptance, only code egress does (ADR-0038).
#[test]
fn loop_skips_digest_without_trust_and_says_what_to_do() {
    let sandbox = Sandbox::new("sem-trust");
    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--skip-scan"]);
    assert_eq!(code, 0, "ciclo sem trust não é erro: {stdout}\n{stderr}");
    assert!(
        stdout.contains("digest pulado") && stdout.contains("kira trust"),
        "nota acionável: {stdout}"
    );
    assert!(
        sandbox.read(".kira/drafts/intro.md").is_empty(),
        "nada foi gerado sem aceite"
    );
}

/// `--skip-digest` makes the cycle fully deterministic (no LLM at all):
/// ingest + publish only.
#[test]
fn loop_skip_digest_never_calls_the_model() {
    let sandbox = Sandbox::new("sem-digest");
    sandbox.prepare();
    let (code, stdout, stderr) = sandbox.run(&["loop", "--once", "--skip-scan", "--skip-digest"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stdout.contains("digest pulado (--skip-digest)"),
        "a nota diz por que pulou: {stdout}"
    );
    assert!(
        sandbox.read(".kira/drafts/intro.md").is_empty(),
        "sem digest, sem rascunho"
    );
    assert!(
        !sandbox.read(".kira/facts.md").is_empty(),
        "o ingest rodou mesmo assim"
    );
}
