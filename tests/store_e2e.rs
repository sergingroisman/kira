//! End-to-end tests of the local store seen from the CLI: the automatic
//! `doctor` session, `kira history`, `kira note` and `kira profile`, plus the
//! first-class fail-open when the store DB path is impossible.
//!
//! Everything is offline: the real SQLite lives in a temp dir via
//! `KIRA_STORE_DIR`, no PAT/org/project is set (`KIRA_CONFIG` points to an
//! absent file), and `kira doctor` therefore fails fast in step 1 — exactly
//! what we want to record as a failed session. No process-global env var is
//! mutated: every env override is passed to the spawned binary only.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Isolated sandbox: `cwd` (project root, detected like in real usage) and a
/// separate store directory.
struct Sandbox {
    /// `…/e2e-…` — removed as a whole on Drop.
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
            "kira-store-e2e-{}-{name}-{nanos}",
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

    /// Path to be used as `KIRA_STORE_DIR` that CANNOT be created: the parent
    /// is a regular file, so `create_dir_all` fails — the fail-open probe.
    fn impossible_store(&self) -> PathBuf {
        let blocker = self.cwd.join("not-a-dir");
        std::fs::write(&blocker, "arquivo comum")
            .unwrap_or_else(|err| panic!("escrever blocker: {err}"));
        blocker.join("inner")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Spawns the built `kira` inside the sandbox and returns (exit code,
/// stdout, stderr). Azure credentials are stripped so `doctor` never touches
/// the network.
fn run_kira(sandbox: &Sandbox, store: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_kira"))
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", store)
        .env("KIRA_CONFIG", sandbox.store.join("absent-config.toml"))
        .env_remove("AZURE_DEVOPS_PAT")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("KIRA_MODEL")
        .env_remove("KIRA_BOARD")
        .output()
        .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn doctor_run_is_recorded_and_history_shows_it() {
    let sandbox = Sandbox::new("history");
    let (code, out, err) = run_kira(&sandbox, &sandbox.store, &["doctor"]);
    assert_eq!(code, 1, "sem PAT o doctor falha — saída: {out}{err}");
    assert!(
        out.contains("Variáveis ausentes"),
        "checkpoint 1 do doctor: {out}"
    );
    assert!(
        !err.contains("aviso: trilha não foi gravada"),
        "store válido não pode avisar falha de trilha: {err}"
    );

    let (code, out, err) = run_kira(&sandbox, &sandbox.store, &["history"]);
    assert_eq!(code, 0, "saída: {out}{err}");
    assert!(out.contains("história do projeto 'proj'"), "{out}");
    assert!(out.contains("✗"), "sessão do doctor falha => ✗: {out}");
    assert!(out.contains("doctor"), "comando na linha: {out}");
    assert!(
        out.contains("1. Configuração"),
        "primeira linha do summary deve aparecer: {out}"
    );

    // A second run lands above the first one; the default history shows both.
    let (_, _, _) = run_kira(&sandbox, &sandbox.store, &["doctor"]);
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["history"]);
    assert_eq!(code, 0);
    assert!(out.contains("#1"), "as duas sessões: {out}");
    assert!(out.contains("#2"), "as duas sessões: {out}");

    // --last 1 keeps only the newest row.
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["history", "--last", "1"]);
    assert_eq!(code, 0);
    assert!(out.contains("#2"), "recuro Recente em --last 1: {out}");
    assert!(
        !out.contains("#1"),
        "--last 1 não pode trazer a sessão mais antiga: {out}"
    );
}

#[test]
fn notes_flow_through_note_and_profile() {
    let sandbox = Sandbox::new("notes");
    let (code, out, err) = run_kira(&sandbox, &sandbox.store, &["note", "build roda com make"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("fato gravado: note-1"), "{out}");
    let (code, out, _) = run_kira(
        &sandbox,
        &sandbox.store,
        &["note", "deploy manual via cargo build"],
    );
    assert_eq!(code, 0);
    assert!(out.contains("note-2"), "{out}");

    let (code, profile, _) = run_kira(&sandbox, &sandbox.store, &["profile"]);
    assert_eq!(code, 0);
    assert!(profile.contains("projeto: proj"), "{profile}");
    let expected_root = {
        let canonical = std::fs::canonicalize(&sandbox.cwd)
            .unwrap_or_else(|err| panic!("canonicalizar cwd: {err}"))
            .to_string_lossy()
            .to_string();
        // Same transform the CLI applies when displaying (drop the Windows
        // extended-length prefix).
        match canonical.strip_prefix("\\\\?\\") {
            Some(inner) => inner.to_string(),
            None => canonical,
        }
    };
    assert!(
        profile.contains(&format!("root: {expected_root}")),
        "root canônico do projeto: {profile}"
    );
    assert!(profile.contains("primeira sessão:"), "{profile}");
    assert!(profile.contains("note-1"), "{profile}");
    assert!(profile.contains("build roda com make"), "{profile}");
    assert!(profile.contains("note-2"), "{profile}");

    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["note", "--remove", "1"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("fato removido: note-1"), "{out}");
    let (code, profile, _) = run_kira(&sandbox, &sandbox.store, &["profile"]);
    assert_eq!(code, 0);
    assert!(!profile.contains("note-1"), "removido some: {profile}");
    assert!(profile.contains("note-2"), "restante fica: {profile}");

    // Unknown note number: a real (non-fail-open) error for a store command.
    let (code, _, err) = run_kira(&sandbox, &sandbox.store, &["note", "--remove", "42"]);
    assert_eq!(code, 1);
    assert!(err.contains("não existe o fato 'note-42'"), "{err}");
    assert!(
        err.contains("kira profile"),
        "erro deve dizer o que fazer: {err}"
    );

    // Form validation.
    let (code, _, err) = run_kira(&sandbox, &sandbox.store, &["note"]);
    assert_eq!(code, 1);
    assert!(err.contains("informe o texto"), "{err}");
    let (code, _, err) = run_kira(
        &sandbox,
        &sandbox.store,
        &["note", "texto", "--remove", "1"],
    );
    assert_eq!(code, 1);
    assert!(err.contains("apenas uma ação"), "{err}");
}

#[test]
fn store_broken_means_only_a_warning_for_trail_commands() {
    let sandbox = Sandbox::new("fail-open");
    let impossible = sandbox.impossible_store();

    // inspector/doctor/test fosters a failed doctor session (pre-existing reason:
    // no PAT) — the store failure may add ONLY the warning line.
    let (code, _, err) = run_kira(&sandbox, &impossible, &["doctor"]);
    assert_eq!(code, 1);
    assert!(
        err.contains("aviso: trilha não foi gravada"),
        "deve avisar que a trilha não foi gravada: {err}"
    );
    assert!(
        err.contains("não foi possível criar o diretório do store"),
        "aviso deve explicar a causa: {err}"
    );
    // Exactly the warning: no second error line from the store itself.
    assert_eq!(
        err.lines().count(),
        1,
        "nada além de um aviso no stderr: {err}"
    );

    // The same broken store does not corrupt a full run's stdout: doctor's
    // diagnosis still printed (checked via its missing-env summary).
    let (code, out, _) = run_kira(&sandbox, &impossible, &["doctor"]);
    assert_eq!(code, 1);
    assert!(out.contains("Variáveis ausentes"), "{out}");

    // history remains usable on a healthy store right after.
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["history"]);
    assert_eq!(code, 0);
    assert!(out.contains("nenhuma sessão registrada"), "{out}");
}

#[test]
fn history_and_profile_without_any_session_are_friendly_offline() {
    let sandbox = Sandbox::new("empty");
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["history"]);
    assert_eq!(code, 0);
    assert!(out.contains("nenhuma sessão registrada"), "{out}");
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["profile"]);
    assert_eq!(code, 0);
    assert!(out.contains("nenhum perfil"), "{out}");
    // Two queries in a row stay consistent (no project materializes).
    let (code, out, _) = run_kira(&sandbox, &sandbox.store, &["profile"]);
    assert_eq!(code, 0);
    assert!(
        out.contains("nenhum perfil"),
        "segunda leitura idêntica: {out}"
    );
}
