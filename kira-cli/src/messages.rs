#![forbid(unsafe_code)]

//! Message types for the channel-based event bus.
//!
//! All background tasks (SSE listener, health checker, agent pollers, command
//! executors) communicate with the event loop exclusively through `UiMessage`.
//! The event loop is the sole writer to `AppState`.

use tokio::time::Instant;

use crate::app::OutputLineKind;
use crate::models::{
    BudgetInfo, ModifiedFile, ProviderStatus, SddPipelineState, SessionEntry, TaskWave, WaveState,
};
use crate::sse::PillarEventKind;

/// Messages sent from background tasks to the event loop via bounded mpsc channel.
#[derive(Debug, Clone)]
pub enum UiMessage {
    // ── Connection status ──────────────────────────────────────────────────
    /// Core health check succeeded; includes the daemon version string.
    CoreConnected { version: String },
    /// Core became unreachable; includes a human-readable reason.
    CoreDisconnected { reason: String },

    // ── SSE pillar events ──────────────────────────────────────────────────
    /// A pillar event arrived via the SSE stream.
    PillarEvent(PillarEventKind),

    // ── Agent lifecycle ────────────────────────────────────────────────────
    /// A new agent was spawned (returned from POST /v1/chat).
    AgentSpawned { agent_id: String },
    /// A streaming output line from an active agent.
    AgentOutput { agent_id: String, line: String },
    /// An agent reached a terminal status.
    AgentTerminal {
        agent_id: String,
        status: String,
        error: Option<String>,
        /// Final answer produced by the agent (the `done(final_result)` payload
        /// or content-only response). Rendered into the chat on completion.
        result: Option<String>,
    },

    // ── HITL ───────────────────────────────────────────────────────────────
    /// A HIGH/CRITICAL risk tool call requires operator approval.
    ApprovalRequired(ApprovalRequest),

    // ── Command results ────────────────────────────────────────────────────
    /// Successful result from an async command execution.
    CommandResult {
        output: String,
        kind: OutputLineKind,
    },
    /// Error from an async command execution.
    CommandError { message: String },

    // ── Message submission ─────────────────────────────────────────────────
    /// POST /v1/chat/yolo succeeded — agent spawned or direct response.
    MessageAccepted {
        agent_id: String,
        /// Direct response content (present on fast-path conceptual answers).
        response: Option<String>,
    },
    /// POST /v1/chat failed (network, timeout, non-2xx).
    MessageFailed { reason: String },

    /// PLAN mode: the workflow paused at the plan-approval gate; the matrix
    /// (tasks + waves + files) is ready for operator y/n.
    PlanReady(crate::core_client::PlanMatrixView),

    /// Workflow run status poll (phase progress, questions, done).
    WorkflowUpdate(crate::core_client::WorkflowRunView),

    // ── Task health ────────────────────────────────────────────────────────
    /// A background task terminated unexpectedly.
    TaskFailed { task_name: String, error: String },

    // ── Sidebar & overlay data refresh ─────────────────────────────────────
    /// Session list response (for session picker).
    SessionList { sessions: Vec<SessionEntry> },
    /// Task/wave status update (for task browser).
    TaskUpdate { waves: Vec<TaskWave> },
    /// Sidebar data refresh (budget, providers, files).
    SidebarRefresh {
        budget: Option<BudgetInfo>,
        providers: Option<Vec<ProviderStatus>>,
        pipeline: Option<SddPipelineState>,
        waves: Option<Vec<WaveState>>,
        modified_files: Option<Vec<ModifiedFile>>,
    },

    // ── Streaming ──────────────────────────────────────────────────────────
    /// Streaming token from active agent.
    StreamingToken { agent_id: String, token: String },
    /// Streaming completed for agent.
    StreamingDone { agent_id: String },
}

/// A HITL approval request received from the Core via SSE.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// The agent that triggered the tool call.
    pub agent_id: String,
    /// Name of the tool requiring approval.
    pub tool_name: String,
    /// Risk classification (e.g. "HIGH", "CRITICAL").
    pub risk_level: String,
    /// Truncated preview of the tool arguments (max 2048 chars).
    pub arguments_preview: String,
    /// Seconds before auto-rejection.
    pub timeout_secs: u64,
    /// When this request was received locally.
    pub received_at: Instant,
}

impl ApprovalRequest {
    /// Short agent ID (first 8 characters of the UUID).
    pub fn short_agent_id(&self) -> &str {
        if self.agent_id.len() >= 8 {
            &self.agent_id[..8]
        } else {
            &self.agent_id
        }
    }

    /// Seconds remaining before auto-rejection.
    pub fn seconds_remaining(&self) -> u64 {
        let elapsed = self.received_at.elapsed().as_secs();
        self.timeout_secs.saturating_sub(elapsed)
    }

    /// Whether the timeout has expired (elapsed >= timeout_secs).
    pub fn is_expired(&self) -> bool {
        self.received_at.elapsed().as_secs() >= self.timeout_secs
    }
}

/// A message queued locally while the Core is unreachable.
#[derive(Debug, Clone)]
pub struct PendingMessage {
    /// The chat message content.
    pub message: String,
    /// The LLM tier at the time of submission.
    pub tier: String,
    /// When this message was queued locally.
    pub queued_at: Instant,
}

/// Tracks a single active agent thread.
#[derive(Debug, Clone)]
pub struct AgentThread {
    /// Full agent UUID.
    pub id: String,
    /// First 8 characters of the UUID for display.
    pub short_id: String,
    /// Agent kind (e.g. "writer", "deployer", "judge").
    pub kind: String,
    /// Current status (e.g. "running", "completed", "failed").
    pub status: String,
    /// Cumulative cost in USD for this agent.
    pub cost_usd: f64,
    /// Last output line received from this agent.
    pub last_output: Option<String>,
}
