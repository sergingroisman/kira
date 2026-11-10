//! End-to-end tests of `kira scan` seen from the CLI: the stack detection
//! error, the "tool missing" path, the "tool ran and failed" path and the
//! determinism gate (two runs on the same state produce a byte-identical
//! `.kira/findings.json`), plus the automatic session trail.
//!
//! Everything is offline: no real `dotnet`/`npm`/`pnpm` is EVER executed,
//! and nothing relies on PATH shadowing — the shims are pointed to via the
//! `KIRA_SCAN_<TOOL>_BIN` env overrides (full shim path, exactly what CI
//! without the tool uses). The "tool missing" case runs with NO overrides
//! (the runner strips them) and an empty PATH.
//! Azure credentials are stripped and the store lives in a temp dir
//! (`KIRA_STORE_DIR`); env overrides are passed to the spawned binary only
//! (no process-global env var is mutated).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Real npm audit JSON format (the reusable fixture of the pure parsers).
const NPM_FAKE_OUTPUT: &str = include_str!("fixtures/scan/scan_npm_audit.json");
/// Real dotnet list package JSON format with zero findings.
const DOTNET_NONE_OUTPUT: &str = include_str!("fixtures/scan/scan_dotnet_none.json");

/// Isolated sandbox: project `cwd`, separate store dir and an empty `shims`
/// dir for the fake tools; the whole base is removed on Drop.
struct Sandbox {
    base: PathBuf,
    cwd: PathBuf,
    store: PathBuf,
    shims: PathBuf,
    empty_path: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        let base = std::env::temp_dir().join(format!(
            "kira-scan-e2e-{}-{name}-{nanos}",
            std::process::id()
        ));
        let cwd = base.join("proj");
        let shims = base.join("shims");
        let empty_path = base.join("path-empty");
        std::fs::create_dir_all(&cwd).unwrap_or_else(|err| panic!("criar workdir {cwd:?}: {err}"));
        std::fs::create_dir_all(&shims)
            .unwrap_or_else(|err| panic!("criar shims {shims:?}: {err}"));
        std::fs::create_dir_all(&empty_path)
            .unwrap_or_else(|err| panic!("criar path vazio: {err}"));
        Sandbox {
            store: base.join("store"),
            base,
            cwd,
            shims,
            empty_path,
        }
    }

    /// Marks the project with `package.json` + `package-lock.json`.
    fn prepare_npm_project(&self) {
        std::fs::write(self.cwd.join("package.json"), "{ \"name\": \"proj\" }")
            .unwrap_or_else(|err| panic!("escrever package.json: {err}"));
        std::fs::write(
            self.cwd.join("package-lock.json"),
            "{ \"lockfileVersion\": 3 }",
        )
        .unwrap_or_else(|err| panic!("escrever package-lock: {err}"));
    }

    /// Marks the project as .NET.
    fn prepare_dotnet_project(&self) {
        std::fs::write(
            self.cwd.join("App.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\" />",
        )
        .unwrap_or_else(|err| panic!("escrever App.csproj: {err}"));
    }

    /// Writes a fake tool that only prints the given JSON (never a real
    /// audit/list run) and returns the shim path (to be set as the
    /// `KIRA_SCAN_<TOOL>_BIN` override — no PATH shadowing involved).
    fn install_fake_tool(&self, tool: &str, output_content: &str, file_name: &str) -> PathBuf {
        let output_file = self.base.join(file_name);
        std::fs::write(&output_file, output_content)
            .unwrap_or_else(|err| panic!("escrever fake output {output_file:?}: {err}"));
        self.write_shim(
            tool,
            &format!("@type \"{}\"\r\n", output_file.display()),
            &format!(
                "#!/bin/sh\necho '{}'\n",
                output_content.trim().replace('\n', " ")
            ),
        )
    }

    /// Writes a fake tool that fails (exit 1) with one short stderr line and
    /// returns the shim path (same `KIRA_SCAN_<TOOL>_BIN` mechanism).
    fn install_failing_tool(&self, tool: &str, stderr_line: &str) -> PathBuf {
        let windows_script = format!("@echo {stderr_line} 1>&2\r\n@exit /b 1\r\n");
        let unix_script = format!("#!/bin/sh\necho '{stderr_line}' 1>&2\nexit 1\n");
        self.write_shim(tool, &windows_script, &unix_script)
    }

    /// Writes the shim file (`.cmd` on Windows, shebang script elsewhere) and
    /// makes it executable; returns its full path for the env override.
    fn write_shim(&self, tool: &str, windows_script: &str, unix_script: &str) -> PathBuf {
        let script = if cfg!(windows) {
            windows_script
        } else {
            unix_script
        };
        let script_path = self.shims.join(if cfg!(windows) {
            format!("{tool}.cmd")
        } else {
            tool.to_string()
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
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Env vars that override the scan tools' binaries (the shims of the happy
/// paths). "npm" => KIRA_SCAN_NPM_BIN.
fn bin_env(tool: &str) -> String {
    format!("KIRA_SCAN_{}_BIN", tool.to_ascii_uppercase())
}

const BIN_ENVS: [&str; 3] = [
    "KIRA_SCAN_DOTNET_BIN",
    "KIRA_SCAN_NPM_BIN",
    "KIRA_SCAN_PNPM_BIN",
];

/// Spawns the built `kira` inside the sandbox. `path_env` REPLACES the PATH
/// (only the "tool missing" case uses an empty one); `env_bins` sets the
/// KIRA_SCAN_*_BIN overrides for the shims. The BIN envs are ALWAYS stripped
/// first, so a "no overrides" run really has none even if the host machine
/// exports one. Credentials are stripped so no scan variant ever touches the
/// network or the keyring.
fn run_kira(
    sandbox: &Sandbox,
    path_env: Option<&Path>,
    env_bins: &[(&str, &Path)],
    args: &[&str],
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env_remove("AZURE_DEVOPS_PAT")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("KIRA_MODEL")
        .env_remove("KIRA_BOARD");
    for bin_env in BIN_ENVS {
        command.env_remove(bin_env);
    }
    for (name, path) in env_bins {
        command.env(name, path);
    }
    if let Some(path_env) = path_env {
        let key = if cfg!(windows) { "Path" } else { "PATH" };
        // The binary itself is executed by absolute path; PATH matters only
        // for the scan tools (fake or absent).
        command.env(key, path_env);
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

#[test]
fn scan_without_any_stack_says_where_to_scan() {
    let sandbox = Sandbox::new("no-stack");
    let (code, out, err) = run_kira(&sandbox, None, &[], &["scan", "--path", "."]);
    assert_eq!(code, 1, "sem marker o scan falha — saída: {out}{err}");
    assert!(
        err.contains("nenhum stack detectado"),
        "erro deve dizer o problema: {err}"
    );
    assert!(
        err.contains(".csproj") || err.contains("package.json"),
        "erro deve dizer o que procura: {err}"
    );
}

#[test]
fn missing_audit_tool_is_actionable() {
    let sandbox = Sandbox::new("missing-tool");
    sandbox.prepare_npm_project();
    // SEM overrides (o runner limpa os KIRA_SCAN_*_BIN) e PATH vazio: o npm
    // não existe => erro acionável "instale", sem body cru e sem confundir
    // "ferramenta falhou" com "ferramenta ausente".
    let (code, out, err) = run_kira(&sandbox, Some(&sandbox.empty_path), &[], &["scan"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("exige o npm"),
        "erro de ausência é o 'instale o npm': {err}"
    );
    assert!(
        err.contains("npm"),
        "erro deve dizer qual ferramenta: {err}"
    );
    assert!(
        err.contains("nodejs.org"),
        "erro deve dizer como instalar: {err}"
    );
    assert!(
        !err.contains('{'),
        "erro não pode conter body cru de worker output: {err}"
    );
    assert!(
        !err.contains("falhou"),
        "ausente não é 'a ferramenta falhou': {err}"
    );
}

/// Tool present (shim via `KIRA_SCAN_NPM_BIN`) that RUNS but fails: compact,
/// actionable error with the exit code and at most the first stderr line as
/// a hint — never the raw body.
#[test]
fn failing_tool_error_is_actionable_without_body() {
    let sandbox = Sandbox::new("tool-fail");
    sandbox.prepare_npm_project();
    let shim = sandbox.install_failing_tool("npm", "npm: saiu de propósito");
    let npm_bin = bin_env("npm");
    let (code, out, err) = run_kira(&sandbox, None, &[(npm_bin.as_str(), &shim)], &["scan"]);
    assert_eq!(code, 1, "ferramenta que falha derruba o scan: {out}{err}");
    assert!(
        err.contains("a ferramenta npm audit falhou (exit 1)"),
        "erro compacto com o exit code: {err}"
    );
    assert!(
        err.contains("(dica: npm: saiu de propósito)"),
        "dica é a primeira linha do stderr: {err}"
    );
    assert!(!err.contains('{'), "erro não pode conter body cru: {err}");
    assert!(
        !err.to_lowercase().contains("exige o npm"),
        "ferramenta presente não é 'ausente': {err}"
    );
}

/// The determinism gate of M1: two scans of the same state produce the SAME
/// findings.json (byte-identical), findings ordered by fingerprint, and the
/// run is recorded in the session trail.
#[test]
fn scan_of_the_fake_npm_audit_is_deterministic_and_recorded() {
    let sandbox = Sandbox::new("npm-deterministic");
    sandbox.prepare_npm_project();
    let shim = sandbox.install_fake_tool("npm", NPM_FAKE_OUTPUT, "npm_output.json");
    let npm_bin = bin_env("npm");
    let (code, out, err) = run_kira(&sandbox, None, &[(npm_bin.as_str(), &shim)], &["scan"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("stacks: npm"),
        "stacks detectadas no stdout: {out}"
    );
    assert!(out.contains("achados: 2"), "{out}");
    assert!(
        out.contains("\n  high: audit 1\n"),
        "contagem por severidade com counts por tool: {out}"
    );
    let findings_path = sandbox.cwd.join(".kira").join("findings.json");
    let first = std::fs::read_to_string(&findings_path)
        .unwrap_or_else(|err| panic!("ler {findings_path:?}: {err}"));
    assert!(out.contains(&findings_path.display().to_string()), "{out}");

    // Segunda execução no MESMO estado: arquivo byte-idêntico.
    let (code, _, _) = run_kira(&sandbox, None, &[(npm_bin.as_str(), &shim)], &["scan"]);
    assert_eq!(code, 0);
    let second = std::fs::read_to_string(&findings_path)
        .unwrap_or_else(|err| panic!("ler findings de novo: {err}"));
    assert_eq!(first, second, "gate de determinismo do M1");

    // Conteúdo: findings ordenados por fingerprint (determinístico), stacks.
    let document: serde_json::Value =
        serde_json::from_str(&first).unwrap_or_else(|err| panic!("parse findings.json: {err}"));
    let fingerprints: Vec<&str> = document
        .get("findings")
        .and_then(|f| f.as_array())
        .map(|findings| {
            findings
                .iter()
                .filter_map(|finding| finding.get("fingerprint").and_then(|v| v.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(fingerprints.len(), 2, "2 achados: {document}");
    let mut sorted = fingerprints.clone();
    sorted.sort();
    assert_eq!(fingerprints, sorted, "ordenado por fingerprint");
    assert!(
        fingerprints.iter().all(|hex| hex.len() == 16),
        "fingerprint FNV-1a 64 em hex: {fingerprints:?}"
    );
    let tool = document
        .pointer("/findings/0/tool")
        .and_then(|tool| tool.as_str());
    assert_eq!(tool, Some("audit"), "{document}");
    let stack_kind = document.pointer("/stacks/0/kind").and_then(|k| k.as_str());
    assert_eq!(stack_kind, Some("npm"), "{document}");

    // A rodada scan entra na trilha (sessão ok, com stacks+achados).
    let (code, history, _) = run_kira(&sandbox, None, &[], &["history"]);
    assert_eq!(code, 0);
    assert!(history.contains("scan"), "comando na trilha: {history}");
    assert!(history.contains("✓"), "scan ok é marcado: {history}");
    assert!(
        history.contains("achados: 2"),
        "summary tem stacks+counts: {history}"
    );
}

#[test]
fn scan_of_no_findings_is_positive_and_empty() {
    let sandbox = Sandbox::new("dotnet-none");
    sandbox.prepare_dotnet_project();
    let shim = sandbox.install_fake_tool("dotnet", DOTNET_NONE_OUTPUT, "dotnet_output.json");
    let dotnet_bin = bin_env("dotnet");
    let (code, out, err) = run_kira(
        &sandbox,
        None,
        &[(dotnet_bin.as_str(), &shim)],
        &["scan", "--path", "."],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("stacks: dotnet"), "{out}");
    assert!(out.contains("achados: 0"), "{out}");
    let findings_path = sandbox.cwd.join(".kira").join("findings.json");
    let document: serde_json::Value = std::fs::read_to_string(&findings_path)
        .unwrap_or_else(|err| panic!("ler {findings_path:?}: {err}"))
        .parse()
        .unwrap_or_else(|err| panic!("parse findings.json: {err}"));
    let empty = document
        .get("findings")
        .and_then(|f| f.as_array())
        .map(Vec::is_empty)
        .unwrap_or(false);
    assert!(empty, "nenhum achado => lista vazia: {document}");
    assert!(out.contains("(nenhum)"), "{out}");
    // `.kira/` é local: a nota explicando isso existe.
    let note = std::fs::read_to_string(sandbox.cwd.join(".kira").join("README.md"))
        .unwrap_or_else(|err| panic!("ler nota do .kira: {err}"));
    assert!(note.contains("LOCAL"), "{note}");
}
