//! End-to-end tests of `kira dashboard` (W6-PR1, ADR-0014).
//!
//! Three proofs, all offline:
//! 1. `--snapshot` with a store/config fixture prints the header (name, `dev:`,
//!    masked PAT) and exits 0 — the raw PAT never appears;
//! 2. **read-only proof**: running the dashboard with and without `--snapshot`
//!    leaves the `store.db` hash byte-identical — nothing writes, not even the
//!    automatic session trail (the command is `records: false`);
//! 3. `KIRA_OLLAMA_URL` on a dead port still works — the dashboard never
//!    touches the network (no probe, no LLM).
//!
//! No process-global env var is mutated: every override is passed to the
//! spawned binary only (same convention as `tests/store_e2e.rs`).

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use kira::store::{self, Store};

/// A fake PAT for the env-resolution path. The masked prefix `Aa2x` is what
/// the header must show; the full value must NEVER appear in the output.
const FAKE_PAT: &str = "Aa2x9g7QdXsLpQ0q8Z3fK1mVcN4yB6hT";

/// Isolated sandbox: project cwd + store dir + config file, all temp.
struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-dashboard-e2e-{}-{name}-{nanos}",
            std::process::id()
        ));
        let cwd = base.join("proj");
        std::fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar workdir {cwd:?}: {err}"));
        Sandbox {
            base: base.clone(),
            cwd,
            store: base.join("store"),
        }
    }

    /// Store fixture: a real project row + one recorded `scan` session (with a
    /// recent timestamp, so the header shows a live age). Closing the store
    /// checkpoints the WAL, leaving a single stable `store.db` to hash.
    /// `with_scan = false` seeds only the project (no scan session) — the
    /// "nunca rodou kira scan" honesty case.
    fn seed_store_with_scan(&self, with_scan: bool) {
        let store_db = Store::open_in_dir(&self.store)
            .unwrap_or_else(|err| panic!("abrir store fixture: {err}"));
        let project = store_db
            .ensure_project(&self.cwd)
            .unwrap_or_else(|err| panic!("criar projeto fixture: {err}"));
        if with_scan {
            store_db
                .record(
                    &project,
                    "scan",
                    "",
                    None,
                    &store::now_rfc3339_utc(),
                    1_200,
                    true,
                    "scan fixture para o dashboard",
                )
                .unwrap_or_else(|err| panic!("gravar sessão scan fixture: {err}"));
        }
    }

    /// Config fixture with `dev_name` — the header shows it as `dev: <nome>`.
    fn config_path(&self) -> PathBuf {
        let config = self.base.join("config.toml");
        std::fs::write(
            &config,
            "dev_name = \"Sérgio\"\norganization = \"org\"\nproject = \"proj\"\n",
        )
        .unwrap_or_else(|err| panic!("escrever config fixture: {err}"));
        config
    }

    /// Hash of the fixture store DB (the object of the read-only proof).
    fn store_hash(&self) -> String {
        let path = self.store.join(store::DB_FILE);
        let bytes = std::fs::read(&path).unwrap_or_else(|err| panic!("ler {path:?}: {err}"));
        // SHA-256 à mão é caro; um hash estável e rápido basta para a prova:
        // a comparação antes/depois captura QUALQUER mutação de byte.
        format!(
            "{:016x}{:016x}",
            fnv1a(&bytes),
            fnv1a(&bytes.iter().rev().cloned().collect::<Vec<u8>>())
        )
    }
}

