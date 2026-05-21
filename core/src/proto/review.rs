//! PR review wire types (TRUST-inspired multi-dim review).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/review.ts")]
pub struct RunReviewRequest {
    /// Path to the workflow YAML, relative to the repo root.
    /// Defaults to `.kira/workflows/pr-review.yaml`.
    #[serde(default)]
    pub workflow: Option<String>,
    /// Feature branch under review.
    pub head: String,
    /// Base branch to diff against. Defaults to `main`.
    #[serde(default)]
    pub base: Option<String>,
    /// Per-agent timeout in seconds. Defaults to 300.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/review.ts")]
pub struct AgentReviewOutcome {
    pub dimension: String,
    pub agent_id: String,
    pub status: String,
    /// Number of findings that survived the meta-review gate.
    pub findings_kept: u32,
    /// Number of findings dropped (hallucinations / low-coverage).
    pub findings_dropped: u32,
    pub gate_overall: String,
    pub gate_failed_at: Option<String>,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/review.ts")]
pub struct RunReviewResponse {
    pub run_id: String,
    pub head: String,
    pub base: String,
    pub workflow: String,
    pub agents: Vec<AgentReviewOutcome>,
    /// Path to the generated REVIEW.md file (under `.kira/runs/<run_id>/`).
    pub review_md_path: String,
    pub total_findings: u32,
    pub duration_ms: u64,
}
