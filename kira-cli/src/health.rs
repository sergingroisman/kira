#![forbid(unsafe_code)]

//! Health checker background task.
//!
//! Periodically calls `GET /v1/health` and sends `UiMessage::CoreConnected`
//! or `UiMessage::CoreDisconnected` on state transitions only.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::core_client::CoreClient;
use crate::messages::UiMessage;

/// Determines whether a health state transition message should be emitted.
///
/// Returns `true` if the status has changed (or this is the first check),
/// meaning a `CoreConnected` or `CoreDisconnected` message should be sent.
/// Returns `false` when the status remains the same (no toast needed).
///
/// This is a pure function extracted for testability.
pub const fn should_emit_message(was_connected: Option<bool>, is_connected: bool) -> bool {
    match (was_connected, is_connected) {
        // First check always emits; transitions emit too
        (None, _) | (Some(false), true) | (Some(true), false) => true,
        // No transition — same status
        (Some(true), true) | (Some(false), false) => false,
    }
}

/// Spawns a background task that periodically checks Core health.
///
/// Performs an initial health check immediately, then repeats every
/// `interval_secs` seconds. Only emits messages on status transitions
/// (connected→disconnected or disconnected→connected).
///
/// Respects the `CancellationToken` for graceful shutdown.
pub fn spawn_health_checker(
    tx: mpsc::Sender<UiMessage>,
    client: CoreClient,
    interval_secs: u64,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut was_connected: Option<bool> = None;

        loop {
            // Perform health check
            let result = client.health().await;

            let is_connected = result.is_ok();

            // Emit message only on transitions
            if should_emit_message(was_connected, is_connected) {
                if is_connected {
                    if let Ok(resp) = result {
                        let _ = tx
                            .send(UiMessage::CoreConnected {
                                version: resp.version,
                            })
                            .await;
                    }
                } else {
                    let reason = match result {
                        Err(e) => e.to_string(),
                        Ok(_) => "unknown".to_string(),
                    };
                    let _ = tx.send(UiMessage::CoreDisconnected { reason }).await;
                }
            }

            was_connected = Some(is_connected);

            // Sleep for interval_secs, but respect cancellation
            let sleep = tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs));
            tokio::select! {
                () = sleep => {}
                () = cancel.cancelled() => {
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    /// Helper: run health checker with a mock scenario and collect messages.
    async fn run_checker_collect(
        client: CoreClient,
        interval_secs: u64,
        run_duration: Duration,
    ) -> Vec<UiMessage> {
        let (tx, mut rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let handle = spawn_health_checker(tx, client, interval_secs, cancel_clone);

        // Let it run for the specified duration
        tokio::time::sleep(run_duration).await;
        cancel.cancel();
        let _ = handle.await;

        // Drain all messages
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        messages
    }

    #[tokio::test]
    async fn emits_disconnected_when_core_unreachable() {
        // Use a port that nothing listens on
        let cfg = crate::config::Config {
            core_url: "http://127.0.0.1:19999".to_string(),
            ..crate::config::Config::default()
        };
        let client = CoreClient::new(&cfg).unwrap();

        let messages = run_checker_collect(client, 1, Duration::from_millis(500)).await;

        assert!(!messages.is_empty(), "should emit at least one message");
        match &messages[0] {
            UiMessage::CoreDisconnected { reason } => {
                assert!(!reason.is_empty());
            }
            other => panic!("expected CoreDisconnected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn respects_cancellation_token() {
        let cfg = crate::config::Config {
            core_url: "http://127.0.0.1:19999".to_string(),
            ..crate::config::Config::default()
        };
        let client = CoreClient::new(&cfg).unwrap();

        let (tx, _rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        let handle = spawn_health_checker(tx, client, 60, cancel_clone);

        // Cancel immediately after first check completes
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel.cancel();

        // Task should finish promptly
        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_ok(),
            "task should exit within 2s after cancellation"
        );
    }

    #[tokio::test]
    async fn no_duplicate_messages_on_same_status() {
        // Core unreachable — should only emit one CoreDisconnected, not repeated ones
        let cfg = crate::config::Config {
            core_url: "http://127.0.0.1:19999".to_string(),
            ..crate::config::Config::default()
        };
        let client = CoreClient::new(&cfg).unwrap();

        // Use a very short interval so multiple checks happen
        let messages = run_checker_collect(client, 1, Duration::from_millis(2500)).await;

        // Should have exactly 1 CoreDisconnected (initial transition only)
        let disconnect_count = messages
            .iter()
            .filter(|m| matches!(m, UiMessage::CoreDisconnected { .. }))
            .count();
        assert_eq!(
            disconnect_count, 1,
            "should emit exactly one CoreDisconnected, got {disconnect_count}",
        );
    }

    // Feature: tui-wiring, Property 3: Health State Transition Toasts
    // **Validates: Requirements 2.2, 2.5, 2.6**

    use proptest::prelude::*;

    /// Simulates the health state machine over a sequence of health check results.
    /// Returns the number of messages that would be emitted.
    fn simulate_transitions(results: &[bool]) -> usize {
        let mut was_connected: Option<bool> = None;
        let mut message_count = 0;

        for &is_connected in results {
            if should_emit_message(was_connected, is_connected) {
                message_count += 1;
            }
            was_connected = Some(is_connected);
        }

        message_count
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 3a: A message is emitted if and only if the status transitions.
        /// For each consecutive pair in the sequence, a message is produced iff the
        /// values differ. The first element always produces a message (initial state).
        #[test]
        fn prop_toast_iff_status_transitions(results in proptest::collection::vec(any::<bool>(), 1..=50)) {
            let actual_count = simulate_transitions(&results);

            // Expected: first element always emits (transition from None),
            // plus one for each consecutive pair where values differ.
            let mut expected_count = 1; // first check always emits
            for window in results.windows(2) {
                if window[0] != window[1] {
                    expected_count += 1;
                }
            }

            prop_assert_eq!(
                actual_count, expected_count,
                "sequence {:?}: expected {} messages, got {}",
                results, expected_count, actual_count
            );
        }

        /// Property 3b: No message is produced when the status remains the same
        /// (after the initial check). A constant sequence of length N should produce
        /// exactly 1 message (the initial transition from None).
        #[test]
        fn prop_no_toast_on_same_status(value: bool, extra_count in 1usize..=49) {
            // Build a constant sequence: all same value
            let results: Vec<bool> = std::iter::repeat_n(value, extra_count + 1).collect();
            let actual_count = simulate_transitions(&results);

            // Only the first check emits a message; all subsequent same-status checks do not.
            prop_assert_eq!(
                actual_count, 1,
                "constant sequence of {} (len {}): expected 1 message, got {}",
                value, results.len(), actual_count
            );
        }

        /// Property 3c: For each consecutive pair in the sequence, should_emit_message
        /// returns true iff the values differ (after the first element).
        #[test]
        fn prop_emit_iff_values_differ(results in proptest::collection::vec(any::<bool>(), 2..=50)) {
            let mut was_connected: Option<bool> = None;

            for &is_connected in &results {
                let emits = should_emit_message(was_connected, is_connected);

                match was_connected {
                    None => {
                        // First check always emits
                        prop_assert!(emits, "first check should always emit");
                    }
                    Some(prev) => {
                        if prev == is_connected {
                            prop_assert!(!emits, "same status {} should not emit", is_connected);
                        } else {
                            prop_assert!(emits, "transition from {} to {} should emit", prev, is_connected);
                        }
                    }
                }

                was_connected = Some(is_connected);
            }
        }
    }
}
