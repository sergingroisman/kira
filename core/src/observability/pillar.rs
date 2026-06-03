//! Pillar event emitter (broadcast + audit JSONL).
//!
//! `emit*` calls are best-effort: a missing audit dir or full subscriber
//! buffer never aborts the caller. Persistence and broadcast are decoupled.
//!
//! v0.5 split:
//!   - `emit` / `emit_full` — both wire (SSE) + telemetry (JSONL).
//!   - `emit_wire` — SSE only (fast UI breadcrumbs).
//!   - `emit_telemetry` — JSONL only (offline analysis, training data).

use chrono::Utc;

use crate::observability::telemetry;
use crate::observability::wire;
use crate::proto::pillar::{Pillar, PillarEvent};

pub use wire::channel;
pub use wire::PillarTx;

/// Emit a minimal event to **both** wire and telemetry.
pub fn emit(tx: &PillarTx, pillar: Pillar, phase: &str, message: impl Into<String>) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id: None,
        agent_id: None,
        ts: Utc::now().to_rfc3339(),
        fields: serde_json::Map::new(),
    };
    wire::send(tx, ev.clone());
    telemetry::append(&ev);
}

/// Emit with structured fields to **both** wire and telemetry.
#[allow(clippy::too_many_arguments)]
pub fn emit_full(
    tx: &PillarTx,
    pillar: Pillar,
    phase: &str,
    message: impl Into<String>,
    run_id: Option<String>,
    agent_id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id,
        agent_id,
        ts: Utc::now().to_rfc3339(),
        fields,
    };
    wire::send(tx, ev.clone());
    telemetry::append(&ev);
}

/// Emit a minimal event to **wire only** (SSE → REPL breadcrumbs).
pub fn emit_wire(tx: &PillarTx, pillar: Pillar, phase: &str, message: impl Into<String>) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id: None,
        agent_id: None,
        ts: Utc::now().to_rfc3339(),
        fields: serde_json::Map::new(),
    };
    wire::send(tx, ev);
}

/// Emit with structured fields to **wire only** (SSE, no JSONL). Used for
/// high-frequency or large-payload UI pushes (e.g. agent terminal results)
/// that shouldn't bloat the audit log.
#[allow(clippy::too_many_arguments)]
pub fn emit_wire_full(
    tx: &PillarTx,
    pillar: Pillar,
    phase: &str,
    message: impl Into<String>,
    agent_id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id: None,
        agent_id,
        ts: Utc::now().to_rfc3339(),
        fields,
    };
    wire::send(tx, ev);
}

/// Emit with structured fields to **telemetry only** (JSONL audit).
#[allow(clippy::too_many_arguments)]
pub fn emit_telemetry(
    pillar: Pillar,
    phase: &str,
    message: impl Into<String>,
    run_id: Option<String>,
    agent_id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id,
        agent_id,
        ts: Utc::now().to_rfc3339(),
        fields,
    };
    telemetry::append(&ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_without_subscribers_does_not_panic() {
        let tx = channel();
        // No rx — channel.send returns Err(SendError) which we swallow.
        emit(&tx, Pillar::P1Sdd, "start", "no subscribers OK");
    }

    #[test]
    fn emit_full_carries_fields() {
        let tx = channel();
        let mut rx = tx.subscribe();
        let mut fields = serde_json::Map::new();
        fields.insert("cost_usd".into(), serde_json::json!(0.012));
        emit_full(
            &tx,
            Pillar::P3Verification,
            "step",
            "judge scored",
            Some("run-1".into()),
            Some("agent-9".into()),
            fields,
        );
        let ev = rx.try_recv().expect("got event");
        assert_eq!(ev.pillar, Pillar::P3Verification);
        assert_eq!(ev.run_id.as_deref(), Some("run-1"));
        assert_eq!(
            ev.fields.get("cost_usd").and_then(serde_json::Value::as_f64),
            Some(0.012)
        );
    }

    #[test]
    fn ts_is_rfc3339() {
        let tx = channel();
        let mut rx = tx.subscribe();
        emit(&tx, Pillar::P7Cognition, "done", "ok");
        let ev = rx.try_recv().unwrap();
        // RFC3339 has 'T' separator and timezone suffix.
        assert!(ev.ts.contains('T'));
        assert!(ev.ts.ends_with('Z') || ev.ts.contains('+') || ev.ts.contains('-'));
    }
}
