//! Wave Executor — processes waves sequentially, tasks within each wave in parallel.
//!
//! For each wave:
//! 1. Spawn one agent per task (up to max_parallel_agents)
//! 2. Wait for all agents in the wave to reach terminal status
//! 3. Apply failure policy (continue/abort/retry)
//! 4. Advance to next wave
//!
//! This module defines the execution logic. The actual agent spawning is
//! delegated to a trait (`TaskSpawner`) so it can be mocked in tests.

use tracing::{debug, info, warn};
use uuid::Uuid;

use super::waves::{FailurePolicy, TaskStatus, WavePlan, WaveStatus};

/// Configuration for the wave executor.
#[derive(Debug, Clone)]
pub struct WaveExecutorConfig {
    pub max_parallel_agents: usize,
    pub failure_policy: FailurePolicy,
    pub retry_max_attempts: u32,
}

impl Default for WaveExecutorConfig {
    fn default() -> Self {
        Self {
            max_parallel_agents: 3,
            failure_policy: FailurePolicy::Continue,
            retry_max_attempts: 2,
        }
    }
}

/// Result of executing a single task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskResult {
    Completed,
    Failed { error: String },
}

/// Trait for spawning and awaiting task execution.
/// Implementations: real AgentManager (production) or mock (tests).
#[allow(async_fn_in_trait)]
pub trait TaskSpawner: Send + Sync {
    /// Spawn a task and wait for it to complete.
    /// Returns the agent_id and the result.
    async fn spawn_and_wait(&self, task_id: Uuid, description: &str) -> (Uuid, TaskResult);
}

