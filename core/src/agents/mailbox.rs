//! FS-backed mailbox for agent-to-agent (and core ↔ agent) messaging.
//!
//! Layout (per plan T5):
//!   <`repo_root>/.kira/mailbox`/<`agent_id`>/
//!     ├── inbox/           pending messages addressed to <`agent_id`>
//!     ├── outbox/          audit copy of messages sent BY <`agent_id`>
//!     └── inbox/.acked/    consumed messages (out of polling)
//!
//! Atomicity: write to `inbox/.tmp-<uuid>` then rename to `inbox/<id>.json`.
//! Polling: `poll(agent_id)` reads `inbox/*.json` (excluding `.acked/`).
//! TTL cleanup is a v0.5 watchdog — for now nothing is auto-deleted.

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::proto::mailbox::{Message, SendRequest};

const MAILBOX_SUBDIR: &str = ".kira/mailbox";

#[derive(Clone)]
pub struct MailboxFs {
    repo_root: PathBuf,
}

impl MailboxFs {
    pub const fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    fn agent_dir(&self, agent_id: &str) -> PathBuf {
        self.repo_root.join(MAILBOX_SUBDIR).join(agent_id)
    }

    fn inbox(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("inbox")
    }

    fn outbox(&self, agent_id: &str) -> PathBuf {
        self.agent_dir(agent_id).join("outbox")
    }

    fn acked(&self, agent_id: &str) -> PathBuf {
        self.inbox(agent_id).join(".acked")
    }

    async fn ensure_dirs(&self, agent_id: &str) -> Result<()> {
        fs::create_dir_all(self.inbox(agent_id)).await?;
        fs::create_dir_all(self.outbox(agent_id)).await?;
        fs::create_dir_all(self.acked(agent_id)).await?;
        Ok(())
    }

    /// Atomic send from `req.from` → `to`. Returns message id + final path.
    #[instrument(skip(self, req), fields(from = %req.from, to = %to, kind = %req.kind))]
    pub async fn send(&self, to: &str, req: SendRequest) -> Result<(String, PathBuf)> {
        self.ensure_dirs(to).await?;
        // Outbox dir only created for the SENDER (if it's an agent).
        if req.from != "core" {
            fs::create_dir_all(self.outbox(&req.from)).await?;
        }

        let id = Uuid::now_v7().to_string();
        let msg = Message {
            id: id.clone(),
            from: req.from.clone(),
            to: to.to_string(),
            kind: req.kind.clone(),
            payload: req.payload.clone(),
            created_at: Utc::now().to_rfc3339(),
        };
        let body = serde_json::to_vec_pretty(&msg)?;

        // Write atomic into inbox.
        let inbox = self.inbox(to);
        let tmp = inbox.join(format!(".tmp-{id}"));
        let final_path = inbox.join(format!("{id}.json"));
        fs::write(&tmp, &body)
            .await
            .with_context(|| format!("tmp write failed: {}", tmp.display()))?;
        fs::rename(&tmp, &final_path)
            .await
            .with_context(|| format!("rename failed: {}", final_path.display()))?;

        // Best-effort audit copy in sender's outbox.
        if req.from != "core" {
            let outbox = self.outbox(&req.from).join(format!("{id}.json"));
            let _ = fs::write(&outbox, &body).await;
        }

        debug!(id = %id, path = %final_path.display(), "mailbox send");
        Ok((id, final_path))
    }

