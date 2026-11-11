//! Copilot backend: the "costly" LLM as a spawn of the Copilot CLI
//! (`copilot -p "<texto>"`) — ADR-0019.
//!
//! No SDK, no HTTP, no key in a file: the CLI is already on the owner's
//! machine and the Avanade license covers it (policy permitting). Ollama
//! remains the DEFAULT — this backend only runs under an explicit
//! `--llm copilot` / `KIRA_LLM=copilot` and a missing/auth-blocked CLI is an
//! actionable error, never a silent fallback to Ollama.
//!
//! Secrets: the prompt is already redacted by the context packer
//! (`redact`), and this module never echoes the prompt, the raw stderr or
//! any env value in a log/error.
//!
//! CLI args: `copilot -p <texto> -s`. The `-p` one-shot prompt flag is the
//! documented spawn shape of ADR-0019; the `-s` silent flag suppresses CLI
//! chrome. NOTE (PR11): `copilot --help` could not be run on the authoring
//! machine (the CLI exists on the owner's machine but did not authenticate
//! there either), so the exact flags live in one testable vector,
//! [`CopilotBackend::args_for`], and the output parser is tolerant — if the
//! silent flag does not exist in the installed version, dropping it from
//! `args_for` is the only change needed (the parser strips chrome anyway).

use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::binresolve;
use crate::error::Error;
use crate::llm::LlmBackend;

/// Name searched on the PATH when `KIRA_COPILOT_BIN` is not set.
const CLI_PROGRAM: &str = "copilot";

/// Env var that forces the binary path (the offline-test contract, same
/// pattern as `KIRA_SCAN_<TOOL>_BIN`).
pub const BIN_ENV: &str = "KIRA_COPILOT_BIN";

/// Env var with the per-call budget in seconds (integer; values ≤ 0 are
/// ignored and the default applies).
pub const TIMEOUT_ENV: &str = "KIRA_COPILOT_TIMEOUT";

/// Default per-call budget: a triage prompt is small, but a cold CLI start
/// (first login refresh, model warm-up) can take a while.
pub const DEFAULT_TIMEOUT_SECS: u64 = 180;

/// Substrings (lowercase) in the CLI stderr that mean "auth problem", not
/// "tool failure" — SAML/SSO of the corporate GitHub account is the expected
/// blocker for the owner (ADR-0019).
const AUTH_MARKERS: [&str; 6] = ["saml", "sso", "not authorized", "401", "403", "login"];

/// Backend that completes one prompt by spawning the Copilot CLI once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotBackend {
    bin: PathBuf,
    timeout: Duration,
}

impl CopilotBackend {
    /// Creates a backend for an ALREADY-RESOLVED binary path and budget.
    /// `from_env` is the production constructor; this one exists so the
    /// offline tests never touch the environment.
    pub fn new(bin: PathBuf, timeout: Duration) -> Self {
        Self { bin, timeout }
    }

    /// Production constructor: resolves the binary from `KIRA_COPILOT_BIN`
    /// (full path, must be an existing file) or the PATH, and the budget
    /// from `KIRA_COPILOT_TIMEOUT` (default
    /// [`DEFAULT_TIMEOUT_SECS`]).
    pub fn from_env() -> Result<Self, Error> {
        let override_bin = binresolve::bin_from_env(BIN_ENV);
        let path_env = std::env::var_os("PATH");
        let bin = resolve_bin(override_bin.as_deref(), path_env.as_deref())?;
        Ok(Self {
            bin,
            timeout: Duration::from_secs(timeout_from_env()),
        })
    }

    /// The CLI invocation for one prompt: `copilot -p <system>\n\n<prompt> -s`.
    ///
    /// Public so the spawn shape stays testable/documentable without running
    /// the CLI (the args vector is the single place to adjust a flag rename).
    pub fn args_for(&self, system: &str, prompt: &str) -> Vec<OsString> {
        let text = format!("{system}\n\n{prompt}");
        vec!["-p".into(), text.into(), "-s".into()]
    }
}

