//! SC2 — Tool-call approval gate for HIGH+ risk tools.
//!
//! v0 ships agent-level approval: when a HIGH-risk tool is invoked the loop
//! pauses and waits for an external `POST /v1/agents/:id/approval` call.
//! CRITICAL tools are rejected immediately (no approval path).
//!
//! The gate uses `tokio::sync::watch` so the agent loop can `await` changes
//! without busy-polling.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::debug;
use uuid::Uuid;

/// Verdict for a pending tool approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalVerdict {
    Pending,
    Approved,
    Rejected,
}

impl ApprovalVerdict {
    pub const fn is_approved(self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// Tracks pending approvals per agent.
///
/// Each agent can have at most one pending approval at a time. When a new
/// request is made the old channel is dropped (the agent would already have
/// finished or timed out).
#[derive(Default, Clone)]
pub struct ApprovalGate {
    txs: Arc<std::sync::Mutex<HashMap<Uuid, watch::Sender<ApprovalVerdict>>>>,
}

impl ApprovalGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new approval request for `agent_id`. Returns the receiver
    /// the agent loop should await on.
    pub fn request(&self, agent_id: Uuid) -> watch::Receiver<ApprovalVerdict> {
        let (tx, rx) = watch::channel(ApprovalVerdict::Pending);
        let mut map = self.txs.lock().expect("approval txs poisoned");
        map.insert(agent_id, tx);
        rx
    }

    /// Respond to a pending approval (approve or reject).
    /// Returns `true` if the agent had a pending request.
    pub fn respond(&self, agent_id: Uuid, approved: bool) -> bool {
        let mut map = self.txs.lock().expect("approval txs poisoned");
        if let Some(tx) = map.remove(&agent_id) {
            let verdict = if approved {
                ApprovalVerdict::Approved
            } else {
                ApprovalVerdict::Rejected
            };
            let _ = tx.send(verdict);
            debug!(agent_id = %agent_id, approved, "approval verdict sent");
            true
        } else {
            false
        }
    }

    /// Clean up an entry when the agent finishes or is cancelled.
    pub fn remove(&self, agent_id: Uuid) {
        let mut map = self.txs.lock().expect("approval txs poisoned");
        map.remove(&agent_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_then_approve() {
        let gate = ApprovalGate::new();
        let rx = gate.request(Uuid::now_v7());
        assert_eq!(*rx.borrow(), ApprovalVerdict::Pending);

        let id = Uuid::now_v7();
        let mut rx = gate.request(id);
        gate.respond(id, true);

        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ApprovalVerdict::Approved);
    }

    #[tokio::test]
    async fn request_then_reject() {
        let gate = ApprovalGate::new();
        let id = Uuid::now_v7();
        let mut rx = gate.request(id);
        gate.respond(id, false);

        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ApprovalVerdict::Rejected);
    }

    #[test]
    fn respond_no_pending_returns_false() {
        let gate = ApprovalGate::new();
        assert!(!gate.respond(Uuid::now_v7(), true));
    }
}
