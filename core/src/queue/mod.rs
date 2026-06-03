//! Message Queue — PostgreSQL-backed resilience buffer (Pilar 3).
//!
//! Provides a durable FIFO queue for incoming messages. The Core responds
//! immediately with `202 Accepted` and a `queue_id`, then a background
//! worker processes messages asynchronously.
//!
//! When `[queue].enabled = false` in tokens.toml, the queue is bypassed
//! and messages are processed synchronously (v0 behavior).

pub mod worker;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a message in the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A message in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    pub queue_id: Uuid,
    pub payload: serde_json::Value,
    pub status: QueueStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub attempts: u32,
    pub error: Option<String>,
}

/// Configuration for the message queue.
#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub enabled: bool,
    pub postgres_url: Option<String>,
    pub max_workers: usize,
    pub max_attempts: u32,
    pub poll_interval_ms: u64,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            postgres_url: None,
            max_workers: 4,
            max_attempts: 3,
            poll_interval_ms: 500,
        }
    }
}

/// Trait for queue storage operations.
pub trait MessageQueue: Send + Sync {
    /// Enqueue a new message. Returns the queue_id.
    fn enqueue(&self, payload: serde_json::Value) -> impl std::future::Future<Output = Result<Uuid>> + Send;

    /// List messages with status `queued` or `processing`, ordered by created_at.
    fn list_pending(&self) -> impl std::future::Future<Output = Result<Vec<QueueMessage>>> + Send;

    /// Claim the next queued message for processing (atomic: queued → processing).
    fn claim_next(&self) -> impl std::future::Future<Output = Result<Option<QueueMessage>>> + Send;

    /// Mark a message as completed.
    fn mark_completed(&self, queue_id: Uuid) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Mark a message as failed, incrementing attempts.
    fn mark_failed(&self, queue_id: Uuid, error: &str, max_attempts: u32) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Cancel a message. Returns Err if already processing.
    fn cancel(&self, queue_id: Uuid) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Count messages with status `queued`.
    fn depth(&self) -> impl std::future::Future<Output = Result<usize>> + Send;
}

// ── In-Memory Implementation (for tests and dev) ──────────────────────────────

use std::sync::Mutex;

/// In-memory queue — useful for tests and when PostgreSQL is not available.
pub struct InMemoryQueue {
    messages: Mutex<Vec<QueueMessage>>,
}

impl InMemoryQueue {
    pub fn new() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
        }
    }
}

impl MessageQueue for InMemoryQueue {
    async fn enqueue(&self, payload: serde_json::Value) -> Result<Uuid> {
        let now = Utc::now();
        let msg = QueueMessage {
            queue_id: Uuid::now_v7(),
            payload,
            status: QueueStatus::Queued,
            created_at: now,
            updated_at: now,
            attempts: 0,
            error: None,
        };
        let id = msg.queue_id;
        self.messages.lock().expect("queue poisoned").push(msg);
        Ok(id)
    }

    async fn list_pending(&self) -> Result<Vec<QueueMessage>> {
        let msgs = self.messages.lock().expect("queue poisoned");
        let mut pending: Vec<QueueMessage> = msgs
            .iter()
            .filter(|m| m.status == QueueStatus::Queued || m.status == QueueStatus::Processing)
            .cloned()
            .collect();
        pending.sort_by_key(|m| m.created_at);
        Ok(pending)
    }

    async fn claim_next(&self) -> Result<Option<QueueMessage>> {
        let mut msgs = self.messages.lock().expect("queue poisoned");
        // Find first queued, ordered by created_at
        let idx = msgs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.status == QueueStatus::Queued)
            .min_by_key(|(_, m)| m.created_at)
            .map(|(i, _)| i);

