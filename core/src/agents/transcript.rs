// Durable transcript per agent run.
// Layout:
//   <runs_dir>/agents/<agent_id>/
//     context.jsonl     — one JSON line per ChatMessageUnified
//     meta.json         — Agent struct snapshot (final)
//     prompt.txt        — system prompt snapshot

use std::path::{Path, PathBuf};
use tracing::warn;
use uuid::Uuid;

use crate::llm::types::ChatMessageUnified;
use crate::proto::agent::Agent;

const TRANSCRIPT_SUBDIR: &str = "agents";

#[derive(Clone)]
pub struct TranscriptWriter {
    dir: PathBuf,
}

impl TranscriptWriter {
    /// Create a `TranscriptWriter` rooted at `runs_dir/agents/<agent_id>/`.
    /// Runs dir follows `.kira/runs/` convention.
    pub fn new(runs_dir: &Path, agent_id: Uuid) -> Self {
        let dir = runs_dir.join(TRANSCRIPT_SUBDIR).join(agent_id.to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warn!(error = ?e, path = %dir.display(), "transcript dir create failed");
        }
        Self { dir }
    }

    fn context_path(&self) -> PathBuf {
        self.dir.join("context.jsonl")
    }

    fn meta_path(&self) -> PathBuf {
        self.dir.join("meta.json")
    }

    fn prompt_path(&self) -> PathBuf {
        self.dir.join("prompt.txt")
    }

    /// Append a single message to context.jsonl (append-only).
    pub fn append(&self, msg: &ChatMessageUnified) {
        let line = match serde_json::to_string(msg) {
            Ok(l) => l,
            Err(e) => {
                warn!(error = ?e, "transcript append serialize failed");
                return;
            }
        };
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.context_path())
        {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
    }

    /// Write the final Agent snapshot as meta.json.
    pub fn write_meta(&self, agent: &Agent) {
        let json = match serde_json::to_string_pretty(agent) {
            Ok(j) => j,
            Err(e) => {
                warn!(error = ?e, "transcript meta serialize failed");
                return;
            }
        };
        let _ = std::fs::write(self.meta_path(), json);
    }

    /// Write the system prompt snapshot (prompt.txt).
    pub fn write_prompt(&self, prompt: &str) {
        let _ = std::fs::write(self.prompt_path(), prompt);
    }

    /// Path to context.jsonl (for reading back).
    pub fn context_path_ref(&self) -> PathBuf {
        self.context_path()
    }
}

/// Restore conversation context from a previous agent run.
/// Returns `None` if no context.jsonl exists or it's unreadable.
pub fn restore_context(runs_dir: &Path, agent_id: Uuid) -> Option<Vec<ChatMessageUnified>> {
    let path = runs_dir
        .join(TRANSCRIPT_SUBDIR)
        .join(agent_id.to_string())
        .join("context.jsonl");
    let content = std::fs::read_to_string(&path).ok()?;
    let messages: Vec<ChatMessageUnified> = content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    if messages.is_empty() {
        None
    } else {
        Some(messages)
    }
}
