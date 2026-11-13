//! End-to-end test of `kira pr draft`: the offline draft from a store fixture
//! (triage session), `.kira/findings.json` fixture and wiki pages.
//!
//! No network, no PAT, no LLM: the binary runs inside a sandbox with
//! `KIRA_STORE_DIR` pointing at a temp SQLite store, exactly like the other
//! e2e suites. `--out` must write the same bytes as stdout; `--json` must be
//! parseable; an item without triage must exit 0 with `—` and the cause.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use kira::store::{Store, WikiPageRow};

const TRIAGE_SUMMARY: &str = "Causa raiz provável:\nTTL do token muito curto.\n\n\
     Arquivos prováveis:\nsrc/auth/session.rs\n\n\
     Confiança:\nmédia\n";

const FINDINGS_JSON: &str = r#"{
  "findings": [
    {
      "tool": "audit",
      "rule": "vulnerable",
      "package": "session-mgmt",
      "version": "1.2.0",
      "severity": "high",
      "path": "src/auth/session.rs",
      "message": "TTL fixo sem configurar",
      "fingerprint": "0000000000000001"
    },
    {
      "tool": "audit",
      "rule": "vulnerable",
      "package": "lodash",
      "version": "4.17.19",
      "severity": "critical",
      "path": "web/package.json",
      "message": "Prototype Pollution",
      "fingerprint": "0000000000000002"
    }
  ],
  "stacks": []
}"#;

const WIKI_BODY: &str = "TTL do token muito curto causa relogin frequente no session e no auth.";

/// Isolated sandbox: cwd (project root) + central store dir, removed on Drop.
struct Sandbox {
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
            "kira-prdraft-e2e-{}-{name}-{nanos}",
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

