//! Mailbox wire types — agent ↔ agent (and core → agent) FS-backed messaging.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/mailbox.ts")]
pub struct SendRequest {
    /// Sender's agent id (or "core" for daemon-originated messages).
    pub from: String,
    /// Free-form message kind ("task", "result", "heartbeat", "broadcast").
    pub kind: String,
    /// Opaque payload (caller-defined JSON).
    pub payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/mailbox.ts")]
pub struct Message {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub payload: serde_json::Value,
    /// ISO 8601 UTC.
    pub created_at: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/mailbox.ts")]
pub struct SendResponse {
    pub id: String,
    pub path: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/mailbox.ts")]
pub struct AckResponse {
    pub acked: bool,
    pub id: String,
}
