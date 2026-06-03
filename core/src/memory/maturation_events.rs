//! Memory Maturation Pillar Event emitters.
//!
//! Convenience functions that emit structured events for all maturation
//! operations. All events use `Pillar::P7Cognition` and are persisted to
//! JSONL audit with `memory_maturation_` prefix.
//!
//! ## Event Types
//!
//! | Event Name                                    | Trigger                          | Phase   |
//! |-----------------------------------------------|----------------------------------|---------|
//! | `memory_maturation_consolidation_started`     | Consolidation pass begins        | `start` |
//! | `memory_maturation_consolidation_completed`   | Consolidation pass succeeds      | `done`  |
//! | `memory_maturation_consolidation_failed`      | Consolidation task fails/panics  | `fail`  |
//! | `memory_maturation_handoff_generated`         | Handoff summary created          | `done`  |
//! | `memory_maturation_handoff_failed`            | Handoff task fails/panics        | `fail`  |
//! | `memory_maturation_promoted_to_procedural`    | Fragment promoted to procedural  | `done`  |
//!
//! ## Audit Write Failures
//!
//! All events are persisted via `pillar::emit_full()` which calls
//! `telemetry::append()`. If the JSONL audit file cannot be written (I/O
//! error), the failure is logged at debug level and the maturation operation
//! continues without aborting. This is by design — observability must never
//! block the critical path.

use uuid::Uuid;

use crate::observability::pillar::{self, PillarTx};
use crate::proto::pillar::Pillar;

// ─── Consolidation Events ────────────────────────────────────────────────────

/// Emit a `memory_maturation_consolidation_started` event.
///
/// Called when a consolidation pass begins (working→episodic or episodic→semantic).
pub fn emit_consolidation_started(
    tx: &PillarTx,
    source_tier: &str,
    target_tier: &str,
    fragment_count: usize,
    session_id: Option<Uuid>,
) {
    let mut fields = serde_json::Map::new();
    fields.insert("source_tier".into(), serde_json::json!(source_tier));
    fields.insert("target_tier".into(), serde_json::json!(target_tier));
    fields.insert("fragment_count".into(), serde_json::json!(fragment_count));
    if let Some(sid) = session_id {
        fields.insert("session_id".into(), serde_json::json!(sid.to_string()));
    }

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "start",
        "memory_maturation_consolidation_started",
        None,
        None,
        fields,
    );
}

/// Emit a `memory_maturation_consolidation_completed` event.
///
/// Called when a consolidation pass succeeds and produces a new fragment.
pub fn emit_consolidation_completed(
    tx: &PillarTx,
    source_tier: &str,
    target_tier: &str,
    fragment_id: Uuid,
    session_id: Option<Uuid>,
) {
    let mut fields = serde_json::Map::new();
    fields.insert("source_tier".into(), serde_json::json!(source_tier));
    fields.insert("target_tier".into(), serde_json::json!(target_tier));
    fields.insert("fragment_id".into(), serde_json::json!(fragment_id.to_string()));
    if let Some(sid) = session_id {
        fields.insert("session_id".into(), serde_json::json!(sid.to_string()));
    }

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "done",
        "memory_maturation_consolidation_completed",
        None,
        None,
        fields,
    );
}

/// Emit a `memory_maturation_consolidation_failed` event.
///
/// Called when a consolidation task fails, times out, or panics.
pub fn emit_consolidation_failed(
    tx: &PillarTx,
    source_tier: &str,
    target_tier: &str,
    error_msg: &str,
) {
    let mut fields = serde_json::Map::new();
    fields.insert("source_tier".into(), serde_json::json!(source_tier));
    fields.insert("target_tier".into(), serde_json::json!(target_tier));
    fields.insert("error".into(), serde_json::json!(error_msg));

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "fail",
        "memory_maturation_consolidation_failed",
        None,
        None,
        fields,
    );
}

// ─── Handoff Events ──────────────────────────────────────────────────────────

/// Emit a `memory_maturation_handoff_generated` event.
///
/// Called when a handoff summary is successfully created and persisted.
pub fn emit_handoff_generated(tx: &PillarTx, session_id: Uuid, summary_length_chars: usize) {
    let mut fields = serde_json::Map::new();
    fields.insert("session_id".into(), serde_json::json!(session_id.to_string()));
    fields.insert(
        "summary_length_chars".into(),
        serde_json::json!(summary_length_chars),
    );

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "done",
        "memory_maturation_handoff_generated",
        None,
        None,
        fields,
    );
}

