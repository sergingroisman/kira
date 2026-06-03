//! Guardrail, Memory, and Queue Pillar Event emitters.
//!
//! Convenience functions that emit structured events for the 3 new pillar
//! event types: GuardrailTriggered, MemoryInjected, QueueDepthChanged.

use crate::observability::pillar::{self, PillarTx};
use crate::proto::pillar::Pillar;

/// Emit a `GuardrailTriggered` event (P5).
///
/// Called when any guardrail level blocks or modifies content.
pub fn emit_guardrail_triggered(
    tx: &PillarTx,
    level: &str,
    reason: &str,
    agent_id: Option<String>,
    tool_name: Option<&str>,
    verdict: Option<&str>,
) {
    let mut fields = serde_json::Map::new();
    fields.insert("level".into(), serde_json::json!(level));
    fields.insert("reason".into(), serde_json::json!(reason));
    if let Some(tool) = tool_name {
        fields.insert("tool_name".into(), serde_json::json!(tool));
    }
    if let Some(v) = verdict {
        fields.insert("verdict".into(), serde_json::json!(v));
    }

    pillar::emit_full(
        tx,
        Pillar::P5Guardrails,
        "guardrail_triggered",
        format!("[{level}] {reason}"),
        None,
        agent_id,
        fields,
    );
}

/// Emit a `MemoryInjected` event (P7).
///
/// Called when memory fragments are injected into a prompt or extracted from a run.
pub fn emit_memory_injected(
    tx: &PillarTx,
    fragment_count: u32,
    run_id: Option<String>,
    agent_id: Option<String>,
    phase: &str, // "retrieval" or "extraction"
) {
    let mut fields = serde_json::Map::new();
    fields.insert("fragment_count".into(), serde_json::json!(fragment_count));
    fields.insert("phase".into(), serde_json::json!(phase));

    pillar::emit_full(
        tx,
        Pillar::P7Cognition,
        "memory_injected",
        format!("memory {phase}: {fragment_count} fragments"),
        run_id,
        agent_id,
        fields,
    );
}

/// Emit a `QueueDepthChanged` event (P6).
///
/// Called when the number of queued messages changes.
pub fn emit_queue_depth_changed(tx: &PillarTx, depth: usize) {
    let mut fields = serde_json::Map::new();
    fields.insert("depth".into(), serde_json::json!(depth));

    pillar::emit_full(
        tx,
        Pillar::P6Resilience,
        "queue_depth_changed",
        format!("queue depth: {depth}"),
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
    fn emit_guardrail_triggered_does_not_panic() {
        let tx = channel();
        emit_guardrail_triggered(
            &tx,
            "input",
            "prompt_injection_detected",
            Some("agent-123".to_string()),
            None,
            None,
        );
    }

    #[test]
    fn emit_guardrail_triggered_with_tool() {
        let tx = channel();
        emit_guardrail_triggered(
            &tx,
            "tool",
            "critical_risk_tool_blocked",
            Some("agent-456".to_string()),
            Some("bash"),
            None,
        );
    }

    #[test]
    fn emit_guardrail_triggered_output_with_verdict() {
        let tx = channel();
        emit_guardrail_triggered(
            &tx,
            "output",
            "harmful content",
            Some("agent-789".to_string()),
            None,
            Some("block"),
        );
    }

    #[test]
    fn emit_memory_injected_does_not_panic() {
        let tx = channel();
        emit_memory_injected(
            &tx,
            5,
            Some("run-abc".to_string()),
            Some("agent-def".to_string()),
            "retrieval",
        );
    }

    #[test]
    fn emit_queue_depth_changed_does_not_panic() {
        let tx = channel();
        emit_queue_depth_changed(&tx, 7);
    }

    #[test]
    fn emit_without_subscribers_is_safe() {
        let tx = channel();
        emit_guardrail_triggered(&tx, "input", "test", None, None, None);
        emit_memory_injected(&tx, 0, None, None, "extraction");
        emit_queue_depth_changed(&tx, 0);
        // Should not panic
    }
}
