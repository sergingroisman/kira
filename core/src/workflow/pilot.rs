//! WorkflowPilot — generic DAG-walking Pilot for any WorkflowDef.
//!
//! Replaces the SDD-specific `SddPilot`. Walks the phase DAG using
//! `EdgeCond` to route transitions. Terminal states (`done`, `failed`)
//! are sentinel phase ids, not enum variants.
//!
//! State lives in `WorkflowRun` (see `crate::proto::run`). The Pilot is
//! pure: it never calls LLMs, FS, or network — only inspects state and
//! returns the next `Intent`.

use std::sync::Arc;

use crate::proto::intent::{Intent, StepOutcome, StepResult};
use crate::proto::run::{WorkflowRun, PHASE_DONE, PHASE_FAILED};
use crate::workflow::def::WorkflowDef;

// Terminal phase sentinel ids.
// Any phase whose `edges` is empty is implicitly terminal.
pub const DONE: &str = PHASE_DONE;
pub const FAILED: &str = PHASE_FAILED;

/// Generic Pilot that drives any `WorkflowDef`.
pub struct WorkflowPilot {
    pub def: Arc<WorkflowDef>,
}

impl WorkflowPilot {
    pub fn new(def: Arc<WorkflowDef>) -> Self {
        Self { def }
    }

    /// Decide the next intent based on `run.current_phase_id`.
    pub async fn next_intent(&self, run: &WorkflowRun) -> Intent {
        let phase_id = &run.current_phase_id;

        // Terminal sentinels.
        if phase_id == DONE {
            return Intent::Done;
        }
        if phase_id == FAILED {
            return Intent::Halt {
                reason: run.error.clone().unwrap_or_else(|| "workflow failed".into()),
            };
        }

        // Pause if questions are pending (set externally by clarifier).
        if !run.pending_questions.is_empty() {
            return Intent::AwaitUserInput {
                questions: run.pending_questions.clone(),
            };
        }

        let Some(phase) = self.def.phase(phase_id) else {
            return Intent::Halt {
                reason: format!("unknown phase: {phase_id}"),
            };
        };

        use crate::workflow::def::PhaseKind;
        match phase.kind {
            PhaseKind::Writer => Intent::RunPhase { phase_id: phase_id.clone() },
            PhaseKind::Execute => Intent::RunExecute { phase_id: phase_id.clone() },
        }
    }

    /// Observe a step result and mutate `run` accordingly.
    ///
    /// On success: resolve the next phase via edge conditions.
    /// On failure: apply retry / escalate / halt logic (mirrors SddPilot).
    pub async fn observe(&self, run: &mut WorkflowRun, result: &StepResult) {
        if result.outcome == StepOutcome::Ok || result.outcome == StepOutcome::Pending {
            // Pending = waiting for user input, don't advance phase.
            if result.outcome == StepOutcome::Pending {
                return;
            }
            // Success: reset retry counter and resolve next phase.
            run.phase_retries.remove(&run.current_phase_id);
            self.advance(run, /*gate_ok=*/ true);
            return;
        }

        // Failure path.
        let phase_id = run.current_phase_id.clone();
        let error = result.message.clone();
        run.error = Some(error.clone());

        let attempt = {
            let entry = run.phase_retries.entry(phase_id.clone()).or_insert(0);
            *entry += 1;
            *entry
        };

        // Max 3 retries, then fail.
        if attempt < 3 {
            // Keep current_phase_id unchanged → next_intent emits same phase again.
            tracing::info!(
                run_id = %run.id,
                phase = %phase_id,
                attempt,
                "workflow pilot: retrying phase"
            );
        } else {
            tracing::warn!(
                run_id = %run.id,
                phase = %phase_id,
                attempt,
                "workflow pilot: halting after max retries"
            );
            run.current_phase_id = FAILED.into();
        }
    }

