//! Telemetry — append-only audit JSONL for offline analysis.
//!
//! Events written here land in `~/.local/share/kira/audit/pillars.jsonl`
//! and are meant for Grafana / training-data export. They are **not**
//! streamed to the REPL.

use chrono::Utc;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{debug, warn};

use crate::proto::pillar::PillarEvent;

/// Write a pillar event to the audit JSONL file.
/// Failures are silently logged (best-effort).
pub fn append(ev: &PillarEvent) {
    if let Err(e) = append_inner(ev) {
        debug!(error = ?e, "telemetry audit append failed (non-fatal)");
    }
}

/// `~/.local/share/kira/audit/pillars.jsonl` (cached on first call).
fn audit_path() -> Option<&'static PathBuf> {
    static AUDIT: OnceLock<Option<PathBuf>> = OnceLock::new();
    AUDIT
        .get_or_init(|| {
            let home = std::env::var("HOME").ok()?;
            let dir = PathBuf::from(home).join(".local/share/kira/audit");
            if let Err(e) = std::fs::create_dir_all(&dir) {
                warn!(error = ?e, dir = %dir.display(), "telemetry audit dir create failed");
                return None;
            }
            Some(dir.join("pillars.jsonl"))
        })
        .as_ref()
}

fn append_inner(ev: &PillarEvent) -> std::io::Result<()> {
    use std::io::Write;
    let Some(path) = audit_path() else {
        return Ok(()); // disabled
    };
    let line = serde_json::to_string(ev)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")
}

/// Convenience: append a raw event without building a `PillarEvent` first.
pub fn append_raw(
    pillar: crate::proto::pillar::Pillar,
    phase: &str,
    message: impl Into<String>,
    run_id: Option<String>,
    agent_id: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
) {
    let ev = PillarEvent {
        pillar,
        phase: phase.to_string(),
        message: message.into(),
        run_id,
        agent_id,
        ts: Utc::now().to_rfc3339(),
        fields,
    };
    append(&ev);
}
