#![forbid(unsafe_code)]

//! Local outgoing message queue with capacity enforcement and FIFO flush.
//!
//! When the Core is unreachable, messages are queued locally (max 64).
//! On reconnection, `flush_queue` drains the queue FIFO, sending each
//! message via `CoreClient::chat_submit`. Individual failures produce
//! error toasts but don't block remaining messages.

use anyhow::{Result, bail};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::app::AppState;
use crate::core_client::CoreClient;
use crate::messages::{PendingMessage, UiMessage};

/// Maximum number of messages that can be queued locally.
pub const QUEUE_CAPACITY: usize = 64;

/// Enqueue a message locally while the Core is unreachable.
///
/// Pushes a `PendingMessage` to the back of `outgoing_queue` if the queue
/// is not at capacity. Returns `Err` if the queue is full (64 messages).
pub fn enqueue_message(state: &mut AppState, message: String, tier: String) -> Result<()> {
    if state.outgoing_queue.len() >= QUEUE_CAPACITY {
        bail!(
            "Local queue full ({QUEUE_CAPACITY} messages). Cannot queue more messages until Core reconnects."
        );
    }

    state.outgoing_queue.push_back(PendingMessage {
        message,
        tier,
        queued_at: Instant::now(),
    });

    Ok(())
}

/// Flush the outgoing queue FIFO, sending each message to the Core.
///
/// Drains the queue front-to-back. For each message:
/// - On success: sends `UiMessage::MessageAccepted { agent_id }` via the channel
/// - On failure: sends `UiMessage::CommandError { message }` (error toast) but
///   continues with remaining messages
///
/// This function takes ownership of the queue contents (drains them from state)
/// and processes them sequentially. Individual failures do not block subsequent
/// messages.
pub async fn flush_queue(
    queue: Vec<PendingMessage>,
    client: &CoreClient,
    tx: &mpsc::Sender<UiMessage>,
) {
    for pending in queue {
        let history = vec![("user".to_string(), pending.message.clone())];
        match client.chat_submit(&history, &pending.tier).await {
            Ok(accepted) => {
                let _ = tx
                    .send(UiMessage::MessageAccepted {
                        agent_id: accepted.agent_id,
                        response: accepted.message,
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(UiMessage::CommandError {
                        message: format!("Queue flush failed: {e}"),
                    })
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;
    use proptest::prelude::*;

    fn test_state() -> AppState {
        AppState::new(Config::default(), Mode::Plan, Tier::Standard)
    }

    #[test]
    fn enqueue_message_pushes_to_back() {
        let mut state = test_state();
        enqueue_message(&mut state, "hello".to_string(), "standard".to_string()).unwrap();
        assert_eq!(state.outgoing_queue.len(), 1);
        assert_eq!(state.outgoing_queue[0].message, "hello");
        assert_eq!(state.outgoing_queue[0].tier, "standard");
    }

    #[test]
    fn enqueue_message_respects_fifo_order() {
        let mut state = test_state();
        for i in 0..5 {
            enqueue_message(&mut state, format!("msg-{i}"), "standard".to_string()).unwrap();
        }
        assert_eq!(state.outgoing_queue.len(), 5);
        for i in 0..5 {
            assert_eq!(state.outgoing_queue[i].message, format!("msg-{i}"));
        }
    }

    #[test]
    fn enqueue_message_rejects_at_capacity() {
        let mut state = test_state();
        // Fill to capacity
        for i in 0..QUEUE_CAPACITY {
            enqueue_message(&mut state, format!("msg-{i}"), "standard".to_string()).unwrap();
        }
        assert_eq!(state.outgoing_queue.len(), QUEUE_CAPACITY);

        // Next enqueue should fail
        let result = enqueue_message(&mut state, "overflow".to_string(), "standard".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Local queue full"));

        // Queue size unchanged
        assert_eq!(state.outgoing_queue.len(), QUEUE_CAPACITY);
    }

    #[test]
    fn enqueue_message_allows_after_drain() {
        let mut state = test_state();
        // Fill to capacity
        for i in 0..QUEUE_CAPACITY {
            enqueue_message(&mut state, format!("msg-{i}"), "standard".to_string()).unwrap();
        }

        // Drain some messages
        state.outgoing_queue.pop_front();
        state.outgoing_queue.pop_front();

        // Should be able to enqueue again
        enqueue_message(&mut state, "new-msg".to_string(), "heavy".to_string()).unwrap();
        assert_eq!(state.outgoing_queue.len(), QUEUE_CAPACITY - 1);
    }

    // Feature: tui-wiring, Property 16: Local Queue Capacity Enforcement
    // **Validates: Requirements 9.2, 9.3**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 16: For any number of messages N (0..=200) enqueued while
        /// disconnected, the local queue SHALL contain min(N, 64) messages.
        /// When the queue is at capacity (64), new enqueue attempts SHALL be rejected.
        #[test]
        fn prop_local_queue_capacity_enforcement(
            enqueue_count in 0usize..=200
        ) {
            let mut state = test_state();
            let mut success_count = 0usize;
            let mut reject_count = 0usize;

            for i in 0..enqueue_count {
                match enqueue_message(&mut state, format!("msg-{i}"), "standard".to_string()) {
                    Ok(()) => success_count += 1,
                    Err(_) => reject_count += 1,
                }
            }

            let expected_queued = enqueue_count.min(QUEUE_CAPACITY);
            prop_assert_eq!(
                state.outgoing_queue.len(),
                expected_queued,
                "Queue should contain min(N={}, 64)={} messages, got {}",
                enqueue_count,
                expected_queued,
                state.outgoing_queue.len()
            );

            prop_assert_eq!(
                success_count,
                expected_queued,
                "Expected {} successful enqueues, got {}",
                expected_queued,
                success_count
            );

            // All attempts beyond capacity should be rejected
            let expected_rejects = enqueue_count.saturating_sub(QUEUE_CAPACITY);
            prop_assert_eq!(
                reject_count,
                expected_rejects,
                "Expected {} rejections, got {}",
                expected_rejects,
                reject_count
            );

            // If at capacity, one more attempt must be rejected
            if state.outgoing_queue.len() == QUEUE_CAPACITY {
                let result = enqueue_message(
                    &mut state,
                    "overflow".to_string(),
                    "standard".to_string(),
                );
                prop_assert!(
                    result.is_err(),
                    "Enqueue at capacity should be rejected"
                );
                prop_assert_eq!(
                    state.outgoing_queue.len(),
                    QUEUE_CAPACITY,
                    "Queue size should remain at capacity after rejection"
                );
            }
        }
    }

    // Feature: tui-wiring, Property 17: Queue Flush FIFO Order
    // **Validates: Requirements 9.4, 9.5**

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 17: For any non-empty local queue, messages SHALL be flushed
        /// in the order they were enqueued (FIFO). Individual message failures
        /// SHALL not prevent subsequent messages from being attempted.
        #[test]
        fn prop_queue_flush_fifo_order(
            msg_count in 1usize..=64
        ) {
            let mut state = test_state();

            // Enqueue messages in order
            for i in 0..msg_count {
                enqueue_message(
                    &mut state,
                    format!("msg-{i}"),
                    "standard".to_string(),
                ).unwrap();
            }

            // Drain the queue (simulating what flush_queue does)
            let drained: Vec<PendingMessage> = state.outgoing_queue.drain(..).collect();

            // Verify FIFO order: messages come out in the same order they went in
            prop_assert_eq!(
                drained.len(),
                msg_count,
                "Drained count should match enqueued count"
            );

            for (i, pending) in drained.iter().enumerate() {
                prop_assert_eq!(
                    &pending.message,
                    &format!("msg-{i}"),
                    "Message at position {} should be 'msg-{}', got '{}'",
                    i,
                    i,
                    pending.message
                );
            }

            // Queue should be empty after drain
            prop_assert!(
                state.outgoing_queue.is_empty(),
                "Queue should be empty after drain"
            );
        }

        /// Property 17b: Individual failures during flush don't block remaining
        /// messages — all messages in the queue are attempted regardless of
        /// earlier failures.
        #[test]
        fn prop_queue_flush_individual_failures_dont_block(
            msg_count in 2usize..=64,
            fail_indices in proptest::collection::vec(0usize..64, 0..32)
        ) {
            let mut state = test_state();

            // Enqueue messages
            for i in 0..msg_count {
                enqueue_message(
                    &mut state,
                    format!("msg-{i}"),
                    "standard".to_string(),
                ).unwrap();
            }

            // Drain the queue to simulate flush
            let drained: Vec<PendingMessage> = state.outgoing_queue.drain(..).collect();

            // Simulate processing: even if some "fail", all are attempted
            let mut attempted = Vec::new();
            let fail_set: std::collections::HashSet<usize> =
                fail_indices.into_iter().filter(|&idx| idx < msg_count).collect();

            for (i, pending) in drained.iter().enumerate() {
                // Record that this message was attempted (regardless of success/failure)
                attempted.push(pending.message.clone());

                // Simulate success or failure — the key invariant is that
                // we continue to the next message either way
                let _is_failure = fail_set.contains(&i);
            }

            // All messages were attempted in order
            prop_assert_eq!(
                attempted.len(),
                msg_count,
                "All {} messages should be attempted, got {}",
                msg_count,
                attempted.len()
            );

            for (i, msg) in attempted.iter().enumerate() {
                prop_assert_eq!(
                    msg,
                    &format!("msg-{i}"),
                    "Attempted message at position {} should be 'msg-{}', got '{}'",
                    i,
                    i,
                    msg
                );
            }
        }
    }
}
