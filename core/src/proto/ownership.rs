//! Ownership registry wire types.
//!
//! Each agent claims a set of glob patterns under one of three modes.
//! A write to a path is allowed iff no OTHER active agent has an `Exclusive`
//! claim covering it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
#[serde(rename_all = "snake_case")]
pub enum OwnershipMode {
    /// Only this agent may write paths matching the patterns.
    Exclusive,
    /// Multiple agents may write; no protection.
    Shared,
    /// Agent is granted read access only.
    ReadOnly,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
pub struct ClaimRequest {
    pub agent_id: String,
    pub patterns: Vec<String>,
    pub mode: OwnershipMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
pub struct Claim {
    pub agent_id: String,
    pub patterns: Vec<String>,
    pub mode: OwnershipMode,
    pub created_at: String,
}

#[derive(Deserialize, Debug, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
pub struct CheckRequest {
    pub agent_id: String,
    pub path: String,
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum WriteDecision {
    Allowed,
    Denied { reason: String, conflicting_agent: Option<String> },
}

#[derive(Serialize, TS)]
#[ts(export, export_to = "../../agent/src/generated/ownership.ts")]
pub struct ReleaseResponse {
    pub released: bool,
    pub agent_id: String,
}
