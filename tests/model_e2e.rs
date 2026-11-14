//! End-to-end test of `kira model list/pull` (HF-2, ADR-0040).
//!
//! No real Ollama and no network: a fake `ollama` is pointed to via the
//! `KIRA_MODEL_OLLAMA_BIN` override (full shim path, exactly what the scan
//! e2e does with `KIRA_SCAN_*_BIN` — no PATH shadowing). The store is
//! isolated with `KIRA_STORE_DIR`, so the automatic session trail never
//! touches the owner's real store.

use std::path::PathBuf;
use std::process::Command;

/// Canned `ollama list` output in the REAL format (header + hf.co rows),
/// captured from an actual `ollama list` — the parse contract lives here.
const LIST_OUTPUT: &str = "\
NAME                             ID              SIZE      MODIFIED
qwen3.5:4b                       d8b0f5e9760c    3.3 GB    3 days ago
hf.co/mlabonne/Llama-3.1-GGUF:Q4_K_M    a26faf0135b2    4.9 GB    4 days ago
dolphin3:8b                      d5ab9ae8e1f2    4.9 GB    4 days ago
";

/// Fake PAT for the leak probe (never a real credential).
const FAKE_PAT: &str = "kira-e2e-secret-token-xyz123";

struct Sandbox {
    cwd: PathBuf,
    store: PathBuf,
    shims: PathBuf,
    base: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "kira-model-e2e-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&base);
        let cwd = base.join("projeto");
        let store = base.join("store");
        let shims = base.join("shims");
        std::fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar {cwd:?}: {err}"));
        std::fs::create_dir_all(&store).unwrap_or_else(|err| panic!("criar {store:?}: {err}"));
        std::fs::create_dir_all(&shims).unwrap_or_else(|err| panic!("criar {shims:?}: {err}"));
        Self {
            cwd,
            store,
            shims,
            base,
        }
    }

    /// Fake `ollama` that prints the canned catalog and exits 0 (the SAME
    /// file on both platforms: `type` on Windows, `cat` elsewhere).
    fn install_list_ollama(&self) -> PathBuf {
        let output_file = self.base.join("ollama_list.txt");
        std::fs::write(&output_file, LIST_OUTPUT)
            .unwrap_or_else(|err| panic!("escrever lista falsa: {err}"));
        self.write_shim(
            &format!("@type \"{}\"\r\n", output_file.display()),
            &format!("#!/bin/sh\ncat '{}'\n", output_file.display()),
        )
    }

    /// Fake `ollama` that fails (exit 1) with one short stderr line.
    fn install_failing_ollama(&self, stderr_line: &str) -> PathBuf {
        self.write_shim(
            &format!("@echo {stderr_line} 1>&2\r\n@exit /b 1\r\n"),
            &format!("#!/bin/sh\necho '{stderr_line}' 1>&2\nexit 1\n"),
        )
    }

    /// Fake `ollama` for pull: writes a MARKER when invoked (the fail-closed
    /// proof — after a refusal the marker must NOT exist) and exits 0.
    fn install_pull_marker_ollama(&self) -> PathBuf {
        let marker = self.shims.join("marker-pull");
        self.write_shim(
            &format!("@type nul > \"{}\"\r\n@echo pulled\r\n", marker.display()),
            &format!("#!/bin/sh\n: > \"{}\"\necho pulled\n", marker.display()),
        )
    }

    /// Writes the shim (`.cmd` on Windows, shebang script elsewhere) and
    /// makes it executable; returns its full path for the env override.
    fn write_shim(&self, windows_script: &str, unix_script: &str) -> PathBuf {
        let script = if cfg!(windows) {
            windows_script
        } else {
            unix_script
        };
        let script_path = self.shims.join(if cfg!(windows) {
            "ollama.cmd".to_string()
        } else {
            "ollama".to_string()
        });
        std::fs::write(&script_path, script)
            .unwrap_or_else(|err| panic!("escrever shim {script_path:?}: {err}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim: {err}"));
        }
        script_path
    }

    fn marker_path(&self) -> PathBuf {
        self.shims.join("marker-pull")
    }

    /// Runs `kira` with the sandbox isolation (store dir, absent config,
    /// keyring off, no board env) plus extra `(key, value)` pairs.
    fn run(&self, args: &[&str], extra: &[(&str, &str)]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_kira"))
            .args(args)
            .current_dir(&self.cwd)
            .env("KIRA_STORE_DIR", &self.store)
            .env("KIRA_CONFIG", self.store.join("absent-config.toml"))
            .env("KIRA_SECRET_STORE", "off")
            .env(
                "KIRA_MODEL_OLLAMA_BIN",
                self.shims.join(if cfg!(windows) {
                    "ollama.cmd"
                } else {
                    "ollama"
                }),
            )
            .env_remove("OLLAMA_MODELS")
            .env_remove("AZURE_DEVOPS_PAT")
            .env_remove("AZURE_DEVOPS_ORG")
            .env_remove("AZURE_DEVOPS_PROJECT")
            .env_remove("KIRA_BOARD")
            .envs(extra.iter().copied())
            .output()
            .unwrap_or_else(|err| panic!("rodar kira {args:?}: {err}"));
        (
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// `kira model list` parses the real `ollama list` format through the shim:
/// names, sizes and no invented content; exit 0.
#[test]
fn model_list_parses_the_shim_catalog() {
    let sandbox = Sandbox::new("list-happy");
    sandbox.install_list_ollama();
    let (code, stdout, stderr) = sandbox.run(&["model", "list"], &[]);
    assert_eq!(code, 0, "lista feliz sai 0: {stdout}{stderr}");
    assert!(
        stdout.contains("qwen3.5:4b") && stdout.contains("3.3 GB"),
        "nome e tamanho da linha parseada: {stdout}"
    );
    assert!(
        stdout.contains("hf.co/mlabonne/Llama-3.1-GGUF:Q4_K_M"),
        "tag hf.co entra inteira: {stdout}"
    );
    assert!(
        !stdout.contains("NAME"),
        "header do ollama não vira linha do catálogo: {stdout}"
    );
}

/// `ollama list` that runs and fails: compact actionable error with the
/// first stderr line as a hint — never a raw stderr dump.
#[test]
fn model_list_failing_shim_is_actionable() {
    let sandbox = Sandbox::new("list-fail");
    sandbox.install_failing_ollama("ollama: saiu de propósito");
    let (code, stdout, stderr) = sandbox.run(&["model", "list"], &[]);
    assert_ne!(code, 0, "falha do ollama é erro: {stdout}");
    assert!(
        stderr.contains("ollama list` falhou (exit 1)"),
        "erro acionável com o exit: {stderr}"
    );
    assert!(
        stderr.contains("dica: ollama: saiu de propósito"),
        "primeira linha do stderr como dica: {stderr}"
    );
    assert!(
        !stderr.contains("cru") && stderr.lines().count() <= 3,
        "nenhum body cru: {stderr}"
    );
}

/// THE fail-closed proof: pull without `--yes` and without a TTY is refused
/// BEFORE any spawn — the shim's marker proves ollama was never called.
#[test]
fn model_pull_without_yes_and_without_tty_refuses_before_any_spawn() {
    let sandbox = Sandbox::new("pull-consent");
    sandbox.install_pull_marker_ollama();
    let (code, stdout, stderr) =
        sandbox.run(&["model", "pull", "hf.co/fake/repo-GGUF:Q4_K_M"], &[]);
    assert_ne!(code, 0, "sem consentimento o pull não passa: {stdout}");
    assert!(
        stderr.contains("--yes"),
        "erro acionável diz o conserto (--yes): {stderr}"
    );
    assert!(
        !sandbox.marker_path().exists(),
        "o ollama NÃO pode ter sido spawnado na recusa: {}",
        sandbox.marker_path().display()
    );
}

/// With `--yes` the pull runs (marker exists) and the summary names the model.
#[test]
fn model_pull_with_yes_runs_the_shim_and_records_the_trail() {
    let sandbox = Sandbox::new("pull-yes");
    sandbox.install_pull_marker_ollama();
    let (code, stdout, stderr) = sandbox.run(
        &["model", "pull", "hf.co/fake/repo-GGUF:Q4_K_M", "--yes"],
        &[],
    );
    assert_eq!(code, 0, "pull com --yes passa: {stdout}{stderr}");
    assert!(
        sandbox.marker_path().exists(),
        "o ollama foi invocado com --yes: {}",
        sandbox.marker_path().display()
    );
    assert!(
        stdout.contains("modelo baixado: hf.co/fake/repo-GGUF:Q4_K_M"),
        "resumo do pull: {stdout}"
    );
    // Log de auditoria no store: a trilha automática gravou a sessão do pull.
    let db = rusqlite::Connection::open(sandbox.store.join("store.db"))
        .unwrap_or_else(|err| panic!("abrir store: {err}"));
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE command = 'model' AND ok = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|err| panic!("contar sessões model: {err}"));
    assert!(count >= 1, "o pull precisa estar na trilha: {count}");
}

/// `kira model list` never leaks a secret: the PAT env stays in the env,
/// the catalog output never prints it (security probe style).
#[test]
fn model_list_never_leaks_the_pat() {
    let sandbox = Sandbox::new("list-secret");
    sandbox.install_list_ollama();
    let (code, stdout, stderr) = sandbox.run(&["model", "list"], &[("AZURE_DEVOPS_PAT", FAKE_PAT)]);
    assert_eq!(code, 0, "list sai 0 mesmo com PAT no ambiente: {stderr}");
    assert!(
        !stdout.contains(FAKE_PAT) && !stderr.contains(FAKE_PAT),
        "o catálogo NUNCA ecoa o PAT: {stdout}{stderr}"
    );
}
