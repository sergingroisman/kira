//! Pilot (P2 — Coordenação alta) — decides next `Intent` from run state.
//!
//! Pure decision layer: NEVER calls LLMs / FS / network directly.
//! Receives a state snapshot, returns the next `Intent`. Orquestrador
//! executes; Pilot observes the result and decides again.

use tracing::{info, warn};

use crate::proto::intent::{Intent, StepOutcome, StepResult};
use crate::proto::sdd::{SddPhase, SddRun};
use crate::router::fallback;

/// Pilot trait — a state machine reduced to two functions.
pub trait Pilot: Send + Sync {
    /// Decide next intent given current state.
    async fn next_intent(&self, run: &SddRun) -> Intent;

    /// Observe a step result. Mutating reactions (escalation, retry state)
    /// live here. Default no-op suits stateless pilots.
    async fn observe(&self, _run: &mut SddRun, _result: &StepResult) {}
}

/// SDD-specific pilot. Walks SPECIFY → DESIGN → TASKS → EXECUTE.
/// Failure policy: FallbackChain per FailureClass (retry/escalate/halt).
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
        if result.outcome != StepOutcome::Failed {
            // Success: reset retry counter for this phase.
            let phase_key = format!("{:?}", run.current_phase);
            run.phase_retries.remove(&phase_key);
            return;
        }

        let phase = run.current_phase;
        let error = result.message.clone();
        run.error = Some(error.clone());

        // FallbackChain: classify failure and decide retry/escalate/halt.
        let phase_key = format!("{phase:?}");
        let attempt = {
            let entry = run.phase_retries.entry(phase_key).or_insert(0);
            *entry += 1;
            *entry
        };

        let decision = fallback::decide(phase, &error, attempt);

        match &decision.intent {
            Intent::RunSddPhase { .. } => {
                // Retry: keep current_phase unchanged so next_intent returns the same phase.
                info!(
                    run_id = %run.id,
                    phase = ?phase,
                    attempt,
                    message = %decision.message,
                    "pilot: retrying phase"
                );
            }
            Intent::EscalateTier { tier, reason } => {
                warn!(
                    run_id = %run.id,
                    phase = ?phase,
                    attempt,
                    ?tier,
                    reason = %reason,
                    "pilot: escalating after retries exhausted"
                );
                // Escalation is a no-op in v0.5 (orchestrator returns Pending).
                // Mark failed so the drive loop terminates cleanly.
                run.current_phase = SddPhase::Failed;
                run.error = Some(reason.clone());
            }
            Intent::Halt { reason } => {
                warn!(
                    run_id = %run.id,
                    phase = ?phase,
                    attempt,
                    reason = %reason,
                    "pilot: halting"
                );
                run.current_phase = SddPhase::Failed;
                run.error = Some(reason.clone());
            }
            _ => {
                run.current_phase = SddPhase::Failed;
            }
        }
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
            phase_retries: Default::default(),
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
    async fn observe_budget_exceeded_halts_immediately() {
        let p = SddPilot;
        let mut run = sample_run(SddPhase::Tasks);
        p.observe(&mut run, &fail("budget cap reached ($1.00)")).await;
        assert_eq!(run.current_phase, SddPhase::Failed);
    }

    #[tokio::test]
    async fn observe_agent_mistake_retries_then_escalates() {
        let p = SddPilot;
        let mut run = sample_run(SddPhase::Specify);
        // max_attempts=3 → allows_retry for attempts 1, 2; escalates at 3
        for i in 1..=2 {
            p.observe(&mut run, &fail("output failed gate check")).await;
            assert_eq!(run.current_phase, SddPhase::Specify, "attempt {i}: expected Specify");
        }
        // attempt 3 → allows_retry(3) = 3 < 3 = false → escalate → Failed
        p.observe(&mut run, &fail("output failed gate check")).await;
        assert_eq!(run.current_phase, SddPhase::Failed, "attempt 3: expected Failed");
    }

    #[tokio::test]
    async fn observe_success_clears_retry_counter() {
        let p = SddPilot;
        let mut run = sample_run(SddPhase::Specify);
        p.observe(&mut run, &fail("connection timeout")).await;
        assert_eq!(*run.phase_retries.get("Specify").unwrap_or(&0), 1);
        p.observe(&mut run, &ok()).await;
        assert_eq!(*run.phase_retries.get("Specify").unwrap_or(&0), 0);
    }

    fn fail(msg: &str) -> StepResult {
        StepResult {
            outcome: StepOutcome::Failed,
            message: msg.into(),
            duration_ms: 100,
            cost_usd: 0.0,
            artifact_path: None,
            fields: serde_json::Map::new(),
        }
    }

    fn ok() -> StepResult {
        StepResult {
            outcome: StepOutcome::Ok,
            message: "ok".into(),
            duration_ms: 100,
            cost_usd: 0.0,
            artifact_path: None,
            fields: serde_json::Map::new(),
        }
    }
}
