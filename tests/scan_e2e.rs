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
    // SEM overrides (o runner limpa os KIRA_SCAN_*_BIN) e PATH vazio: a
    // RESOLUÇÃO which-style não acha o npm => erro acionável "instale o npm"
    // SEM spawn (vale em qualquer locale do OS — nada de texto localizado é
    // parseado), sem body cru e sem confundir "ferramenta falhou" com
    // "ferramenta ausente".
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
    assert!(
        !err.contains("dica"),
        "ausência por resolução não gera dica de stderr: {err}"
    );
}

/// PR-F4: a ferramenta é achada pelo PATH (which-style) SEM override nenhum —
/// o shim `npm` no dir do PATH resolve e roda (no Windows via `cmd /C` por ser
/// `.cmd`, no Unix direto pelo exec bit).
#[test]
fn tool_is_resolved_from_the_path_without_any_override() {
    let sandbox = Sandbox::new("path-resolve");
    sandbox.prepare_npm_project();
    sandbox.install_fake_tool("npm", NPM_FAKE_OUTPUT, "npm_output.json");
    let (code, out, err) = run_kira(&sandbox, Some(&sandbox.shims), &[], &["scan"]);
    assert_eq!(code, 0, "shim achado pelo PATH roda: {out}{err}");
    assert!(out.contains("stacks: npm"), "{out}");
    assert!(out.contains("achados: 2"), "{out}");
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
    let printed = out
        .lines()
        .find_map(|line| {
            // "arquivo: <caminho>" pode estar no meio de uma linha.
            line.find("arquivo: ")
                .map(|index| line[index + "arquivo: ".len()..].to_string())
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("'arquivo:' ausente no stdout: {out}"));
    let printed_file = std::fs::canonicalize(&printed)
        .unwrap_or_else(|err| panic!("o caminho impresso deve existir: {err}"));
    let expected_file = std::fs::canonicalize(&findings_path)
        .unwrap_or_else(|err| panic!("ler findings path: {err}"));
    assert_eq!(
        printed_file, expected_file,
        "o caminho impresso é o arquivo escrito (indiferente ao formato no Windows): {out}"
    );

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

/// Real-shaped `pnpm audit --json` without vulnerabilities (the same shape the
/// teishoku capture has: `advisories` map + `metadata`) — parseable by both the
/// old event-array parser and the real-format one (0 findings either way).
const PNPM_EMPTY_REAL: &str = r#"{"advisories":{},"muted":[],"metadata":{"vulnerabilities":{"info":0,"low":0,"moderate":0,"high":0,"critical":0},"dependencies":0,"devDependencies":0,"optionalDependencies":0,"totalDependencies":0}}"#;

/// W2-fix PR-F1 (real bug of the teishoku monorepo): a pnpm WORKSPACE scans
/// as ONE pnpm stack — `pnpm audit` runs once at the root and `npm audit`
/// NEVER runs on the members. The failing npm shim makes any npm invocation
/// fail the whole run, so exit 0 is the proof it was never invoked.
#[test]
fn pnpm_workspace_scans_once_at_the_root_and_never_runs_npm() {
    let sandbox = Sandbox::new("pnpm-workspace");
    std::fs::write(sandbox.cwd.join("package.json"), "{ \"name\": \"root\" }")
        .unwrap_or_else(|err| panic!("escrever package.json: {err}"));
    std::fs::write(
        sandbox.cwd.join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\n",
    )
    .unwrap_or_else(|err| panic!("escrever pnpm-lock: {err}"));
    std::fs::write(
        sandbox.cwd.join("pnpm-workspace.yaml"),
        "packages:\n  - 'apps/*'\n  - 'packages/*'\n",
    )
    .unwrap_or_else(|err| panic!("escrever pnpm-workspace: {err}"));
    for member in ["apps/api", "packages/shared"] {
        let dir = sandbox.cwd.join(member);
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {member}: {err}"));
        std::fs::write(dir.join("package.json"), "{ \"name\": \"member\" }")
            .unwrap_or_else(|err| panic!("escrever package.json do membro: {err}"));
    }
    let pnpm_shim = sandbox.install_fake_tool("pnpm", PNPM_EMPTY_REAL, "pnpm_output.json");
    let npm_shim = sandbox.install_failing_tool("npm", "npm NÃO podia ser invocado aqui");
    let pnpm_bin = bin_env("pnpm");
    let npm_bin = bin_env("npm");
    let envs: [(&str, &Path); 2] = [
        (pnpm_bin.as_str(), &pnpm_shim),
        (npm_bin.as_str(), &npm_shim),
    ];

    let (code, out, err) = run_kira(&sandbox, None, &envs, &["scan"]);
    assert_eq!(code, 0, "o scan do workspace não pode falhar: {out}{err}");
    assert!(
        out.contains("stacks: pnpm |"),
        "UMA stack pnpm no resumo (não 'pnpm, npm, npm'): {out}"
    );
    assert!(
        !out.contains("pnpm, npm"),
        "membros não são stacks npm: {out}"
    );
    assert!(
        out.contains("members: apps/api, packages/shared"),
        "membros dobrados na raiz, ordenados: {out}"
    );
    assert!(out.contains("achados: 0"), "{out}");

    let findings_path = sandbox.cwd.join(".kira").join("findings.json");
    let first =
        std::fs::read_to_string(&findings_path).unwrap_or_else(|err| panic!("ler findings: {err}"));
    let document: serde_json::Value =
        serde_json::from_str(&first).unwrap_or_else(|err| panic!("parse findings.json: {err}"));
    assert_eq!(
        document.pointer("/stacks/0/kind").and_then(|k| k.as_str()),
        Some("pnpm"),
        "{document}"
    );
    let members: Vec<&str> = document
        .pointer("/stacks/0/members")
        .and_then(|m| m.as_array())
        .map(|items| items.iter().filter_map(|item| item.as_str()).collect())
        .unwrap_or_default();
    assert_eq!(members, ["apps/api", "packages/shared"], "{document}");

    // Duas execuções no mesmo estado são byte-idênticas (gate do M1).
    let (code, _, _) = run_kira(&sandbox, None, &envs, &["scan"]);
    assert_eq!(code, 0);
    let second = std::fs::read_to_string(&findings_path)
        .unwrap_or_else(|err| panic!("ler findings de novo: {err}"));
    assert_eq!(first, second, "gate de determinismo do M1");
}
