//! SC3 — OS Command Injection Defense.
//!
//! Three layers:
//! 1. **Executable allowlist** — only these may run.
//! 2. **Argument regex** — alphanumeric + safe punctuation; rejects metacharacters.
//! 3. **No shell expansion** — `tokio::process::Command::args(&[...])` separate args.
//!
//! Plus timeout (caller enforces) + env_clear (handler enforces).

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;

/// Executables permitted in the `bash` tool. Tighten per agent kind in v0.5.
pub const BASH_ALLOWLIST: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "find", "git", "cargo", "bun", "npm",
    "pnpm", "node", "rg", "sd", "wc", "diff", "stat", "echo", "pwd", "which",
];

/// Forbidden metacharacters (documented; enforced via regex below).
/// `& | ; $ > < \ ! ' " ( ) backtick` plus whitespace within args.
pub static SAFE_ARG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_./@:=,\-+]+$").expect("hardcoded regex"));

pub const BASH_TIMEOUT_DEFAULT_SECS: u64 = 30;
pub const BASH_TIMEOUT_MAX_SECS: u64 = 300;
pub const BASH_OUTPUT_MAX_BYTES: usize = 256 * 1024; // 256 KiB cap

/// Validate `argv` against allowlist + regex.
/// Returns `(executable, args)` borrowed from `argv`, or an error.
pub fn validate_argv(argv: &[String]) -> Result<(&str, Vec<&str>)> {
    let (exe, args) = argv
        .split_first()
        .context("argv empty — first element must be executable")?;

    if !BASH_ALLOWLIST.contains(&exe.as_str()) {
        anyhow::bail!(
            "executable '{}' not in allowlist (SC3). Allowed: {:?}",
            exe,
            BASH_ALLOWLIST
        );
    }

    for (i, a) in args.iter().enumerate() {
        if !SAFE_ARG_RE.is_match(a) {
            anyhow::bail!(
                "arg #{} '{}' contains forbidden chars (SC3 regex {})",
                i,
                a,
                SAFE_ARG_RE.as_str()
            );
        }
    }

    Ok((exe.as_str(), args.iter().map(String::as_str).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_exe_passes() {
        let argv = vec!["git".into(), "status".into()];
        let (exe, args) = validate_argv(&argv).expect("should pass");
        assert_eq!(exe, "git");
        assert_eq!(args, vec!["status"]);
    }

    #[test]
    fn rm_blocked() {
        let argv = vec!["rm".into(), "-rf".into(), "/".into()];
        let err = validate_argv(&argv).unwrap_err();
        assert!(err.to_string().contains("not in allowlist"));
    }

    #[test]
    fn metachar_blocked() {
        let argv = vec!["ls".into(), "; cat /etc/passwd".into()];
        let err = validate_argv(&argv).unwrap_err();
        assert!(err.to_string().contains("forbidden chars"));
    }

    #[test]
    fn safe_arg_with_dashes_and_paths() {
        let argv = vec!["ls".into(), "-la".into(), "./src/main.rs".into()];
        validate_argv(&argv).expect("should pass");
    }

    #[test]
    fn empty_argv_fails() {
        let argv: Vec<String> = vec![];
        let err = validate_argv(&argv).unwrap_err();
        assert!(err.to_string().contains("argv empty"));
    }
}
