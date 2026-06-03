//! WorkflowDef — declarative methodology (TOML → DAG of Phases).
//!
//! A Workflow encodes a development methodology as data, not code.
//! Phases form a DAG; edges carry conditions that route based on gate
//! outcome or sizing. The narrow waist is always `tasks.json`.
//!
//! ```toml
//! # core/workflows/sdd.toml
//! id = "sdd"
//! sizing = "loc_heuristic"
//!
//! [[phase]]
//! id       = "specify"
//! prompt   = "sdd/specify.md"
//! gate     = "judge"
//! clarify  = true
//! produces = "spec.md"
//! edges    = [{ to = "design" }]
//! ```

use serde::{Deserialize, Serialize};

// ── Top-level definition ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    pub id: String,
    #[serde(default)]
    pub description: String,
    /// Named sizing service (e.g. `"loc_heuristic"`). `None` = run all phases.
    #[serde(default)]
    pub sizing: Option<String>,
    #[serde(rename = "phase")]
    pub phases: Vec<PhaseDef>,
}

impl WorkflowDef {
    /// Find a phase by id. `None` if not present.
    pub fn phase(&self, id: &str) -> Option<&PhaseDef> {
        self.phases.iter().find(|p| p.id == id)
    }

    /// The entry-point phase (first in the list).
    pub fn entry(&self) -> Option<&PhaseDef> {
        self.phases.first()
    }
}

// ── Phase ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDef {
    pub id: String,
    /// Prompt key resolved by the Orchestrator. Relative path under
    /// `core/src/agents/prompts/`, e.g. `"sdd/specify.md"`.
    pub prompt: String,
    /// Named gate service (e.g. `"judge"`, `"detective"`). `None` = no gate.
    #[serde(default)]
    pub gate: Option<String>,
    /// Whether this phase may pause to ask the user clarifying questions.
    #[serde(default)]
    pub clarify: bool,
    /// Artifact name this phase produces (e.g. `"spec.md"`, `"tasks.json"`).
    /// Stored in `WorkflowRun.artifacts[produces]` after a successful phase.
    #[serde(default)]
    pub produces: Option<String>,
    /// Execution strategy for this phase.
    #[serde(default)]
    pub kind: PhaseKind,
    /// DAG edges — ordered; first matching condition is taken.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

// ── Phase kind ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    /// Spawn a Writer agent to produce an artifact file.
    #[default]
    Writer,
    /// Narrow-waist fan-out: parse tasks.json → Waves parallel execution.
    Execute,
}

// ── Edge / condition ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Target phase id.
    pub to: String,
    /// Condition under which this edge is taken.
    #[serde(default)]
    pub when: EdgeCond,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EdgeCond {
    /// Always follow this edge (unconditional transition).
    #[default]
    Always,
    /// Follow only if the gate produced a `pass` verdict.
    GatePassed,
    /// Follow only if the gate produced a `fail` verdict (→ remediation).
    GateFailed,
    /// Follow only if the sizing service flagged this phase as skippable.
    SizingSkip,
}

impl EdgeCond {
    /// Evaluate whether this condition holds given the gate outcome string
    /// (`"pass"` / `"fail"` / `None`) and a sizing-skip flag.
    pub fn matches(&self, gate_ok: Option<bool>, sizing_skip: bool) -> bool {
        match self {
            Self::Always => true,
            Self::GatePassed => gate_ok.unwrap_or(true),
            Self::GateFailed => !gate_ok.unwrap_or(true),
            Self::SizingSkip => sizing_skip,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_cond_always_matches_everything() {
        assert!(EdgeCond::Always.matches(Some(true), false));
        assert!(EdgeCond::Always.matches(Some(false), false));
        assert!(EdgeCond::Always.matches(None, false));
    }

    #[test]
    fn edge_cond_gate_passed() {
        assert!(EdgeCond::GatePassed.matches(Some(true), false));
        assert!(!EdgeCond::GatePassed.matches(Some(false), false));
        assert!(EdgeCond::GatePassed.matches(None, false)); // no gate = pass
    }

    #[test]
    fn edge_cond_gate_failed() {
        assert!(!EdgeCond::GateFailed.matches(Some(true), false));
        assert!(EdgeCond::GateFailed.matches(Some(false), false));
    }

    #[test]
    fn edge_cond_sizing_skip() {
        assert!(EdgeCond::SizingSkip.matches(None, true));
        assert!(!EdgeCond::SizingSkip.matches(None, false));
    }

    #[test]
    fn workflow_def_phase_lookup() {
        let def = WorkflowDef {
            id: "test".into(),
            description: String::new(),
            sizing: None,
            phases: vec![
                PhaseDef {
                    id: "a".into(),
                    prompt: "a.md".into(),
                    gate: None, clarify: false, produces: None,
                    kind: PhaseKind::Writer,
                    edges: vec![Edge { to: "b".into(), when: EdgeCond::Always }],
                },
                PhaseDef {
                    id: "b".into(),
                    prompt: "b.md".into(),
                    gate: None, clarify: false, produces: None,
                    kind: PhaseKind::Execute,
                    edges: vec![],
                },
            ],
        };
        assert_eq!(def.entry().unwrap().id, "a");
        assert_eq!(def.phase("b").unwrap().kind, PhaseKind::Execute);
        assert!(def.phase("missing").is_none());
    }
}
