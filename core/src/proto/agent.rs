//! Agent wire types — exported to TypeScript via `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Writer,
    Deployer,
    Judge,
    Reviewer,
}

impl AgentKind {
    /// Tier the kind defaults to. Matches plan T7.
    pub fn default_tier(self) -> &'static str {
        match self {
            Self::Writer => "standard",
            Self::Deployer => "light",
            Self::Judge => "heavy",
            Self::Reviewer => "heavy",
        }
    }

    pub fn system_prompt(self) -> &'static str {
        // Shared principles + kind-specific instructions, glued at compile time.
        const SEPARATOR: &str = "\n\n---\n\n";
        const WRITER: &str = concat!(
            include_str!("../agents/prompts/_principles.md"),
            "\n\n---\n\n",
            include_str!("../agents/prompts/writer.md"),
        );
        const DEPLOYER: &str = concat!(
            include_str!("../agents/prompts/_principles.md"),
            "\n\n---\n\n",
            include_str!("../agents/prompts/deployer.md"),
        );
        const JUDGE: &str = concat!(
            include_str!("../agents/prompts/_principles.md"),
            "\n\n---\n\n",
            include_str!("../agents/prompts/judge.md"),
        );
        const REVIEWER: &str = concat!(
            include_str!("../agents/prompts/_principles.md"),
            "\n\n---\n\n",
            include_str!("../agents/prompts/reviewer.md"),
        );
        let _ = SEPARATOR; // documentation marker
        match self {
            Self::Writer => WRITER,
            Self::Deployer => DEPLOYER,
            Self::Judge => JUDGE,
            Self::Reviewer => REVIEWER,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Spawning,
    Active,
    Stale,
    Completed,
    Failed,
    Stopped,
    Cancelled,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
pub struct SpawnAgentRequest {
    pub kind: AgentKind,
    pub task: String,
    /// Existing worktree to attach. If absent, daemon spawns a new one from `base` (default "main").
    #[serde(default)]
    pub worktree_id: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
    /// Per-task budget cap in USD. Defaults from server config.
    #[serde(default)]
    pub budget_usd: Option<f64>,
    /// Max iterations of the agent loop. Defaults to 1 in v0 (single-shot).
    #[serde(default)]
    pub max_iters: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
pub struct Agent {
    pub id: String,
    pub kind: AgentKind,
    pub status: AgentStatus,
    pub worktree_id: String,
    pub task: String,
    pub tier: String,
    pub model: Option<String>,
    pub iters: u32,
    pub max_iters: u32,
    pub budget_usd_cap: f64,
    pub usd_spent: f64,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
    /// ISO 8601 of the most recent iters increment. Used by the
    /// heartbeat watchdog to detect stalled agents.
    pub last_progress_at: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
pub struct CancelResponse {
    pub cancelled: bool,
    pub id: String,
}
