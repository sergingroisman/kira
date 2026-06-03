//! Wire — SSE broadcast channel for REPL / UI events.
//!
//! Events sent here are consumed by `GET /v1/events/stream` and rendered
//! as breadcrumbs in the REPL. They are **not** persisted.

use tokio::sync::broadcast;
use tracing::debug;

use crate::proto::pillar::PillarEvent;

pub type PillarTx = broadcast::Sender<PillarEvent>;

/// Build a fresh broadcast channel with capacity 1024 (drop-oldest).
pub fn channel() -> PillarTx {
    let (tx, _rx) = broadcast::channel(1024);
    tx
}

/// Send an event to all active SSE subscribers.
/// "No subscribers" is silently ignored — that's normal when the REPL
/// is not connected.
pub fn send(tx: &PillarTx, ev: PillarEvent) {
    if let Err(e) = tx.send(ev) {
        debug!(error = ?e, "wire broadcast dropped (no subscribers)");
    }
}
