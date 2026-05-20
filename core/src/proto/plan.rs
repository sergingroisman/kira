//! Plan/Act split wire types — exported to TypeScript via `ts-rs`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::proto::agent::AgentKind;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Emitted by planner; awaiting human approval.
    Draft,
    /// Approved; executor agent spawned.
    Approved,
    /// Executor agent running.
    Executing,
    /// Executor finished successfully.
    Done,
    /// Executor failed.
    Failed,
    /// Cancelled before or during execution.
    Cancelled,
    /// TTL elapsed without approval.
    Expired,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
pub struct PlanStep {
    pub idx: u32,
    pub description: String,
    /// Optional hint of which tool the step will use.
    #[serde(default)]
    pub tool: Option<String>,
    /// Optional rationale / why this step.
    #[serde(default)]
    pub rationale: Option<String>,
    /// FS paths the step expects to write (used for ownership + change sufficiency).
    #[serde(default)]
    pub owned_files: Vec<String>,
    /// Rough LOC estimate (used by change sufficiency check in v0.5).
    #[serde(default)]
    pub estimated_loc: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
pub struct Plan {
    pub id: String,
    pub goal: String,
    pub agent_kind: AgentKind,
    pub steps: Vec<PlanStep>,
    pub status: PlanStatus,
    /// ISO 8601 UTC.
    pub created_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub approved_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    /// When approved, the executor agent id.
    #[serde(default)]
    pub executor_agent_id: Option<String>,
    /// Raw model output (for debugging).
    #[serde(default)]
    pub raw_planner_output: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
pub struct DraftRequest {
    pub goal: String,
    pub agent_kind: AgentKind,
    /// Optional TTL override (seconds). Defaults to 3600.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
pub struct ApproveResponse {
    pub plan_id: String,
    pub executor_agent_id: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/plan.ts")]
pub struct CancelPlanResponse {
    pub cancelled: bool,
    pub id: String,
}
