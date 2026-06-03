#![forbid(unsafe_code)]

//! SSE Listener — connects to GET /v1/events/stream and dispatches events.
//!
//! Runs as a background Tokio task. Communicates with the event loop
//! exclusively through `mpsc::Sender<UiMessage>`. Reconnects with exponential
//! backoff (1s → 60s) on disconnect. Detects stale connections (90s without
//! any event) and reconnects. Respects `CancellationToken` for graceful shutdown.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::backoff::ExponentialBackoff;
use crate::core_client::CoreClient;
use crate::messages::{ApprovalRequest, UiMessage};

/// Stale connection threshold: if no event received for this duration, reconnect.
const STALE_TIMEOUT: Duration = Duration::from_secs(90);

/// Path for the SSE events stream endpoint.
const PATH_EVENTS_STREAM: &str = "/v1/events/stream";

// ── Event types ───────────────────────────────────────────────────────────────

/// SSE event types emitted by the Core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PillarEventKind {
    BudgetWarn {
        pct: u8,
    },
    BudgetStop,
    GuardrailTriggered {
        level: String,
        reason: String,
        agent_id: Option<String>,
    },
    MemoryInjected {
        fragment_count: u32,
        agent_id: Option<String>,
    },
    QueueDepthChanged {
        depth: u32,
    },
    Heartbeat {
        agent_id: String,
    },
    AwaitingApproval {
        agent_id: String,
        tool_name: String,
        risk_level: String,
        arguments_preview: String,
        timeout_secs: u64,
    },
    /// Generic pipeline activity parsed from a raw `PillarEvent` (the Core
    /// emits these as nameless `data:` SSE frames). Drives the Layers View.
    Activity {
        pillar: String,
        phase: String,
        message: String,
    },
    /// A tool started — pushes a Running (spinner) activity chip.
    ToolActivityStart { tool: String, target: String },
    /// A tool finished — flips the Running chip to ✓/✗ with detail.
    ToolActivity {
        tool: String,
        target: String,
        ok: bool,
        detail: Vec<String>,
    },
    /// Agent reached a terminal state — pushed over SSE so the TUI updates
    /// instantly instead of waiting for the 2s status poll.
    AgentTerminal {
        agent_id: String,
        status: String,
        result: Option<String>,
        error: Option<String>,
    },
    Unknown(String),
}

