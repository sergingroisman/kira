//! Agents subsystem.
//! v0 ships only `mailbox` (no agent loop yet — that lands Sem 9).
//! v0.5+: agent kinds (writer/deployer/judge), ownership claim integration,
//!        Pilot/Orchestrator hierarchy.

pub mod ast;
pub mod code_search;
pub mod detective;
pub mod inject;
pub mod mailbox;
pub mod ownership;
pub mod spawn;
pub mod store;
pub mod tools;
pub mod transcript;
pub mod truncate;

pub use detective::DetectiveRunner;
pub use spawn::AgentManager;
