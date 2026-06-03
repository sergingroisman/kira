#![forbid(unsafe_code)]

//! Agent Polling — spawns a Tokio task per active agent to poll status.
//!
//! Each polling task runs every 2 seconds until the agent reaches a terminal
//! status (completed, failed, cancelled, stopped). Communicates with the event
//! loop exclusively via `mpsc::Sender<UiMessage>` — no direct `AppState` access.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::core_client::CoreClient;
use crate::messages::UiMessage;

/// Terminal statuses that stop polling.
const TERMINAL_STATUSES: &[&str] = &["completed", "failed", "cancelled", "stopped"];

/// Polling interval between status checks.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Spawn a polling task for a single agent.
///
/// Polls `client.agent_status(&agent_id)` every 2 seconds. On terminal status,
/// sends `UiMessage::AgentTerminal` and exits. Uses `tokio::select!` to respect
/// the `CancellationToken` for graceful shutdown.
pub fn spawn_agent_poller(
    agent_id: String,
    tx: mpsc::Sender<UiMessage>,
    client: CoreClient,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let short_id = &agent_id[..agent_id.len().min(8)];

        loop {
            // Sleep with cancellation support
            tokio::select! {
                () = tokio::time::sleep(POLL_INTERVAL) => {}
                () = cancel.cancelled() => {
                    debug!(agent_id = %short_id, "poller cancelled");
                    return;
                }
            }

            match client.agent_status(&agent_id).await {
                Ok(json_str) => {
                    // Parse the JSON to extract status
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        let status = v
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown");

                        debug!(agent_id = %short_id, status, "poll tick");

                        // Check if terminal
                        if TERMINAL_STATUSES.contains(&status) {
                            let error = v.get("error").and_then(|e| e.as_str()).map(String::from);
                            // The agent's final answer lives in `result` (see
                            // core Agent::result). Forward it so the event loop
                            // can render it into the chat panel.
                            let result =
                                v.get("result").and_then(|r| r.as_str()).map(String::from);

                            let msg = UiMessage::AgentTerminal {
                                agent_id: agent_id.clone(),
                                status: status.to_string(),
                                error,
                                result,
                            };

                            if tx.send(msg).await.is_err() {
                                warn!(agent_id = %short_id, "channel closed, stopping poller");
                            }
                            return;
                        }

                        // Optionally send AgentOutput if there's new output in the response
                        if let Some(output) = v.get("output").and_then(|o| o.as_str())
                            && !output.is_empty()
                        {
                            let msg = UiMessage::AgentOutput {
                                agent_id: agent_id.clone(),
                                line: output.to_string(),
                            };
                            if tx.send(msg).await.is_err() {
                                warn!(agent_id = %short_id, "channel closed, stopping poller");
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(agent_id = %short_id, error = %e, "poll failed");
                    // Continue polling on next cycle
                }
            }
        }
    })
}

/// Check if a given status string is terminal.
pub fn is_terminal(status: &str) -> bool {
    TERMINAL_STATUSES.contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_statuses() {
        assert!(is_terminal("completed"));
        assert!(is_terminal("failed"));
        assert!(is_terminal("cancelled"));
        assert!(is_terminal("stopped"));
        assert!(!is_terminal("running"));
        assert!(!is_terminal("active"));
        assert!(!is_terminal("unknown"));
    }
}
