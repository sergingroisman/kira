//! Shared process/binary helpers: one `which`-style resolver for every
//! spawned external tool (scan tools and the Copilot CLI).
//!
//! The logic used to live privately in [`crate::scan`]; PR11 (ADR-0019) gave
//! the Copilot backend the same contract, so the pure core moved here —
//! behavior of the scan resolvers is unchanged (they now delegate).
//!
//! Rules kept from the scan phase:
//! - resolution happens BEFORE the spawn: absence never reaches the exec;
//! - the search is a pure function of the PATH value (never parsed from
//!   OS-localized text);
//! - `.cmd`/`.bat` on Windows go through a `cmd /C` wrapper (the std would
//!   try to exec them as binaries);
//! - an error hint takes only the first non-empty stderr line, capped, with
//!   control chars removed — never the raw body.

use std::path::PathBuf;

/// Reads an env-var binary override (`KIRA_SCAN_*_BIN`, `KIRA_COPILOT_BIN`):
/// trimmed, absent/empty is `None`.
pub fn bin_from_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// `which`-style search on the given PATH value (`None`/empty => not found).
#[cfg(windows)]
pub fn find_on_path(path_env: Option<&std::ffi::OsStr>, program: &str) -> Option<PathBuf> {
    let path_env = path_env?;
    let path_ext = std::env::var_os("PATHEXT")
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
    for dir in std::env::split_paths(path_env) {
        for ext in path_ext.split(';') {
            let probe = format!("{program}{ext}");
            if dir.join(&probe).is_file() {
                // NTFS matches case-insensitively (`npm.COM` finds `npm.cmd`):
                // return the REAL on-disk name, never the probe, so messages
                // and paths show what actually exists.
                return Some(real_entry_name(&dir, &probe).unwrap_or_else(|| dir.join(&probe)));
            }
        }
    }
    None
}

/// On-disk entry in `dir` whose name matches `probe` case-insensitively.
#[cfg(windows)]
fn real_entry_name(dir: &std::path::Path, probe: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(probe) {
            Some(entry.path())
        } else {
            None
        }
    })
}

/// Unix variant: `<tool>` must exist as a file with any execute bit.
#[cfg(unix)]
pub fn find_on_path(path_env: Option<&std::ffi::OsStr>, program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path_env = path_env?;
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(program);
        if let Ok(metadata) = std::fs::metadata(&candidate)
            && metadata.is_file()
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Some(candidate);
        }
    }
    None
}

/// Other platforms (not built here): plain existence, like the Windows one.
#[cfg(not(any(windows, unix)))]
pub fn find_on_path(path_env: Option<&std::ffi::OsStr>, program: &str) -> Option<PathBuf> {
    let path_env = path_env?;
    std::env::split_paths(path_env)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// True when the path points to a `.cmd`/`.bat` script (std would try to exec
/// it as a binary instead of handing it to the shell).
pub fn is_cmd_script(bin: &str) -> bool {
    match std::path::Path::new(bin).extension() {
        Some(ext) => ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"),
        None => false,
    }
}

/// Compact raw hint for an error message: first non-empty line, trimmed,
/// control chars removed, capped at 200 chars. Empty stderr => empty string
/// (no hint at all). Never the raw body.
pub fn first_stderr_line(stderr: &str) -> String {
    let Some(line) = stderr.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return String::new();
    };
    let hint: String = line
        .chars()
        .filter(|ch| !ch.is_control())
        .take(200)
        .collect();
    hint.trim_end().to_string()
}
