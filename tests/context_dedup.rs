//! End-to-end do dedup contíguo nos trechos (PR38, onda TE).
//!
//! Drive o binário real: um projeto com imports duplicados em bloco (fixture
//! dourado `tests/fixtures/context/dedup/`) tem que chegar ao prompt com a
//! PRIMEIRA ocorrência + marcador contável (` ... ×N`), sem repetição nenhuma
//! no bloco de trechos, e o `--json` registra a economia medida
//! (`dedup_chars_saved`) que o ledger de economia (schema v14) grava.
//! Determinismo byte a byte: duas execuções no mesmo estado ⇒ mesmo pack.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Fixture dourado: o run contíguo de imports duplicados que o dedup colapsa.
const BOILERPLATE: &str = include_str!("fixtures/context/dedup/boilerplate.rs");
/// Fixture do segundo arquivo: linhas do mesmo termo, sem repetição — o dedup
/// NÃO pode tocar (e o mesmo texto não atravessa arquivo).
const DISTINTO: &str = include_str!("fixtures/context/dedup/distinto.rs");

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
            "kira-dedup-e2e-{}-{tag}-{nanos}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
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
            .env_remove("AZURE_DEVOPS_PAT")
            .env("KIRA_SECRET_STORE", "off")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_MODEL")
            .env_remove("KIRA_BOARD")
            .env_remove("KIRA_INDEX_MAX_FILES")
            .output()
            .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Work item cujo termo de busca é exatamente o do fixture dourado: os
/// keywords extraem `session_token` (≥ 5 chars) e as linhas dos imports
/// duplicados casam com ele.
fn write_item(sandbox: &Sandbox) {
    sandbox.write(
        "item.json",
        r#"{
  "id": 4821,
  "rev": 1,
  "fields": {
    "System.Id": 4821,
    "System.WorkItemType": "Bug",
    "System.Title": "session_token duplicado no boilerplate",
    "System.State": "Active",
    "System.Description": ""
  }
}"#,
    );
}

#[test]
fn the_deduped_pack_is_deterministic_and_carries_the_marker() {
    let sandbox = Sandbox::new("determinism-marker");
    write_item(&sandbox);
    sandbox.write("src/legacy/boilerplate.rs", BOILERPLATE);
    sandbox.write("src/other/distinto.rs", DISTINTO);

    let first = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(first.0, 0, "stderr: {}", first.2);
    let second = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(second.0, 0, "stderr: {}", second.2);
    // Aceite (c): mesmo estado ⇒ pack byte a byte idêntico (gate do M1).
    assert_eq!(first.1, second.1, "duas runs ⇒ mesmo pack");
    let json = &first.1;

    let parsed: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    let code = parsed["pack"]["code"]
        .as_array()
        .unwrap_or_else(|| panic!("code deve ser lista: {json}"))
        .clone();
    assert!(!code.is_empty(), "o fixture entra no pack: {json}");

    // Aceite (b): nenhuma repetição sobrevive no bloco de trechos — o par
    // (caminho, texto) é único (a primeira ocorrência fica; repetidas saem
    // contadas). E a economia medida existe e é positiva.
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut max_omitted = 0usize;
    for excerpt in &code {
        let key = (
            excerpt["path"].as_str().unwrap_or_default().to_string(),
            excerpt["text"].as_str().unwrap_or_default().to_string(),
        );
        assert!(
            !seen.contains(&key),
            "repetida sobreviveu ao dedup: {key:?} em {code:?}"
        );
        seen.push(key);
        let omitted = excerpt["omitted"].as_u64().unwrap_or(0) as usize;
        max_omitted = max_omitted.max(omitted);
    }
    assert!(
        max_omitted >= 1,
        "o fixture dourado TEM que ter um run colapsado: {code:?}"
    );
    let saved = parsed["pack"]["dedup_chars_saved"].as_u64().unwrap_or(0) as usize;
    assert!(
        saved > 0,
        "economia medida tem que ser positiva no fixture: {json}"
    );

    // O render humano mostra o marcador contável — e o arquivo `distinto.rs`
    // (linhas sem repetição) continua inteiro, cada âncora com dono.
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--explain"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains(" ... ×"), "marcador no render: {stdout}");
    assert!(
        stdout.contains("src/other/distinto.rs"),
        "arquivo sem repetição não é tocado: {stdout}"
    );
}

/// O `history` do triage registra a categoria nova do ledger (schema v14) —
/// aqui via e2e do binário, com o ollama falso do sandbox de triage? Não:
/// `kira context` não fala com modelo e não grava sessão — o round trip do
/// ledger está coberto pelo teste de migração do store e pelo e2e do triage.
/// O que ESTE teste prova é o contrato do comando de leitura: o `--json`
/// exporta o número que o ledger vai gravar.
#[test]
fn the_json_report_exports_the_dedup_savings_field() {
    let sandbox = Sandbox::new("json-field");
    write_item(&sandbox);
    sandbox.write("src/legacy/boilerplate.rs", BOILERPLATE);
    let (code, stdout, stderr) = sandbox.run(&["context", "--file", "item.json", "--json"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("JSON inválido: {err}"));
    assert!(
        parsed["pack"]["dedup_chars_saved"].is_u64(),
        "campo de economia do dedup no --json: {stdout}"
    );
}
