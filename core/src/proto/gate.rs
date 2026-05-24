//! Completion Gates wire types — Spektor pillar P4.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/gate.ts")]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    StructuralValidation,
    FileIntegrity,
    ChangeSufficiency,
    EmbeddingFaithfulness,
    JudgeLlm,
    /// L6 — Anti-hallucination meta-review (TRUST-inspired).
    /// Parses reviewer-style output and verifies each `evidence_quote`
    /// is a literal substring of the supplied haystack.
    MetaReview,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/gate.ts")]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    Pass,
    Fail,
    Skipped,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/gate.ts")]
pub struct GateResult {
    pub gate: GateKind,
    pub outcome: GateOutcome,
    pub score: Option<f64>,
    pub reason: Option<String>,
    pub suggestion: Option<String>,
    pub duration_ms: u64,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/gate.ts")]
pub struct RunGatesRequest {
    /// Spec / acceptance criteria the output should match.
    pub spec: String,
    /// Output to evaluate.
    pub output: String,
    /// Optional ownership context: which paths the producer claimed.
    #[serde(default)]
    pub owned_paths: Vec<String>,
    /// Optional list of files that were actually written.
    #[serde(default)]
    pub touched_paths: Vec<String>,
    /// Optional estimated LOC declared by the producer.
    #[serde(default)]
    pub estimated_loc: Option<u32>,
    /// Optional actual LOC of the diff.
    #[serde(default)]
    pub actual_loc: Option<u32>,
    /// Skip the embedding gate (saves an API call).
    #[serde(default)]
    pub skip_embedding: bool,
    /// Skip the judge gate (saves a Heavy LLM call).
    #[serde(default)]
    pub skip_judge: bool,
    /// Source-of-truth text the meta-review gate uses to verify
    /// `evidence_quote` literals. Typically the diff or the concatenated
    /// content of changed files. When empty, the gate is skipped.
    #[serde(default)]
    pub haystack: String,
    /// Skip the meta-review gate.
    #[serde(default)]
    pub skip_meta_review: bool,
    /// Optional worktree path for L1 to run real lint/typecheck.
    /// When empty, L1 falls back to structural heuristics (regex truncation
    /// + brace balance). When provided + project type detected, L1 runs
    /// `cargo check` / `bun lint` / `tsc --noEmit`.
    #[serde(default)]
    pub worktree_path: String,
    /// Skip the structural lint check (force heuristic-only L1).
    #[serde(default)]
    pub skip_structural_lint: bool,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/gate.ts")]
pub struct RunGatesResponse {
    pub results: Vec<GateResult>,
    pub overall: GateOutcome,
    pub failed_at: Option<GateKind>,
    pub duration_ms: u64,
}
