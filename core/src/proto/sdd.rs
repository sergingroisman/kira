//! Spec-Driven Execution (P1) — wire types.
//!
//! Pipeline: SPECIFY → DESIGN → TASKS → EXECUTE, with auto-sizing that
//! collapses trivial work directly to TASKS+EXECUTE. Each phase produces
//! a typed artifact persisted under `.kira/runs/<run_id>/`:
//!
//! - `spec.md`        — requirements + acceptance criteria
//! - `design.md`      — architectural decisions
//! - `tasks.json`     — atomic tasks with AC + `owned_files`
//! - `execution.jsonl` — per-task append log

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// SDD lifecycle states. `current_phase` advances on judge pass.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
#[serde(rename_all = "snake_case")]
pub enum SddPhase {
    Specify,
    Design,
    Tasks,
    Execute,
    Done,
    Failed,
}

/// Kind drives the auto-sizing heuristic + prompt selection per phase.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SddKind {
    BugFix,
    Feature,
    Docs,
    Refactor,
    #[default]
    Unknown,
}


/// Auto-sizing verdict — drives which phases run vs skip.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
#[serde(rename_all = "snake_case")]
pub enum SizingVerdict {
    /// Run full 4-phase pipeline.
    Full,
    /// Skip SPECIFY+DESIGN, jump to TASKS+EXECUTE.
    SkipSpecifyDesign,
    /// Skip everything, single-shot EXECUTE (very tiny — e.g. typo fix).
    /// Reserved for v0.5+; v0 collapses to `SkipSpecifyDesign`.
    SingleShot,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct StartSddRequest {
    pub goal: String,
    /// Hint for kind. Engine still validates against goal length / keywords.
    #[serde(default)]
    pub kind: Option<SddKind>,
    /// Caller estimate of LOC. Used by auto-sizing heuristic.
    #[serde(default)]
    pub estimated_loc: Option<u32>,
    /// Caller estimate of file count. Used by auto-sizing heuristic.
    #[serde(default)]
    pub estimated_files: Option<u32>,
    /// Force the full pipeline (skip auto-sizing).
    #[serde(default)]
    pub force_full_pipeline: Option<bool>,
    /// Optional worktree id to attach to (else daemon spawns one).
    #[serde(default)]
    pub worktree_id: Option<String>,
    /// Base branch when auto-spawning worktree.
    #[serde(default)]
    pub base: Option<String>,
}

/// A single option within a `PlanQuestion`.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct PlanOption {
    pub label: String,
    pub description: String,
}

/// A clarifying question the SDD Pilot asks the user during SPECIFY/DESIGN.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct PlanQuestion {
    pub id: String,
    pub question: String,
    pub options: Vec<PlanOption>,
    #[serde(default)]
    pub multi_select: bool,
}

/// User answers to pending questions. `answers` maps question_id → selected labels.
#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct AnswerQuestionsRequest {
    pub answers: std::collections::HashMap<String, Vec<String>>,
}

/// Resolve a plan-approval gate. `approved=false` halts the run.
#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct ApprovePlanRequest {
    pub approved: bool,
}

/// SDD run snapshot — persisted in redb, streamed via SSE on changes.
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct SddRun {
    pub id: String,
    pub goal: String,
    pub kind: SddKind,
    pub sizing: SizingVerdict,
    pub current_phase: SddPhase,
    pub phases_run: Vec<SddPhase>,
    pub phases_skipped: Vec<SddPhase>,

    /// Absolute paths to per-phase artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_log_path: Option<String>,

    /// Worktree this run owns (auto-spawned or reused).
    pub worktree_id: String,
    pub worktree_path: String,

    /// Per-phase verdicts (judge score 0-10, gate overall).
    pub phase_verdicts: Vec<PhaseVerdict>,

    /// Cumulative cost so far.
    pub usd_spent: f64,

    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// FallbackChain attempt counters: phase_name → failure count.
    /// Used by Pilot to decide retry vs escalate vs halt.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub phase_retries: std::collections::HashMap<String, u32>,

    /// Questions the Pilot needs answered before proceeding.
    /// Non-empty → drive loop pauses, REPL shows PlanQuestions UI.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_questions: Vec<PlanQuestion>,

    /// Answers collected from the user: question_id → selected labels.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub question_answers: std::collections::HashMap<String, Vec<String>>,

    /// Resolved Q+A pairs preserved for downstream phase context.
    /// Populated by `answer_questions` so the SPECIFY Writer sees the
    /// human-readable question text alongside the chosen options.
    /// `question_answers` keys by UUID which is opaque to the LLM.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clarifications: Vec<ClarificationEntry>,
}

/// A resolved clarifying Q+A pair. Stored on `SddRun.clarifications` after
/// the user answers via `/v1/sdd/{id}/questions`. Read by the orchestrator
/// when building the SPECIFY Writer task context (P1, 2026-05-25).
#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct ClarificationEntry {
    pub question: String,
    pub answers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct PhaseVerdict {
    pub phase: SddPhase,
    pub judge_score: Option<f64>,
    pub gate_overall: String,
    pub artifact_path: Option<String>,
    pub duration_ms: u64,
}

#[derive(Serialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/sdd.ts")]
pub struct SddListResponse {
    pub runs: Vec<SddRun>,
}
