//! E2E do `kira forecast` (FC-02): a jornada real é `kira scan` → série no
//! store → previsão com erro medido. O scan de verdade exige ferramentas de
//! stack no PATH, então o e2e semeia a série pela MESMA API que o
//! `scan_command` chama (`Store::record_findings_series`) e prova que o
//! binário lê a série, prevê, mede o erro contra as baselines — e que a
//! série curta demais vira erro acionável, nunca forecast fingido.

use std::path::PathBuf;

use kira::store::{FindingsCounts, Store, project_id_for_root};

struct Sandbox {
    /// Raiz do projeto-fixture (deve existir e ser detectável).
    root: PathBuf,
    /// Diretório do store (`KIRA_STORE_DIR` do filho).
    store: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("kira-forecast-e2e-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("proj");
        std::fs::create_dir_all(root.join("src")).unwrap_or_else(|err| panic!("criar raiz: {err}"));
        std::fs::write(root.join("src/lib.rs"), "pub fn ok() {}\n")
            .unwrap_or_else(|err| panic!("escrever fixture: {err}"));
        let store = base.join("store");
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar store: {err}"));
        Self { root, store }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap_or(&self.root));
    }
}

fn run_kira(sandbox: &Sandbox, args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_kira"))
        .args(args)
        .current_dir(&sandbox.root)
        .env("KIRA_STORE_DIR", &sandbox.store)
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

fn seed_series(sandbox: &Sandbox, totals: &[usize]) {
    let store = Store::open_in_dir(&sandbox.store).unwrap_or_else(|err| panic!("abrir: {err}"));
    let project_id =
        project_id_for_root(&sandbox.root).unwrap_or_else(|err| panic!("project id: {err}"));
    for (index, total) in totals.iter().enumerate() {
        let counts = FindingsCounts {
            total: *total,
            critical: usize::from(index % 2 == 0),
            ..FindingsCounts::default()
        };
        store
            .record_findings_series(&project_id, &counts)
            .unwrap_or_else(|err| panic!("semeiar ponto {index}: {err}"));
    }
    // Solta o handle antes do filho abrir o mesmo banco (WAL aguenta as duas
    // conexões, mas fechar é mais limpo e evita lock no Windows).
    drop(store);
}

#[test]
fn forecast_reads_the_series_predicts_and_reports_the_measured_error() {
    let sandbox = Sandbox::new("ok");
    // Série 1..=8: a tendência explica tudo, então o backtest tem que
    // provar MAE 0.0 para a tendência e vencer as duas baselines.
    seed_series(&sandbox, &[1, 2, 3, 4, 5, 6, 7, 8]);
    let (code, stdout, stderr) = run_kira(
        &sandbox,
        &["forecast", "--json", "--horizon", "2", "--field", "total"],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|err| panic!("JSON do forecast: {err}"));
    assert_eq!(report["series_points"], 8, "{report}");
    assert_eq!(report["horizon"], 2, "{report}");
    let predictions = report["predictions"]
        .as_array()
        .unwrap_or_else(|| panic!("predictions: {report}"));
    assert_eq!(predictions.len(), 2, "{report}");
    assert!(
        (predictions[0].as_f64().unwrap_or(0.0) - 9.0).abs() < 0.5,
        "tendência de 1..=8 prevê ~9: {report}"
    );
    let backtest = report["backtest"]
        .as_array()
        .unwrap_or_else(|| panic!("backtest: {report}"));
    assert_eq!(backtest.len(), 3, "3 baselines: {report}");
    assert_eq!(report["best_baseline"], "tendência", "{report}");
    let trend_mae = backtest[0]["mae"].as_f64().unwrap_or(f64::MAX);
    assert!(
        trend_mae < 0.05,
        "tendência perfeita em série linear: MAE {trend_mae}"
    );
}

#[test]
fn forecast_text_output_carries_series_predictions_band_and_backtest() {
    let sandbox = Sandbox::new("text");
    seed_series(&sandbox, &[1, 2, 3, 4, 5, 6, 7, 8]);
    let (code, stdout, stderr) = run_kira(&sandbox, &["forecast", "--horizon", "2"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("série: 8 ponto(s) de scan"), "{stdout}");
    assert!(
        stdout.contains("previsão para os próximos 2 scan(s)"),
        "{stdout}"
    );
    assert!(stdout.contains("banda p10–p90"), "{stdout}");
    assert!(
        stdout.contains("backtest (últimos 2 ponto(s) reservados"),
        "{stdout}"
    );
    assert!(stdout.contains("tendência"), "{stdout}");
    assert!(stdout.contains("último valor"), "{stdout}");
    assert!(stdout.contains("média móvel(3)"), "{stdout}");
    assert!(stdout.contains("melhor baseline: tendência"), "{stdout}");
    assert!(stdout.contains("ADR-0016"), "{stdout}");
}

#[test]
fn forecast_without_series_is_actionable_and_never_fakes_a_prediction() {
    let sandbox = Sandbox::new("vazio");
    // Nenhum ponto: o forecast da casa recusa com erro acionável — nunca
    // devolve previsão fingida (o aceite do FC-02).
    let (code, stdout, stderr) = run_kira(&sandbox, &["forecast"]);
    assert_ne!(code, 0, "série vazia deve falhar");
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(
        stderr.contains("pelo menos 2"),
        "erro acionável aponta o que fazer: {stderr}"
    );
    assert!(stderr.contains("kira scan"), "aponta o insumo: {stderr}");
}

#[test]
fn forecast_unknown_field_lists_the_valid_fields() {
    let sandbox = Sandbox::new("campo");
    seed_series(&sandbox, &[1, 2, 3]);
    let (code, _stdout, stderr) = run_kira(&sandbox, &["forecast", "--field", "zzz"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("total | critical | high | medium | low | other"),
        "lista os campos válidos: {stderr}"
    );
}