    /// Returns all pending messages in `agent_id`'s inbox (excluding `.acked/`).
    #[instrument(skip(self), fields(agent_id = %agent_id))]
    pub async fn poll(&self, agent_id: &str) -> Result<Vec<Message>> {
        let inbox = self.inbox(agent_id);
        if !inbox.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&inbox).await?;
        let mut messages: Vec<Message> = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            // Skip the .acked/ directory and any tmp files.
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            match parse_message(&path).await {
                Ok(msg) => messages.push(msg),
                Err(e) => debug!(?path, error = ?e, "skipping unparseable message"),
            }
        }
        // Sort by created_at (UUID v7 is time-ordered but we sort the parsed field).
        messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(messages)
    }

    /// Move a message from `inbox/<id>.json` to `inbox/.acked/<id>.json`.
    /// Returns `true` if the message existed (and was moved), `false` otherwise.
    #[instrument(skip(self), fields(agent_id = %agent_id, id = %id))]
    pub async fn ack(&self, agent_id: &str, id: &str) -> Result<bool> {
        let inbox = self.inbox(agent_id);
        let src = inbox.join(format!("{id}.json"));
        if !src.exists() {
            return Ok(false);
        }
        let acked_dir = self.acked(agent_id);
        fs::create_dir_all(&acked_dir).await?;
        let dst = acked_dir.join(format!("{id}.json"));
        fs::rename(&src, &dst).await?;
        Ok(true)
    }
}

async fn parse_message(path: &Path) -> Result<Message> {
    let body = fs::read(path).await?;
    let msg: Message = serde_json::from_slice(&body)?;
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir_unique(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("kira-mailbox-{label}-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn req(from: &str, kind: &str, payload: serde_json::Value) -> SendRequest {
        SendRequest {
            from: from.to_string(),
            kind: kind.to_string(),
            payload,
        }
    }

    #[tokio::test]
    async fn send_then_poll_returns_message() {
        let root = tempdir_unique("send-poll");
        let mb = MailboxFs::new(root.clone());
        let (id, path) = mb
            .send("agent-b", req("agent-a", "task", serde_json::json!({"goal": "x"})))
            .await
            .unwrap();
        assert!(path.exists(), "message file should exist on disk");
        let pending = mb.poll("agent-b").await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert_eq!(pending[0].from, "agent-a");
        assert_eq!(pending[0].kind, "task");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn ack_moves_to_acked_subdir() {
        let root = tempdir_unique("ack");
        let mb = MailboxFs::new(root.clone());
        let (id, _) = mb
            .send("agent-b", req("core", "ping", serde_json::json!(null)))
            .await
            .unwrap();
        assert!(mb.ack("agent-b", &id).await.unwrap());
        let pending = mb.poll("agent-b").await.unwrap();
        assert!(pending.is_empty(), "polled inbox should be empty after ack");
        let acked = mb.acked("agent-b").join(format!("{id}.json"));
        assert!(acked.exists(), "acked file should exist in .acked/");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn poll_returns_messages_in_created_at_order() {
        let root = tempdir_unique("order");
        let mb = MailboxFs::new(root.clone());
        let (id1, _) = mb.send("x", req("a", "t1", serde_json::json!(1))).await.unwrap();
        // Sleep a hair so created_at differs at millisecond resolution.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let (id2, _) = mb.send("x", req("a", "t2", serde_json::json!(2))).await.unwrap();
        let msgs = mb.poll("x").await.unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].id, id1);
        assert_eq!(msgs[1].id, id2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn ack_missing_returns_false() {
        let root = tempdir_unique("missing-ack");
        let mb = MailboxFs::new(root.clone());
        // Polling a never-used inbox creates nothing and returns empty.
        let pending = mb.poll("ghost").await.unwrap();
        assert!(pending.is_empty());
        assert!(!mb.ack("ghost", "no-such-id").await.unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn skips_dotfiles_and_acked_dir_in_poll() {
        let root = tempdir_unique("skip-dot");
        let mb = MailboxFs::new(root.clone());
        let (id, _) = mb.send("x", req("a", "k", serde_json::json!({}))).await.unwrap();
        // Drop a `.hidden` file directly in the inbox; poll must ignore it.
        let inbox = mb.inbox("x");
        std::fs::write(inbox.join(".hidden"), b"junk").unwrap();
        let msgs = mb.poll("x").await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].id, id);
        let _ = std::fs::remove_dir_all(&root);
    }
}