/// Resolves the CLI binary: override first (must be an existing file), then
/// the `which`-style PATH search. Nothing found => [`Error::CopilotMissing`]
/// — the error is the contract; there is NO fallback.
fn resolve_bin(
    override_bin: Option<&str>,
    path_env: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, Error> {
    let bin = if let Some(bin) = override_bin {
        let path = PathBuf::from(bin);
        if !path.is_file() {
            return Err(Error::CopilotMissing);
        }
        path
    } else {
        binresolve::find_on_path(path_env, CLI_PROGRAM).ok_or(Error::CopilotMissing)?
    };
    Ok(bin)
}

/// `KIRA_COPILOT_TIMEOUT` in seconds (trimmed integer), defaulting to
/// [`DEFAULT_TIMEOUT_SECS`] when absent or unparsable.
fn timeout_from_env() -> u64 {
    std::env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

/// Windows only: `.cmd`/`.bat` need the shell wrapper (same contract as the
/// scan tools), everything else spawns directly.
fn build_command(bin: &PathBuf, args: &[OsString]) -> Command {
    if binresolve::is_cmd_script(&bin.to_string_lossy()) {
        let mut command = Command::new("cmd");
        command.args(["/C", "chcp 65001 >nul 2>&1 &"]);
        command.arg(bin);
        command.args(args);
        command
    } else {
        let mut command = Command::new(bin);
        command.args(args);
        command
    }
}

/// Reads a child stream fully as bytes (never fails the caller on invalid
/// UTF-8 — the lossy conversion happens later, at classification time).
fn read_all(stream: Option<impl Read>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(mut stream) = stream {
        let _ = stream.read_to_end(&mut bytes);
    }
    bytes
}

impl LlmBackend for CopilotBackend {
    async fn complete(&self, system: &str, prompt: &str) -> Result<String, Error> {
        let bin = self.bin.clone();
        let args = self.args_for(system, prompt);
        let timeout = self.timeout;
        // The blocking spawn (wait loop + kill) runs on a dedicated thread so
        // the async runtime stays responsive.
        let outcome = tokio::task::spawn_blocking(move || run_copilot(bin, args, timeout)).await;
        match outcome {
            Ok(result) => result,
            Err(join_err) => Err(Error::CopilotFailed {
                hint: format!("spawn interno falhou: {join_err}"),
            }),
        }
    }
}

/// The blocking spawn: run the CLI once with a hard wall-clock budget, then
/// classify (parse / auth / failure / timeout).
fn run_copilot(bin: PathBuf, args: Vec<OsString>, timeout: Duration) -> Result<String, Error> {
    let mut command = build_command(&bin, &args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(source) => {
            // The binary was resolved first, so NotFound here is a race (file
            // deleted between resolve and spawn); anything else is runtime
            // noise — compact in both cases, no raw OS text.
            let hint = if source.kind() == std::io::ErrorKind::NotFound {
                "o binário sumiu entre a resolução e o spawn".to_string()
            } else {
                "não foi possível executar o CLI".to_string()
            };
            return Err(Error::CopilotFailed { hint });
        }
    };
    // Piped streams are drained on dedicated threads WHILE the wait loop
    // polls: a CLI that writes past the OS pipe buffer without reading its
    // stderr would otherwise deadlock the caller.
    let stdout_stream = child.stdout.take();
    let stderr_stream = child.stderr.take();
    let stdout_task = std::thread::spawn(move || read_all(stdout_stream));
    let stderr_task = std::thread::spawn(move || read_all(stderr_stream));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Error::CopilotTimeout {
                        secs: timeout.as_secs(),
                    });
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                return Err(Error::CopilotFailed {
                    hint: "falha ao aguardar o término do CLI".to_string(),
                });
            }
        }
    };
    let stdout_bytes = stdout_task.join().unwrap_or_default();
    let stderr_bytes = stderr_task.join().unwrap_or_default();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).into_owned();
    if status.success() {
        let stdout_text = String::from_utf8_lossy(&stdout_bytes).into_owned();
        parse_output(&stdout_text)
    } else if is_auth_problem(&stderr_text) {
        Err(Error::CopilotAuth)
    } else {
        let hint = binresolve::first_stderr_line(&stderr_text);
        let hint = if hint.is_empty() {
            "sem mensagem do CLI no stderr".to_string()
        } else {
            hint
        };
        Err(Error::CopilotFailed { hint })
    }
}

