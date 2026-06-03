//! Waves — parallel execution model for SDD tasks.
//!
//! Tasks are grouped into waves based on their dependency graph.
//! Tasks within the same wave have no dependencies on each other and
//! can execute in parallel. The Orchestrator processes waves sequentially,
//! with a synchronization barrier between each wave.
//!
//! Planning: dependency analysis → topological sort → wave assignment.
//! Execution: wave N completes → wave N+1 starts.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

// ── Data Model ────────────────────────────────────────────────────────────────

/// A complete wave execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavePlan {
    pub waves: Vec<Wave>,
}

/// A single wave — a group of tasks that can execute in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub index: u32,
    pub tasks: Vec<WaveTask>,
    pub status: WaveStatus,
}

/// A task within a wave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveTask {
    pub task_id: Uuid,
    pub description: String,
    /// IDs of tasks that must complete before this one can start.
    pub dependencies: Vec<Uuid>,
    /// Agent assigned to execute this task (set during execution).
    pub agent_id: Option<Uuid>,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Policy for handling failures within a wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Skip failed task, continue to next wave.
    Continue,
    /// Halt execution, mark remaining tasks as skipped.
    Abort,
    /// Retry failed task up to max_attempts before deciding.
    Retry,
}

impl FailurePolicy {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "abort" => Self::Abort,
            "retry" => Self::Retry,
            _ => Self::Continue,
        }
    }
}

// ── Planner ───────────────────────────────────────────────────────────────────

/// Input for the wave planner: a task with its dependencies.
#[derive(Debug, Clone)]
pub struct TaskDef {
    pub task_id: Uuid,
    pub description: String,
    pub dependencies: Vec<Uuid>,
}

/// Plan waves from a list of tasks with dependencies.
///
/// Uses topological sort (Kahn's algorithm) to assign each task to the
/// earliest possible wave. Tasks with no unresolved dependencies go in wave 0.
///
/// Returns `Err` if the dependency graph contains a cycle.
pub fn plan(tasks: &[TaskDef]) -> Result<WavePlan, PlanError> {
    if tasks.is_empty() {
        return Ok(WavePlan { waves: Vec::new() });
    }

    // Build adjacency and in-degree maps
    let task_ids: HashSet<Uuid> = tasks.iter().map(|t| t.task_id).collect();
    let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
    let mut dependents: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut task_map: HashMap<Uuid, &TaskDef> = HashMap::new();

    for task in tasks {
        task_map.insert(task.task_id, task);
        in_degree.entry(task.task_id).or_insert(0);

        for dep in &task.dependencies {
            if !task_ids.contains(dep) {
                return Err(PlanError::MissingDependency {
                    task_id: task.task_id,
                    missing_dep: *dep,
                });
            }
            *in_degree.entry(task.task_id).or_insert(0) += 1;
            dependents.entry(*dep).or_default().push(task.task_id);
        }
    }

    // Kahn's algorithm with wave assignment
    let mut queue: VecDeque<Uuid> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut wave_assignment: HashMap<Uuid, u32> = HashMap::new();
    let mut processed = 0usize;

    // Assign wave 0 to all tasks with no dependencies
    for &id in &queue {
        wave_assignment.insert(id, 0);
    }

    while let Some(current) = queue.pop_front() {
        processed += 1;
        let current_wave = wave_assignment[&current];

        if let Some(deps) = dependents.get(&current) {
            for &dep_id in deps {
                let deg = in_degree.get_mut(&dep_id).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    // This task goes in the wave after its latest dependency
                    let dep_wave = wave_assignment.entry(dep_id).or_insert(0);
                    *dep_wave = (*dep_wave).max(current_wave + 1);
                    queue.push_back(dep_id);
                }
            }
        }
    }

    if processed != tasks.len() {
        return Err(PlanError::CyclicDependency);
    }

    // Group tasks by wave
    let max_wave = wave_assignment.values().copied().max().unwrap_or(0);
    let mut waves: Vec<Wave> = (0..=max_wave)
        .map(|i| Wave {
            index: i,
            tasks: Vec::new(),
            status: WaveStatus::Pending,
        })
        .collect();

    for task in tasks {
        let wave_idx = wave_assignment[&task.task_id] as usize;
        waves[wave_idx].tasks.push(WaveTask {
            task_id: task.task_id,
            description: task.description.clone(),
            dependencies: task.dependencies.clone(),
            agent_id: None,
            status: TaskStatus::Pending,
        });
    }

    Ok(WavePlan { waves })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    CyclicDependency,
    MissingDependency { task_id: Uuid, missing_dep: Uuid },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CyclicDependency => write!(f, "dependency graph contains a cycle"),
            Self::MissingDependency { task_id, missing_dep } => {
                write!(f, "task {task_id} depends on {missing_dep} which is not in the plan")
            }
        }
    }
}

