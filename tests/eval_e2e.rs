//! End-to-end test of `kira triage --eval` (W2-bis, ADR-0018): the fake
//! backend is a `copilot` CLI shim (`KIRA_COPILOT_BIN`, the same offline
//! contract as the triage e2e) — no network, no real Ollama.
//!
//! Covers: the per-item table + gate line, the exit code as the gate
//! (1 when < 70%, 0 when >= 70%), the `--json` report, the actionable
//! empty-directory error with the schema, and `--seed` (3 clearly synthetic
//! fixtures copied into an empty directory).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Unique scratch dir per test (fixtures, shims, captures never collide).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "kira-eval-e2e-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    fs::remove_dir_all(&dir).unwrap_or(());
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
    dir
}

/// A minimal work item the offline triage can read (`WorkItem::from_api_json`).
const ITEM_BODY: &str = r#"{
  "id": 529511,
  "rev": 1,
  "fields": {
    "System.Id": 529511,
    "System.WorkItemType": "Bug",
    "System.Title": "Sessão expira com token válido",
    "System.State": "Active"
  }
}"#;

/// Writes one eval item folder: `item.json` + `gold.json` with the given label.
fn write_item(
    dir: &Path,
    pasta: &str,
    item_id: i32,
    causa_raiz: &str,
    arquivos: &[&str],
    confianca: &str,
) -> PathBuf {
    let pasta_dir = dir.join(pasta);
    fs::create_dir_all(&pasta_dir).unwrap_or_else(|err| panic!("criar {pasta_dir:?}: {err}"));
    let item = ITEM_BODY.replace("529511", &item_id.to_string());
    fs::write(pasta_dir.join("item.json"), item)
        .unwrap_or_else(|err| panic!("escrever item.json: {err}"));
    let arquivos_json: Vec<String> = arquivos.iter().map(|a| format!("\"{a}\"")).collect();
    let gold = format!(
        "{{\n  \"causa_raiz\": \"{causa_raiz}\",\n  \"arquivos\": [{}],\n  \"confianca\": \"{confianca}\"\n}}\n",
        arquivos_json.join(", ")
    );
    fs::write(pasta_dir.join("gold.json"), gold)
        .unwrap_or_else(|err| panic!("escrever gold.json: {err}"));
    pasta_dir
}

