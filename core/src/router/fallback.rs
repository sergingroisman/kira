//! `FallbackChain` — maps `FailureClass` to next `Intent` (P6 Resiliência).
//!
//! Stateless decision function: given failure class, phase, and attempt count,
//! returns the next Intent. Pilot/SddRun own the attempt state.
//!
//! Chain logic per class:
//! | Class           | Retry? | Exhaustion          |
//! |-----------------|--------|---------------------|
//! | AgentMistake    | ≤3     | EscalateTier(Heavy) |
//! | InfraTransient  | ≤4     | Halt (breaker owns) |
//! | InfraPermanent  | ≤1     | EscalateTier(Std)   |
//! | BudgetExceeded  | none   | Halt                |
//! | Stall           | none   | Halt                |
//! | Cancelled       | none   | Halt                |

use crate::proto::intent::{Intent, TierHint};
use crate::proto::sdd::SddPhase;
use crate::router::failure::{classify, FailureClass, RetryPolicy};

/// Decision produced by `decide()`.
pub struct FallbackDecision {
    /// The Intent the Pilot should return next.
    pub intent: Intent,
    /// Human-readable rationale for logging.
    pub message: String,
}

/// Decide the next Intent given a failed phase.
///
/// `attempt` is 1-indexed: 1 = first failure, 2 = second failure, etc.
/// Caller must increment and store the attempt count before calling.
pub fn decide(failed_phase: SddPhase, error: &str, attempt: u32) -> FallbackDecision {
    let class = classify(error);
    let policy = RetryPolicy::for_class(class);

    // Terminal classes — no retry regardless of attempt.
    match class {
        FailureClass::BudgetExceeded => {
            return FallbackDecision {
                intent: Intent::Halt { reason: error.to_string() },
                message: "budget exceeded — halting".into(),
            };
        }
        FailureClass::Stall => {
            return FallbackDecision {
                intent: Intent::Halt { reason: error.to_string() },
                message: "stall — halting (lesson already fired by agent loop)".into(),
            };
        }
        FailureClass::Cancelled => {
            return FallbackDecision {
                intent: Intent::Halt { reason: "cancelled".into() },
                message: "cancelled".into(),
            };
        }
        _ => {}
    }

    // Retryable classes.
    if policy.allows_retry(attempt) {
        return FallbackDecision {
            intent: Intent::RunSddPhase { phase: failed_phase },
            message: format!(
                "{class:?}: retry {attempt}/{max} for phase {failed_phase:?}",
                max = policy.max_attempts,
            ),
        };
    }

    // Retries exhausted.
    if policy.escalate_on_exhaustion {
        let tier = match class {
            FailureClass::InfraPermanent => TierHint::Standard, // switch provider
            _ => TierHint::Heavy,                               // heavier model
        };
        return FallbackDecision {
            intent: Intent::EscalateTier {
                tier,
                reason: format!("{class:?}: {attempt} retries exhausted for phase {failed_phase:?}"),
            },
            message: format!("{class:?}: escalating to {tier:?}"),
        };
    }

    FallbackDecision {
        intent: Intent::Halt {
            reason: format!(
                "{class:?}: {attempt} retries exhausted for phase {failed_phase:?} — no recovery path"
            ),
        },
        message: format!("{class:?}: halting after {attempt} retries"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_exceeded_always_halts() {
        let d = decide(SddPhase::Specify, "budget cap reached ($1.00)", 1);
        assert!(matches!(d.intent, Intent::Halt { .. }));
    }

    #[test]
    fn stall_always_halts() {
        let d = decide(SddPhase::Design, "stall: same tool repeated", 1);
        assert!(matches!(d.intent, Intent::Halt { .. }));
    }

    #[test]
    fn cancelled_always_halts() {
        let d = decide(SddPhase::Tasks, "cancelled", 1);
        assert!(matches!(d.intent, Intent::Halt { .. }));
    }

    #[test]
    fn agent_mistake_retries_while_below_max() {
        // max_attempts=3 → allows_retry(attempt) = attempt < 3
        // attempts 1 and 2 → retry; attempt 3 → escalate
        for att in 1..=2 {
            let d = decide(SddPhase::Specify, "output failed gate check", att);
            assert!(
                matches!(d.intent, Intent::RunSddPhase { phase: SddPhase::Specify }),
                "attempt {att}: expected retry, got {:?}",
                d.intent
            );
        }
    }

    #[test]
    fn agent_mistake_escalates_at_max_attempts() {
        // attempt=3 == max_attempts → allows_retry(3) = 3 < 3 = false → escalate
        let d = decide(SddPhase::Specify, "output failed gate check", 3);
        assert!(matches!(d.intent, Intent::EscalateTier { tier: TierHint::Heavy, .. }));
    }

    #[test]
    fn infra_transient_retries_while_below_max() {
        // max_attempts=4 → attempts 1, 2, 3 → retry
        for att in 1..=3 {
            let d = decide(SddPhase::Design, "connection timeout", att);
            assert!(
                matches!(d.intent, Intent::RunSddPhase { .. }),
                "attempt {att}: expected retry"
            );
        }
    }

    #[test]
    fn infra_transient_halts_at_max_attempts() {
        // attempt=4 == max_attempts → no retry, no escalation → halt
        let d = decide(SddPhase::Design, "connection timeout", 4);
        assert!(matches!(d.intent, Intent::Halt { .. }));
    }

    #[test]
    fn infra_permanent_escalates_to_standard() {
        let d = decide(SddPhase::Tasks, "401 Unauthorized", 2); // attempt 2 = exhausted (max=1)
        assert!(matches!(d.intent, Intent::EscalateTier { tier: TierHint::Standard, .. }));
    }
}
