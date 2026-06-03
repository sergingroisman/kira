//! Bash tool wire types — exported to TypeScript via `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/bash.ts")]
pub struct BashRequest {
    pub argv: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "default_bash_timeout")]
    #[ts(type = "number")]
    pub timeout_secs: u64,
}

const fn default_bash_timeout() -> u64 {
    crate::safety::cmd_allowlist::BASH_TIMEOUT_DEFAULT_SECS
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/bash.ts")]
pub struct BashResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    #[ts(type = "number")]
    pub duration_ms: u128,
    pub truncated: bool,
}