        if let Some(i) = idx {
            msgs[i].status = QueueStatus::Processing;
            msgs[i].updated_at = Utc::now();
            Ok(Some(msgs[i].clone()))
        } else {
            Ok(None)
        }
    }

    async fn mark_completed(&self, queue_id: Uuid) -> Result<()> {
        let mut msgs = self.messages.lock().expect("queue poisoned");
        if let Some(msg) = msgs.iter_mut().find(|m| m.queue_id == queue_id) {
            msg.status = QueueStatus::Completed;
            msg.updated_at = Utc::now();
        }
        Ok(())
    }

    async fn mark_failed(&self, queue_id: Uuid, error: &str, max_attempts: u32) -> Result<()> {
        let mut msgs = self.messages.lock().expect("queue poisoned");
        if let Some(msg) = msgs.iter_mut().find(|m| m.queue_id == queue_id) {
            msg.attempts += 1;
            msg.error = Some(error.to_string());
            msg.updated_at = Utc::now();
            if msg.attempts >= max_attempts {
                msg.status = QueueStatus::Failed;
            } else {
                msg.status = QueueStatus::Queued; // retry
            }
        }
        Ok(())
    }

    async fn cancel(&self, queue_id: Uuid) -> Result<()> {
        let mut msgs = self.messages.lock().expect("queue poisoned");
        if let Some(msg) = msgs.iter_mut().find(|m| m.queue_id == queue_id) {
            if msg.status == QueueStatus::Processing {
                anyhow::bail!("message_already_processing");
            }
            msg.status = QueueStatus::Cancelled;
            msg.updated_at = Utc::now();
        }
        Ok(())
    }

    async fn depth(&self) -> Result<usize> {
        let msgs = self.messages.lock().expect("queue poisoned");
        Ok(msgs.iter().filter(|m| m.status == QueueStatus::Queued).count())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enqueue_and_list_pending() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({"test": 1})).await.unwrap();
        let pending = q.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].queue_id, id);
        assert_eq!(pending[0].status, QueueStatus::Queued);
    }

    #[tokio::test]
    async fn claim_next_transitions_to_processing() {
        let q = InMemoryQueue::new();
        q.enqueue(serde_json::json!({"msg": 1})).await.unwrap();

        let msg = q.claim_next().await.unwrap().unwrap();
        assert_eq!(msg.status, QueueStatus::Processing);
        assert_eq!(q.depth().await.unwrap(), 0); // no longer queued
    }

    #[tokio::test]
    async fn claim_next_returns_none_when_empty() {
        let q = InMemoryQueue::new();
        assert!(q.claim_next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fifo_ordering() {
        let q = InMemoryQueue::new();
        let id1 = q.enqueue(serde_json::json!({"order": 1})).await.unwrap();
        let id2 = q.enqueue(serde_json::json!({"order": 2})).await.unwrap();

        let first = q.claim_next().await.unwrap().unwrap();
        assert_eq!(first.queue_id, id1);

        let second = q.claim_next().await.unwrap().unwrap();
        assert_eq!(second.queue_id, id2);
    }

    #[tokio::test]
    async fn mark_completed() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({})).await.unwrap();
        q.claim_next().await.unwrap();
        q.mark_completed(id).await.unwrap();

        let pending = q.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn mark_failed_retries_below_max() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({})).await.unwrap();
        q.claim_next().await.unwrap();
        q.mark_failed(id, "oops", 3).await.unwrap();

        // Should be back to queued (retry)
        assert_eq!(q.depth().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn mark_failed_final_at_max_attempts() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({})).await.unwrap();
        q.claim_next().await.unwrap();
        // Simulate reaching max_attempts
        q.mark_failed(id, "err1", 2).await.unwrap(); // attempts=1, retry
        q.claim_next().await.unwrap(); // claim again
        q.mark_failed(id, "err2", 2).await.unwrap(); // attempts=2, failed

        assert_eq!(q.depth().await.unwrap(), 0); // not queued
        let pending = q.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0); // not pending
    }

    #[tokio::test]
    async fn cancel_queued_message() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({})).await.unwrap();
        q.cancel(id).await.unwrap();
        assert_eq!(q.depth().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn cancel_processing_message_fails() {
        let q = InMemoryQueue::new();
        let id = q.enqueue(serde_json::json!({})).await.unwrap();
        q.claim_next().await.unwrap(); // now processing
        let err = q.cancel(id).await.unwrap_err();
        assert!(err.to_string().contains("message_already_processing"));
    }

    #[tokio::test]
    async fn depth_counts_only_queued() {
        let q = InMemoryQueue::new();
        q.enqueue(serde_json::json!({})).await.unwrap();
        q.enqueue(serde_json::json!({})).await.unwrap();
        q.claim_next().await.unwrap(); // one is now processing

        assert_eq!(q.depth().await.unwrap(), 1);
    }
}
