//! End-to-end tests of `kira rea` (TE PR41) seen from the CLI: the shim
//! happy path (output arrives REDACTED and caped), the "tool ran and
//! failed" path (compact actionable error, never the raw stderr body), the
//! "binary missing" path (no spawn at all) and the passthrough of args.
//!
//! Everything is offline: no real `rea` is EVER executed — the shims are
//! pointed to via the `KIRA_REA_BIN` override (no PATH shadowing involved).
//! The "binary missing" case runs with NO override and an empty PATH.
//! The store lives in a temp dir (`KIRA_STORE_DIR`) and credentials are
//! stripped so no variant ever touches the network or the keyring.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Isolated sandbox: project `cwd`, separate store dir and an empty `shims`
/// dir for the fake rea; the whole base is removed on Drop.
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
            "kira-rea-e2e-{}-{name}-{nanos}",
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

    /// Fake rea that echoes its args and prints one output line, then exits
    /// 0. Returns the shim path (the `KIRA_REA_BIN` override).
    fn install_fake_rea(&self, output_line: &str) -> PathBuf {
        let win = format!("@echo args=%*\r\necho {output_line}\r\n",);
        let unix = format!("#!/bin/sh\necho \"args=$*\"\necho '{output_line}'\n");
        self.write_shim("rea", &win, &unix)
    }

    /// Fake rea that fails (exit 1) with one short stderr line.
    fn install_failing_rea(&self, stderr_line: &str) -> PathBuf {
        let windows_script = format!("@echo {stderr_line} 1>&2\r\n@exit /b 1\r\n");
        let unix_script = format!("#!/bin/sh\necho '{stderr_line}' 1>&2\nexit 1\n");
        self.write_shim("rea", &windows_script, &unix_script)
    }

    /// Fake rea that prints more than the 8 KiB cap, to prove the cut.
    fn install_big_rea(&self) -> PathBuf {
        // 6000 x "á" = 12000 bytes UTF-8 (2 bytes cada) — acima do cap de
        // 8 KiB. O corte deve respeitar char boundary (nunca um 'á' pela
        // metade, o que apareceria como U+FFFD na saída).
        let line = "á".repeat(6000);
        let win = format!("@echo {line}\r\n");
        let unix = format!("#!/bin/sh\ncat <<'EOF'\n{line}\nEOF\n");
        self.write_shim("rea", &win, &unix)
    }

    /// Writes the shim file (`.cmd` on Windows, shebang script elsewhere)
    /// and makes it executable; returns its full path for the env override.
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

