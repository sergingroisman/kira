//! Bash tool wire types.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct BashRequest {
    /// First element = executable name (must be in allowlist).
    /// Subsequent = args (must match SAFE_ARG_RE).
    /// Example: `["git", "status", "--short"]`
    pub argv: Vec<String>,

    #[serde(default)]
    pub cwd: Option<String>,

    #[serde(default = "default_bash_timeout")]
    pub timeout_secs: u64,
}

fn default_bash_timeout() -> u64 {
    crate::safety::cmd_allowlist::BASH_TIMEOUT_DEFAULT_SECS
}

#[derive(Serialize)]
pub struct BashResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub truncated: bool,
}
