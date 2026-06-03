//! Real TaskSpawner — spawns agents via AgentManager and polls to completion.

use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::proto::agent::{AgentKind, AgentStatus, SpawnAgentRequest};
use super::wave_executor::{TaskResult, TaskSpawner};

/// Default timeout for a single task execution (30 minutes).
const TASK_TIMEOUT_SECS: u64 = 1800;
/// Default max iterations per task agent.
const TASK_MAX_ITERS: u32 = 50;

/// Real TaskSpawner that uses AgentManager to spawn and poll agents.
pub struct AgentTaskSpawner {
    agents: AgentManager,
    worktree_id: String,
}

impl AgentTaskSpawner {
    pub fn new(agents: AgentManager, worktree_id: String) -> Self {
        Self { agents, worktree_id }
    }
}

impl TaskSpawner for AgentTaskSpawner {
    async fn spawn_and_wait(&self, task_id: Uuid, description: &str) -> (Uuid, TaskResult) {
        let req = SpawnAgentRequest {
            kind: AgentKind::Writer,
            task: description.to_string(),
            tier: None,
            worktree_id: Some(self.worktree_id.clone()),
            base: None,
            budget_usd: None,
            max_iters: Some(TASK_MAX_ITERS),
            yolo_mode: None,
            background: None,
            timeout_secs: Some(TASK_TIMEOUT_SECS),
        };

        let agent = match self.agents.spawn(req).await {
            Ok(a) => a,
            Err(e) => {
                warn!(task_id = %task_id, error = %e, "wave spawner: agent spawn failed");
                return (Uuid::nil(), TaskResult::Failed {
                    error: format!("spawn failed: {e}"),
                });
            }
        };

        let agent_id = Uuid::parse_str(&agent.id).unwrap_or_else(|_| Uuid::nil());
        debug!(task_id = %task_id, agent_id = %agent_id, "wave spawner: agent spawned, polling");

        // Poll until terminal status
        let deadline = Instant::now() + Duration::from_secs(TASK_TIMEOUT_SECS);
        loop {
            if Instant::now() > deadline {
                warn!(task_id = %task_id, agent_id = %agent_id, "wave spawner: timeout");
                return (agent_id, TaskResult::Failed {
                    error: "task timeout".to_string(),
                });
            }

            tokio::time::sleep(Duration::from_secs(2)).await;

            let Some(a) = self.agents.get(agent_id) else {
                return (agent_id, TaskResult::Failed {
                    error: "agent not found".to_string(),
                });
            };

            match a.status {
                AgentStatus::Completed => {
                    return (agent_id, TaskResult::Completed);
                }
                AgentStatus::Failed | AgentStatus::Stopped | AgentStatus::Cancelled | AgentStatus::Stale => {
                    let err = a.error.unwrap_or_else(|| format!("{:?}", a.status));
                    return (agent_id, TaskResult::Failed { error: err });
                }
                _ => continue,
            }
        }
    }
}