/// Emit a `memory_maturation_handoff_failed` event.
///
/// Called when a handoff generation task panics or terminates unexpectedly
/// (caught via `JoinHandle`). This is distinct from normal handoff failures
/// (LLM timeout, malformed JSON) which are handled gracefully within
/// `generate_handoff` and do not emit a failure event.
pub fn emit_handoff_failed(tx: &PillarTx, session_id: Uuid, error_msg: &str) {
    let mut fields = serde_json::Map::new();
    fields.insert("session_id".into(), serde_json::json!(session_id.to_string()));
    fields.insert("error".into(), serde_json::json!(error_msg));

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "fail",
        "memory_maturation_handoff_failed",
        None,
        None,
        fields,
    );
}

// ─── Promotion Events ────────────────────────────────────────────────────────

/// Emit a `memory_maturation_promoted_to_procedural` event.
///
/// Called when a semantic-tier fragment is promoted to procedural tier.
pub fn emit_promoted_to_procedural(tx: &PillarTx, fragment_id: Uuid, content_preview: &str) {
    let mut fields = serde_json::Map::new();
    fields.insert("fragment_id".into(), serde_json::json!(fragment_id.to_string()));
    fields.insert("content_preview".into(), serde_json::json!(content_preview));

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "done",
        "memory_maturation_promoted_to_procedural",
        None,
        None,
        fields,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::pillar::channel;

    #[test]
    fn emit_consolidation_started_does_not_panic() {
        let tx = channel();
        emit_consolidation_started(&tx, "working", "episodic", 5, Some(Uuid::now_v7()));
    }

    #[test]
    fn emit_consolidation_started_without_session() {
        let tx = channel();
        emit_consolidation_started(&tx, "episodic", "semantic", 8, None);
    }

    #[test]
    fn emit_consolidation_completed_does_not_panic() {
        let tx = channel();
        emit_consolidation_completed(
            &tx,
            "working",
            "episodic",
            Uuid::now_v7(),
            Some(Uuid::now_v7()),
        );
    }

    #[test]
    fn emit_consolidation_failed_does_not_panic() {
        let tx = channel();
        emit_consolidation_failed(&tx, "working", "episodic", "LLM timeout");
    }

    #[test]
    fn emit_handoff_generated_does_not_panic() {
        let tx = channel();
        emit_handoff_generated(&tx, Uuid::now_v7(), 1500);
    }

    #[test]
    fn emit_handoff_failed_does_not_panic() {
        let tx = channel();
        emit_handoff_failed(&tx, Uuid::now_v7(), "task panicked: index out of bounds");
    }

    #[test]
    fn emit_promoted_to_procedural_does_not_panic() {
        let tx = channel();
        emit_promoted_to_procedural(&tx, Uuid::now_v7(), "Always use parameterized queries for SQL");
    }

    #[test]
    fn emit_without_subscribers_is_safe() {
        let tx = channel();
        // No subscribers — all emits should succeed silently.
        emit_consolidation_started(&tx, "working", "episodic", 3, None);
        emit_consolidation_completed(&tx, "working", "episodic", Uuid::now_v7(), None);
        emit_consolidation_failed(&tx, "episodic", "semantic", "circuit breaker open");
        emit_handoff_generated(&tx, Uuid::now_v7(), 800);
        emit_handoff_failed(&tx, Uuid::now_v7(), "panic in handoff task");
        emit_promoted_to_procedural(&tx, Uuid::now_v7(), "Use JWT for auth");
    }

    #[test]
    fn emit_handoff_failed_carries_fields() {
        let tx = channel();
        let mut rx = tx.subscribe();
        let session_id = Uuid::now_v7();
        emit_handoff_failed(&tx, session_id, "task panicked");
        let ev = rx.try_recv().expect("got event");
        assert_eq!(ev.pillar, Pillar::P7Cognition);
        assert_eq!(ev.phase, "fail");
        assert_eq!(ev.message, "memory_maturation_handoff_failed");
        assert_eq!(
            ev.fields.get("session_id").and_then(|v| v.as_str()),
            Some(session_id.to_string()).as_deref()
        );
        assert_eq!(
            ev.fields.get("error").and_then(|v| v.as_str()),
            Some("task panicked")
        );
    }

    #[test]
    fn emit_consolidation_started_carries_fields() {
        let tx = channel();
        let mut rx = tx.subscribe();
        let session_id = Uuid::now_v7();
        emit_consolidation_started(&tx, "working", "episodic", 7, Some(session_id));
        let ev = rx.try_recv().expect("got event");
        assert_eq!(ev.pillar, Pillar::P7Cognition);
        assert_eq!(ev.phase, "start");
        assert_eq!(ev.message, "memory_maturation_consolidation_started");
        assert_eq!(
            ev.fields.get("source_tier").and_then(|v| v.as_str()),
            Some("working")
        );
        assert_eq!(
            ev.fields.get("target_tier").and_then(|v| v.as_str()),
            Some("episodic")
        );
        assert_eq!(
            ev.fields.get("fragment_count").and_then(|v| v.as_u64()),
            Some(7)
        );
        assert_eq!(
            ev.fields.get("session_id").and_then(|v| v.as_str()),
            Some(session_id.to_string()).as_deref()
        );
    }

    #[test]
    fn emit_promoted_to_procedural_carries_fields() {
        let tx = channel();
        let mut rx = tx.subscribe();
        let frag_id = Uuid::now_v7();
        emit_promoted_to_procedural(&tx, frag_id, "Always validate input");
        let ev = rx.try_recv().expect("got event");
        assert_eq!(ev.pillar, Pillar::P7Cognition);
        assert_eq!(ev.phase, "done");
        assert_eq!(ev.message, "memory_maturation_promoted_to_procedural");
        assert_eq!(
            ev.fields.get("fragment_id").and_then(|v| v.as_str()),
            Some(frag_id.to_string()).as_deref()
        );
        assert_eq!(
            ev.fields.get("content_preview").and_then(|v| v.as_str()),
            Some("Always validate input")
        );
    }

    #[test]
    fn all_events_use_p7_cognition() {
        let tx = channel();
        let mut rx = tx.subscribe();

        emit_consolidation_started(&tx, "working", "episodic", 1, None);
        emit_consolidation_completed(&tx, "working", "episodic", Uuid::now_v7(), None);
        emit_consolidation_failed(&tx, "working", "episodic", "err");
        emit_handoff_generated(&tx, Uuid::now_v7(), 100);
        emit_handoff_failed(&tx, Uuid::now_v7(), "err");
        emit_promoted_to_procedural(&tx, Uuid::now_v7(), "preview");

        for _ in 0..6 {
            let ev = rx.try_recv().expect("got event");
            assert_eq!(ev.pillar, Pillar::P7Cognition, "all maturation events must use P7Cognition");
        }
    }

    #[test]
    fn all_events_have_memory_maturation_prefix() {
        let tx = channel();
        let mut rx = tx.subscribe();

        emit_consolidation_started(&tx, "working", "episodic", 1, None);
        emit_consolidation_completed(&tx, "working", "episodic", Uuid::now_v7(), None);
        emit_consolidation_failed(&tx, "working", "episodic", "err");
        emit_handoff_generated(&tx, Uuid::now_v7(), 100);
        emit_handoff_failed(&tx, Uuid::now_v7(), "err");
        emit_promoted_to_procedural(&tx, Uuid::now_v7(), "preview");

        for _ in 0..6 {
            let ev = rx.try_recv().expect("got event");
            assert!(
                ev.message.starts_with("memory_maturation_"),
                "event message '{}' must start with 'memory_maturation_' prefix",
                ev.message
            );
        }
    }

    #[test]
    fn all_events_have_rfc3339_timestamp() {
        let tx = channel();
        let mut rx = tx.subscribe();

        emit_consolidation_started(&tx, "working", "episodic", 1, None);
        let ev = rx.try_recv().expect("got event");
        // RFC3339 has 'T' separator and timezone suffix.
        assert!(ev.ts.contains('T'), "timestamp must be RFC3339");
        assert!(
            ev.ts.ends_with('Z') || ev.ts.contains('+') || ev.ts.contains('-'),
            "timestamp must have timezone"
        );
    }
}
