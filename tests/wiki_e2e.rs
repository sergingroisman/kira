//! End-to-end for `kira wiki` (W1/PR28, ADR-0013).
//!
//! Drives the real binary: the point is the contract at the CLI boundary —
//! `.kira/wiki/` is created where the project is, secrets never reach the disk,
//! and `list`/`show` read back what `decision` wrote.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("kira-wiki-e2e-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        Self { cwd, store }
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
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

    fn wiki_dir(&self) -> PathBuf {
        self.cwd.join(".kira").join("wiki")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if let Some(base) = self.cwd.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("ler {path:?}: {err}"))
}

#[test]
fn list_on_an_empty_wiki_explains_how_to_start() {
    let sandbox = Sandbox::new("empty");
    let (code, stdout, stderr) = sandbox.run(&["wiki", "list"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("está vazia"), "{stdout}");
    assert!(stdout.contains("kira wiki decision"), "acionável: {stdout}");
}

#[test]
fn decision_creates_the_committed_page_and_masks_secrets() {
    let sandbox = Sandbox::new("decision");
    let (code, stdout, stderr) = sandbox.run(&[
        "wiki",
        "decision",
        "usar token=AbC123XyZ789QqQ no pipeline noturno",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let page = sandbox.wiki_dir().join("decisoes.md");
    assert!(
        page.is_file(),
        ".kira/wiki/decisoes.md deveria existir: {stdout}"
    );
    let content = read(&page);
    assert!(content.contains("pipeline noturno"), "{content}");
    assert!(content.contains("kira-wiki page=decisoes"), "{content}");
    assert!(
        !content.contains("AbC123XyZ789QqQ"),
        "segredo foi para o disco: {content}"
    );
    assert!(content.contains("[REDACTED]"), "{content}");
}

#[test]
fn list_and_show_read_back_what_was_written() {
    let sandbox = Sandbox::new("roundtrip");
    let (code, _, stderr) = sandbox.run(&["wiki", "decision", "primeira decisão registrada"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "list"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("decisoes"), "{stdout}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "show", "decisoes"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("primeira decisão registrada"), "{stdout}");
}

#[test]
fn show_of_a_missing_page_suggests_the_next_step() {
    let sandbox = Sandbox::new("missing");
    let (code, stdout, stderr) = sandbox.run(&["wiki", "show", "contexto"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("ainda não existe"), "{stdout}");
    assert!(stdout.contains("kira wiki decision"), "{stdout}");
}

#[test]
fn unknown_page_id_is_refused_without_writing_anything() {
    let sandbox = Sandbox::new("denied");
    let (code, _, stderr) = sandbox.run(&["wiki", "show", "../../etc/passwd"]);
    assert_ne!(code, 0, "id inválido deveria falhar");
    assert!(stderr.contains("erro:"), "{stderr}");
    assert!(!sandbox.wiki_dir().exists(), "nada pode ser criado");
}

#[test]
fn reindex_then_search_finds_the_page_through_the_real_binary() {
    let sandbox = Sandbox::new("index");
    let (code, _, stderr) = sandbox.run(&[
        "wiki",
        "decision",
        "o fix roda dentro de um jail minimo com rlimits",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "reindex"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("1 páginas"), "{stdout}");
    assert!(stdout.contains("FTS5"), "engine esperado: {stdout}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "search", "rlimits"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("decisoes"), "{stdout}");
    assert!(stdout.contains("jail"), "snippet com contexto: {stdout}");
}

#[test]
fn promote_takes_staged_pages_into_the_committed_wiki() {
    let sandbox = Sandbox::new("promote");
    let staging = sandbox.cwd.join(".kira").join("wiki-staging");
    fs::create_dir_all(&staging).unwrap_or_else(|err| panic!("criar {staging:?}: {err}"));
    fs::write(
        staging.join("bugs-99-token-sem-rotacao.md"),
        "<!-- kira-wiki staged updated_at=2026-11-10T00:00:00Z -->\n\ncausa raiz: token sem rotação\n",
    )
    .unwrap_or_else(|err| panic!("staged: {err}"));

    let (code, stdout, stderr) = sandbox.run(&["wiki", "promote"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("bugs/99-token-sem-rotacao"), "{stdout}");

    let page = sandbox
        .wiki_dir()
        .join("bugs")
        .join("99-token-sem-rotacao.md");
    assert!(page.is_file(), "página promovida deveria existir");
    assert!(
        !staging.join("bugs-99-token-sem-rotacao.md").exists(),
        "staging limpo"
    );
}

#[test]
fn promote_with_empty_staging_says_so() {
    let sandbox = Sandbox::new("promote-empty");
    let (code, stdout, stderr) = sandbox.run(&["wiki", "promote"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("nada em .kira/wiki-staging/"), "{stdout}");
}

#[test]
fn export_prints_one_bundle_for_every_page() {
    let sandbox = Sandbox::new("export");
    let (code, _, stderr) = sandbox.run(&["wiki", "decision", "primeira decisão"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "export"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.starts_with("# KIRA wiki"), "{stdout}");
    assert!(stdout.contains("## decisoes"), "{stdout}");
    assert!(stdout.contains("primeira decisão"), "{stdout}");
}

#[test]
fn export_out_refuses_to_leave_the_project() {
    let sandbox = Sandbox::new("export-guard");
    let (code, _, stderr) = sandbox.run(&["wiki", "decision", "algo"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, _, stderr) = sandbox.run(&["wiki", "export", "--out", "../fora.md"]);
    assert_ne!(code, 0, "export para fora deveria falhar");
    assert!(stderr.contains("erro:"), "{stderr}");
}

#[test]
fn search_without_reindex_is_actionable_not_silent() {
    let sandbox = Sandbox::new("noindex");
    let (code, _, stderr) = sandbox.run(&["wiki", "decision", "alguma decisão"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "search", "decisão"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("nenhuma página"), "{stdout}");
    assert!(stdout.contains("kira wiki reindex"), "acionável: {stdout}");
}

/// Writes a bug page the way the episodic capture does, so the test never needs
/// a model: the contract under test is `forget`, not the capture.
fn write_bug_page(sandbox: &Sandbox, slug: &str, body: &str) -> PathBuf {
    let path = sandbox.wiki_dir().join("bugs").join(format!("{slug}.md"));
    fs::create_dir_all(path.parent().unwrap_or(&sandbox.cwd))
        .unwrap_or_else(|err| panic!("criar bugs/: {err}"));
    fs::write(
        &path,
        format!("<!-- kira-wiki page=bugs/{slug} updated_at=2026-11-10T22:18:00Z -->\n\n{body}\n"),
    )
    .unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    path
}

#[test]
fn forget_deletes_the_page_and_its_search_entries() {
    let sandbox = Sandbox::new("forget");
    let path = write_bug_page(
        &sandbox,
        "4821",
        "# Bug 4821\n\n## Episódio 2026-11-10T22:18:00Z · item 4821\n- Causa raiz prevista: TTL do token curto\n",
    );

    let (code, _, stderr) = sandbox.run(&["wiki", "reindex"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let (code, stdout, stderr) = sandbox.run(&["wiki", "search", "TTL"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("bugs/4821"), "indexado antes: {stdout}");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "forget", "bugs/4821"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("apagada"), "{stdout}");
    assert!(!path.exists(), "o markdown deveria sumir do disco");

    let (code, stdout, stderr) = sandbox.run(&["wiki", "search", "TTL"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("nenhuma página"),
        "o índice não pode guardar a página esquecida: {stdout}"
    );
}

#[test]
fn forget_of_a_missing_page_is_not_an_error() {
    let sandbox = Sandbox::new("forget-missing");
    let (code, stdout, stderr) = sandbox.run(&["wiki", "forget", "bugs/9999"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("não existe"), "{stdout}");
    assert!(stdout.contains("kira wiki list"), "acionável: {stdout}");
}

#[test]
fn forget_refuses_a_traversing_page_id() {
    let sandbox = Sandbox::new("forget-denied");
    let victim = sandbox.cwd.join("importante.txt");
    fs::write(&victim, "não pode sumir").unwrap_or_else(|err| panic!("criar {victim:?}: {err}"));
    let (code, _, stderr) = sandbox.run(&["wiki", "forget", "../../importante.txt"]);
    assert_ne!(code, 0, "id com '..' deveria falhar");
    assert!(stderr.contains("erro:"), "{stderr}");
    assert!(victim.is_file(), "nada fora de .kira/wiki pode ser apagado");
}