/// Parse a raw `PillarEvent` JSON frame (no `event:` name) into an `Activity`.
/// The Core's `/v1/events/stream` serializes `PillarEvent` directly, so this is
/// the path that actually feeds the Layers View during a normal turn.
fn parse_raw_pillar_event(data: &str) -> Option<PillarEventKind> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let pillar = v.get("pillar")?.as_str()?.to_string();
    let phase = v
        .get("phase")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string();

    // Terminal push: the Core emits a wire-only PillarEvent with
    // phase="agent_terminal" carrying status/result/error in `fields`.
    if phase == "agent_terminal"
        && let Some(agent_id) = v.get("agent_id").and_then(|a| a.as_str())
    {
        let fields = v.get("fields");
        let get = |k: &str| {
            fields
                .and_then(|f| f.get(k))
                .and_then(|x| x.as_str())
                .map(str::to_string)
        };
        return Some(PillarEventKind::AgentTerminal {
            agent_id: agent_id.to_string(),
            status: get("status").unwrap_or_default(),
            result: get("result"),
            error: get("error"),
        });
    }

    // Tool started → Running chip.
    if phase == "tool_start"
        && let Some(fields) = v.get("fields")
    {
        let tool = fields.get("tool").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if !tool.is_empty() {
            return Some(PillarEventKind::ToolActivityStart {
                tool,
                target: fields.get("target").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            });
        }
    }

    // Tool finished → activity-log chip.
    if phase == "tool_result"
        && let Some(fields) = v.get("fields")
    {
        let tool = fields.get("tool").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if !tool.is_empty() {
            let detail = fields
                .get("detail")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            return Some(PillarEventKind::ToolActivity {
                tool,
                target: fields.get("target").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                ok: fields.get("ok").and_then(serde_json::Value::as_bool).unwrap_or(true),
                detail,
            });
        }
    }

    // HIGH-risk tool approval gate — the Core bridges it here so the TUI can
    // pop the approval modal (the typed AgentEvent isn't on the SSE stream).
    if phase == "awaiting_approval"
        && let Some(agent_id) = v.get("agent_id").and_then(|a| a.as_str())
    {
        let fields = v.get("fields");
        let fget = |k: &str| fields.and_then(|f| f.get(k)).and_then(|x| x.as_str());
        return Some(PillarEventKind::AwaitingApproval {
            agent_id: agent_id.to_string(),
            tool_name: fget("tool_name").unwrap_or("").to_string(),
            risk_level: fget("risk_level").unwrap_or("HIGH").to_string(),
            arguments_preview: fget("arguments").unwrap_or("").chars().take(2048).collect(),
            timeout_secs: fields
                .and_then(|f| f.get("timeout_secs"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(120),
        });
    }

    let message = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    Some(PillarEventKind::Activity {
        pillar,
        phase,
        message,
    })
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse a raw SSE event into a typed event.
pub fn parse_event(event_type: &str, data: &str) -> PillarEventKind {
    match event_type {
        "BudgetWarn" => {
            #[allow(clippy::cast_possible_truncation)]
            let pct = serde_json::from_str::<serde_json::Value>(data)
                .ok()
                .and_then(|v| v.get("pct")?.as_u64())
                .unwrap_or(0) as u8;
            PillarEventKind::BudgetWarn { pct }
        }
        "BudgetStop" => PillarEventKind::BudgetStop,
        "GuardrailTriggered" => {
            let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
            PillarEventKind::GuardrailTriggered {
                level: v
                    .get("level")
                    .and_then(|l| l.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                reason: v
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string(),
                agent_id: v.get("agent_id").and_then(|a| a.as_str()).map(String::from),
            }
        }
        "MemoryInjected" => {
            let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
            #[allow(clippy::cast_possible_truncation)]
            let fragment_count = v
                .get("fragment_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            PillarEventKind::MemoryInjected {
                fragment_count,
                agent_id: v.get("agent_id").and_then(|a| a.as_str()).map(String::from),
            }
        }
        "QueueDepthChanged" => {
            let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
            #[allow(clippy::cast_possible_truncation)]
            let depth = v
                .get("depth")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            PillarEventKind::QueueDepthChanged { depth }
        }
        "Heartbeat" => {
            let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
            PillarEventKind::Heartbeat {
                agent_id: v
                    .get("agent_id")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string(),
            }
        }
        "AwaitingApproval" => {
            let v: serde_json::Value = serde_json::from_str(data).unwrap_or_default();
            PillarEventKind::AwaitingApproval {
                agent_id: v
                    .get("agent_id")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string(),
                tool_name: v
                    .get("tool_name")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                risk_level: v
                    .get("risk_level")
                    .and_then(|r| r.as_str())
                    .unwrap_or("HIGH")
                    .to_string(),
                arguments_preview: v
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(2048)
                    .collect(),
                timeout_secs: v
                    .get("timeout_secs")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(120),
            }
        }
        other => PillarEventKind::Unknown(other.to_string()),
    }
}

// ── Listener ──────────────────────────────────────────────────────────────────

/// Spawn the SSE listener as a background task.
///
/// Connects to `{base_url}/v1/events/stream` and sends parsed events through
/// the channel. Reconnects with exponential backoff on failure. Detects stale
/// connections (90s without any event). Respects `CancellationToken` for
/// graceful shutdown.
pub fn spawn_listener(
    tx: mpsc::Sender<UiMessage>,
    client: CoreClient,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = ExponentialBackoff::new(1, 60);

        loop {
            if cancel.is_cancelled() {
                debug!("SSE: shutdown requested, exiting listener");
                return;
            }

            let url = format!("{}{}", client.base_url(), PATH_EVENTS_STREAM);
            info!(url = %url, "SSE: connecting");

            match connect_and_listen(&url, &tx, &cancel).await {
                Ok(()) => {
                    // Clean disconnect (stream ended normally) — reconnect with backoff
                    debug!("SSE: stream ended cleanly, will reconnect");
                    let _ = tx
                        .send(UiMessage::CoreDisconnected {
                            reason: "SSE stream ended".to_string(),
                        })
                        .await;
                }
                Err(SseError::Cancelled) => {
                    debug!("SSE: cancelled during connection");
                    return;
                }
                Err(SseError::Stale) => {
                    warn!("SSE: connection stale (no event for 90s), reconnecting");
                    let _ = tx
                        .send(UiMessage::CoreDisconnected {
                            reason: "connection stale (90s timeout)".to_string(),
                        })
                        .await;
                }
                Err(SseError::Connection(e)) => {
                    warn!(error = %e, "SSE: connection failed");
                    let _ = tx
                        .send(UiMessage::CoreDisconnected {
                            reason: format!("SSE error: {e}"),
                        })
                        .await;
                }
            }

            // Backoff before reconnecting
            let delay = backoff.next_delay();
            debug!(?delay, "SSE: waiting before reconnect");

            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = cancel.cancelled() => {
                    debug!("SSE: shutdown during backoff");
                    return;
                }
            }
        }
    })
}

