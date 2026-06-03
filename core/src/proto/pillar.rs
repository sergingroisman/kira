//! Pillar transparency wire types (REPL breadcrumbs).
//!
//! Spektor/Kira 7 pillars + free-form metadata. Emitted as SSE frames on
//! `/v1/events/stream` and persisted append-only in `audit/pillars.jsonl`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/pillar.ts")]
#[serde(rename_all = "snake_case")]
pub enum Pillar {
    /// P1 — Spec-Driven Execution (Plan/Act, workflows YAML, SDD phases).
    P1Sdd,
    /// P2 — Coordenação Pilot (agent spawn, mailbox, parallel orchestration).
    P2Coordination,
    /// P3 — Verification & Judge (judge agent, embedding faithfulness).
    P3Verification,
    /// P4 — Completion Gates (L1-L6 cascade).
    P4Gates,
    /// P5 — Guardrails (ownership, budget, OWASP SC1-SC12, allowlist).
    P5Guardrails,
    /// P6 — Resiliência (circuit breaker, fallback chain, stall detection).
    P6Resilience,
    /// P7 — Observabilidade + Cognition Lessons (audit, lessons, bundles).
    P7Cognition,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/pillar.ts")]
pub struct PillarEvent {
    pub pillar: Pillar,
    /// Lifecycle marker: `start` | `step` | `fallback` | `fail` | `done`.
    pub phase: String,
    /// Human-readable one-liner, <120 chars.
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// RFC3339 timestamp.
    pub ts: String,
    /// Free-form structured fields (cost, model, scores, durations).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub fields: serde_json::Map<String, serde_json::Value>,
}