/// True when the CLI stderr carries an auth/SAML/SSO signature (case-
/// insensitive) — see [`AUTH_MARKERS`]. The stderr itself is never echoed;
/// this is only a classifier.
fn is_auth_problem(stderr: &str) -> bool {
    let lowered = stderr.to_lowercase();
    AUTH_MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// Tolerant parser of the CLI stdout (ADR-0019): the exact chrome changes
/// between CLI versions, so the parser only makes assumptions it documents:
///
/// 1. ANSI escape sequences (CSI/OSC/styles) are removed everywhere;
/// 2. banner lines are dropped by a narrow heuristic — a line IS chrome when
///    it starts with a box-drawing char (U+2500..=U+257F), a braille spinner
///    (U+2800..=U+28FF), a check/cross mark, or names the CLI itself
///    ("copilot", "using model", case-insensitive);
/// 3. leading/trailing blank lines are trimmed (interior blank lines are
///    kept — the triage answer has sections);
/// 4. nothing left => [`Error::CopilotFailed`] (an empty answer is a
///    failure, not silence).
pub fn parse_output(stdout: &str) -> Result<String, Error> {
    let plain = strip_ansi(stdout);
    let body: String = plain
        .lines()
        .filter(|line| !is_banner_line(line))
        .collect::<Vec<&str>>()
        .join("\n");
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err(Error::CopilotFailed {
            hint: "o CLI terminou com sucesso mas não devolveu texto na saída".to_string(),
        });
    }
    Ok(body)
}

/// True for a CLI-chrome line — see [`parse_output`] item 2.
fn is_banner_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    let box_or_spinner =
        (0x2500..=0x257F).contains(&(first as u32)) || (0x2800..=0x28FF).contains(&(first as u32));
    let lowered = trimmed.to_lowercase();
    box_or_spinner
        || matches!(first, '✓' | '✗' | '✔' | '✘')
        || lowered.contains("copilot")
        || lowered.contains("using model")
}

