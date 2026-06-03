//! `GET /v1/events/stream` — pillar transparency SSE side-channel.
//!
//! Subscribes to `AppState.pillar_tx` and forwards every `PillarEvent` as
//! `data: <json>\n\n`. Optional `run_id` query param filters by run.
//! Drop-oldest on slow client; keep-alive every 15s.
//!
//! Session lifecycle hooks:
//! - On SSE open: calls `handoff::on_session_open(db)` and emits the injection
//!   as the first event if a pending handoff exists.
//! - On SSE close: calls `handoff::should_process_disconnect(db)` and if true,
//!   spawns handoff generation + working→episodic consolidation.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, info, instrument, warn};

use crate::memory::handoff;
use crate::server::AppState;

#[derive(Deserialize, Debug, Default)]
pub struct EventsQuery {
    /// Optional `run_id` filter. When set, only events with matching `run_id`
    /// pass through (events with no `run_id` are dropped).
    pub run_id: Option<String>,
}

#[instrument(skip(state), fields(run_id = ?q.run_id))]
pub async fn stream(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // ── Session open hook: inject handoff if pending ──
    let handoff_injection = state.db.as_ref().and_then(|db| {
        let injection = handoff::on_session_open(db);
        if injection.is_some() {
            info!("handoff injected on SSE session open");
        }
        injection
    });

    let rx = state.pillar_tx.subscribe();
    let filter = q.run_id;
    let pillar_stream = BroadcastStream::new(rx).filter_map(move |item| {
        let filter = filter.clone();
        async move {
            match item {
                Ok(ev) => {
                    if let Some(want) = &filter {
                        match &ev.run_id {
                            Some(got) if got == want => {}
                            _ => return None,
                        }
                    }
                    let json = serde_json::to_string(&ev)
                        .unwrap_or_else(|_| String::from("{\"error\":\"serialize_failed\"}"));
                    Some(Ok(Event::default().data(json)))
                }
                Err(e) => {
                    warn!(error = ?e, "pillar stream lagged");
                    None
                }
            }
        }
    });

    // Prepend handoff injection event if present.
    let initial_events: Vec<Result<Event, Infallible>> = if let Some(injection) = handoff_injection {
        let payload = serde_json::json!({
            "type": "HandoffInjected",
            "content": injection,
        });
        vec![Ok(Event::default().data(payload.to_string()))]
    } else {
        vec![]
    };

    let initial_stream = futures::stream::iter(initial_events);

    // Wrap the combined stream with a drop guard for session close hook.
    // Box::pin is needed because filter_map produces a non-Unpin future.
    let combined: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(initial_stream.chain(pillar_stream));
    let guarded = SessionDropGuard::new(combined, state);

    Sse::new(guarded).keep_alive(KeepAlive::new())
}

// ─── Session Drop Guard ──────────────────────────────────────────────────────

/// A stream wrapper that triggers session-close hooks when the SSE connection
/// is dropped (client disconnects).
struct SessionDropGuard {
    inner: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>,
    state: Option<AppState>,
}

impl SessionDropGuard {
    fn new(inner: Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>, state: AppState) -> Self {
        Self {
            inner,
            state: Some(state),
        }
    }
}

impl Stream for SessionDropGuard {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl Drop for SessionDropGuard {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            // Session close hook: check debounce and spawn background tasks.
            tokio::spawn(async move {
                on_session_close(state).await;
            });
        }
    }
}

/// Session close handler: triggers handoff generation and working→episodic
/// consolidation if the disconnect is not a reconnect within the debounce window.
async fn on_session_close(state: AppState) {
    let Some(ref db) = state.db else {
        debug!("no redb available — skipping session close hooks");
        return;
    };

    if !handoff::should_process_disconnect(db) {
        debug!("SSE disconnect within debounce window — skipping session close tasks");
        return;
    }

    info!("SSE session closed — spawning background maturation tasks");

    // Check if maturation is enabled.
    let maturation_cfg = state
        .tokens
        .as_ref()
        .map(|t| t.memory.maturation.clone())
        .unwrap_or_default();

    if !maturation_cfg.enabled {
        debug!("maturation disabled — skipping session close tasks");
        return;
    }

    // Trigger consolidation via the consolidator state command channel.
    if let Some(ref cs) = state.consolidator_state {
        use crate::memory::consolidator::ConsolidationCmd;
        if let Err(e) = cs.cmd_tx.send(ConsolidationCmd::TriggerNow).await {
            warn!(error = %e, "failed to send TriggerNow to consolidation timer");
        } else {
            debug!("triggered working→episodic consolidation on session close");
        }
    }

    // Note: Handoff generation requires session messages which are not available
    // at the SSE layer. The handoff is generated by the agent loop on session end
    // (when messages are available). This hook ensures consolidation is triggered.
    // If a higher-level component (e.g., agent manager) has session messages,
    // it should call generate_handoff directly.
}
