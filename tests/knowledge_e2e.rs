//! End-to-end test of `kira knowledge` (onda SK, SK-2): owner markdown in
//! `<store_dir>/knowledge/*.md`, derived FTS5 index, reindex from scratch,
//! redaction before the store and deterministic anchored search.
//!
//! No network, no LLM: the command is pure disk + derived index in an
//! isolated `KIRA_STORE_DIR`. The owner directory is only ever READ.

use std::path::PathBuf;
use std::process::Command;

use rusqlite::Connection;

const FAKE_TOKEN: &str = "AbC123XyZ789QqQ";

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "kira-knowledge-e2e-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        std::fs::create_dir_all(cwd.join("src"))
            .unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        std::fs::write(cwd.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("manifesto");
        std::fs::write(cwd.join("src/main.rs"), "fn main() {}\n").expect("main");
        Self { base, cwd, store }
    }

    /// O diretório do dono segue o store (`<store_dir>/knowledge/`).
    fn knowledge_dir(&self) -> PathBuf {
        self.store.join("knowledge")
    }

    /// Caminho do store.db (para conferir o índice gravado).
    fn db(&self) -> PathBuf {
        self.store.join("store.db")
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
            .env_remove("KIRA_BOARD")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    fn write_knowledge(&self, file: &str, body: &str) {
        let dir = self.knowledge_dir();
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        std::fs::write(dir.join(file), body).unwrap_or_else(|err| panic!("escrever {file}: {err}"));
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A document with a fake secret in the body (must be masked in the index).
fn pbm_doc() -> String {
    format!("# Pague Menos PBM\n\n- backend .NET 10 no cliente\n- credencial: token={FAKE_TOKEN}\n")
}

/// reindex counts the *.md files of the owner directory and search anchors
/// by file stem; two runs produce byte-identical output.
#[test]
fn knowledge_reindex_and_search_are_anchored_and_deterministic() {
    let sandbox = Sandbox::new("busca");
    sandbox.write_knowledge("pbm.md", &pbm_doc());
    sandbox.write_knowledge("viagem-tap.md", "# Viagem TAP\n\n- ida e volta\n");

    let (code, stdout, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex: {stdout}{stderr}");
    assert!(
        stdout.contains("índice de conhecimento: 2 documento(s)"),
        "{stdout}"
    );

    let (code, first, stderr) = sandbox.run(&["knowledge", "search", "backend"]);
    assert_eq!(code, 0, "search: {first} / {stderr}");
    assert!(
        first.contains("knowledge:pbm") && first.contains("Pague Menos PBM"),
        "âncora é o stem + título: {first}"
    );
    let (code, second, _) = sandbox.run(&["knowledge", "search", "backend"]);
    assert_eq!(code, 0);
    assert_eq!(first, second, "busca determinística: {first} vs {second}");

    // Sem cabeçalho: o título é o nome do arquivo — determinístico dos dois
    // jeitos.
    sandbox.write_knowledge("sem-titulo.md", "só corpo, sem cabeçalho\n");
    let (code, stdout, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex 2: {stdout}{stderr}");
    assert!(
        stdout.contains("índice de conhecimento: 3 documento(s)"),
        "{stdout}"
    );
    let (code, stdout, _) = sandbox.run(&["knowledge", "search", "sem-titulo"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("knowledge:sem-titulo"),
        "stem vira título no fallback: {stdout}"
    );
}

/// The store NEVER keeps a secret: the reindexed text went in redacted, so
/// the `knowledge` table carries no raw token. The search then returns no
/// hit for it (the empty-hint echoes the user's own query — same precedent
/// as `skills search` — so the real assertion lives in the store and in the
/// absence of an anchored hit).
#[test]
fn knowledge_index_stores_redacted_text_only() {
    let sandbox = Sandbox::new("redacao");
    sandbox.write_knowledge("pbm.md", &pbm_doc());

    let (code, _, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex: {stderr}");

    let conn = Connection::open(sandbox.db()).expect("abrir store");
    let leaked: i64 = conn
        .query_row(
            "SELECT count(*) FROM knowledge WHERE body LIKE ?1 OR title LIKE ?1",
            [format!("%{FAKE_TOKEN}%")],
            |row| row.get(0),
        )
        .expect("contar segredo");
    assert_eq!(leaked, 0, "o store nunca guarda segredo");
    let docs: i64 = conn
        .query_row("SELECT count(*) FROM knowledge", [], |row| row.get(0))
        .expect("contar documentos");
    assert_eq!(docs, 1, "o documento entrou (já redigido)");

    // A busca não vaza o segredo nem como hit ancorado.
    let (code, stdout, _) = sandbox.run(&["knowledge", "search", FAKE_TOKEN]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains("knowledge:pbm"),
        "índice redigido não casa o segredo: {stdout}"
    );
}

/// Reindex is from scratch: a file removed from disk leaves the index too.
#[test]
fn knowledge_reindex_from_scratch_removes_gone_files() {
    let sandbox = Sandbox::new("do-zero");
    sandbox.write_knowledge("pbm.md", &pbm_doc());
    sandbox.write_knowledge("viagem-tap.md", "# Viagem TAP\n\n- ida e volta\n");
    let (code, _, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex: {stderr}");

    std::fs::remove_file(sandbox.knowledge_dir().join("pbm.md")).expect("remover pbm.md");
    let (code, stdout, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex 2: {stdout}{stderr}");
    assert!(
        stdout.contains("índice de conhecimento: 1 documento(s)"),
        "só o que resta no disco: {stdout}"
    );
    let (code, stdout, _) = sandbox.run(&["knowledge", "search", "backend"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("nenhum conhecimento casou"),
        "documento removido não pode casar: {stdout}"
    );
}

/// `list` without any knowledge dir is a hint with the actionable next step,
/// not an error; `list` shows the indexed docs with title and size.
#[test]
fn knowledge_list_is_free_and_empty_dir_gets_a_hint() {
    let sandbox = Sandbox::new("lista");

    let (code, stdout, stderr) = sandbox.run(&["knowledge", "list"]);
    assert_eq!(code, 0, "list sem diretório sai 0: {stdout}{stderr}");
    assert!(
        stdout.contains("nenhum texto em") && stdout.contains("kira knowledge reindex"),
        "dica acionável: {stdout}"
    );

    sandbox.write_knowledge("pbm.md", &pbm_doc());
    let (code, _, stderr) = sandbox.run(&["knowledge", "reindex"]);
    assert_eq!(code, 0, "reindex: {stderr}");

    let (code, stdout, stderr) = sandbox.run(&["knowledge", "list"]);
    assert_eq!(code, 0, "list: {stdout}{stderr}");
    assert!(
        stdout.contains("knowledge:pbm") && stdout.contains("Pague Menos PBM"),
        "listagem com âncora e título: {stdout}"
    );
}

/// Search without a reindex is a hint (the derived index is empty), never a
/// failure — the truth is on disk and nothing was destroyed.
#[test]
fn knowledge_search_without_index_gets_the_hint() {
    let sandbox = Sandbox::new("sem-indice");
    sandbox.write_knowledge("pbm.md", &pbm_doc());

    let (code, stdout, stderr) = sandbox.run(&["knowledge", "search", "backend"]);
    assert_eq!(code, 0, "search sem índice não é erro: {stdout}{stderr}");
    assert!(
        stdout.contains("nenhum conhecimento casou") && stdout.contains("kira knowledge reindex"),
        "{stdout}"
    );
}