/// Removes ANSI escape sequences (CSI `ESC [ … final-byte`, OSC `ESC ] …
/// BEL|ST`, and a bare `ESC` with a non-`[`/`]` byte) — pure, no deps.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                let mut prev_st = false;
                for next in chars.by_ref() {
                    if next == '\u{7}' {
                        break;
                    }
                    if prev_st && next == '\\' {
                        break;
                    }
                    prev_st = next == '\u{1b}';
                }
            }
            _ => {
                // A stray escape byte: drop it (and any attached single byte)
                // instead of leaking raw control text into the answer.
                let _ = chars.next();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// The clean three-section answer every fake CLI returns.
    const ANSWER: &str = "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia";

    /// ANSI-styled, banner-laden stdout a CLI version could produce: colors
    /// around a box border, a green check line, then the real answer.
    const DIRTY_STDOUT: &str = "\u{1b}[1m\u{1b}[36m╭──────────────────────────╮\u{1b}[0m\n\u{1b}[32m✓ Connected to Copilot CLI\u{1b}[0m\n\u{1b}[2mUsing model: gpt-4.1\u{1b}[0m\nCausa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia\n\n";

    /// Unique scratch dir per test (shims written here, never env vars).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-copilot-test-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    /// Writes a fake `copilot` shim (`.cmd` on Windows, shebang script
    /// elsewhere) and makes it executable; returns its full path.
    fn write_shim(dir: &Path, windows_script: &str, unix_script: &str) -> PathBuf {
        let (name, script) = if cfg!(windows) {
            (String::from("copilot.cmd"), windows_script)
        } else {
            (String::from("copilot"), unix_script)
        };
        let path = dir.join(&name);
        fs::write(&path, script).unwrap_or_else(|err| panic!("escrever shim {path:?}: {err}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|err| panic!("chmod shim {path:?}: {err}"));
        }
        path
    }

    /// A shim that prints the dirty (ANSI + banner) canned answer. The ANSI
    /// codes prove the real-process stripping; every banner line names the
    /// CLI ("copilot"), the pattern the tolerant parser filters.
    fn shim_success(dir: &Path) -> PathBuf {
        let unix = "#!/bin/sh\nprintf '\\033[1m\\033[36mCopilot CLI v0.9\\033[0m\\n'\nprintf '\\033[32mCopilot: connected\\033[0m\\n'\nprintf 'Causa raiz provável:\\nTTL do token muito curto.\\nArquivos prováveis:\\nsrc/auth/session.rs\\nConfiança:\\nmédia\\n'\n".to_string();
        let windows = "@echo Causa raiz provável:\r\n@echo TTL do token muito curto.\r\n@echo Arquivos prováveis:\r\n@echo src/auth/session.rs\r\n@echo Confiança:\r\n@echo média\r\n".to_string();
        write_shim(dir, &windows, &unix)
    }

    #[test]
    fn parse_output_strips_ansi_and_banner_lines() {
        let parsed = parse_output(DIRTY_STDOUT)
            .unwrap_or_else(|err| panic!("saída suja deve parsear: {err}"));
        assert_eq!(parsed, ANSWER, "ANSI e banner devem sair: {parsed}");
        assert!(
            !parsed.contains('\u{1b}'),
            "nenhum byte de escape sobrevive: {parsed}"
        );
    }

    #[test]
    fn parse_output_keeps_interior_blank_lines_and_trims_edges() {
        let parsed = parse_output("banner com copilot\n\nprimeira\n\nsegunda\n\n")
            .unwrap_or_else(|err| panic!("parse não pode falhar: {err}"));
        assert_eq!(parsed, "primeira\n\nsegunda", "resultado: {parsed}");
    }

    #[test]
    fn parse_output_empty_is_a_copilot_failed() {
        for empty in ["", "   \n \n", "\u{1b}[31m\u{1b}[0m"] {
            let Err(Error::CopilotFailed { .. }) = parse_output(empty) else {
                panic!("saída vazia deve ser CopilotFailed: {empty:?}");
            };
        }
    }

    #[test]
    fn args_for_compose_system_and_prompt_with_silent_flag() {
        let backend = CopilotBackend::new(
            PathBuf::from("/tmp/copilot"),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        let args = backend.args_for("sistema", "pergunta");
        let texts: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(texts.first().map(String::as_str), Some("-p"), "{texts:?}");
        assert_eq!(texts.last().map(String::as_str), Some("-s"), "{texts:?}");
        assert!(
            texts.contains(&String::from("sistema\n\npergunta")),
            "o texto composto system+prompt vai inteiro num único argumento: {texts:?}"
        );
    }

    #[test]
    fn resolve_bin_missing_everywhere_is_copilot_missing() {
        assert!(
            matches!(
                resolve_bin(Some("/kira/sem/copilot-que-nao-existe"), None),
                Err(Error::CopilotMissing)
            ),
            "override inexistente deve ser CopilotMissing"
        );
        assert!(
            matches!(
                resolve_bin(Some("   "), Some(std::ffi::OsStr::new("/usr/bin"))),
                Err(Error::CopilotMissing)
            ),
            "override vazio não pode apontar binário: deve ser CopilotMissing"
        );
    }

    #[test]
    fn resolve_bin_prefers_the_override_and_then_searches_the_path() {
        let dir = scratch("resolve");
        let shim = shim_success(&dir);
        let found = resolve_bin(Some(shim.to_string_lossy().as_ref()), None)
            .unwrap_or_else(|err| panic!("override deve resolver: {err}"));
        assert_eq!(found, shim);
        // No override: the same shim is found on the given PATH.
        let found = resolve_bin(None, Some(dir.as_os_str()))
            .unwrap_or_else(|err| panic!("PATH deve resolver: {err}"));
        assert_eq!(found, shim);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn complete_returns_the_clean_answer_from_the_fake_cli() {
        let dir = scratch("success");
        let bin = shim_success(&dir);
        let backend = CopilotBackend::new(bin, Duration::from_secs(30));
        let text = backend
            .complete("sistema", "prompt")
            .await
            .unwrap_or_else(|err| panic!("CLI falso deve responder: {err}"));
        assert!(text.contains("Causa raiz provável:"), "resposta: {text}");
        assert!(
            text.contains("src/auth/session.rs"),
            "resposta completa: {text}"
        );
        assert!(!text.contains('\u{1b}'), "ANSI não sobrevive: {text}");
        assert!(!text.contains("Copilot"), "banner não sobrevive: {text}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn saml_stderr_becomes_copilot_auth_with_all_four_steps() {
        let dir = scratch("saml");
        let bin = write_shim(
            &dir,
            "@echo Traceback (most recent call last): 1>&2\r\n@echo SAML SSO assertion required 1>&2\r\n@exit /b 1\r\n",
            "#!/bin/sh\necho 'Traceback (most recent call last):' 1>&2\necho 'SAML SSO assertion required (Sapling)' 1>&2\nexit 1\n",
        );
        let backend = CopilotBackend::new(bin, Duration::from_secs(30));
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("CLI com SAML no stderr deve falhar com CopilotAuth");
        };
        assert!(
            matches!(err, Error::CopilotAuth),
            "variante deve ser CopilotAuth: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.to_lowercase().contains("copilot auth login"),
            "passo 1 (login) deve estar na mensagem: {message}"
        );
        assert!(
            message.to_lowercase().contains("grant")
                || message.to_lowercase().contains("authorize"),
            "passo 2 (botão Grant/Authorize) deve estar na mensagem: {message}"
        );
        assert!(
            message.contains("https://github.com/settings/connections/applications"),
            "passo 2 deve apontar a página de apps OAuth: {message}"
        );
        assert!(
            message.contains("TI"),
            "passo 3 (aprovação da TI) deve estar na mensagem: {message}"
        );
        assert!(
            message.contains("--llm ollama"),
            "passo 4 (fallback explícito) deve estar na mensagem: {message}"
        );
        assert!(
            !message.contains("Traceback"),
            "stderr cru nunca aparece na mensagem: {message}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn non_auth_failure_keeps_only_the_first_stderr_line() {
        let dir = scratch("failure");
        let bin = write_shim(
            &dir,
            "@echo quota esgotada do CLI 1>&2\r\n@echo segunda linha secreta 1>&2\r\n@exit /b 3\r\n",
            "#!/bin/sh\necho 'quota esgotada do CLI' 1>&2\necho 'segunda linha secreta' 1>&2\nexit 3\n",
        );
        let backend = CopilotBackend::new(bin, Duration::from_secs(30));
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("exit != 0 deve virar erro");
        };
        let Error::CopilotFailed { hint } = &err else {
            panic!("variante deve ser CopilotFailed: {err:?}");
        };
        assert_eq!(hint, "quota esgotada do CLI", "dica: {hint}");
        let message = err.to_string();
        assert!(
            !message.contains("segunda linha secreta"),
            "só a 1ª linha do stderr pode aparecer: {message}"
        );
        assert!(
            message.contains("copilot -p teste"),
            "mensagem deve sugerir o teste fora do KIRA: {message}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn silent_success_with_empty_stdout_is_a_failure() {
        let dir = scratch("empty");
        let bin = write_shim(&dir, "@exit /b 0\r\n", "#!/bin/sh\nexit 0\n");
        let backend = CopilotBackend::new(bin, Duration::from_secs(30));
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("saída vazia deve virar erro");
        };
        let Error::CopilotFailed { hint } = &err else {
            panic!("variante deve ser CopilotFailed: {err:?}");
        };
        assert!(hint.contains("não devolveu texto"), "dica: {hint}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn slow_cli_is_killed_at_the_timeout_budget() {
        let dir = scratch("timeout");
        let bin = write_shim(
            &dir,
            "@ping -n 8 127.0.0.1 >nul\r\n",
            "#!/bin/sh\nsleep 5\n",
        );
        let backend = CopilotBackend::new(bin, Duration::from_secs(1));
        let started = Instant::now();
        let Err(err) = backend.complete("sistema", "prompt").await else {
            panic!("script dormindo 5s com budget de 1s deve falhar");
        };
        assert!(
            matches!(err, Error::CopilotTimeout { secs: 1 }),
            "variante deve ser CopilotTimeout(1): {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains("1s") && message.contains("KIRA_COPILOT_TIMEOUT"),
            "mensagem deve citar o limite e o env: {message}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(4),
            "o kill deve acontecer perto do budget, não após o sleep do script: {:?}",
            started.elapsed()
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
