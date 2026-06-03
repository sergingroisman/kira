//! Observability helpers — pillar transparency, audit JSONL.
//!
//! **Ryuk** — codename for the observability subsystem (Death Note: the shinigami
//! who watches silently). Ryuk = pillar events (SSE + JSONL) + audit + bundle hash.
//!
//! `pillar` exports `emit*` helpers that fan-out a `PillarEvent` to
//! all SSE subscribers AND append to `~/.local/share/kira/audit/pillars.jsonl`.
//!
//! v0.5 split:
//!   - `wire` — SSE broadcast channel consumed by the REPL.
//!   - `telemetry` — append-only JSONL for offline analysis / training data.

pub mod bundle;
pub mod guardrail_events;
pub mod otel;
pub mod pillar;
pub mod telemetry;
pub mod wire;

/// Ryuk = alias for the observability subsystem (docs / UI naming).
/// Use `crate::observability::pillar` for all runtime calls.
pub use self as ryuk;
