//! Pilot/Orquestrador wire types (P2).
//!
//! `Intent` = what to do next (decided by Pilot).
//! `StepResult` = how it went (reported by Orquestrador).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::proto::sdd::{PlanQuestion, SddPhase};

/// Tier hint used by `EscalateTier` intent.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/intent.ts")]
#[serde(rename_all = "snake_case")]
pub enum TierHint {
    Light,
    Standard,
    Heavy,
    LongContext,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/intent.ts")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    // ── Generic workflow intents (WorkflowPilot) ───────────────────────────
    /// Run a Writer-kind phase identified by `phase_id`.
    RunPhase { phase_id: String },
    /// Fan-out Execute phase: parse tasks.json → Waves.
    RunExecute { phase_id: String },

    // ── Legacy SDD-specific intents (SddPilot — deprecated, remove in P2) ─
    /// Execute a specific SDD phase (legacy).
    RunSddPhase { phase: SddPhase },
    /// Spawn the EXECUTE phase task fan-out (legacy).
    RunSddExecute,

    // ── Shared intents ─────────────────────────────────────────────────────
    /// Escalate to a higher tier (e.g. judge low confidence → Heavy retry).
    EscalateTier { tier: TierHint, reason: String },
    /// Pause and ask the human a question. Non-blocking — state is persisted.
    RequestHumanReview { question: String },
    /// Pause execution, store questions, wait for user answers.
    AwaitUserInput { questions: Vec<PlanQuestion> },
    /// Terminal success.
    Done,
    /// Terminal failure with reason.
    Halt { reason: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/intent.ts")]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    Ok,
    Failed,
    Pending,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/intent.ts")]
pub struct StepResult {
    pub outcome: StepOutcome,
    pub message: String,
    pub duration_ms: u64,
    pub cost_usd: f64,
    /// Optional path to artifact produced by this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    /// Optional structured fields (`judge_score`, `gate_overall`, etc).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub fields: serde_json::Map<String, serde_json::Value>,
}
