//! Classification function — maps (pillar, phase) pairs to conceptual layers.
//!
//! Phase-specific rules take precedence over pillar-only rules.
//! Returns `None` for events that cannot be classified (ignored by the Layers View).

use super::state::ConceptualLayer;

// ---------------------------------------------------------------------------
// Pillar enum (local mirror of core::proto::pillar::Pillar)
// ---------------------------------------------------------------------------

/// KIRA's 7 internal pillars. Mirrors `core::proto::pillar::Pillar` without
/// requiring a crate dependency on `kira-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pillar {
    /// P1 — Spec-Driven Execution.
    P1Sdd,
    /// P2 — Coordination (Pilot + Orchestrator).
    P2Coordination,
    /// P3 — Verification & Judge.
    P3Verification,
    /// P4 — Completion Gates.
    P4Gates,
    /// P5 — Guardrails.
    P5Guardrails,
    /// P6 — Resilience.
    P6Resilience,
    /// P7 — Observability + Cognition.
    P7Cognition,
}

impl Pillar {
    /// Parse the snake_case wire form emitted by `core::proto::pillar::Pillar`
    /// (serde `rename_all = "snake_case"`). Returns `None` for unknown values.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "p1_sdd" => Self::P1Sdd,
            "p2_coordination" => Self::P2Coordination,
            "p3_verification" => Self::P3Verification,
            "p4_gates" => Self::P4Gates,
            "p5_guardrails" => Self::P5Guardrails,
            "p6_resilience" => Self::P6Resilience,
            "p7_cognition" => Self::P7Cognition,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a `(pillar, phase)` pair into a conceptual layer.
///
/// Returns `None` if the event cannot be classified (should be ignored).
/// Phase-specific rules take precedence over pillar-only rules.
pub fn classify(pillar: Pillar, phase: &str) -> Option<ConceptualLayer> {
    let phase_lower = phase.to_ascii_lowercase();

    // Phase-specific rules (highest precedence)
    match pillar {
        Pillar::P7Cognition => {
            if phase_lower.contains("memory") || phase_lower.contains("rag") {
                return Some(ConceptualLayer::Perception);
            }
            if phase_lower.contains("context") || phase_lower.contains("file_read") {
                return Some(ConceptualLayer::Perception);
            }
            // P7Cognition without matching phase keywords → unclassified
            return None;
        }
        Pillar::P2Coordination
            if phase_lower.contains("spawn") || phase_lower.contains("wave") =>
        {
            return Some(ConceptualLayer::Execution);
        }
        Pillar::P6Resilience => {
            if phase_lower.contains("tool") || phase_lower.contains("execute") {
                return Some(ConceptualLayer::Execution);
            }
            // P6Resilience without matching phase keywords → unclassified
            return None;
        }
        _ => {}
    }

    // Pillar-only rules (lower precedence)
    match pillar {
        Pillar::P1Sdd | Pillar::P2Coordination | Pillar::P3Verification => {
            Some(ConceptualLayer::Reasoning)
        }
        Pillar::P5Guardrails | Pillar::P4Gates => Some(ConceptualLayer::Security),
        _ => None,
    }
}
