//! Pilot (P2 — Coordenação alta) — decides next `Intent` from run state.
//!
//! Pure decision layer: NEVER calls LLMs / FS / network directly.
//! Receives a state snapshot, returns the next `Intent`. Orquestrador
//! executes; Pilot observes the result and decides again.

use crate::proto::intent::{Intent, StepOutcome, StepResult};
use crate::proto::sdd::{SddPhase, SddRun};

/// Pilot trait — a state machine reduced to two functions.
pub trait Pilot: Send + Sync {
    /// Decide next intent given current state.
    async fn next_intent(&self, run: &SddRun) -> Intent;

    /// Observe a step result. Mutating reactions (e.g. escalation flag)
    /// live here. Default no-op suits stateless pilots.
    async fn observe(&self, _run: &mut SddRun, _result: &StepResult) {}
}

/// SDD-specific pilot. Walks SPECIFY → DESIGN → TASKS → EXECUTE.
/// Handles auto-sizing (skips SPECIFY+DESIGN when `sizing == SkipSpecifyDesign`).
pub struct SddPilot;

impl Pilot for SddPilot {
    async fn next_intent(&self, run: &SddRun) -> Intent {
        match run.current_phase {
            SddPhase::Specify => Intent::RunSddPhase { phase: SddPhase::Specify },
            SddPhase::Design => Intent::RunSddPhase { phase: SddPhase::Design },
            SddPhase::Tasks => Intent::RunSddPhase { phase: SddPhase::Tasks },
            SddPhase::Execute => Intent::RunSddExecute,
            SddPhase::Done => Intent::Done,
            SddPhase::Failed => Intent::Halt {
                reason: run
                    .error
                    .clone()
                    .unwrap_or_else(|| "SDD entered Failed without error".into()),
            },
        }
    }

    async fn observe(&self, run: &mut SddRun, result: &StepResult) {
        if result.outcome == StepOutcome::Failed {
            run.error = Some(result.message.clone());
        }
        // Halt-on-fail policy v0: any failed step terminates.
        // v0.5+: classify failure → RetryWith / EscalateTier per FailureClass.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::proto::sdd::SddKind;

    fn sample_run(phase: SddPhase) -> SddRun {
        SddRun {
            id: "test".into(),
            goal: "g".into(),
            kind: SddKind::Feature,
            sizing: crate::proto::sdd::SizingVerdict::Full,
            current_phase: phase,
            phases_run: vec![],
            phases_skipped: vec![],
            spec_path: None,
            design_path: None,
            tasks_path: None,
            execution_log_path: None,
            worktree_id: "wt".into(),
            worktree_path: "/tmp/wt".into(),
            phase_verdicts: vec![],
            usd_spent: 0.0,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            finished_at: None,
            error: None,
        }
    }

    #[tokio::test]
    async fn pilot_specify_emits_run_phase() {
        let p = SddPilot;
        let run = sample_run(SddPhase::Specify);
        match p.next_intent(&run).await {
            Intent::RunSddPhase { phase } => assert_eq!(phase, SddPhase::Specify),
            other => panic!("unexpected intent: {other:?}"),
        }
    }

    #[tokio::test]
    async fn pilot_execute_emits_run_execute() {
        let p = SddPilot;
        let run = sample_run(SddPhase::Execute);
        assert!(matches!(p.next_intent(&run).await, Intent::RunSddExecute));
    }

    #[tokio::test]
    async fn pilot_done_emits_done() {
        let p = SddPilot;
        let run = sample_run(SddPhase::Done);
        assert!(matches!(p.next_intent(&run).await, Intent::Done));
    }

    #[tokio::test]
    async fn pilot_failed_emits_halt_with_error() {
        let p = SddPilot;
        let mut run = sample_run(SddPhase::Failed);
        run.error = Some("boom".into());
        match p.next_intent(&run).await {
            Intent::Halt { reason } => assert_eq!(reason, "boom"),
            other => panic!("unexpected intent: {other:?}"),
        }
    }

    #[tokio::test]
    async fn observe_failed_sets_error() {
        let p = SddPilot;
        let mut run = sample_run(SddPhase::Tasks);
        let result = StepResult {
            outcome: StepOutcome::Failed,
            message: "phase agent timeout".into(),
            duration_ms: 100,
            cost_usd: 0.0,
            artifact_path: None,
            fields: serde_json::Map::new(),
        };
        p.observe(&mut run, &result).await;
        assert_eq!(run.error.as_deref(), Some("phase agent timeout"));
    }
}
