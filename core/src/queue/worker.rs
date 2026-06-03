//! Queue Worker — long-running Tokio task that consumes the message queue.
//!
//! Processes messages FIFO with configurable concurrency (semaphore).
//! Retries failed messages with exponential backoff up to max_attempts.

use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use super::{MessageQueue, QueueConfig};

/// Processor trait — called by the worker for each message.
pub trait MessageProcessor: Send + Sync {
    fn process(&self, payload: &serde_json::Value) -> impl std::future::Future<Output = anyhow::Result<()>> + Send;
}

/// Start the queue worker as a background Tokio task.
///
/// Returns a `JoinHandle` that runs until the process shuts down.
/// The worker polls the queue at `config.poll_interval_ms` when idle.
pub fn spawn_worker<Q: MessageQueue + 'static, P: MessageProcessor + 'static>(
    queue: Arc<Q>,
    processor: Arc<P>,
    config: QueueConfig,
) -> tokio::task::JoinHandle<()> {
    let semaphore = Arc::new(Semaphore::new(config.max_workers));

    tokio::spawn(async move {
        info!(
            max_workers = config.max_workers,
            poll_ms = config.poll_interval_ms,
            "queue worker started"
        );

        loop {
            // Acquire semaphore permit (limits concurrency)
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    error!("semaphore closed — worker shutting down");
                    break;
                }
            };

            // Try to claim next message
            let msg = match queue.claim_next().await {
                Ok(Some(m)) => m,
                Ok(None) => {
                    // No messages — release permit and sleep
                    drop(permit);
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        config.poll_interval_ms,
                    ))
                    .await;
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, "queue claim_next failed");
                    drop(permit);
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        config.poll_interval_ms,
                    ))
                    .await;
                    continue;
                }
            };

            let queue_clone = queue.clone();
            let processor_clone = processor.clone();
            let max_attempts = config.max_attempts;
            let queue_id = msg.queue_id;

            // Process in a spawned task (concurrency via semaphore)
            tokio::spawn(async move {
                debug!(queue_id = %queue_id, "processing message");

                match processor_clone.process(&msg.payload).await {
                    Ok(()) => {
                        if let Err(e) = queue_clone.mark_completed(queue_id).await {
                            error!(queue_id = %queue_id, error = %e, "failed to mark completed");
                        }
                    }
                    Err(e) => {
                        warn!(queue_id = %queue_id, error = %e, "message processing failed");
                        if let Err(mark_err) = queue_clone
                            .mark_failed(queue_id, &e.to_string(), max_attempts)
                            .await
                        {
                            error!(queue_id = %queue_id, error = %mark_err, "failed to mark failed");
                        }
                    }
                }

                drop(permit); // Release semaphore
            });
        }
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::InMemoryQueue;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingProcessor {
        count: AtomicU32,
    }

    impl CountingProcessor {
        fn new() -> Self {
            Self { count: AtomicU32::new(0) }
        }
    }

    impl MessageProcessor for CountingProcessor {
        async fn process(&self, _payload: &serde_json::Value) -> anyhow::Result<()> {
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    struct FailingProcessor;

    impl MessageProcessor for FailingProcessor {
        async fn process(&self, _payload: &serde_json::Value) -> anyhow::Result<()> {
            anyhow::bail!("processing error")
        }
    }

    #[tokio::test]
    async fn worker_processes_messages() {
        let queue = Arc::new(InMemoryQueue::new());
        let processor = Arc::new(CountingProcessor::new());

        // Enqueue 3 messages
        queue.enqueue(serde_json::json!({"msg": 1})).await.unwrap();
        queue.enqueue(serde_json::json!({"msg": 2})).await.unwrap();
        queue.enqueue(serde_json::json!({"msg": 3})).await.unwrap();

        let config = QueueConfig {
            enabled: true,
            max_workers: 2,
            max_attempts: 3,
            poll_interval_ms: 10,
            ..Default::default()
        };

        let handle = spawn_worker(queue.clone(), processor.clone(), config);

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        handle.abort();

        assert_eq!(processor.count.load(Ordering::Relaxed), 3);
        assert_eq!(queue.depth().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn worker_retries_on_failure() {
        let queue = Arc::new(InMemoryQueue::new());
        let processor = Arc::new(FailingProcessor);

        queue.enqueue(serde_json::json!({"msg": 1})).await.unwrap();

        let config = QueueConfig {
            enabled: true,
            max_workers: 1,
            max_attempts: 2,
            poll_interval_ms: 10,
            ..Default::default()
        };

        let handle = spawn_worker(queue.clone(), processor, config);

        // Wait for retries
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        handle.abort();

        // After max_attempts, message should be failed
        let pending = queue.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0); // not pending anymore (failed)
    }
}
