//! Agents subsystem.
//! v0 ships only `mailbox` (no agent loop yet — that lands Sem 9).
//! v0.5+: agent kinds (writer/deployer/judge), ownership claim integration,
//!        Pilot/Orchestrator hierarchy.

pub mod mailbox;
pub mod ownership;
