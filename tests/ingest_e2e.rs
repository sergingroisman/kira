//! End-to-end de `kira ingest` (ADR-0038, M1).
//!
//! O contrato no limite do CLI é: mesma árvore ⇒ mesmo `.kira/digest.md` **byte a
//! byte**, sem rede (`--skip-scan`), com segredo mascarado no corpo dos arquivos-chave
//! e com o que a denylist decide sendo respeitado de verdade (monorepo entra,
//! `node_modules` não). Tudo offline: store em tempdir, repositório de fixture.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        Self::with_dir_name(tag, "projeto")
    }

    fn with_dir_name(tag: &str, dir_name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-ingest-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join(dir_name);
        let store = base.join("store");
        fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        Self { base, cwd, store }
    }

    fn write(&self, rel: &str, body: &str) {
        let path = self.cwd.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
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
            .env_remove("KIRA_INDEX_MAX_FILES")
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }

    fn digest(&self) -> String {
        fs::read_to_string(self.cwd.join(".kira/digest.md"))
            .unwrap_or_else(|err| panic!("ler digest: {err}"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn seed(sandbox: &Sandbox) {
    sandbox.write(
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    );
    sandbox.write("README.md", "# Demo\n\nprojeto de prova do ingest.\n");
    sandbox.write("src/main.rs", "fn main() {}\n");
    // Monorepo: era invisível quando `packages` estava na lista de diretórios pulados.
    sandbox.write("packages/core/src/lib.rs", "pub fn core() {}\n");
    // Ruído que nunca deve entrar.
    sandbox.write("node_modules/dep/index.js", "module.exports = 1;\n");
    sandbox.write("target/debug/lixo.rs", "fn lixo() {}\n");
}

#[test]
fn two_ingests_produce_a_byte_identical_digest() {
    let sandbox = Sandbox::new("identico");
    seed(&sandbox);

    let (code, stdout, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(
        stdout.contains("ingerido: 4 arquivo(s)"),
        "stdout: {stdout}"
    );
    let first = sandbox.digest();

    let (code, _, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "segunda execução falhou: {stderr}");
    let second = sandbox.digest();

    assert_eq!(first, second, "o digest não é determinístico");
    // Sem relógio no artefato: nada de hora/data no cabeçalho.
    assert!(!first.contains("T00:"), "digest não pode ter timestamp");
}

#[test]
fn monorepo_source_is_ingested_and_noise_is_not() {
    let sandbox = Sandbox::new("monorepo");
    seed(&sandbox);
    let (code, _, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let digest = sandbox.digest();
    assert!(digest.contains("packages/"), "monorepo ausente:\n{digest}");
    assert!(
        digest.contains("lib.rs"),
        "arquivo do monorepo ausente:\n{digest}"
    );
    assert!(
        !digest.contains("node_modules"),
        "node_modules entrou:\n{digest}"
    );
    assert!(!digest.contains("target/"), "target entrou:\n{digest}");
    assert!(
        digest.contains("stacks: rust"),
        "stack não detectada:\n{digest}"
    );
}

#[test]
fn secrets_in_key_files_are_masked_in_the_digest() {
    let sandbox = Sandbox::new("segredo");
    seed(&sandbox);
    sandbox.write(
        "src/auth.rs",
        "pub const TOKEN: &str = \"ghp_abcdefghijklmnopqrstuvwxyz0123456789\";\n",
    );
    let (code, _, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let digest = sandbox.digest();
    assert!(
        !digest.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"),
        "token vazou para o digest:\n{digest}"
    );
}

#[test]
fn json_report_is_parseable_and_carries_the_same_numbers() {
    let sandbox = Sandbox::new("json");
    seed(&sandbox);
    let (code, stdout, stderr) = sandbox.run(&["ingest", "--skip-scan", "--json"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("JSON inválido ({err}): {stdout}"));
    assert_eq!(report["files_ingested"], 4, "report: {stdout}");
    assert_eq!(report["findings_total"], serde_json::Value::Null);
    assert!(
        report["digest_path"]
            .as_str()
            .is_some_and(|path| path.ends_with(".kira/digest.md")),
        "digest_path: {stdout}"
    );
}

/// Regressão do ADR-0038: projeto cuja raiz se chama `build` era ignorado inteiro.
#[test]
fn project_root_named_build_is_ingested() {
    let sandbox = Sandbox::with_dir_name("raizbuild", "build");
    sandbox.write("Cargo.toml", "[package]\nname = \"dentro\"\n");
    sandbox.write("src/main.rs", "fn main() {}\n");
    let (code, stdout, stderr) = sandbox.run(&["ingest", "--skip-scan"]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    // A raiz `build/` precisa ser ingerida: era esse nome "reservado" na RAIZ que
    // zerava índice e digest antes do ADR-0038.
    let digest = sandbox.digest();
    assert!(
        digest.contains("Cargo.toml"),
        "a raiz precisa ser ingerida:\n{digest}"
    );
    assert!(
        digest.contains("src/main.rs"),
        "o código da raiz precisa entrar:\n{digest}"
    );
}
