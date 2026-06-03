#![forbid(unsafe_code)]

//! Transient/permanent error classification and retry logic with exponential backoff.
//!
//! - Transient errors (5xx, timeout, connection refused): retry with backoff, max 5 attempts.
//! - Permanent errors (4xx): no retry, return immediately.
//! - Unknown status codes default to transient (retry).

use std::future::Future;

use anyhow::{Result, anyhow};

use crate::backoff::ExponentialBackoff;

// ── Error Classification ──────────────────────────────────────────────────────

/// Classification of an HTTP error for retry decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Eligible for retry: 5xx, timeout, connection refused, unknown codes.
    Transient,
    /// No retry: 4xx client errors.
    Permanent,
}

/// Classify an HTTP status code into transient or permanent.
///
/// - 400..=499 → `Permanent` (client error, no retry)
/// - 500..=599 → `Transient` (server error, retry)
/// - Other → `Transient` (default to retry for unknown/unexpected codes)
#[allow(clippy::match_same_arms)] // Explicit 5xx arm for documentation clarity
pub const fn classify_error(status_code: u16) -> ErrorKind {
    match status_code {
        400..=499 => ErrorKind::Permanent,
        500..=599 => ErrorKind::Transient,
        _ => ErrorKind::Transient,
    }
}

// ── Retry Error ───────────────────────────────────────────────────────────────

/// Error returned by operations that can be retried.
///
/// Wraps the underlying error with a classification so `retry_with_backoff`
/// can decide whether to retry or bail immediately.
#[derive(Debug)]
pub struct RetryError {
    pub kind: ErrorKind,
    pub source: anyhow::Error,
}

impl RetryError {
    /// Create a transient retry error.
    pub fn transient(err: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: ErrorKind::Transient,
            source: err.into(),
        }
    }

    /// Create a permanent retry error.
    pub fn permanent(err: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: ErrorKind::Permanent,
            source: err.into(),
        }
    }

    /// Create a retry error from an HTTP status code and message.
    pub fn from_status(status_code: u16, message: impl Into<String>) -> Self {
        Self {
            kind: classify_error(status_code),
            source: anyhow!("{}", message.into()),
        }
    }
}

// ── Retry with Backoff ────────────────────────────────────────────────────────

/// Default maximum number of retry attempts.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;

/// Retry an async operation with exponential backoff.
///
/// - `operation`: a closure that returns a `Future` producing `Result<T, RetryError>`.
/// - `max_attempts`: maximum number of attempts (including the first try).
///
/// Behavior:
/// 1. Calls `operation()` and awaits the result.
/// 2. On success: returns `Ok(value)`.
/// 3. On `Permanent` error: returns immediately with the error (no retry).
/// 4. On `Transient` error: waits the backoff delay, then retries.
/// 5. After `max_attempts` exhausted: returns the last error.
///
/// Uses `ExponentialBackoff::new(1, 60)` for delays between retries.
pub async fn retry_with_backoff<F, Fut, T>(mut operation: F, max_attempts: u32) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, RetryError>>,
{
    let mut backoff = ExponentialBackoff::new(1, 60);
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(retry_err) => match retry_err.kind {
                ErrorKind::Permanent => {
                    return Err(retry_err.source);
                }
                ErrorKind::Transient => {
                    last_error = Some(retry_err.source);
                    if attempt < max_attempts {
                        let delay = backoff.next_delay();
                        tokio::time::sleep(delay).await;
                    }
                }
            },
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("retry exhausted with no error captured")))
}

