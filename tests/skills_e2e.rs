//! End-to-end test of `kira skills` (onda SK, SK-1): bundled catalog, user
//! skills in `.kira/skills/`, validation + redaction on `add`, deterministic
//! FTS5 search and `show`.
//!
//! No network, no LLM: the command is pure catalog + derived index in an
//! isolated `KIRA_STORE_DIR`.

use std::path::{Path, PathBuf};
use std::process::Command;

const FAKE_TOKEN: &str = "AbC123XyZ789QqQ";

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "kira-skills-e2e-{tag}-{}-{}",
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

    fn write_file(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.base.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        std::fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
        path
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

/// A valid user skill with a fake secret in the body (must be masked on add).
fn user_skill_body() -> String {
    format!(
        "---\nname: testing\ndescription: Use when writing unit tests for Rust code.\nlicense: MIT\nauthor: Ana\nversion: 1.0.0\n---\n\n# Testing\n\n- configure: token={FAKE_TOKEN}\n- prefira teste pequeno\n"
    )
}

/// The bundled catalog is embedded in the binary and listed with its origin.
#[test]
fn skills_list_shows_the_bundled_catalog() {
    let sandbox = Sandbox::new("bundled");
    let (code, stdout, stderr) = sandbox.run(&["skills", "list"]);
    assert_eq!(code, 0, "list sai 0: {stdout}{stderr}");
    assert!(
        stdout.contains("react@1.0.0  [bundled]"),
        "react bundled com origem: {stdout}"
    );
    assert!(
        stdout.contains("drawio@1.0.0  [bundled]"),
        "drawio bundled: {stdout}"
    );
    assert!(
        stdout.contains("Use when editing React/JSX components"),
        "descrição com trigger: {stdout}"
    );
}

/// `add` validates the frontmatter, REDACTs the body and copies to
/// `.kira/skills/<name>.md`; the skill then lists with `origin: user`.
#[test]
fn skills_add_validates_redacts_and_lists_as_user() {
    let sandbox = Sandbox::new("add");
    let file = sandbox.write_file("origem/testing.md", &user_skill_body());
    let (code, stdout, stderr) = sandbox.run(&["skills", "add", &file.display().to_string()]);
    assert_eq!(code, 0, "add válido sai 0: {stdout}{stderr}");
    assert!(
        stdout.contains("skill adicionado: testing@1.0.0"),
        "{stdout}"
    );

    let stored = sandbox.read(".kira/skills/testing.md");
    assert!(!stored.is_empty(), "o skill foi gravado em .kira/skills/");
    assert!(
        !stored.contains(FAKE_TOKEN),
        "o corpo gravado precisa sair redigido: {stored}"
    );

    let (code, stdout, stderr) = sandbox.run(&["skills", "list"]);
    assert_eq!(code, 0, "list: {stderr}");
    assert!(
        stdout.contains("testing@1.0.0  [user]"),
        "o skill do usuário aparece com origem: {stdout}"
    );
}

/// A file without the required frontmatter is refused with an actionable
/// error and NOTHING is written into `.kira/skills/`.
#[test]
fn skills_add_refuses_invalid_frontmatter() {
    let sandbox = Sandbox::new("invalido");
    let file = sandbox.write_file("origem/quebrado.md", "# sem frontmatter\n");
    let (code, stdout, stderr) = sandbox.run(&["skills", "add", &file.display().to_string()]);
    assert_ne!(code, 0, "sem frontmatter não entra: {stdout}");
    assert!(
        stderr.contains("frontmatter ausente"),
        "erro acionável: {stderr}"
    );
    assert!(
        !Path::new(&sandbox.cwd).join(".kira/skills").exists(),
        "nada pode ser gravado na recusa"
    );
}

/// Search finds the user skill through the derived FTS5 index, anchored as
/// `skill:<name>@<version>`, and two runs produce byte-identical output.
#[test]
fn skills_search_is_anchored_and_deterministic() {
    let sandbox = Sandbox::new("busca");
    let file = sandbox.write_file("origem/testing.md", &user_skill_body());
    let (code, _, stderr) = sandbox.run(&["skills", "add", &file.display().to_string()]);
    assert_eq!(code, 0, "add: {stderr}");

    let (code, first, stderr) = sandbox.run(&["skills", "search", "unit tests"]);
    assert_eq!(code, 0, "search: {first} / {stderr}");
    assert!(
        first.contains("skill:testing@1.0.0"),
        "âncora de procedência: {first}"
    );
    let (code, second, _) = sandbox.run(&["skills", "search", "unit tests"]);
    assert_eq!(code, 0);
    assert_eq!(first, second, "busca determinística: {first} vs {second}");
}

/// `show` returns the body of the chosen skill (bundled included).
#[test]
fn skills_show_returns_the_body() {
    let sandbox = Sandbox::new("mostrar");
    let (code, stdout, stderr) = sandbox.run(&["skills", "show", "react"]);
    assert_eq!(code, 0, "show: {stderr}");
    assert!(stdout.contains("skill: react@1.0.0 [bundled]"), "{stdout}");
    assert!(stdout.contains("Componentes"), "corpo do react: {stdout}");
}

/// Unknown skill name is an actionable error, not an empty success.
#[test]
fn skills_show_unknown_name_is_actionable() {
    let sandbox = Sandbox::new("nao-existe");
    let (code, _, stderr) = sandbox.run(&["skills", "show", "nao-existe"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("não encontrado") && stderr.contains("skills list"),
        "erro diz o conserto: {stderr}"
    );
}