/// Spawns the built `kira` inside the sandbox. `path_env` REPLACES the PATH
/// (only the "binary missing" case uses an empty one); `rea_bin` sets the
/// `KIRA_REA_BIN` override. The override is ALWAYS stripped first, so a
/// "no override" run really has none even if the host machine exports one.
/// Credentials are stripped so no variant touches the network or the keyring.
fn run_kira(
    sandbox: &Sandbox,
    path_env: Option<&Path>,
    rea_bin: Option<&Path>,
    args: &[&str],
) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kira"));
    command
        .args(args)
        .current_dir(&sandbox.cwd)
        .env("KIRA_STORE_DIR", &sandbox.store)
        .env_remove("AZURE_DEVOPS_PAT")
        .env("KIRA_SECRET_STORE", "off")
        .env_remove("AZURE_DEVOPS_ORG")
        .env_remove("AZURE_DEVOPS_PROJECT")
        .env_remove("KIRA_MODEL")
        .env_remove("KIRA_BOARD")
        .env_remove("KIRA_REA_BIN");
    if let Some(bin) = rea_bin {
        command.env("KIRA_REA_BIN", bin);
    }
    if let Some(path_env) = path_env {
        let key = if cfg!(windows) { "Path" } else { "PATH" };
        // O binário roda por caminho absoluto; o PATH só importa para o rea.
        command.env(key, path_env);
    }
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("executar kira {args:?}: {err}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// PAT-shaped secret the shim prints; `kira rea` must never echo it.
const FAKE_PAT: &str = "token=AbC123XyZ789QqQ";

#[test]
fn rea_redacts_secret_and_passes_args_through() {
    let sandbox = Sandbox::new("redact-args");
    let shim = sandbox.install_fake_rea(FAKE_PAT);

    let (code, stdout, _stderr) =
        run_kira(&sandbox, None, Some(&shim), &["rea", "--diff", "HEAD~1"]);
    assert_eq!(code, 0, "shim ok ⇒ exit 0");
    // Passthrough: o rea recebeu os args exatamente como passados.
    let joined = stdout.replace("\r\n", "\n");
    assert!(
        joined.contains("--diff HEAD~1"),
        "args passthrough: {joined}"
    );
    // Redação: o segredo do output NUNCA chega ao terminal.
    assert!(
        !joined.contains("AbC123XyZ789QqQ"),
        "segredo vazou: {joined}"
    );
    assert!(joined.contains("[REDACTED]"), "máscara aplicada: {joined}");
}

#[test]
fn rea_failure_is_compact_and_actionable() {
    let sandbox = Sandbox::new("failure");
    let shim = sandbox.install_failing_rea("runtime inválido no rea");

    let (code, stdout, stderr) = run_kira(&sandbox, None, Some(&shim), &["rea"]);
    assert_ne!(code, 0, "rea exit 1 ⇒ kira sai != 0");
    // Erro acionável com a dica da primeira linha — nunca o corpo cru. O
    // canal é o mesmo dos outros comandos: `print_error` escreve em stderr.
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(
        stderr.contains("a ferramenta rea falhou (exit 1)"),
        "mensagem: {stderr}"
    );
    assert!(
        stderr.contains("(dica: runtime inválido no rea)"),
        "{stderr}"
    );
    assert!(stderr.contains("rode o comando à mão"), "{stderr}");
}

#[test]
fn rea_missing_binary_is_actionable_without_spawn() {
    let sandbox = Sandbox::new("missing");
    // PATH vazio e sem override: nada existe para rodar.
    let (code, stdout, stderr) = run_kira(
        &sandbox,
        Some(&sandbox.empty_path),
        None,
        &["rea", "--diff"],
    );
    assert_ne!(code, 0);
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(stderr.contains("instale o rea"), "mensagem: {stderr}");
    assert!(
        stderr.contains("KIRA_REA_BIN"),
        "dica do override: {stderr}"
    );
}

#[test]
fn rea_override_pointing_to_missing_file_never_spawns() {
    let sandbox = Sandbox::new("override-missing");
    let fake = sandbox.base.join("rea-que-nao-existe");
    let (code, stdout, stderr) = run_kira(&sandbox, None, Some(&fake), &["rea"]);
    assert_ne!(code, 0);
    assert!(stdout.is_empty(), "erro não vai para stdout: {stdout}");
    assert!(stderr.contains("instale o rea"), "mensagem: {stderr}");
}

#[test]
fn rea_output_over_cap_is_truncated_with_marker() {
    let sandbox = Sandbox::new("cap");
    let shim = sandbox.install_big_rea();

    let (code, stdout, _stderr) = run_kira(&sandbox, None, Some(&shim), &["rea"]);
    assert_eq!(code, 0);
    let joined = stdout.replace("\r\n", "\n");
    // O teto vale (8 KiB + margem do marcador): a saída é MUITO menor que
    // os 12000 bytes que o shim imprime.
    assert!(
        joined.len() < 12000,
        "saída sem cap: {} bytes",
        joined.len()
    );
    assert!(
        joined.contains("truncada em 8 KiB"),
        "marcador honesto: {}",
        &joined[..joined.len().min(400)]
    );
    // Corte em char boundary: se um 'á' tivesse partido ao meio, a conversão
    // lossy teria deixado U+FFFD na saída.
    assert!(
        !joined.contains('\u{FFFD}'),
        "UTF-8 partido no corte do cap: {}",
        &joined[..joined.len().min(200)]
    );
}