    /// Advance `run.current_phase_id` by evaluating edges of the current phase.
    ///
    /// Takes the first edge whose `EdgeCond` matches. If no edge matches,
    /// transitions to `done` (implicit terminal).
    fn advance(&self, run: &mut WorkflowRun, gate_ok: bool) {
        let phase_id = run.current_phase_id.clone();
        let Some(phase) = self.def.phase(&phase_id) else {
            run.current_phase_id = FAILED.into();
            return;
        };

        let skipped = &run.skipped_phases;
        for edge in &phase.edges {
            // SizingSkip: check if the target phase was pre-skipped.
            let sizing_skip = skipped.contains(&edge.to);
            if edge.when.matches(Some(gate_ok), sizing_skip) {
                // If the next phase was sizing-skipped, jump past it recursively.
                if sizing_skip && edge.when != crate::workflow::def::EdgeCond::SizingSkip {
                    // This edge is not a SizingSkip edge but the target was skipped —
                    // this shouldn't happen in well-formed workflows. Skip to next edge.
                    continue;
                }
                run.current_phase_id = edge.to.clone();
                // Plan-approval gate: if we're about to enter an Execute-kind
                // phase (the wave fan-out) and this run requires approval,
                // raise the pause flag. The drive loop blocks until approve_plan.
                if run.require_plan_approval
                    && self
                        .def
                        .phase(&run.current_phase_id)
                        .is_some_and(|p| p.kind == crate::workflow::def::PhaseKind::Execute)
                {
                    run.awaiting_plan_approval = true;
                }
                return;
            }
        }

        // No edges matched → workflow complete.
        run.current_phase_id = DONE.into();
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::def::{Edge, EdgeCond, PhaseDef, PhaseKind, WorkflowDef};

    fn make_def(phases: Vec<(&str, PhaseKind, Vec<(&str, EdgeCond)>)>) -> Arc<WorkflowDef> {
        Arc::new(WorkflowDef {
            id: "test".into(),
            description: String::new(),
            sizing: None,
            phases: phases
                .into_iter()
                .map(|(id, kind, edges)| PhaseDef {
                    id: id.into(),
                    prompt: format!("{id}.md"),
                    gate: None,
                    clarify: false,
                    produces: None,
                    kind,
                    edges: edges
                        .into_iter()
                        .map(|(to, when)| Edge { to: to.into(), when })
                        .collect(),
                })
                .collect(),
        })
    }

    fn run(phase_id: &str) -> WorkflowRun {
        WorkflowRun {
            current_phase_id: phase_id.into(),
            ..Default::default()
        }
    }

    fn ok_result() -> StepResult {
        StepResult {
            outcome: StepOutcome::Ok,
            message: "ok".into(),
            duration_ms: 0,
            cost_usd: 0.0,
            artifact_path: None,
            fields: serde_json::Map::new(),
        }
    }

    fn fail_result(msg: &str) -> StepResult {
        StepResult {
            outcome: StepOutcome::Failed,
            message: msg.into(),
            duration_ms: 0,
            cost_usd: 0.0,
            artifact_path: None,
            fields: serde_json::Map::new(),
        }
    }

    #[tokio::test]
    async fn linear_workflow_advances_on_success() {
        let def = make_def(vec![
            ("a", PhaseKind::Writer, vec![("b", EdgeCond::Always)]),
            ("b", PhaseKind::Execute, vec![]),
        ]);
        let pilot = WorkflowPilot::new(def);
        let mut run = run("a");

        match pilot.next_intent(&run).await {
            Intent::RunPhase { phase_id } => assert_eq!(phase_id, "a"),
            other => panic!("unexpected: {other:?}"),
        }

        pilot.observe(&mut run, &ok_result()).await;
        assert_eq!(run.current_phase_id, "b");
    }

    #[tokio::test]
    async fn execute_phase_emits_run_execute() {
        let def = make_def(vec![("exec", PhaseKind::Execute, vec![])]);
        let pilot = WorkflowPilot::new(def);
        let run = run("exec");
        match pilot.next_intent(&run).await {
            Intent::RunExecute { phase_id } => assert_eq!(phase_id, "exec"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn done_emits_done_intent() {
        let def = make_def(vec![("a", PhaseKind::Writer, vec![])]);
        let pilot = WorkflowPilot::new(def);
        let run = run(DONE);
        assert!(matches!(pilot.next_intent(&run).await, Intent::Done));
    }

    #[tokio::test]
    async fn failed_emits_halt() {
        let def = make_def(vec![("a", PhaseKind::Writer, vec![])]);
        let pilot = WorkflowPilot::new(def);
        let mut run = run(FAILED);
        run.error = Some("boom".into());
        match pilot.next_intent(&run).await {
            Intent::Halt { reason } => assert_eq!(reason, "boom"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_edges_transitions_to_done() {
        let def = make_def(vec![("only", PhaseKind::Writer, vec![])]);
        let pilot = WorkflowPilot::new(def);
        let mut run = run("only");
        pilot.observe(&mut run, &ok_result()).await;
        assert_eq!(run.current_phase_id, DONE);
    }

    #[tokio::test]
    async fn retry_keeps_phase_on_failure() {
        let def = make_def(vec![
            ("a", PhaseKind::Writer, vec![("b", EdgeCond::Always)]),
            ("b", PhaseKind::Writer, vec![]),
        ]);
        let pilot = WorkflowPilot::new(def);
        let mut run = run("a");

        pilot.observe(&mut run, &fail_result("err")).await;
        assert_eq!(run.current_phase_id, "a", "attempt 1 → stay");

        pilot.observe(&mut run, &fail_result("err")).await;
        assert_eq!(run.current_phase_id, "a", "attempt 2 → stay");

        pilot.observe(&mut run, &fail_result("err")).await;
        assert_eq!(run.current_phase_id, FAILED, "attempt 3 → halt");
    }

    #[tokio::test]
    async fn success_after_retry_clears_counter() {
        let def = make_def(vec![
            ("a", PhaseKind::Writer, vec![("b", EdgeCond::Always)]),
            ("b", PhaseKind::Writer, vec![]),
        ]);
        let pilot = WorkflowPilot::new(def);
        let mut run = run("a");

        pilot.observe(&mut run, &fail_result("err")).await;
        assert_eq!(*run.phase_retries.get("a").unwrap_or(&0), 1);

        pilot.observe(&mut run, &ok_result()).await;
        assert_eq!(*run.phase_retries.get("a").unwrap_or(&0), 0, "cleared on success");
        assert_eq!(run.current_phase_id, "b");
    }
}
