//! Background watchdog tasks spawned at daemon startup.
//!
//! v0 ships 2 watchdogs (per plan T9):
//!   - `agent_heartbeat_scanner` (60s): Active agents with no progress in 5min → Stale
//!   - `worktree_orphan_cleanup` (60s): worktrees with no active agent + age >10min → destroy
//!
//! Both run forever in `tokio::spawn`d tasks. They are intentionally simple
//! (no graceful shutdown) — when the daemon exits, the runtime drops them.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::time::Duration;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::proto::agent::AgentStatus;
use crate::worktree::WorktreeManager;

const HEARTBEAT_SCAN_PERIOD: Duration = Duration::from_secs(60);
const AGENT_STALE_AFTER: ChronoDuration = ChronoDuration::minutes(5);

const CLEANUP_SCAN_PERIOD: Duration = Duration::from_secs(60);
const WORKTREE_ORPHAN_AGE: ChronoDuration = ChronoDuration::minutes(10);

/// Boot all watchdogs. Tasks live for the lifetime of the runtime.
pub fn spawn_all(agents: AgentManager, worktrees: WorktreeManager) {
    tokio::spawn(agent_heartbeat_scanner(agents.clone()));
    tokio::spawn(worktree_orphan_cleanup(agents, worktrees));
    info!("watchdogs spawned: heartbeat_scanner + worktree_orphan_cleanup");
}

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn is_terminal(status: AgentStatus) -> bool {
    matches!(
        status,
        AgentStatus::Completed
            | AgentStatus::Failed
            | AgentStatus::Cancelled
            | AgentStatus::Stopped
            | AgentStatus::Stale
    )
}

// ============================================================================
// agent_heartbeat_scanner
// ============================================================================

#[instrument(skip(agents))]
async fn agent_heartbeat_scanner(agents: AgentManager) {
    let mut ticker = tokio::time::interval(HEARTBEAT_SCAN_PERIOD);
    // Skip the immediate first tick — startup is busy enough.
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let now = Utc::now();
        let mut marked = 0_usize;
        for a in agents.list() {
            if a.status != AgentStatus::Active && a.status != AgentStatus::Spawning {
                continue;
            }
            let Some(last) = a.last_progress_at.as_deref().and_then(parse_iso) else {
                continue;
            };
            if now.signed_duration_since(last) > AGENT_STALE_AFTER {
                if let Ok(uuid) = Uuid::parse_str(&a.id) {
                    agents.mark_stale(uuid);
                    marked += 1;
                    warn!(agent_id = %a.id, last_progress = %a.last_progress_at.as_deref().unwrap_or("?"), "agent marked Stale");
                }
            }
        }
        if marked > 0 {
            info!(marked, "heartbeat_scanner pass");
        }
    }
}

// ============================================================================
// worktree_orphan_cleanup
// ============================================================================

#[instrument(skip(agents, worktrees))]
async fn worktree_orphan_cleanup(agents: AgentManager, worktrees: WorktreeManager) {
    let mut ticker = tokio::time::interval(CLEANUP_SCAN_PERIOD);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let now = Utc::now();
        let all_agents = agents.list();
        let mut destroyed = 0_usize;

        for wt in worktrees.list() {
            // Skip worktrees still in active states.
            if !matches!(
                wt.status,
                crate::proto::worktree::WorktreeStatus::Idle
                    | crate::proto::worktree::WorktreeStatus::Stale
                    | crate::proto::worktree::WorktreeStatus::Completed
                    | crate::proto::worktree::WorktreeStatus::Failed
            ) {
                continue;
            }

            // Any non-terminal agent referencing this worktree → keep.
            let used_by_live_agent = all_agents
                .iter()
                .any(|a| a.worktree_id == wt.id && !is_terminal(a.status));
            if used_by_live_agent {
                continue;
            }

            // Check age.
            let Some(created) = parse_iso(&wt.created_at) else {
                continue;
            };
            if now.signed_duration_since(created) <= WORKTREE_ORPHAN_AGE {
                continue;
            }

            let Ok(uuid) = Uuid::parse_str(&wt.id) else {
                continue;
            };
            match worktrees.destroy(uuid, true).await {
                Ok(_) => {
                    destroyed += 1;
                    info!(worktree_id = %wt.id, "orphan worktree destroyed");
                }
                Err(e) => {
                    warn!(worktree_id = %wt.id, error = ?e, "orphan destroy failed");
                }
            }
        }

        if destroyed > 0 {
            info!(destroyed, "worktree_orphan_cleanup pass");
        }
    }
}