    /// Records a triage session for the item, the wiki pages and the findings
    /// file — the whole fixture of the happy path.
    fn setup_full_fixture(&self) {
        let store = Store::open_in_dir(&self.store)
            .unwrap_or_else(|err| panic!("abrir store fixture: {err}"));
        let root = self
            .cwd
            .canonicalize()
            .unwrap_or_else(|err| panic!("canonicalizar cwd: {err}"));
        let project = store
            .ensure_project(&root)
            .unwrap_or_else(|err| panic!("garantir projeto: {err}"));
        store
            .record(
                &project,
                "triage",
                "4821",
                Some("qwen:4b"),
                "2026-01-05T10:00:00Z",
                10,
                true,
                TRIAGE_SUMMARY,
                Some(4821),
            )
            .unwrap_or_else(|err| panic!("gravar sessão triage: {err}"));
        store
            .wiki_replace(&[WikiPageRow {
                slug: "bugs/4821-sessao-expirada".to_string(),
                kind: "bugs".to_string(),
                path: ".kira/wiki/bugs/4821-sessao-expirada.md".to_string(),
                title: "Sessão expirada com token válido".to_string(),
                updated_at: "2026-01-04T00:00:00Z".to_string(),
                content_hash: "0f0f0f0f0f0f0f0f".to_string(),
                body: WIKI_BODY.to_string(),
            }])
            .unwrap_or_else(|err| panic!("gravar páginas de wiki: {err}"));
        let kira_dir = self.cwd.join(".kira");
        std::fs::create_dir_all(&kira_dir).unwrap_or_else(|err| panic!("criar .kira: {err}"));
        std::fs::write(kira_dir.join("findings.json"), FINDINGS_JSON)
            .unwrap_or_else(|err| panic!("escrever findings.json: {err}"));
        std::fs::write(
            self.cwd.join("item.json"),
            include_str!("fixtures/work_item_4821.json"),
        )
        .unwrap_or_else(|err| panic!("escrever item.json: {err}"));
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Spawns the built `kira` inside the sandbox; credentials are stripped so no
/// command can reach the network by accident.
fn run_kira(sandbox: &Sandbox, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_kira"))
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env("KIRA_CONFIG", sandbox.store.join("absent-config.toml"))
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("KIRA_MODEL")
        .env_remove("KIRA_BOARD")
        .env_remove("KIRA_LLM")
        .output()
        .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn draft_do_item_completo_tem_as_secoes_e_as_evidencias() {
    let sandbox = Sandbox::new("full");
    sandbox.setup_full_fixture();

    let (code, stdout, stderr) =
        run_kira(&sandbox, &["pr", "draft", "4821", "--file", "item.json"]);
    assert_eq!(code, 0, "exit 0 no caminho feliz; stderr: {stderr}");
    assert!(
        stdout.contains("# Sessão expirada com token válido\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("## Contexto\nItem #4821 do tipo Bug no estado Active."),
        "{stdout}"
    );
    assert!(
        stdout.contains("## Causa raiz provável\nTTL do token muito curto.\n\nConfiança: média"),
        "{stdout}"
    );
    assert!(
        stdout.contains("- revisar `src/auth/session.rs` (citado pelo triage)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("src/auth/session.rs — achado audit/high: TTL fixo sem configurar"),
        "{stdout}"
    );
    assert!(
        stdout.contains(".kira/wiki/bugs/4821-sessao-expirada.md — trecho:"),
        "{stdout}"
    );
    // Achado NÃO relacionado aos arquivos citados não entra (anti-invenção).
    assert!(!stdout.contains("web/package.json"), "{stdout}");
    assert!(!stdout.contains("Prototype Pollution"), "{stdout}");
    assert!(
        stdout.contains("## Riscos\nhigh: session-mgmt 1.2.0"),
        "{stdout}"
    );
    assert!(stdout.contains("o merge é sempre humano"), "{stdout}");

    // DETERMINISMO: duas execuções no mesmo estado, byte a byte.
    let (code_again, stdout_again, _) =
        run_kira(&sandbox, &["pr", "draft", "4821", "--file", "item.json"]);
    assert_eq!(code_again, 0);
    assert_eq!(stdout, stdout_again, "mesma entrada = mesma saída");
}

#[test]
fn out_escreve_o_mesmo_conteudo_do_stdout() {
    let sandbox = Sandbox::new("out");
    sandbox.setup_full_fixture();

    let (code, stdout, _) = run_kira(
        &sandbox,
        &[
            "pr",
            "draft",
            "4821",
            "--out",
            "draft.md",
            "--file",
            "item.json",
        ],
    );
    assert_eq!(code, 0);
    let written = std::fs::read_to_string(sandbox.cwd.join("draft.md"))
        .unwrap_or_else(|err| panic!("ler draft.md: {err}"));
    assert_eq!(written, stdout, "--out grava o mesmo conteúdo do stdout");
}

#[test]
fn json_e_parseavel_e_carrega_estrutura() {
    let sandbox = Sandbox::new("json");
    sandbox.setup_full_fixture();

    let (code, stdout, _) = run_kira(
        &sandbox,
        &["pr", "draft", "4821", "--json", "--file", "item.json"],
    );
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout --json deve ser JSON: {err} — {stdout}"));
    assert_eq!(value["item_id"], 4821);
    assert_eq!(value["title"], "Sessão expirada com token válido");
    assert!(
        value["causa_raiz"]
            .as_str()
            .unwrap_or_default()
            .contains("TTL do token muito curto."),
        "{value}"
    );
    assert!(
        value["evidencias"]
            .as_array()
            .unwrap_or_else(|| panic!("evidencias deve ser lista"))
            .iter()
            .any(|line| line
                .as_str()
                .unwrap_or_default()
                .contains("achado audit/high")),
        "{value}"
    );
    // O markdown inteiro viaja junto para consumo por máquina.
    assert!(
        value["markdown"]
            .as_str()
            .unwrap_or_default()
            .contains("## Evidências")
    );
}

#[test]
fn item_sem_triage_sai_dash_com_a_causa_e_exit_0() {
    let sandbox = Sandbox::new("sem-triage");
    // Sem fixture: store vazio, sem findings, sem wiki.

    let (code, stdout, stderr) = run_kira(&sandbox, &["pr", "draft", "9999"]);
    assert_eq!(code, 0, "sem triage NÃO é erro; stderr: {stderr}");
    assert!(
        stdout.contains("## Causa raiz provável\n— (não há dado; rode kira triage 9999 antes)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("## Mudança proposta\n— (não há dado; rode kira triage 9999 antes)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "## Evidências\n— (não há dado; rode kira triage 9999 antes ou rode kira scan)"
        ),
        "{stdout}"
    );
    // Checklist continua completo (template, não dado).
    assert!(stdout.contains("o merge é sempre humano"), "{stdout}");
}

#[test]
fn store_ausente_continua_saindo_dash_com_causa() {
    // Store em caminho impossível: o fail-open mantém o rascunho honesto.
    let sandbox = Sandbox::new("store-impossivel");
    let blocker = sandbox.cwd.join("not-a-dir");
    std::fs::write(&blocker, "arquivo comum")
        .unwrap_or_else(|err| panic!("escrever blocker: {err}"));
    let impossible = blocker.join("inner");

    let output = Command::new(env!("CARGO_BIN_EXE_kira"))
        .args(["pr", "draft", "1"])
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &impossible)
        .env("KIRA_CONFIG", sandbox.store.join("absent-config.toml"))
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .output()
        .unwrap_or_else(|err| panic!("executar kira: {err}"));
    assert_eq!(output.status.code(), Some(0), "fail-open mantém exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("— (não há dado; rode kira triage 1 antes)"),
        "{stdout}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("aviso: store indisponível"), "{stderr}");
}

/// Redação de ponta a ponta: token no summary gravado no store não vaza no
/// rascunho que pode ir para PR público.
#[test]
fn token_no_summary_do_triage_nao_vaza_no_rascunho() {
    let sandbox = Sandbox::new("redacao");
    let store =
        Store::open_in_dir(&sandbox.store).unwrap_or_else(|err| panic!("abrir store: {err}"));
    let root = sandbox
        .cwd
        .canonicalize()
        .unwrap_or_else(|err| panic!("canonicalizar cwd: {err}"));
    let project = store
        .ensure_project(&root)
        .unwrap_or_else(|err| panic!("garantir projeto: {err}"));
    store
        .record(
            &project,
            "triage",
            "7",
            None,
            "2026-01-05T10:00:00Z",
            10,
            true,
            "Causa raiz provável:\nPAT colado no log: token=AbC123XyZ789QqQ.\n\nArquivos prováveis:\nsrc/a.rs\n\nConfiança:\nalta\n",
            Some(7),
        )
        .unwrap_or_else(|err| panic!("gravar sessão: {err}"));

    let (code, stdout, _) = run_kira(&sandbox, &["pr", "draft", "7"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("AbC123XyZ789QqQ"), "PAT vazou: {stdout}");
    assert!(
        stdout.contains("token="),
        "a chave continua legível: {stdout}"
    );
    assert!(stdout.contains("[REDACTED]"), "{stdout}");
}