impl std::error::Error for PlanError {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: Uuid, desc: &str, deps: Vec<Uuid>) -> TaskDef {
        TaskDef {
            task_id: id,
            description: desc.to_string(),
            dependencies: deps,
        }
    }

    #[test]
    fn empty_tasks_produces_empty_plan() {
        let plan = plan(&[]).unwrap();
        assert!(plan.waves.is_empty());
    }

    #[test]
    fn single_task_goes_to_wave_0() {
        let id = Uuid::now_v7();
        let plan = plan(&[task(id, "only task", vec![])]).unwrap();
        assert_eq!(plan.waves.len(), 1);
        assert_eq!(plan.waves[0].index, 0);
        assert_eq!(plan.waves[0].tasks.len(), 1);
        assert_eq!(plan.waves[0].tasks[0].task_id, id);
    }

    #[test]
    fn independent_tasks_same_wave() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        let plan = plan(&[
            task(a, "task A", vec![]),
            task(b, "task B", vec![]),
            task(c, "task C", vec![]),
        ])
        .unwrap();

        assert_eq!(plan.waves.len(), 1);
        assert_eq!(plan.waves[0].tasks.len(), 3);
    }

    #[test]
    fn linear_chain_produces_sequential_waves() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        let plan = plan(&[
            task(a, "first", vec![]),
            task(b, "second", vec![a]),
            task(c, "third", vec![b]),
        ])
        .unwrap();

        assert_eq!(plan.waves.len(), 3);
        assert_eq!(plan.waves[0].tasks[0].task_id, a);
        assert_eq!(plan.waves[1].tasks[0].task_id, b);
        assert_eq!(plan.waves[2].tasks[0].task_id, c);
    }

    #[test]
    fn diamond_dependency() {
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();
        let d = Uuid::now_v7();

        let plan = plan(&[
            task(a, "A", vec![]),
            task(b, "B", vec![a]),
            task(c, "C", vec![a]),
            task(d, "D", vec![b, c]),
        ])
        .unwrap();

        assert_eq!(plan.waves.len(), 3);
        // Wave 0: A
        assert_eq!(plan.waves[0].tasks.len(), 1);
        // Wave 1: B, C (parallel)
        assert_eq!(plan.waves[1].tasks.len(), 2);
        // Wave 2: D
        assert_eq!(plan.waves[2].tasks.len(), 1);
        assert_eq!(plan.waves[2].tasks[0].task_id, d);
    }

    #[test]
    fn mixed_dependencies() {
        // A (no deps) → wave 0
        // B (no deps) → wave 0
        // C (depends on A) → wave 1
        // D (depends on A, B) → wave 1
        // E (depends on C, D) → wave 2
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();
        let d = Uuid::now_v7();
        let e = Uuid::now_v7();

        let plan = plan(&[
            task(a, "A", vec![]),
            task(b, "B", vec![]),
            task(c, "C", vec![a]),
            task(d, "D", vec![a, b]),
            task(e, "E", vec![c, d]),
        ])
        .unwrap();

        assert_eq!(plan.waves.len(), 3);
        assert_eq!(plan.waves[0].tasks.len(), 2); // A, B
        assert_eq!(plan.waves[1].tasks.len(), 2); // C, D
        assert_eq!(plan.waves[2].tasks.len(), 1); // E
    }

    #[test]
    fn cyclic_dependency_returns_error() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();

        let result = plan(&[
            task(a, "A", vec![b]),
            task(b, "B", vec![a]),
        ]);

        assert_eq!(result.unwrap_err(), PlanError::CyclicDependency);
    }

    #[test]
    fn missing_dependency_returns_error() {
        let a = Uuid::now_v7();
        let missing = Uuid::now_v7();

        let result = plan(&[task(a, "A", vec![missing])]);

        match result.unwrap_err() {
            PlanError::MissingDependency { task_id, missing_dep } => {
                assert_eq!(task_id, a);
                assert_eq!(missing_dep, missing);
            }
            _ => panic!("expected MissingDependency"),
        }
    }

    #[test]
    fn all_tasks_assigned_to_waves() {
        let ids: Vec<Uuid> = (0..10).map(|_| Uuid::now_v7()).collect();
        let tasks: Vec<TaskDef> = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let deps = if i > 0 { vec![ids[i - 1]] } else { vec![] };
                task(id, &format!("task {i}"), deps)
            })
            .collect();

        let plan = plan(&tasks).unwrap();
        let total_tasks: usize = plan.waves.iter().map(|w| w.tasks.len()).sum();
        assert_eq!(total_tasks, 10);
    }

    #[test]
    fn wave_indices_are_sequential() {
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let c = Uuid::now_v7();

        let plan = plan(&[
            task(a, "A", vec![]),
            task(b, "B", vec![a]),
            task(c, "C", vec![b]),
        ])
        .unwrap();

        for (i, wave) in plan.waves.iter().enumerate() {
            assert_eq!(wave.index, i as u32);
        }
    }

    #[test]
    fn failure_policy_from_str() {
        assert_eq!(FailurePolicy::from_str("continue"), FailurePolicy::Continue);
        assert_eq!(FailurePolicy::from_str("abort"), FailurePolicy::Abort);
        assert_eq!(FailurePolicy::from_str("retry"), FailurePolicy::Retry);
        assert_eq!(FailurePolicy::from_str("unknown"), FailurePolicy::Continue);
    }
}