// ── Internal error type ───────────────────────────────────────────────────────

enum SseError {
    /// The cancellation token was triggered.
    Cancelled,
    /// No event received within the stale timeout.
    Stale,
    /// Network or parse error.
    Connection(anyhow::Error),
}

impl From<anyhow::Error> for SseError {
    fn from(e: anyhow::Error) -> Self {
        Self::Connection(e)
    }
}

// ── Connection logic ──────────────────────────────────────────────────────────

/// Connect to the SSE stream and process events until disconnect or cancellation.
async fn connect_and_listen(
    url: &str,
    tx: &mpsc::Sender<UiMessage>,
    cancel: &CancellationToken,
) -> Result<(), SseError> {
    use futures::StreamExt;

    let http_client = reqwest::Client::new();

    let response = tokio::select! {
        result = http_client
            .get(url)
            .header("Accept", "text/event-stream")
            .send() => {
            result.map_err(|e| SseError::Connection(e.into()))?
        }
        () = cancel.cancelled() => {
            return Err(SseError::Cancelled);
        }
        () = tokio::time::sleep(Duration::from_secs(5)) => {
            return Err(SseError::Connection(anyhow::anyhow!("SSE: connection timeout (5s)")));
        }
    };

    if !response.status().is_success() {
        return Err(SseError::Connection(anyhow::anyhow!(
            "SSE: server returned {}",
            response.status()
        )));
    }

    // Connection successful — notify event loop and reset backoff is handled by caller
    let _ = tx
        .send(UiMessage::CoreConnected {
            version: "sse".to_string(),
        })
        .await;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut event_type = String::new();
    let mut last_event_at = Instant::now();

    loop {
        let stale_deadline = tokio::time::sleep_until(last_event_at + STALE_TIMEOUT);

        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        last_event_at = Instant::now();
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // Process complete SSE messages (double newline separated)
                        while let Some(pos) = buffer.find("\n\n") {
                            let message = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            let mut data = String::new();
                            for line in message.lines() {
                                if let Some(et) = line.strip_prefix("event: ") {
                                    event_type = et.trim().to_string();
                                } else if let Some(d) = line.strip_prefix("data: ") {
                                    data = d.trim().to_string();
                                }
                            }

                            if !event_type.is_empty() {
                                let parsed = parse_event(&event_type, &data);
                                dispatch_event(&parsed, tx).await;
                                event_type.clear();
                            } else if !data.is_empty() {
                                // Nameless frame = raw PillarEvent (the Core's
                                // default for pillar transparency events).
                                if let Some(parsed) = parse_raw_pillar_event(&data) {
                                    dispatch_event(&parsed, tx).await;
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        return Err(SseError::Connection(e.into()));
                    }
                    None => {
                        // Stream ended
                        return Ok(());
                    }
                }
            }
            () = stale_deadline => {
                return Err(SseError::Stale);
            }
            () = cancel.cancelled() => {
                return Err(SseError::Cancelled);
            }
        }
    }
}