/// Convenience wrapper using the default max attempts (5).
pub async fn retry<F, Fut, T>(operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, RetryError>>,
{
    retry_with_backoff(operation, DEFAULT_MAX_ATTEMPTS).await
}

// ── Helper: classify reqwest errors ───────────────────────────────────────────

/// Classify a `reqwest` error into a `RetryError`.
///
/// - Timeout → Transient
/// - Connection errors → Transient
/// - Other → Transient (default to retry)
pub fn classify_reqwest_error(err: reqwest::Error) -> RetryError {
    // All reqwest errors (timeout, connection, etc.) are transient
    RetryError::transient(err)
}

/// Build a `RetryError` from an HTTP response status and body.
///
/// Uses `classify_error` on the status code to determine the kind.
pub fn classify_response_error(status_code: u16, body: String) -> RetryError {
    let message = format!("HTTP {status_code}: {body}");
    RetryError::from_status(status_code, message)
}

// ── Duration helper for tests ─────────────────────────────────────────────────

/// Compute the expected total backoff duration for N transient failures
/// (useful for test assertions).
#[cfg(test)]
fn expected_backoff_total(attempts: u32) -> std::time::Duration {
    let mut total = std::time::Duration::ZERO;
    let mut backoff = ExponentialBackoff::new(1, 60);
    // Backoff delays happen between attempts (attempts - 1 delays)
    for _ in 1..attempts {
        total += backoff.next_delay();
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    #[test]
    fn classify_4xx_as_permanent() {
        for code in 400..=499 {
            assert_eq!(classify_error(code), ErrorKind::Permanent);
        }
    }

    #[test]
    fn classify_5xx_as_transient() {
        for code in 500..=599 {
            assert_eq!(classify_error(code), ErrorKind::Transient);
        }
    }

    #[test]
    fn classify_other_as_transient() {
        // Codes outside 4xx/5xx default to transient
        for code in [100, 200, 301, 302, 600, 999] {
            assert_eq!(classify_error(code), ErrorKind::Transient);
        }
    }

    #[tokio::test]
    async fn retry_succeeds_on_first_attempt() {
        let result = retry_with_backoff(|| async { Ok::<_, RetryError>(42) }, 5).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_with_backoff(
            move || {
                let c = counter_clone.clone();
                async move {
                    let attempt = c.fetch_add(1, Ordering::SeqCst) + 1;
                    if attempt < 3 {
                        Err(RetryError::transient(anyhow!("transient error")))
                    } else {
                        Ok(attempt)
                    }
                }
            },
            5,
        )
        .await;

        assert_eq!(result.unwrap(), 3);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_stops_on_permanent_error() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<u32> = retry_with_backoff(
            move || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(RetryError::permanent(anyhow!("bad request")))
                }
            },
            5,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bad request"));
        // Should only attempt once — permanent errors don't retry
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_exhausts_max_attempts() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<u32> = retry_with_backoff(
            move || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(RetryError::transient(anyhow!("still failing")))
                }
            },
            3,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("still failing"));
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn retry_error_from_status_classifies_correctly() {
        let err_4xx = RetryError::from_status(404, "not found");
        assert_eq!(err_4xx.kind, ErrorKind::Permanent);

        let err_5xx = RetryError::from_status(503, "service unavailable");
        assert_eq!(err_5xx.kind, ErrorKind::Transient);
    }

    #[test]
    fn expected_backoff_total_computes_correctly() {
        // 3 attempts = 2 delays: 1s + 2s = 3s
        assert_eq!(expected_backoff_total(3), Duration::from_secs(3));
        // 1 attempt = 0 delays
        assert_eq!(expected_backoff_total(1), Duration::ZERO);
    }

    // Feature: tui-wiring, Property 18: Transient vs Permanent Error Classification
    // **Validates: Requirements 9.6, 9.7**

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 18: For any HTTP status code in 100..=599:
        /// - 500..=599 → Transient (eligible for retry with backoff, max 5 attempts)
        /// - 400..=499 → Permanent (no retry, immediate discard)
        #[test]
        fn prop_error_classification(
            status_code in 100u16..=599
        ) {
            let result = classify_error(status_code);

            match status_code {
                500..=599 => {
                    prop_assert_eq!(
                        result,
                        ErrorKind::Transient,
                        "Status {} should be classified as Transient, got {:?}",
                        status_code,
                        result
                    );
                }
                400..=499 => {
                    prop_assert_eq!(
                        result,
                        ErrorKind::Permanent,
                        "Status {} should be classified as Permanent, got {:?}",
                        status_code,
                        result
                    );
                }
                _ => {
                    // Non-error codes (1xx, 2xx, 3xx) default to Transient
                    // per the implementation's conservative retry policy
                    prop_assert_eq!(
                        result,
                        ErrorKind::Transient,
                        "Status {} should default to Transient, got {:?}",
                        status_code,
                        result
                    );
                }
            }
        }

        /// Property 18b: 5xx codes are always transient (retryable).
        #[test]
        fn prop_5xx_always_transient(
            status_code in 500u16..=599
        ) {
            let result = classify_error(status_code);
            prop_assert_eq!(
                result,
                ErrorKind::Transient,
                "All 5xx codes must be Transient; {} gave {:?}",
                status_code,
                result
            );
        }

        /// Property 18c: 4xx codes are always permanent (non-retryable).
        #[test]
        fn prop_4xx_always_permanent(
            status_code in 400u16..=499
        ) {
            let result = classify_error(status_code);
            prop_assert_eq!(
                result,
                ErrorKind::Permanent,
                "All 4xx codes must be Permanent; {} gave {:?}",
                status_code,
                result
            );
        }
    }
}
