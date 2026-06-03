//! Circuit breaker — lock-free atomic state machine.
//!
//! States: `Closed` → `Open` (>= N failures in window) → `HalfOpen` (after timeout)
//!         → `Closed` (probe ok) | `Open` (probe fails)
//!
//! Per plan defaults: 5 failures / 60s window → Open 30s → `HalfOpen` 1 probe.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU8, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const STATE_CLOSED: u8 = 0;
const STATE_OPEN: u8 = 1;
const STATE_HALF_OPEN: u8 = 2;

/// Persistable snapshot of a breaker's runtime state (C2).
/// Stored in `TBL_BREAKERS` as JSON, keyed by `provider:model`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerSnapshot {
    pub state: u8,
    pub failure_count: u32,
    pub window_start_unix_ms: i64,
    pub opened_at_unix_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct BreakerConfig {
    pub failure_threshold: u32,
    pub window: Duration,
    pub open_duration: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            window: Duration::from_mins(1),
            open_duration: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    config: BreakerConfig,
    state: AtomicU8,
    failure_count: AtomicU32,
    window_start_unix_ms: AtomicI64,
    opened_at_unix_ms: AtomicI64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(BreakerConfig::default())
    }
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl CircuitBreaker {
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            config,
            state: AtomicU8::new(STATE_CLOSED),
            failure_count: AtomicU32::new(0),
            window_start_unix_ms: AtomicI64::new(now_unix_ms()),
            opened_at_unix_ms: AtomicI64::new(0),
        }
    }

    /// True if the breaker is currently blocking calls.
    /// Transitions Open → `HalfOpen` automatically when `open_duration` elapses.
    pub fn is_open(&self) -> bool {
        match self.state.load(Ordering::Acquire) {
            STATE_CLOSED => false,
            STATE_HALF_OPEN => false, // one probe allowed
            STATE_OPEN => {
                let opened_at = self.opened_at_unix_ms.load(Ordering::Acquire);
                let elapsed_ms = now_unix_ms().saturating_sub(opened_at);
                if elapsed_ms as u128 >= self.config.open_duration.as_millis() {
                    // try to flip to HalfOpen; if someone beat us, still allow one
                    let _ = self.state.compare_exchange(
                        STATE_OPEN,
                        STATE_HALF_OPEN,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    pub fn record_success(&self) {
        match self.state.load(Ordering::Acquire) {
            STATE_HALF_OPEN | STATE_OPEN => {
                self.state.store(STATE_CLOSED, Ordering::Release);
                self.failure_count.store(0, Ordering::Release);
                self.window_start_unix_ms
                    .store(now_unix_ms(), Ordering::Release);
            }
            _ => {
                self.failure_count.store(0, Ordering::Release);
            }
        }
    }

    /// C2 — capture current state as a persistable snapshot.
    pub fn snapshot(&self) -> BreakerSnapshot {
        BreakerSnapshot {
            state: self.state.load(Ordering::Acquire),
            failure_count: self.failure_count.load(Ordering::Acquire),
            window_start_unix_ms: self.window_start_unix_ms.load(Ordering::Acquire),
            opened_at_unix_ms: self.opened_at_unix_ms.load(Ordering::Acquire),
        }
    }

    /// C2 — restore breaker state from a snapshot. Respects `open_duration` TTL:
    /// if an Open snapshot is older than `open_duration`, the restored breaker
    /// starts as Closed (the natural timeout already elapsed).
    pub fn restore(config: BreakerConfig, snap: &BreakerSnapshot) -> Self {
        let now = now_unix_ms();
        let mut state = snap.state;
        let mut opened_at = snap.opened_at_unix_ms;
        if state == STATE_OPEN {
            let elapsed_ms = now.saturating_sub(opened_at);
            if elapsed_ms as u128 >= config.open_duration.as_millis() {
                state = STATE_CLOSED;
                opened_at = 0;
            }
        }
        Self {
            config,
            state: AtomicU8::new(state),
            failure_count: AtomicU32::new(snap.failure_count),
            window_start_unix_ms: AtomicI64::new(snap.window_start_unix_ms),
            opened_at_unix_ms: AtomicI64::new(opened_at),
        }
    }

    pub fn record_failure(&self) {
        let now = now_unix_ms();
        let state = self.state.load(Ordering::Acquire);

        if state == STATE_HALF_OPEN {
            self.state.store(STATE_OPEN, Ordering::Release);
            self.opened_at_unix_ms.store(now, Ordering::Release);
            return;
        }

        // Reset window if elapsed.
        let window_start = self.window_start_unix_ms.load(Ordering::Acquire);
        if now.saturating_sub(window_start) as u128 >= self.config.window.as_millis() {
            self.window_start_unix_ms.store(now, Ordering::Release);
            self.failure_count.store(1, Ordering::Release);
        } else {
            let count = self.failure_count.fetch_add(1, Ordering::AcqRel) + 1;
            if count >= self.config.failure_threshold && state == STATE_CLOSED {
                self.state.store(STATE_OPEN, Ordering::Release);
                self.opened_at_unix_ms.store(now, Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn fast_config() -> BreakerConfig {
        BreakerConfig {
            failure_threshold: 3,
            window: Duration::from_mins(1),
            open_duration: Duration::from_millis(100),
        }
    }

    #[test]
    fn starts_closed() {
        let b = CircuitBreaker::new(fast_config());
        assert!(!b.is_open());
    }

    #[test]
    fn opens_after_threshold() {
        let b = CircuitBreaker::new(fast_config());
        b.record_failure();
        b.record_failure();
        assert!(!b.is_open());
        b.record_failure(); // hits 3
        assert!(b.is_open());
    }

    #[test]
    fn half_opens_after_duration() {
        let b = CircuitBreaker::new(fast_config());
        for _ in 0..3 {
            b.record_failure();
        }
        assert!(b.is_open());
        sleep(Duration::from_millis(150));
        // Probe call allowed → state observed as not-open.
        assert!(!b.is_open());
    }

    #[test]
    fn success_in_half_open_closes() {
        let b = CircuitBreaker::new(fast_config());
        for _ in 0..3 {
            b.record_failure();
        }
        sleep(Duration::from_millis(150));
        let _ = b.is_open();
        b.record_success();
        assert!(!b.is_open());
        assert_eq!(b.state.load(Ordering::Acquire), STATE_CLOSED);
    }

    #[test]
    fn failure_in_half_open_reopens() {
        let b = CircuitBreaker::new(fast_config());
        for _ in 0..3 {
            b.record_failure();
        }
        sleep(Duration::from_millis(150));
        let _ = b.is_open();
        b.record_failure();
        assert!(b.is_open());
    }

    #[test]
    fn snapshot_restore_preserves_open_state() {
        let cfg = BreakerConfig {
            failure_threshold: 3,
            window: Duration::from_mins(1),
            open_duration: Duration::from_mins(1), // long TTL so it survives.
        };
        let b = CircuitBreaker::new(cfg);
        for _ in 0..3 {
            b.record_failure();
        }
        assert!(b.is_open());

        let snap = b.snapshot();
        let restored = CircuitBreaker::restore(cfg, &snap);
        assert!(restored.is_open(), "restored breaker should remain Open");
    }

    #[test]
    fn snapshot_restore_respects_open_duration_ttl() {
        let cfg = fast_config(); // open_duration = 100ms
        let b = CircuitBreaker::new(cfg);
        for _ in 0..3 {
            b.record_failure();
        }
        let snap = b.snapshot();
        // Wait past open_duration before restoring.
        sleep(Duration::from_millis(150));
        let restored = CircuitBreaker::restore(cfg, &snap);
        // Should restore as Closed because the open window already elapsed.
        assert!(!restored.is_open());
        assert_eq!(restored.state.load(Ordering::Acquire), STATE_CLOSED);
    }
}
