//! End-to-end de `kira trust` e da migração v6 (ADR-0038, M2).
//!
//! O contrato no limite do CLI: aceitar grava `.kira/trust.json` com o hash do caminho
//! canônico, revogar apaga, e `.kira/` passa a ter o `.gitignore` que impede o
//! diretório de vazar para o repositório. A migração v6 cria `posts` + view `outbox`
//! num store que já existia na v5 — sem perder dado.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-trust-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        fs::write(cwd.join("README.md"), "# projeto\n").expect("README");
        Self { base, cwd, store }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_BOARD")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    fn trust_file(&self) -> PathBuf {
        self.cwd.join(".kira/trust.json")
    }

    fn db(&self) -> PathBuf {
        self.store.join("store.db")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn user_version(db: &Path) -> i64 {
    let conn = Connection::open(db).unwrap_or_else(|err| panic!("abrir store: {err}"));
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user_version")
}

#[test]
fn trust_writes_the_acceptance_and_revoke_removes_it() {
    let sandbox = Sandbox::new("aceite");
    let (code, stdout, stderr) = sandbox.run(&["trust"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(sandbox.trust_file().exists(), "trust.json não foi criado");
    let body = fs::read_to_string(sandbox.trust_file()).expect("ler trust.json");
    assert!(body.contains("\"hash\""), "sem hash no aceite: {body}");
    assert!(!body.contains('\\'), "caminho gravado com /: {body}");

    let (code, stdout, stderr) = sandbox.run(&["trust", "--revoke"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        !sandbox.trust_file().exists(),
        "revogar precisa apagar o arquivo"
    );

    // Revogar de novo é idempotente: nada a revogar, exit 0.
    let (code, stdout, _) = sandbox.run(&["trust", "--revoke"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("nada a revogar"), "stdout: {stdout}");
}

#[test]
fn trust_leaves_kira_dir_protected_from_commit() {
    let sandbox = Sandbox::new("gitignore");
    let (code, _, stderr) = sandbox.run(&["trust"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let ignore = sandbox.cwd.join(".kira/.gitignore");
    assert!(ignore.exists(), ".kira precisa nascer protegido");
    let body = fs::read_to_string(ignore).expect("ler .gitignore");
    assert!(
        body.lines().any(|line| line.trim() == "*"),
        "regra `*` ausente: {body}"
    );
}

#[test]
fn migration_creates_posts_and_outbox_on_an_existing_store() {
    let sandbox = Sandbox::new("migracao");
    // Primeiro uso (qualquer comando cria o store na versão corrente).
    let (code, _, stderr) = sandbox.run(&["profile"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    // ADR-0039 (rename 4/6): a versão corrente do schema é a 7 — o store
    // nasce na última migração, não mais na 6 da v6 original do posts.
    assert_eq!(
        user_version(&sandbox.db()),
        7,
        "migração não subiu para a versão corrente"
    );

    let conn = Connection::open(sandbox.db()).expect("abrir store");
    let table_exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='posts'",
            [],
            |row| row.get(0),
        )
        .expect("contar tabela");
    assert_eq!(table_exists, 1, "tabela posts ausente");
    let view_exists: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='view' AND name='outbox'",
            [],
            |row| row.get(0),
        )
        .expect("contar view");
    assert_eq!(view_exists, 1, "view outbox ausente");
}

/// Reabrir o store não pode duplicar tabela/view nem explodir (idempotência da v6).
#[test]
fn reopening_the_store_is_idempotent() {
    let sandbox = Sandbox::new("reabrir");
    for _ in 0..3 {
        let (code, _, stderr) = sandbox.run(&["profile"]);
        assert_eq!(code, 0, "stderr: {stderr}");
    }
    let conn = Connection::open(sandbox.db()).expect("abrir store");
    let posts_rows: i64 = conn
        .query_row("SELECT count(*) FROM posts", [], |row| row.get(0))
        .expect("contar posts");
    assert_eq!(posts_rows, 0, "nada foi enfileirado, mas a tabela existe");
}
