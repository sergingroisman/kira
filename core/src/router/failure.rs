//! Failure classification + `RetryPolicy` (P6 — Resiliência).
//!
//! Classifies an error into one of 6 buckets so downstream layers
//! (Pilot, `FallbackChain`, lessons) can apply different policies:
//!
//! | Class            | Treatment                                  |
//! |------------------|--------------------------------------------|
//! | AgentMistake     | retry with feedback (re-prompt)            |
//! | InfraTransient   | retry with same model (5xx, timeout, dns)  |
//! | InfraPermanent   | escalate to fallback chain (4xx auth/quota)|
//! | BudgetExceeded   | halt — operator decision                   |
//! | Stall            | halt + record lesson                       |
//! | Cancelled        | halt — user/system intent                  |

use serde::{Deserialize, Serialize};
use std::time::Duration;
use ts_rs::TS;

/// Failure classification — covers every termination route an agent can take.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/failure.ts")]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Agent produced wrong output (judge fail, gate fail, AC unmet).
    AgentMistake,
    /// Network/5xx/timeout — retry same provider+model viable.
    InfraTransient,
    /// 4xx auth/quota/feature-disabled — switch provider needed.
    InfraPermanent,
    /// `BudgetGuard` hit cap.
    BudgetExceeded,
    /// Stall detector fired (same-tool / output-unchanged / no-progress).
    Stall,
    /// User or supervisor cancelled.
    Cancelled,
}

/// Classify an error string. Heuristic; v0 covers the high-traffic cases.
pub fn classify(error_message: &str) -> FailureClass {
    let m = error_message.to_ascii_lowercase();
    if m.starts_with("stall:") || m.contains("stall detect") {
        return FailureClass::Stall;
    }
    if m.contains("cancelled") || m.contains("canceled") {
        return FailureClass::Cancelled;
    }
    if m.contains("budget") || m.contains("hard-stop") || m.contains("hard stop") {
        return FailureClass::BudgetExceeded;
    }
    if m.contains("401") || m.contains("403") || m.contains("unauthorized")
        || m.contains("forbidden") || m.contains("quota") || m.contains("rate limit")
    {
        return FailureClass::InfraPermanent;
    }
    if m.contains("timeout") || m.contains("5xx") || m.contains("502")
        || m.contains("503") || m.contains("504") || m.contains("connection")
        || m.contains("dns") || m.contains("network")
    {
        return FailureClass::InfraTransient;
    }
    FailureClass::AgentMistake
}

/// `RetryPolicy` — per-class retry budget + backoff.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_initial: Duration,
    pub backoff_max: Duration,
    pub escalate_on_exhaustion: bool,
}

impl RetryPolicy {
    /// Default policy per class. Tuned for solo dev / chinese stack costs.
    pub const fn for_class(class: FailureClass) -> Self {
        match class {
            FailureClass::AgentMistake => Self {
                max_attempts: 3,
                backoff_initial: Duration::from_secs(2),
                backoff_max: Duration::from_secs(8),
                escalate_on_exhaustion: true, // judge feedback → reviewer / heavier tier
            },
            FailureClass::InfraTransient => Self {
                max_attempts: 4,
                backoff_initial: Duration::from_millis(500),
                backoff_max: Duration::from_secs(8),
                escalate_on_exhaustion: false, // breaker handles
            },
            FailureClass::InfraPermanent => Self {
                max_attempts: 1, // no retry — switch provider
                backoff_initial: Duration::from_secs(0),
                backoff_max: Duration::from_secs(0),
                escalate_on_exhaustion: true,
            },
            FailureClass::BudgetExceeded => Self {
                max_attempts: 0, // halt
                backoff_initial: Duration::from_secs(0),
                backoff_max: Duration::from_secs(0),
                escalate_on_exhaustion: false,
            },
            FailureClass::Stall => Self {
                max_attempts: 0, // halt — re-run won't help w/ same prompt
                backoff_initial: Duration::from_secs(0),
                backoff_max: Duration::from_secs(0),
                escalate_on_exhaustion: false,
            },
            FailureClass::Cancelled => Self {
                max_attempts: 0,
                backoff_initial: Duration::from_secs(0),
                backoff_max: Duration::from_secs(0),
                escalate_on_exhaustion: false,
            },
        }
    }

    /// Backoff for attempt N (1-indexed). Exponential capped.
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return self.backoff_initial;
        }
        let factor = 1u64 << (attempt - 1).min(6);
        let candidate = self.backoff_initial.saturating_mul(factor as u32);
        candidate.min(self.backoff_max)
    }

    pub const fn allows_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_stall() {
        assert_eq!(classify("stall: tool 'bash' called 3 times"), FailureClass::Stall);
        assert_eq!(classify("Stall detected: output unchanged"), FailureClass::Stall);
    }

    #[test]
    fn classify_budget() {
        assert_eq!(
            classify("budget cap reached ($1.00)"),
            FailureClass::BudgetExceeded
        );
        assert_eq!(
            classify("monthly hard-stop hit"),
            FailureClass::BudgetExceeded
        );
    }

    #[test]
    fn classify_infra_transient() {
        assert_eq!(classify("connection timeout"), FailureClass::InfraTransient);
        assert_eq!(classify("502 Bad Gateway"), FailureClass::InfraTransient);
        assert_eq!(classify("dns resolution failed"), FailureClass::InfraTransient);
    }

    #[test]
    fn classify_infra_permanent() {
        assert_eq!(classify("401 Unauthorized"), FailureClass::InfraPermanent);
        assert_eq!(classify("quota exceeded for project"), FailureClass::InfraPermanent);
    }

    #[test]
    fn classify_cancelled() {
        assert_eq!(classify("Cancelled by user"), FailureClass::Cancelled);
    }

    #[test]
    fn classify_defaults_agent_mistake() {
        assert_eq!(
            classify("output failed faithfulness check"),
            FailureClass::AgentMistake
        );
    }

    #[test]
    fn policy_stall_no_retry() {
        let p = RetryPolicy::for_class(FailureClass::Stall);
        assert!(!p.allows_retry(0));
    }

    #[test]
    fn policy_agent_mistake_allows_3() {
        let p = RetryPolicy::for_class(FailureClass::AgentMistake);
        assert!(p.allows_retry(0));
        assert!(p.allows_retry(1));
        assert!(p.allows_retry(2));
        assert!(!p.allows_retry(3));
    }

    #[test]
    fn policy_backoff_capped() {
        let p = RetryPolicy::for_class(FailureClass::AgentMistake);
        let b = p.backoff_for(10);
        assert!(b <= p.backoff_max);
    }

    #[test]
    fn policy_backoff_exponential() {
        let p = RetryPolicy::for_class(FailureClass::AgentMistake);
        let b1 = p.backoff_for(1);
        let b2 = p.backoff_for(2);
        let b3 = p.backoff_for(3);
        assert!(b2 >= b1);
        assert!(b3 >= b2);
    }
}