/// Writes a fake `copilot` shim printing the HIT answer for every prompt
/// (`Causa raiz provável: TTL do token muito curto` / `Arquivos prováveis:
/// src/auth/session.rs` / `Confiança: alta`). The shim is invoked once per
/// item by the eval. Returns its path.
fn hit_shim(dir: &Path) -> PathBuf {
    let (name, script) = if cfg!(windows) {
        (
            "copilot.cmd",
            "@echo Causa raiz provável:\r\n@echo TTL do token muito curto.\r\n@echo Arquivos prováveis:\r\n@echo src/auth/session.rs\r\n@echo Confiança:\r\n@echo alta\r\n"
                .to_string(),
        )
    } else {
        (
            "copilot",
            "#!/bin/sh\nprintf 'Causa raiz provável:\\nTTL do token muito curto.\\nArquivos prováveis:\\nsrc/auth/session.rs\\nConfiança:\\nalta\\n'\n"
                .to_string(),
        )
    };
    let path = dir.join(name);
    fs::create_dir_all(dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
    fs::write(&path, script).unwrap_or_else(|err| panic!("escrever shim {path:?}: {err}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|err| panic!("chmod shim: {err}"));
    }
    path
}

/// Runs the built `kira` binary with the eval env (isolated store, dead
/// Ollama URL is NOT set here: the shim backend never touches it).
fn run_kira(store: &Path, shim: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .env("KIRA_STORE_DIR", store)
        .env("KIRA_CONFIG", store.join("absent-config.toml"))
        .env_remove("KIRA_LLM")
        .env_remove("KIRA_MODEL")
        .env_remove("AZURE_DEVOPS_PAT")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT");
    match shim {
        Some(shim) => {
            command.env("KIRA_COPILOT_BIN", shim);
        }
        None => {
            command.env_remove("KIRA_COPILOT_BIN");
        }
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

/// The mixed run: one item the fake backend answers right, one it answers
/// wrong ⇒ 1/2 = 50% < 70% ⇒ exit 1, table + gate line on stdout.
#[test]
fn eval_mixed_fixtures_exit_1_with_table_and_gate_line() {
    let base = scratch("mixed");
    let dir = base.join("gold");
    write_item(
        &dir,
        "01-hit",
        529511,
        "TTL do token muito curto",
        &["src/auth/session.rs"],
        "alta",
    );
    write_item(
        &dir,
        "02-miss",
        529512,
        "Corrupção de memória no serviço de cache distribuído",
        &["src/cache/service.cs"],
        "baixa",
    );
    let shim = hit_shim(&base.join("shim"));
    let (code, stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
        ],
    );
    assert_eq!(code, 1, "50% < 70% deve dar exit 1 — stderr: {stderr}");
    let aperta = |texto: &str| texto.split_whitespace().collect::<Vec<&str>>().join(" ");
    let linhas: Vec<String> = stdout.lines().map(&aperta).collect();
    assert!(
        linhas.contains(&"id | causa % | arquivos ok? | veredito".to_string()),
        "tabela: {stdout}"
    );
    assert!(
        linhas.contains(&"529511 | 100% | sim | ACERTOU".to_string()),
        "{stdout}"
    );
    assert!(
        linhas.contains(&"529512 | 0% | não | ERROU".to_string()),
        "{stdout}"
    );
    assert!(
        stdout.contains("GATE M2: 1/2 = 50% (limiar 70%) — NÃO PASSOU"),
        "{stdout}"
    );
    fs::remove_dir_all(&base).unwrap_or(());
}

/// Both items hit ⇒ 2/2 = 100% ≥ 70% ⇒ exit 0 with PASSOU.
#[test]
fn eval_all_hits_exit_0() {
    let base = scratch("allhits");
    let dir = base.join("gold");
    write_item(
        &dir,
        "01-hit",
        529511,
        "TTL do token muito curto",
        &["src/auth/session.rs"],
        "alta",
    );
    write_item(
        &dir,
        "02-hit",
        529513,
        "TTL do token muito curto",
        &["src/auth/session.rs"],
        "media",
    );
    let shim = hit_shim(&base.join("shim"));
    let (code, stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
        ],
    );
    assert_eq!(code, 0, "100% >= 70% deve dar exit 0 — stderr: {stderr}");
    assert!(
        stdout.contains("GATE M2: 2/2 = 100% (limiar 70%) — PASSOU"),
        "{stdout}"
    );
    fs::remove_dir_all(&base).unwrap_or(());
}

/// `--json` is machine-parseable and carries the same fields plus the
/// predicted/gold texts.
#[test]
fn eval_json_output_is_parseable_and_complete() {
    let base = scratch("json");
    let dir = base.join("gold");
    write_item(
        &dir,
        "01-hit",
        529511,
        "TTL do token muito curto",
        &["src/auth/session.rs"],
        "alta",
    );
    write_item(
        &dir,
        "02-miss",
        529512,
        "Corrupção de memória no serviço de cache distribuído",
        &["src/cache/service.cs"],
        "baixa",
    );
    let shim = hit_shim(&base.join("shim"));
    let (code, stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
            "--json",
        ],
    );
    assert_eq!(code, 1, "50% < 70% deve dar exit 1 — stderr: {stderr}");
    let report: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|err| panic!("json inválido: {err}\n{stdout}"));
    let itens = report["itens"]
        .as_array()
        .unwrap_or_else(|| panic!("itens"));
    assert_eq!(itens.len(), 2, "itens: {report}");
    assert_eq!(report["gate"]["acertos"], 1);
    assert_eq!(report["gate"]["total"], 2);
    assert_eq!(report["gate"]["passou"], false);
    assert_eq!(report["limiares"]["causa"], 0.5);
    assert_eq!(report["limiares"]["gate_pct"], 70.0);
    assert_eq!(itens[0]["id"], 529511);
    assert_eq!(itens[0]["veredito"], "ACERTOU");
    assert_eq!(itens[0]["causa_score"], 1.0);
    assert_eq!(itens[1]["veredito"], "ERROU");
    assert!(
        itens[1]["causa_prevista"]
            .as_str()
            .unwrap_or("")
            .contains("TTL"),
        "o texto previsto viaja no json: {}",
        itens[1]
    );
    assert_eq!(
        itens[1]["causa_gold"],
        "Corrupção de memória no serviço de cache distribuído"
    );
    // A linha do gate continua legível no stderr (contrato humano + JSON).
    assert!(
        stderr.contains("GATE M2: 1/2 = 50% (limiar 70%) — NÃO PASSOU"),
        "{stderr}"
    );
    fs::remove_dir_all(&base).unwrap_or(());
}

