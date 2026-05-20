//! Worktree wire types — exported to TypeScript via `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/worktree.ts")]
pub struct SpawnRequest {
    /// Base branch to fork from. Defaults to "main".
    #[serde(default = "default_base")]
    pub base: String,
    /// Optional agent kind hint stored alongside the worktree.
    #[serde(default)]
    pub agent_kind: Option<String>,
}

fn default_base() -> String {
    "main".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/worktree.ts")]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    Spawning,
    Idle,
    Active,
    Stale,
    Completed,
    Failed,
    Cleaning,
    Destroyed,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/worktree.ts")]
pub struct Worktree {
    pub id: String,
    pub branch: String,
    pub path: String,
    pub base_branch: String,
    pub status: WorktreeStatus,
    pub agent_kind: Option<String>,
    /// ISO 8601 UTC.
    pub created_at: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/worktree.ts")]
pub struct DestroyResponse {
    pub destroyed: bool,
    pub branch_deleted: bool,
    pub id: String,
}