/// Execute a wave plan to completion.
///
/// Processes waves sequentially. Within each wave, spawns tasks in parallel
/// (limited by `config.max_parallel_agents`). Returns the updated plan with
/// statuses filled in.
pub async fn execute<S: TaskSpawner>(
    mut plan: WavePlan,
    spawner: &S,
    config: &WaveExecutorConfig,
    pillar: Option<&crate::observability::pillar::PillarTx>,
) -> WavePlan {
    let total_waves = plan.waves.len();
    for wave_idx in 0..plan.waves.len() {
        info!(wave = wave_idx, tasks = plan.waves[wave_idx].tasks.len(), "executing wave");
        plan.waves[wave_idx].status = WaveStatus::Running;

        // Live progress for the TUI (SSE wire event → "EXECUTE — wave N/M").
        if let Some(tx) = pillar {
            crate::observability::pillar::emit_wire(
                tx,
                crate::proto::pillar::Pillar::P6Resilience,
                "wave_progress",
                format!(
                    "EXECUTE — wave {}/{} ({} tarefas)",
                    wave_idx + 1,
                    total_waves,
                    plan.waves[wave_idx].tasks.len()
                ),
            );
        }

        // Spawn tasks in chunks of max_parallel_agents
        let task_count = plan.waves[wave_idx].tasks.len();
        let mut task_idx = 0;

        while task_idx < task_count {
            let chunk_end = (task_idx + config.max_parallel_agents).min(task_count);

            // Build one future per pending task in the chunk, then run them
            // CONCURRENTLY with join_all. `spawn_and_wait` borrows `spawner`
            // immutably, so no Arc/'static needed — futures share `&S`.
            let mut slots = Vec::new();
            let mut futs = Vec::new();
            for i in task_idx..chunk_end {
                if plan.waves[wave_idx].tasks[i].status != TaskStatus::Pending {
                    continue; // already processed (e.g. retry)
                }
                plan.waves[wave_idx].tasks[i].status = TaskStatus::Running;
                let task_id = plan.waves[wave_idx].tasks[i].task_id;
                let desc = plan.waves[wave_idx].tasks[i].description.clone();
                slots.push(i);
                futs.push(async move { (task_id, spawner.spawn_and_wait(task_id, &desc).await) });
            }

            let results = futures::future::join_all(futs).await;

            for (i, (task_id, (agent_id, result))) in slots.into_iter().zip(results) {
                plan.waves[wave_idx].tasks[i].agent_id = Some(agent_id);
                match result {
                    TaskResult::Completed => {
                        plan.waves[wave_idx].tasks[i].status = TaskStatus::Completed;
                        debug!(wave = wave_idx, task = %task_id, "task completed");
                    }
                    TaskResult::Failed { ref error } => {
                        warn!(wave = wave_idx, task = %task_id, error = %error, "task failed");
                        plan.waves[wave_idx].tasks[i].status = TaskStatus::Failed;
                    }
                }
            }

            task_idx = chunk_end;
        }

        // Evaluate wave result
        let any_failed = plan.waves[wave_idx]
            .tasks
            .iter()
            .any(|t| t.status == TaskStatus::Failed);

        if any_failed {
            match config.failure_policy {
                FailurePolicy::Abort => {
                    warn!(wave = wave_idx, "wave has failures — aborting remaining waves");
                    plan.waves[wave_idx].status = WaveStatus::Failed;
                    // Mark remaining waves as skipped
                    for w in &mut plan.waves[wave_idx + 1..] {
                        w.status = WaveStatus::Skipped;
                        for t in &mut w.tasks {
                            t.status = TaskStatus::Skipped;
                        }
                    }
                    break;
                }
                FailurePolicy::Continue => {
                    info!(wave = wave_idx, "wave has failures — continuing (policy: continue)");
                    plan.waves[wave_idx].status = WaveStatus::Completed; // partial success
                }
                FailurePolicy::Retry => {
                    // For now, treat as continue (retry logic is a future enhancement)
                    info!(wave = wave_idx, "wave has failures — continuing (policy: retry, not yet implemented)");
                    plan.waves[wave_idx].status = WaveStatus::Completed;
                }
            }
        } else {
            plan.waves[wave_idx].status = WaveStatus::Completed;
        }
    }

    plan
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::waves::{self, TaskDef};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct MockSpawner {
        call_count: AtomicU32,
        fail_tasks: Vec<Uuid>,
    }

    impl MockSpawner {
        fn new() -> Self {
            Self {
                call_count: AtomicU32::new(0),
                fail_tasks: Vec::new(),
            }
        }

        fn with_failures(fail_tasks: Vec<Uuid>) -> Self {
            Self {
                call_count: AtomicU32::new(0),
                fail_tasks,
            }
        }
    }

    impl TaskSpawner for MockSpawner {
        async fn spawn_and_wait(&self, task_id: Uuid, _description: &str) -> (Uuid, TaskResult) {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            let agent_id = Uuid::now_v7();
            if self.fail_tasks.contains(&task_id) {
                (agent_id, TaskResult::Failed { error: "mock failure".to_string() })
            } else {
                (agent_id, TaskResult::Completed)
            }
        }
    }

    #[tokio::test]
    async fn executes_all_tasks_in_order() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        let tasks = vec![
            TaskDef { task_id: a, description: "A".into(), dependencies: vec![] },
            TaskDef { task_id: b, description: "B".into(), dependencies: vec![a] },
            TaskDef { task_id: c, description: "C".into(), dependencies: vec![b] },
        ];

        let plan = waves::plan(&tasks).unwrap();
        let spawner = MockSpawner::new();
        let config = WaveExecutorConfig::default();

        let result = execute(plan, &spawner, &config, None).await;

        assert_eq!(spawner.call_count.load(Ordering::Relaxed), 3);
        assert!(result.waves.iter().all(|w| w.status == WaveStatus::Completed));
        assert!(result.waves.iter().flat_map(|w| &w.tasks).all(|t| t.status == TaskStatus::Completed));
    }

    #[tokio::test]
    async fn abort_policy_stops_on_failure() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        let tasks = vec![
            TaskDef { task_id: a, description: "A".into(), dependencies: vec![] },
            TaskDef { task_id: b, description: "B".into(), dependencies: vec![a] },
            TaskDef { task_id: c, description: "C".into(), dependencies: vec![b] },
        ];

        let plan = waves::plan(&tasks).unwrap();
        let spawner = MockSpawner::with_failures(vec![a]); // A fails
        let config = WaveExecutorConfig {
            failure_policy: FailurePolicy::Abort,
            ..Default::default()
        };

        let result = execute(plan, &spawner, &config, None).await;

        // Wave 0 failed, waves 1 and 2 skipped
        assert_eq!(result.waves[0].status, WaveStatus::Failed);
        assert_eq!(result.waves[1].status, WaveStatus::Skipped);
        assert_eq!(result.waves[2].status, WaveStatus::Skipped);
        // Only 1 task was spawned (A)
        assert_eq!(spawner.call_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn continue_policy_proceeds_after_failure() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        let tasks = vec![
            TaskDef { task_id: a, description: "A".into(), dependencies: vec![] },
            TaskDef { task_id: b, description: "B".into(), dependencies: vec![a] },
        ];

        let plan = waves::plan(&tasks).unwrap();
        let spawner = MockSpawner::with_failures(vec![a]); // A fails
        let config = WaveExecutorConfig {
            failure_policy: FailurePolicy::Continue,
            ..Default::default()
        };

        let result = execute(plan, &spawner, &config, None).await;

        // Both waves executed despite A failing
        assert_eq!(result.waves[0].status, WaveStatus::Completed); // partial
        assert_eq!(result.waves[1].status, WaveStatus::Completed);
        assert_eq!(spawner.call_count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn empty_plan_executes_without_error() {
        let plan = WavePlan { waves: Vec::new() };
        let spawner = MockSpawner::new();
        let config = WaveExecutorConfig::default();

        let result = execute(plan, &spawner, &config, None).await;
        assert!(result.waves.is_empty());
        assert_eq!(spawner.call_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn agent_ids_are_assigned() {
        let a = Uuid::now_v7();
        let tasks = vec![
            TaskDef { task_id: a, description: "A".into(), dependencies: vec![] },
        ];

        let plan = waves::plan(&tasks).unwrap();
        let spawner = MockSpawner::new();
        let config = WaveExecutorConfig::default();

        let result = execute(plan, &spawner, &config, None).await;
        assert!(result.waves[0].tasks[0].agent_id.is_some());
    }
}
