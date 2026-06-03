#![allow(dead_code)]

use std::time::Duration;

/// Exponential backoff calculator for reconnection delays.
///
/// Formula: `min(2^(failures-1), max_secs)` seconds.
/// After `reset()`, the next call to `next_delay` returns `initial_secs` (i.e. 2^0 = 1s when initial_secs=1).
pub struct ExponentialBackoff {
    initial_secs: u64,
    max_secs: u64,
    failures: u32,
}

impl ExponentialBackoff {
    /// Create a new backoff calculator.
    ///
    /// - `initial_secs`: the delay after the first failure (2^0 = 1 when initial_secs=1).
    /// - `max_secs`: the maximum delay cap.
    pub const fn new(initial_secs: u64, max_secs: u64) -> Self {
        Self {
            initial_secs,
            max_secs,
            failures: 0,
        }
    }

    /// Increment the failure count and return the next backoff delay.
    ///
    /// The delay is `min(2^(failures-1), max_secs)` seconds, where failures is 1-indexed.
    pub fn next_delay(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        let exp = self.failures.saturating_sub(1);
        let backoff_secs = self
            .initial_secs
            .saturating_mul(2u64.saturating_pow(exp))
            .min(self.max_secs);
        Duration::from_secs(backoff_secs)
    }

    /// Reset the failure count to zero.
    ///
    /// After reset, the next call to `next_delay` will return `initial_secs` seconds.
    pub const fn reset(&mut self) {
        self.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tui-wiring, Property 5: Exponential Backoff Computation
    // **Validates: Requirements 3.4, 3.5, 10.3**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 5a: For N consecutive failures (1..=20), the Nth delay equals
        /// min(2^(N-1), 60) seconds.
        #[test]
        fn prop_backoff_delay_equals_formula(n in 1u32..=20u32) {
            let mut backoff = ExponentialBackoff::new(1, 60);
            let mut last_delay = Duration::from_secs(0);
            for _ in 0..n {
                last_delay = backoff.next_delay();
            }
            let expected_secs = 2u64.saturating_pow(n - 1).min(60);
            prop_assert_eq!(last_delay, Duration::from_secs(expected_secs));
        }

        /// Property 5b: After reset (simulating successful connection), the next
        /// delay returns to 1 second (initial_secs).
        #[test]
        fn prop_backoff_resets_to_initial_after_success(n in 1u32..=20u32) {
            let mut backoff = ExponentialBackoff::new(1, 60);
            // Simulate N failures
            for _ in 0..n {
                backoff.next_delay();
            }
            // Simulate successful connection → reset
            backoff.reset();
            // Next failure after reset should start at 1s (2^0)
            let delay_after_reset = backoff.next_delay();
            prop_assert_eq!(delay_after_reset, Duration::from_secs(1));
        }
    }

    #[test]
    fn first_failure_returns_initial() {
        let mut b = ExponentialBackoff::new(1, 60);
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn exponential_growth() {
        let mut b = ExponentialBackoff::new(1, 60);
        assert_eq!(b.next_delay(), Duration::from_secs(1)); // 2^0
        assert_eq!(b.next_delay(), Duration::from_secs(2)); // 2^1
        assert_eq!(b.next_delay(), Duration::from_secs(4)); // 2^2
        assert_eq!(b.next_delay(), Duration::from_secs(8)); // 2^3
        assert_eq!(b.next_delay(), Duration::from_secs(16)); // 2^4
        assert_eq!(b.next_delay(), Duration::from_secs(32)); // 2^5
    }

    #[test]
    fn caps_at_max() {
        let mut b = ExponentialBackoff::new(1, 60);
        for _ in 0..10 {
            b.next_delay();
        }
        // 2^9 = 512, capped at 60
        assert_eq!(b.next_delay(), Duration::from_mins(1));
    }

    #[test]
    fn reset_restores_initial() {
        let mut b = ExponentialBackoff::new(1, 60);
        b.next_delay(); // 1s
        b.next_delay(); // 2s
        b.next_delay(); // 4s
        b.reset();
        assert_eq!(b.next_delay(), Duration::from_secs(1)); // back to 2^0
    }

    #[test]
    fn custom_initial_and_max() {
        let mut b = ExponentialBackoff::new(2, 10);
        assert_eq!(b.next_delay(), Duration::from_secs(2)); // 2 * 2^0
        assert_eq!(b.next_delay(), Duration::from_secs(4)); // 2 * 2^1
        assert_eq!(b.next_delay(), Duration::from_secs(8)); // 2 * 2^2
        assert_eq!(b.next_delay(), Duration::from_secs(10)); // 2 * 2^3 = 16, capped at 10
    }

    #[test]
    fn saturating_does_not_panic() {
        let mut b = ExponentialBackoff::new(1, 60);
        // Simulate many failures — should not overflow or panic
        for _ in 0..100 {
            let d = b.next_delay();
            assert!(d <= Duration::from_mins(1));
        }
    }
}