/// Dispatch a parsed event as a `UiMessage` through the channel.
async fn dispatch_event(event: &PillarEventKind, tx: &mpsc::Sender<UiMessage>) {
    let msg = match event {
        PillarEventKind::AwaitingApproval {
            agent_id,
            tool_name,
            risk_level,
            arguments_preview,
            timeout_secs,
        } => UiMessage::ApprovalRequired(ApprovalRequest {
            agent_id: agent_id.clone(),
            tool_name: tool_name.clone(),
            risk_level: risk_level.clone(),
            arguments_preview: arguments_preview.clone(),
            timeout_secs: *timeout_secs,
            received_at: Instant::now(),
        }),
        PillarEventKind::Heartbeat { .. } => {
            // Heartbeats keep the connection alive but don't need to be forwarded
            return;
        }
        PillarEventKind::Unknown(name) => {
            warn!(event_type = %name, "SSE: unknown event type, ignoring");
            return;
        }
        other => UiMessage::PillarEvent(other.clone()),
    };

    if tx.send(msg).await.is_err() {
        debug!("SSE: channel closed, stopping dispatch");
    }
}

// ── Legacy compatibility ──────────────────────────────────────────────────────

/// Apply a parsed event to the shared app state (legacy path).
///
/// Retained for backward compatibility with existing code that uses `SharedState`.
/// New code should use the channel-based `spawn_listener` instead.
pub async fn apply_event(state: &crate::app::SharedState, event: PillarEventKind) {
    use crate::app::{OutputLineKind, ToastKind};

    let mut s = state.lock().await;
    match event {
        PillarEventKind::BudgetWarn { pct } => {
            s.push_toast(ToastKind::Warning, format!("Budget: {pct}% consumed"));
        }
        PillarEventKind::BudgetStop => {
            s.budget_stopped = true;
            s.push_output(
                "⚠ BUDGET STOP — press any key to acknowledge".to_string(),
                OutputLineKind::Error,
            );
        }
        PillarEventKind::GuardrailTriggered { level, reason, .. } => {
            s.push_toast(ToastKind::Warning, format!("Guardrail [{level}]: {reason}"));
        }
        PillarEventKind::MemoryInjected { fragment_count, .. } => {
            if fragment_count > 0 {
                s.push_toast(
                    ToastKind::Info,
                    format!("Memory: {fragment_count} fragments injected"),
                );
            }
        }
        PillarEventKind::QueueDepthChanged { depth } => {
            s.queue_depth = depth;
        }
        PillarEventKind::Heartbeat { .. }
        | PillarEventKind::Unknown(_)
        | PillarEventKind::Activity { .. }
        | PillarEventKind::AwaitingApproval { .. }
        | PillarEventKind::ToolActivityStart { .. }
        | PillarEventKind::ToolActivity { .. }
        | PillarEventKind::AgentTerminal { .. } => {
            // No-op for legacy path; AgentTerminal handled in the active TUI path.
        }
    }
}

/// Configuration for the legacy SSE listener.
pub struct SseConfig {
    pub base_url: String,
    pub run_id_filter: Option<String>,
}

