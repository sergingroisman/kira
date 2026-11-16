//! End-to-end do círculo de aprovação humana (F2, PR18) — a guarda da
//! autonomia vista pelo CLI.
//!
//! O pedido nasce no store (hoje quem cria é o código, não o CLI), então o
//! teste grava a linha `pending` direto no `store.db` — a mesma porta que o
//! query do `kira approvals` lê — e prova o resto pelo binário real:
//! listar → aprovar/recusar → estado final, com a decisão sendo ÚNICA.

use std::path::PathBuf;
use std::process::Command;

use rusqlite::Connection;

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "kira-approvals-e2e-{}-{tag}-{nanos}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&store).expect("store");
        std::fs::write(cwd.join("README.md"), "# fixture\n").expect("readme");
        Sandbox { base, cwd, store }
    }

    fn db(&self) -> PathBuf {
        self.store.join("store.db")
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_LLM")
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    /// Cria um pedido `pending` direto no store (é o que o ciclo/fix faria) e
    /// devolve o id. O store já existe porque o primeiro `kira approvals` o
    /// cria com o schema corrente.
    fn seed_request(&self, action: &str, target: &str, summary: &str) -> i64 {
        let conn = Connection::open(self.db()).expect("abrir store");
        conn.execute(
            "INSERT INTO approvals (action, target, summary, state, created_at) \
             VALUES (?1, ?2, ?3, 'pending', '2026-11-16T00:00:00Z')",
            rusqlite::params![action, target, summary],
        )
        .expect("semear pedido");
        conn.last_insert_rowid()
    }

    fn state_of(&self, id: i64) -> String {
        let conn = Connection::open(self.db()).expect("abrir store");
        conn.query_row(
            "SELECT state FROM approvals WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get::<_, String>(0),
        )
        .expect("estado")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

#[test]
fn empty_state_says_there_is_nothing_waiting() {
    let sandbox = Sandbox::new("empty");
    let (code, stdout, stderr) = sandbox.run(&["approvals"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("nenhuma escrita no mundo aguardando aprovação"),
        "{stdout}"
    );
}

#[test]
fn approve_flow_lists_then_decides_once() {
    let sandbox = Sandbox::new("approve");
    // Primeiro uso cria o store no schema corrente.
    let _ = sandbox.run(&["approvals"]);
    let id = sandbox.seed_request("fix.apply", "src/main.rs", "aplicar patch do fix");

    let (code, stdout, stderr) = sandbox.run(&["approvals"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains(&format!("id {id} · fix.apply · src/main.rs")),
        "{stdout}"
    );
    assert!(stdout.contains("[pending]"), "{stdout}");
    assert!(stdout.contains("kira approve <id>"), "{stdout}");

    let (code, stdout, stderr) = sandbox.run(&["approve", &id.to_string()]);
    assert_eq!(code, 0, "approve falhou: {stderr}");
    assert!(
        stdout.contains(&format!("pedido {id} aprovado por dono")),
        "{stdout}"
    );
    assert_eq!(sandbox.state_of(id), "approved");

    // Decisão é única — a segunda não desfaz nem reescreve.
    let (code, stdout, _) = sandbox.run(&["approve", &id.to_string(), "--reject"]);
    assert_eq!(code, 0, "segunda decisão não deve explodir");
    assert!(stdout.contains("já estava `approved`"), "{stdout}");
    assert_eq!(sandbox.state_of(id), "approved");

    // Sem pendentes, mas visível no histórico.
    let (code, stdout, _) = sandbox.run(&["approvals"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("nenhuma escrita no mundo aguardando aprovação"),
        "{stdout}"
    );
    let (code, stdout, _) = sandbox.run(&["approvals", "--all"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pedidos (1)"), "{stdout}");
    assert!(stdout.contains("[approved]"), "{stdout}");
}

#[test]
fn reject_keeps_the_write_blocked() {
    let sandbox = Sandbox::new("reject");
    let _ = sandbox.run(&["approvals"]);
    let id = sandbox.seed_request("board.comment", "item-43", "comentar no item 43");

    let (code, stdout, stderr) = sandbox.run(&["approve", &id.to_string(), "--reject"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains(&format!("pedido {id} recusado")),
        "{stdout}"
    );
    assert_eq!(sandbox.state_of(id), "rejected");

    let (code, stdout, _) = sandbox.run(&["approvals"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("nenhuma escrita no mundo aguardando aprovação"),
        "recusado não é pendente: {stdout}"
    );
}

#[test]
fn unknown_id_is_actionable_and_never_approves_anything() {
    let sandbox = Sandbox::new("unknown");
    let (code, _stdout, stderr) = sandbox.run(&["approve", "99"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("não há pedido de aprovação com id 99"),
        "{stderr}"
    );
    assert!(stderr.contains("kira approvals"), "{stderr}");
}

#[test]
fn decided_by_flag_is_recorded() {
    let sandbox = Sandbox::new("by");
    let _ = sandbox.run(&["approvals"]);
    let id = sandbox.seed_request("publish.remote", "blog/post", "publicar post");

    let (code, _stdout, stderr) = sandbox.run(&["approve", &id.to_string(), "--by", "sergin"]);
    assert_eq!(code, 0, "{stderr}");
    let conn = Connection::open(sandbox.db()).expect("abrir store");
    let decided_by: String = conn
        .query_row(
            "SELECT decided_by FROM approvals WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .expect("decided_by");
    assert_eq!(decided_by, "sergin");
}
