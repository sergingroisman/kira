//! WorkflowRun — generic run state for any Workflow.
//!
//! Replaces the SDD-specific `SddRun` as the state carrier. Artifacts are
//! stored in a `HashMap<name, path>` instead of typed fields, so any phase
//! can produce any artifact without changes to this struct.
//!
//! Terminal phase ids (sentinels):
//! - `PHASE_DONE`   = `"$done"`
//! - `PHASE_FAILED` = `"$failed"`

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::proto::sdd::PlanQuestion;

/// Sentinel: workflow completed successfully.
pub const PHASE_DONE: &str = "$done";
/// Sentinel: workflow halted with error.
pub const PHASE_FAILED: &str = "$failed";

/// Generic workflow run snapshot. Persisted in redb, streamed via SSE.
#[derive(Serialize, Deserialize, Debug, Clone, TS, Default)]
#[ts(export, export_to = "../../agent/src/generated/run.ts")]
pub struct WorkflowRun {
    pub id: String,
    pub goal: String,
    /// Workflow id (matches the `id` field in `workflow.toml`).
    pub workflow_id: String,
    /// Kind hint ("feature", "bug_fix", "docs", "refactor", "unknown").
    #[serde(default)]
    pub kind: String,
    /// Current phase id. `PHASE_DONE` / `PHASE_FAILED` are terminal sentinels.
    pub current_phase_id: String,
    /// Ids of phases that ran to completion.
    #[serde(default)]
    pub phases_run: Vec<String>,
    /// Ids of phases skipped by the sizing service.
    #[serde(default)]
    pub skipped_phases: Vec<String>,
    /// Artifacts produced by phases: name → absolute path.
    /// e.g. `{"spec.md": "/path/to/.kira/runs/<id>/spec.md"}`.
    #[serde(default)]
    pub artifacts: HashMap<String, String>,
    /// Absolute path to the execution log (`execution.jsonl`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_log_path: Option<String>,
    pub worktree_id: String,
    pub worktree_path: String,
    /// Per-phase verdicts (gate score + overall).
    #[serde(default)]
    pub phase_verdicts: Vec<PhaseVerdict>,
    pub usd_spent: f64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// FallbackChain attempt counters: phase_id → failure count.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub phase_retries: HashMap<String, u32>,
    /// Questions pending user answers before proceeding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_questions: Vec<PlanQuestion>,
    /// Resolved Q+A pairs for downstream phase context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clarifications: Vec<ClarificationEntry>,
    /// User answers: question_id → selected labels.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub question_answers: HashMap<String, Vec<String>>,
    /// Gate flag from the start request — when true, the drive loop pauses
    /// before the EXECUTE phase fans out waves, for operator plan approval.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_plan_approval: bool,
    /// Live pause flag — true while the run waits for plan approval.
    /// Raised when advancing into an Execute-kind phase under
    /// `require_plan_approval`; cleared by `approve_plan`. Mirrors
    /// `pending_questions` pause semantics.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub awaiting_plan_approval: bool,
}

impl WorkflowRun {
    pub fn is_terminal(&self) -> bool {
        self.current_phase_id == PHASE_DONE || self.current_phase_id == PHASE_FAILED
    }

    /// Convenience: get the tasks.json artifact path (the narrow waist).
    pub fn tasks_path(&self) -> Option<&str> {
        self.artifacts.get("tasks.json").map(String::as_str)
    }

    /// Convenience: set an artifact by name.
    pub fn set_artifact(&mut self, name: impl Into<String>, path: impl Into<String>) {
        self.artifacts.insert(name.into(), path.into());
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/run.ts")]
pub struct PhaseVerdict {
    pub phase_id: String,
    pub judge_score: Option<f64>,
    pub gate_overall: String,
    pub artifact_path: Option<String>,
    pub duration_ms: u64,
}

/// Resolved Q+A pair from the clarification step.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/run.ts")]
pub struct ClarificationEntry {
    pub question: String,
    pub answers: Vec<String>,
}

/// Request to start a new workflow run.
#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/run.ts")]
pub struct StartWorkflowRequest {
    pub goal: String,
    /// Workflow to use (e.g. `"sdd"`, `"gsd2"`). Defaults to `"sdd"`.
    #[serde(default = "default_workflow")]
    pub workflow: String,
    /// Kind hint for sizing heuristics.
    #[serde(default)]
    pub kind: Option<String>,
    /// LOC estimate for the sizing service.
    #[serde(default)]
    pub estimated_loc: Option<u32>,
    /// File count estimate for the sizing service.
    #[serde(default)]
    pub estimated_files: Option<u32>,
    /// Force the full pipeline regardless of sizing.
    #[serde(default)]
    pub force_full_pipeline: Option<bool>,
    /// Attach to an existing worktree (else daemon spawns one).
    #[serde(default)]
    pub worktree_id: Option<String>,
    /// Base branch when auto-spawning a worktree.
    #[serde(default)]
    pub base: Option<String>,
    /// Require operator approval of the task plan before EXECUTE waves run.
    /// Set by the CLI in PLAN mode (and AUTO→plan). YOLO leaves it false.
    #[serde(default)]
    pub require_plan_approval: Option<bool>,
}

fn default_workflow() -> String { "sdd".into() }

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/run.ts")]
pub struct WorkflowListResponse {
    pub runs: Vec<WorkflowRun>,
}