/// Spawn the legacy SSE listener (retained for backward compatibility).
///
/// Prefer `spawn_listener(tx, client, cancel)` for new code.
pub fn spawn_listener_legacy(
    state: crate::app::SharedState,
    config: SseConfig,
) -> tokio::task::JoinHandle<()> {
    use crate::app::CoreStatus;

    const MAX_BACKOFF: u64 = 60;

    tokio::spawn(async move {
        let mut backoff_secs = 1u64;

        loop {
            let url = if let Some(ref run_id) = config.run_id_filter {
                format!("{}/v1/events/stream?run_id={run_id}", config.base_url)
            } else {
                format!("{}/v1/events/stream", config.base_url)
            };

            info!(url = %url, "SSE: connecting (legacy)");
            {
                let mut s = state.lock().await;
                s.push_output(
                    "SSE: connecting...".to_string(),
                    crate::app::OutputLineKind::System,
                );
            }

            match connect_and_listen_legacy(&url, &state).await {
                Ok(()) => {
                    backoff_secs = 1;
                }
                Err(e) => {
                    warn!(error = %e, backoff_secs, "SSE: connection failed (legacy)");
                    {
                        let mut s = state.lock().await;
                        s.core_status = CoreStatus::Disconnected;
                    }
                }
            }

            debug!(backoff_secs, "SSE: reconnecting after backoff (legacy)");
            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF);
        }
    })
}

