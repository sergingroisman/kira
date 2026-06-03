//! Audit trail logger — writes ACP envelopes as JSONL lines to
//! `audit/acp-{YYYYMMDD}.jsonl`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut logger = AuditLogger::new("audit");
//! logger.log(&Envelope::new("my.channel", serde_json::json!({"ok": true}))).await?;
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncWriteExt, BufWriter};
use uuid::Uuid;

/// Envelope wrapping an ACP message for the audit trail.
///
/// Every envelope carries a [`Uuid`] v7 (time‑sortable), an ISO‑8601 timestamp,
/// a logical channel identifier, and an arbitrary JSON payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Unique message identifier (UUID v7, time‑ordered).
    pub id: Uuid,
    /// ISO‑8601 timestamp (RFC 3339) when the envelope was created.
    pub timestamp: String,
    /// Logical channel / topic (e.g. `"llm.call"`, `"tool.exec"`).
    pub channel: String,
    /// Free‑form payload (arbitrary JSON value).
    pub payload: serde_json::Value,
}

impl Envelope {
    /// Create a new envelope, auto‑generating a UUID v7 and the current
    /// timestamp.
    #[must_use]
    pub fn new(channel: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::now_v7(),
            timestamp: Utc::now().to_rfc3339(),
            channel: channel.into(),
            payload,
        }
    }
}

/// Lazy audit‑logger that writes JSONL lines to
/// `{base_dir}/acp-{YYYYMMDD}.jsonl`.
///
/// The directory and file are created on the first call to [`log`].  If the
/// calendar date rolls over the file is automatically rotated (previous file
/// is closed; new one opened and appended to).
pub struct AuditLogger {
    /// Base directory for audit logs (default: `"audit"`).
    base_dir: PathBuf,
    /// Cached buffered writer — re‑opened when the date changes.
    writer: Option<BufWriter<tokio::fs::File>>,
    /// Date‑stamp (`YYYYMMDD`) of the currently open file.
    current_date: Option<String>,
}

impl AuditLogger {
    /// Create a new `AuditLogger` rooted at `base_dir`.
    ///
    /// No I/O is performed until [`log`](Self::log) is called.
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            writer: None,
            current_date: None,
        }
    }

    /// Return the date‑specific file path: `{base_dir}/acp-{date}.jsonl`.
    fn file_path(&self, date: &str) -> PathBuf {
        self.base_dir.join(format!("acp-{date}.jsonl"))
    }

    /// Append a single envelope as a JSONL line to today's file.
    ///
    /// On the first call (or when the date rolls over):
    /// - Creates `{base_dir}/` (and parents) via `create_dir_all`.
    /// - Opens (or creates) `acp-{YYYYMMDD}.jsonl` in append mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created, the file cannot be
    /// opened, or serialisation fails.
    pub async fn log(&mut self, envelope: &Envelope) -> anyhow::Result<()> {
        let today = Utc::now().format("%Y%m%d").to_string();

        // Re‑open if the date changed or no file is open yet.
        if self.current_date.as_ref().is_none_or(|d| *d != today) {
            tokio::fs::create_dir_all(&self.base_dir).await?;

            let path = self.file_path(&today);
            let file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await?;
            self.writer = Some(BufWriter::new(file));
            self.current_date = Some(today);
        }

        let line = serde_json::to_string(envelope)?;
        let writer = self.writer.as_mut().expect("writer should be initialised");

        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    /// Verify that `log` creates the directory and appends a valid JSONL line.
    #[tokio::test]
    async fn log_creates_dir_and_appends() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut logger = AuditLogger::new(dir.path());

        let envelope = Envelope::new("test.channel", serde_json::json!({"key": "value"}));
        logger.log(&envelope).await.expect("log");

        let today = Utc::now().format("%Y%m%d");
        let path = dir.path().join(format!("acp-{today}.jsonl"));

        let content = fs::read_to_string(&path).await.expect("read back");
        let parsed: Envelope = serde_json::from_str(content.trim()).expect("parse");

        assert_eq!(parsed.channel, "test.channel");
        assert_eq!(parsed.payload, serde_json::json!({"key": "value"}));
        assert_eq!(parsed.id, envelope.id);
        assert_eq!(parsed.timestamp, envelope.timestamp);
    }

    /// Verify that two `log` calls produce two separate JSONL lines.
    #[tokio::test]
    async fn log_appends_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut logger = AuditLogger::new(dir.path());

        let e1 = Envelope::new("ch1", serde_json::json!(1));
        let e2 = Envelope::new("ch2", serde_json::json!(2));

        logger.log(&e1).await.expect("log e1");
        logger.log(&e2).await.expect("log e2");

        let today = Utc::now().format("%Y%m%d");
        let path = dir.path().join(format!("acp-{today}.jsonl"));
        let content = fs::read_to_string(&path).await.expect("read back");
        let lines: Vec<&str> = content.trim().lines().collect();

        assert_eq!(lines.len(), 2, "expected 2 JSONL lines");
    }
}
