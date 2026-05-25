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
    Explorer,
}

impl AgentKind {
    /// Which tools this agent kind is allowed to use.
    /// `None` = inherit all built-in tools (default).
    pub const fn allowed_tools(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Writer | Self::Deployer => None,
            Self::Judge | Self::Reviewer | Self::Explorer => Some(&[
                "done",
                "file_read",
                "web_search",
                "web_fetch",
            ]),
        }
    }

    /// Tier the kind defaults to. Matches plan T7.
    pub const fn default_tier(self) -> &'static str {
        match self {
            Self::Writer => "standard",
            Self::Deployer => "light",
            Self::Judge => "heavy",
            Self::Reviewer => "heavy",
            Self::Explorer => "standard",
        }
    }

    pub const fn system_prompt(self) -> &'static str {
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
        const EXPLORER: &str = concat!(
            include_str!("../agents/prompts/_principles.md"),
            "\n\n---\n\n",
            include_str!("../agents/prompts/explorer.md"),
        );
        let _ = SEPARATOR; // documentation marker
        match self {
            Self::Writer => WRITER,
            Self::Deployer => DEPLOYER,
            Self::Judge => JUDGE,
            Self::Reviewer => REVIEWER,
            Self::Explorer => EXPLORER,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Pre-spawn state (reserved for future agent pool).
    Idle,
    /// Worktree being created / agent initializing.
    Spawning,
    /// Running synchronously (blocking the caller).
    ActiveForeground,
    /// Running asynchronously (background).
    ActiveBackground,
    /// Watchdog detected abandonment.
    Stale,
    /// Successfully completed.
    Completed,
    /// Unrecoverable error.
    Failed,
    /// Stall defense triggered (agent stuck).
    Stopped,
    /// User requested cancellation.
    Cancelled,
    /// System-forced termination (timeout, watchdog kill).
    Killed,
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
    /// YOLO mode: bypass plan gates, use reduced gate cascade (L1+L2 only).
    /// Faster, cheaper, but less thorough evaluation.
    #[serde(default)]
    pub yolo_mode: Option<bool>,
    /// Run in background (non-blocking). Defaults to foreground.
    #[serde(default)]
    pub background: Option<bool>,
    /// Timeout in seconds. Agent is cancelled if it exceeds this.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
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

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
pub struct ApprovalRequest {
    /// true = approve the pending HIGH-risk tool call.
    /// false = reject it (agent receives an error tool result).
    pub approved: bool,
    /// Optional human feedback explaining the decision.
    /// Injected into the tool result when the call is rejected.
    #[serde(default)]
    pub feedback: Option<String>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/agent.ts")]
pub struct ApprovalResponse {
    /// true if the agent had a pending approval request.
    pub resolved: bool,
    pub id: String,
    /// true if the tool was approved.
    pub approved: bool,
}

/// Agent lifecycle event — published on `EventBus<AgentEvent>`.
///
/// Consumers (watchdogs, orchestrator, REPL) subscribe to these
/// instead of polling `AgentStore`. This is the async alternative
/// to the FS-mailbox polling pattern used in v0.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AgentEvent {
    Spawned {
        agent_id: String,
        kind: AgentKind,
        worktree_id: String,
    },
    Heartbeat {
        agent_id: String,
        iters: u32,
        usd_spent: f64,
    },
    Completed {
        agent_id: String,
        result: Option<String>,
        iters: u32,
        usd_spent: f64,
    },
    Failed {
        agent_id: String,
        error: String,
        iters: u32,
        usd_spent: f64,
    },
    Stalled {
        agent_id: String,
        reason: String,
    },
    Cancelled {
        agent_id: String,
    },
}
