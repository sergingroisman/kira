//! Wire types — JSON schemas for `/v1/*` endpoints.
//!
//! v0.5+: annotate with `#[derive(TS)]` to generate `.d.ts` for the Bun REPL.

pub mod agent;
pub mod bash;
pub mod chat;
pub mod file;
pub mod mailbox;
pub mod ownership;
pub mod worktree;