/// An empty (or absent) directory without `--seed` is an actionable error
/// carrying the schema (item.json + gold.json with a 5-line example).
#[test]
fn eval_empty_dir_without_seed_is_an_actionable_schema_error() {
    let base = scratch("empty");
    let dir = base.join("vazio");
    fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
    let shim = hit_shim(&base.join("shim"));
    let (code, stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
        ],
    );
    assert_eq!(code, 1, "diretório vazio é erro — stdout: {stdout}");
    assert!(stdout.is_empty(), "nada de medição falsa: {stdout}");
    assert!(stderr.contains("item.json"), "schema no erro: {stderr}");
    assert!(stderr.contains("gold.json"), "schema no erro: {stderr}");
    assert!(stderr.contains("causa_raiz"), "exemplo do gold: {stderr}");
    assert!(stderr.contains("--seed"), "dica do --seed: {stderr}");
    fs::remove_dir_all(&base).unwrap_or(());
}

/// `--seed` in an empty directory copies the 3 clearly synthetic fixtures and
/// measures them (the fake backend answers right only for seed 01).
#[test]
fn eval_seed_copies_synthetic_fixtures_and_measures_them() {
    let base = scratch("seed");
    let dir = base.join("gold");
    let shim = hit_shim(&base.join("shim"));
    let (code, stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
            "--seed",
        ],
    );
    assert!(
        stderr.contains("seeds sintéticas copiadas"),
        "nota do seed: {stderr}"
    );
    // As 3 pastas existem, com os 2 arquivos, e são nomeadas como sintéticas.
    for nome in [
        "seed-sintetico-01",
        "seed-sintetico-02",
        "seed-sintetico-03",
    ] {
        let pasta = dir.join(nome);
        assert!(
            pasta.join("item.json").is_file(),
            "falta item.json de {nome}"
        );
        assert!(
            pasta.join("gold.json").is_file(),
            "falta gold.json de {nome}"
        );
    }
    let gold = fs::read_to_string(dir.join("seed-sintetico-01/gold.json"))
        .unwrap_or_else(|err| panic!("ler gold seed: {err}"));
    assert!(
        gold.contains("sintética"),
        "a seed se declara sintética no próprio rótulo: {gold}"
    );
    // Seed 01 acerta (resposta falsa = TTL), 02/03 erram ⇒ 1/3 < 70% ⇒ exit 1.
    assert_eq!(code, 1, "1/3 < 70% — stderr: {stderr}");
    assert!(
        stdout.contains("seed-sintetico-01")
            || stdout.contains("GATE M2: 1/3 = 33% (limiar 70%) — NÃO PASSOU"),
        "{stdout}"
    );
    assert!(
        stdout.contains("GATE M2: 1/3 = 33% (limiar 70%) — NÃO PASSOU"),
        "{stdout}"
    );
    fs::remove_dir_all(&base).unwrap_or(());
}

/// `--seed` refuses to touch a NON-empty directory (owner labels are never
/// overwritten) and a missing backend fails loudly — no fake measurement.
#[test]
fn eval_seed_refuses_nonempty_dir_and_missing_backend_fails_loudly() {
    let base = scratch("seed-guard");
    let dir = base.join("gold");
    write_item(
        &dir,
        "01-hit",
        529511,
        "TTL do token muito curto",
        &["src/auth/session.rs"],
        "alta",
    );
    let shim = hit_shim(&base.join("shim"));
    let (code, _stdout, stderr) = run_kira(
        &base.join("store"),
        Some(&shim),
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
            "--seed",
        ],
    );
    assert_eq!(code, 1, "--seed em pasta cheia é erro: {stderr}");
    assert!(stderr.contains("não está vazio"), "{stderr}");
    // Backend caro sem o binário: erro acionável, nunca medição com dado falso.
    let (code, _stdout, stderr) = run_kira(
        &base.join("store"),
        None,
        &[
            "triage",
            "--eval",
            dir.to_string_lossy().as_ref(),
            "--llm",
            "copilot",
        ],
    );
    assert_eq!(code, 1, "backend ausente é erro: {stderr}");
    assert!(stderr.contains("Copilot"), "{stderr}");
    fs::remove_dir_all(&base).unwrap_or(());
}