/// Legacy connect-and-listen (uses SharedState directly).
async fn connect_and_listen_legacy(
    url: &str,
    state: &crate::app::SharedState,
) -> anyhow::Result<()> {
    use crate::app::CoreStatus;
    use futures::StreamExt;

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Accept", "text/event-stream")
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("SSE: server returned {}", response.status());
    }

    {
        let mut s = state.lock().await;
        s.core_status = CoreStatus::Connected {
            version: "sse".to_string(),
        };
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut event_type = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(pos) = buffer.find("\n\n") {
            let message = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            let mut data = String::new();
            for line in message.lines() {
                if let Some(et) = line.strip_prefix("event: ") {
                    event_type = et.trim().to_string();
                } else if let Some(d) = line.strip_prefix("data: ") {
                    data = d.trim().to_string();
                }
            }

            if !event_type.is_empty() {
                let parsed = parse_event(&event_type, &data);
                apply_event(state, parsed).await;
                event_type.clear();
            }
        }
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use super::*;
    use crate::app::{AppState, Mode, Tier};
    use crate::config::Config;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn make_state() -> crate::app::SharedState {
        Arc::new(Mutex::new(AppState::new(
            Config::default(),
            Mode::Yolo,
            Tier::Standard,
        )))
    }

    #[test]
    fn parse_budget_warn() {
        let event = parse_event("BudgetWarn", r#"{"pct": 80}"#);
        assert_eq!(event, PillarEventKind::BudgetWarn { pct: 80 });
    }

    #[test]
    fn parse_budget_stop() {
        let event = parse_event("BudgetStop", "{}");
        assert_eq!(event, PillarEventKind::BudgetStop);
    }

    #[test]
    fn parse_guardrail_triggered() {
        let event = parse_event(
            "GuardrailTriggered",
            r#"{"level": "input", "reason": "injection detected", "agent_id": "abc123"}"#,
        );
        assert_eq!(
            event,
            PillarEventKind::GuardrailTriggered {
                level: "input".to_string(),
                reason: "injection detected".to_string(),
                agent_id: Some("abc123".to_string()),
            }
        );
    }

    #[test]
    fn parse_memory_injected() {
        let event = parse_event(
            "MemoryInjected",
            r#"{"fragment_count": 3, "agent_id": "x"}"#,
        );
        assert_eq!(
            event,
            PillarEventKind::MemoryInjected {
                fragment_count: 3,
                agent_id: Some("x".to_string()),
            }
        );
    }

    #[test]
    fn parse_queue_depth() {
        let event = parse_event("QueueDepthChanged", r#"{"depth": 7}"#);
        assert_eq!(event, PillarEventKind::QueueDepthChanged { depth: 7 });
    }

    #[test]
    fn parse_unknown_event() {
        let event = parse_event("SomethingNew", "{}");
        assert_eq!(event, PillarEventKind::Unknown("SomethingNew".to_string()));
    }

    #[test]
    fn parse_awaiting_approval() {
        let event = parse_event(
            "AwaitingApproval",
            r#"{"agent_id": "abc-123", "tool_name": "bash", "risk_level": "CRITICAL", "arguments": "rm -rf /", "timeout_secs": 60}"#,
        );
        assert_eq!(
            event,
            PillarEventKind::AwaitingApproval {
                agent_id: "abc-123".to_string(),
                tool_name: "bash".to_string(),
                risk_level: "CRITICAL".to_string(),
                arguments_preview: "rm -rf /".to_string(),
                timeout_secs: 60,
            }
        );
    }

    #[test]
    fn parse_awaiting_approval_truncates_arguments() {
        let long_args = "x".repeat(3000);
        let data = format!(
            r#"{{"agent_id": "a", "tool_name": "t", "risk_level": "HIGH", "arguments": "{long_args}", "timeout_secs": 120}}"#
        );
        let event = parse_event("AwaitingApproval", &data);
        if let PillarEventKind::AwaitingApproval {
            arguments_preview, ..
        } = event
        {
            assert_eq!(arguments_preview.len(), 2048);
        } else {
            panic!("expected AwaitingApproval");
        }
    }

    #[tokio::test]
    async fn apply_budget_warn_creates_toast() {
        let state = make_state();
        apply_event(&state, PillarEventKind::BudgetWarn { pct: 85 }).await;
        let s = state.lock().await;
        assert_eq!(s.toasts.len(), 1);
        assert!(s.toasts[0].message.contains("85%"));
    }

    #[tokio::test]
    async fn apply_budget_stop_sets_flag() {
        let state = make_state();
        apply_event(&state, PillarEventKind::BudgetStop).await;
        let s = state.lock().await;
        assert!(s.budget_stopped);
    }

    #[tokio::test]
    async fn apply_queue_depth_updates_counter() {
        let state = make_state();
        apply_event(&state, PillarEventKind::QueueDepthChanged { depth: 5 }).await;
        let s = state.lock().await;
        assert_eq!(s.queue_depth, 5);
    }

    #[tokio::test]
    async fn spawn_listener_respects_cancellation() {
        let (tx, mut rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();

        // Cancel immediately — the listener should exit without blocking
        cancel.cancel();

        let cfg = crate::config::Config::default();
        let client = CoreClient::new(&cfg).unwrap();
        let handle = spawn_listener(tx, client, cancel);

        // Should complete quickly
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("listener should exit within 2s")
            .expect("task should not panic");

        // No messages expected (cancelled before connecting)
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn dispatch_event_sends_pillar_event() {
        let (tx, mut rx) = mpsc::channel(16);

        let event = PillarEventKind::BudgetWarn { pct: 75 };
        dispatch_event(&event, &tx).await;

        let msg = rx.try_recv().expect("should receive message");
        match msg {
            UiMessage::PillarEvent(PillarEventKind::BudgetWarn { pct }) => {
                assert_eq!(pct, 75);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_event_sends_approval_required() {
        let (tx, mut rx) = mpsc::channel(16);

        let event = PillarEventKind::AwaitingApproval {
            agent_id: "agent-1".to_string(),
            tool_name: "bash".to_string(),
            risk_level: "HIGH".to_string(),
            arguments_preview: "ls -la".to_string(),
            timeout_secs: 120,
        };
        dispatch_event(&event, &tx).await;

        let msg = rx.try_recv().expect("should receive message");
        match msg {
            UiMessage::ApprovalRequired(req) => {
                assert_eq!(req.agent_id, "agent-1");
                assert_eq!(req.tool_name, "bash");
                assert_eq!(req.risk_level, "HIGH");
                assert_eq!(req.timeout_secs, 120);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_event_skips_heartbeat() {
        let (tx, mut rx) = mpsc::channel(16);

        let event = PillarEventKind::Heartbeat {
            agent_id: "x".to_string(),
        };
        dispatch_event(&event, &tx).await;

        // Heartbeats should not produce a message
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn dispatch_event_skips_unknown() {
        let (tx, mut rx) = mpsc::channel(16);

        let event = PillarEventKind::Unknown("FutureEvent".to_string());
        dispatch_event(&event, &tx).await;

        // Unknown events should not produce a message
        assert!(rx.try_recv().is_err());
    }

    // ── Property-Based Tests ──────────────────────────────────────────────────

    // Feature: tui-wiring, Property 4: SSE Event Parsing Correctness
    // **Validates: Requirements 3.2, 3.3, 3.8**

    use proptest::prelude::*;

    /// Known event type names used by `parse_event`.
    const KNOWN_EVENT_TYPES: &[&str] = &[
        "BudgetWarn",
        "BudgetStop",
        "GuardrailTriggered",
        "MemoryInjected",
        "QueueDepthChanged",
        "Heartbeat",
        "AwaitingApproval",
    ];

    /// Serialize a `PillarEventKind` to its SSE format (event_type, json_data).
    fn serialize_event(event: &PillarEventKind) -> (String, String) {
        match event {
            PillarEventKind::BudgetWarn { pct } => {
                ("BudgetWarn".to_string(), format!(r#"{{"pct": {pct}}}"#))
            }
            PillarEventKind::BudgetStop => ("BudgetStop".to_string(), "{}".to_string()),
            PillarEventKind::ToolActivity { tool, ok, .. } => (
                "ToolActivity".to_string(),
                format!(r#"{{"tool": "{tool}", "ok": {ok}}}"#),
            ),
            PillarEventKind::ToolActivityStart { tool, .. } => (
                "ToolActivityStart".to_string(),
                format!(r#"{{"tool": "{tool}"}}"#),
            ),
            PillarEventKind::GuardrailTriggered {
                level,
                reason,
                agent_id,
            } => {
                let agent_id_json = agent_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |id| format!(r#""{id}""#));
                (
                    "GuardrailTriggered".to_string(),
                    format!(
                        r#"{{"level": "{level}", "reason": "{reason}", "agent_id": {agent_id_json}}}"#
                    ),
                )
            }
            PillarEventKind::MemoryInjected {
                fragment_count,
                agent_id,
            } => {
                let agent_id_json = agent_id
                    .as_ref()
                    .map_or_else(|| "null".to_string(), |id| format!(r#""{id}""#));
                (
                    "MemoryInjected".to_string(),
                    format!(
                        r#"{{"fragment_count": {fragment_count}, "agent_id": {agent_id_json}}}"#
                    ),
                )
            }
            PillarEventKind::QueueDepthChanged { depth } => (
                "QueueDepthChanged".to_string(),
                format!(r#"{{"depth": {depth}}}"#),
            ),
            PillarEventKind::Heartbeat { agent_id } => (
                "Heartbeat".to_string(),
                format!(r#"{{"agent_id": "{agent_id}"}}"#),
            ),
            PillarEventKind::AwaitingApproval {
                agent_id,
                tool_name,
                risk_level,
                arguments_preview,
                timeout_secs,
            } => (
                "AwaitingApproval".to_string(),
                format!(
                    r#"{{"agent_id": "{agent_id}", "tool_name": "{tool_name}", "risk_level": "{risk_level}", "arguments": "{arguments_preview}", "timeout_secs": {timeout_secs}}}"#
                ),
            ),
            PillarEventKind::Activity {
                pillar,
                phase,
                message,
            } => (
                "Activity".to_string(),
                format!(
                    r#"{{"pillar": "{pillar}", "phase": "{phase}", "message": "{message}"}}"#
                ),
            ),
            PillarEventKind::AgentTerminal { agent_id, status, .. } => (
                "AgentTerminal".to_string(),
                format!(r#"{{"agent_id": "{agent_id}", "status": "{status}"}}"#),
            ),
            PillarEventKind::Unknown(name) => (name.clone(), "{}".to_string()),
        }
    }

    /// Strategy to generate arbitrary `PillarEventKind` variants with valid data.
    /// Uses alphanumeric strings to avoid JSON escaping issues in serialization.
    fn arb_pillar_event_kind() -> impl Strategy<Value = PillarEventKind> {
        prop_oneof![
            // BudgetWarn: pct is u8
            any::<u8>().prop_map(|pct| PillarEventKind::BudgetWarn { pct }),
            // BudgetStop: no fields
            Just(PillarEventKind::BudgetStop),
            // GuardrailTriggered
            (
                "[a-zA-Z0-9_]{1,20}",
                "[a-zA-Z0-9_ ]{1,50}",
                proptest::option::of("[a-zA-Z0-9_]{1,20}")
            )
                .prop_map(|(level, reason, agent_id)| {
                    PillarEventKind::GuardrailTriggered {
                        level,
                        reason,
                        agent_id,
                    }
                }),
            // MemoryInjected
            (any::<u32>(), proptest::option::of("[a-zA-Z0-9_]{1,20}")).prop_map(
                |(fragment_count, agent_id)| {
                    PillarEventKind::MemoryInjected {
                        fragment_count,
                        agent_id,
                    }
                }
            ),
            // QueueDepthChanged
            any::<u32>().prop_map(|depth| PillarEventKind::QueueDepthChanged { depth }),
            // Heartbeat
            "[a-zA-Z0-9_-]{1,36}".prop_map(|agent_id| PillarEventKind::Heartbeat { agent_id }),
            // AwaitingApproval (arguments_preview limited to <=2048 to ensure round-trip)
            (
                "[a-zA-Z0-9_-]{1,36}",
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z]{1,10}",
                "[a-zA-Z0-9_ ]{0,200}",
                1u64..=600u64,
            )
                .prop_map(
                    |(agent_id, tool_name, risk_level, arguments_preview, timeout_secs)| {
                        PillarEventKind::AwaitingApproval {
                            agent_id,
                            tool_name,
                            risk_level,
                            arguments_preview,
                            timeout_secs,
                        }
                    },
                ),
        ]
    }

    /// Strategy to generate strings that do NOT match any known event type.
    fn arb_unknown_event_type() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9_]{1,30}".prop_filter("must not match a known event type", |s| {
            !KNOWN_EVENT_TYPES.contains(&s.as_str())
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property 4a: For any valid PillarEventKind variant, serializing to SSE
        /// format and parsing back produces an equivalent variant.
        #[test]
        fn prop_sse_round_trip(event in arb_pillar_event_kind()) {
            let (event_type, data) = serialize_event(&event);
            let parsed = parse_event(&event_type, &data);

            // For AwaitingApproval, the parser truncates arguments_preview to 2048 chars.
            // Our generator constrains arguments_preview to <=200 chars, so round-trip
            // should be exact.
            prop_assert_eq!(parsed, event,
                "Round-trip failed for event_type={:?}, data={:?}", event_type, data);
        }

        /// Property 4b: For any string that does not match a known event type,
        /// parse_event returns Unknown without panicking.
        #[test]
        fn prop_unknown_event_returns_unknown(event_type in arb_unknown_event_type()) {
            let parsed = parse_event(&event_type, "{}");
            prop_assert_eq!(parsed, PillarEventKind::Unknown(event_type));
        }

        /// Property 4c: AwaitingApproval with arguments longer than 2048 chars
        /// truncates to exactly 2048 chars during parsing.
        #[test]
        fn prop_awaiting_approval_truncates_long_arguments(
            agent_id in "[a-zA-Z0-9_-]{1,36}",
            tool_name in "[a-zA-Z0-9_]{1,30}",
            risk_level in "[a-zA-Z]{1,10}",
            extra_len in 2049usize..4000usize,
            timeout_secs in 1u64..=600u64,
        ) {
            // Generate arguments longer than 2048 using only safe chars
            let long_args: String = "a".repeat(extra_len);
            let data = format!(
                r#"{{"agent_id": "{agent_id}", "tool_name": "{tool_name}", "risk_level": "{risk_level}", "arguments": "{long_args}", "timeout_secs": {timeout_secs}}}"#
            );
            let parsed = parse_event("AwaitingApproval", &data);

            if let PillarEventKind::AwaitingApproval { arguments_preview, .. } = &parsed {
                prop_assert_eq!(arguments_preview.len(), 2048,
                    "Expected truncation to 2048, got {}", arguments_preview.len());
            } else {
                prop_assert!(false, "Expected AwaitingApproval variant, got {:?}", parsed);
            }
        }
    }
}