/// FNV-1a 64 sobre bytes — a mesma família de hash que a store usa para ids;
/// suficiente para detectar qualquer alteração do arquivo (a força da prova é
/// o `assert_eq!` antes/depois, não a resistência a colisões).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Spawns the built `kira` inside the sandbox (non-TTY by construction: the
/// child's stdout is a pipe), returning (exit code, stdout, stderr).
fn run_kira(sandbox: &Sandbox, extra_env: &[(&str, &str)], args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", sandbox.config_path())
        .env("AZURE_DEVOPS_PAT", FAKE_PAT)
        // Nada de rede: Ollama apontado para uma porta morta em TODA execução.
        .env("KIRA_OLLAMA_URL", "http://127.0.0.1:1")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("NO_COLOR");
    for (key, value) in extra_env {
        command.env(key, value);
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

#[test]
fn snapshot_prints_header_with_dev_and_masked_pat_and_exits_zero() {
    let sandbox = Sandbox::new("snapshot");
    sandbox.seed_store_with_scan(true);
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    // Nome do projeto (a pasta do sandbox) e o campo do dev do config.
    assert!(out.contains("projeto: proj"), "{out}");
    assert!(out.contains("dev: Sérgio"), "{out}");
    // PAT mascarado: prefixo + `****`, nunca o valor cru.
    assert!(out.contains("Aa2x****"), "{out}");
    assert!(
        !out.contains(FAKE_PAT),
        "o PAT completo NUNCA pode aparecer na saída: {out}"
    );
    // Store com sessão de scan: idade visível; `achados: —` com a causa
    // concreta (o fixture tem scan, mas não tem findings.json).
    assert!(out.contains("último scan: há"), "{out}");
    assert!(
        out.contains("achados: — (findings.json ausente — rode `kira scan` de novo)"),
        "causa honesta esperada: {out}"
    );
    // Rodapé de teclas presente no modo texto também.
    assert!(out.contains("teclas: q sair"), "{out}");
}

#[test]
fn dashboard_with_and_without_snapshot_never_touch_the_store() {
    let sandbox = Sandbox::new("readonly-hash");
    sandbox.seed_store_with_scan(true);
    let before = sandbox.store_hash();
    assert!(!before.is_empty(), "store fixture precisa existir");

    // Sem --snapshot: stdout é pipe (não-TTY) => modo texto, MESMA promessa.
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    let after_plain = sandbox.store_hash();
    assert_eq!(
        before, after_plain,
        "dashboard (não-TTY) mutou o store.db — violação do contrato read-only"
    );

    // Com --snapshot: idem, byte a byte.
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    let after_snapshot = sandbox.store_hash();
    assert_eq!(
        before, after_snapshot,
        "dashboard --snapshot mutou o store.db — violação do contrato read-only"
    );
}

#[test]
fn dashboard_works_with_ollama_on_a_dead_port() {
    // A porta morta já é o default de run_kira; aqui é a prova explícita do
    // contrato 3 da spec: nenhum probe de rede bloqueia ou falha o snapshot.
    let sandbox = Sandbox::new("dead-port");
    sandbox.seed_store_with_scan(true);
    let (code, out, err) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "não pode haver dependência de rede: {out}{err}");
    assert!(out.contains("llm: ollama"), "{out}");
    // Sem hang e sem spinner: a saída é completa e termina com o rodapé.
    assert!(out.trim_end().ends_with("? ajuda"), "{out}");
}

#[test]
fn missing_scan_shows_the_dash_with_the_nunca_rodou_cause() {
    // Projeto existe, mas nenhuma sessão de `scan` na trilha: a causa
    // honesta é "nunca rodou kira scan" — nunca estimativa.
    let sandbox = Sandbox::new("no-scan");
    sandbox.seed_store_with_scan(false);
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("achados: — (nunca rodou kira scan)"), "{out}");
    assert!(
        out.contains("último scan: — (nunca rodou kira scan)"),
        "{out}"
    );
}

#[test]
fn absent_store_is_an_actionable_message_with_exit_zero() {
    // Contrato 4 da spec: sem store.db, o dashboard é diagnóstico (exit 0) e
    // diz o que fazer — "rode kira scan ou kira doctor".
    let sandbox = Sandbox::new("no-store");
    let (code, out, _) = run_kira(&sandbox, &[], &["dashboard", "--snapshot"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nenhuma trilha em"), "{out}");
    assert!(out.contains("rode `kira scan` ou `kira doctor`"), "{out}");
    assert!(out.contains("projeto: — ("), "{out}");
}
