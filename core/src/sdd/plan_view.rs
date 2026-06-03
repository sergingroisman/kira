//! Plan view + shared `tasks.json` parsing.
//!
//! The TASKS agent emits `tasks.json` with string ids (`"t1"`), `owned_files`,
//! and `depends_on` (string ids) — see `agents/prompts/sdd/tasks.md`. The wave
//! planner ([`waves::plan`]) works in `Uuid` space, so we map each string id to
//! a deterministic UUIDv5 and resolve `depends_on` through the same map. This
//! is the single source of truth for turning `tasks.json` into [`TaskDef`]s —
//! used by EXECUTE, the `/waves` endpoint, and the plan-approval matrix.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sdd::waves::{self, TaskDef};

/// Fixed namespace for deriving stable task UUIDs from string ids.
const TASK_NS: Uuid = Uuid::from_u128(0x6b69_7261_7464_7546_5f74_6173_6b5f_6e73);

/// Map a `tasks.json` string id (e.g. `"t1"`) to a deterministic UUID.
/// Stable across the matrix and EXECUTE so dependencies resolve.
fn task_uuid(id: &str) -> Uuid {
    Uuid::new_v5(&TASK_NS, id.as_bytes())
}

/// Parse the `tasks` array of a `tasks.json` value into wave-planner inputs.
///
/// Reads the real emitted schema: `id` (string), `title`, `depends_on`
/// (string ids). Unknown / missing deps are dropped. Returns `None` if the
/// value has no `tasks` array.
pub fn parse_task_defs(parsed: &serde_json::Value) -> Option<Vec<TaskDef>> {
    let tasks = parsed.get("tasks").and_then(|v| v.as_array())?;
    Some(
        tasks
            .iter()
            .map(|t| {
                let id_str = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let description = format!(
                    "{}\n\n{}",
                    title,
                    t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                );
                let dependencies = depends_on(t);
                TaskDef {
                    task_id: task_uuid(id_str),
                    description,
                    dependencies,
                }
            })
            .collect(),
    )
}

/// Extract `depends_on` (string ids) resolved to deterministic UUIDs.
fn depends_on(task: &serde_json::Value) -> Vec<Uuid> {
    task.get("depends_on")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|d| d.as_str().map(task_uuid))
                .collect()
        })
        .unwrap_or_default()
}

/// A single task as shown in the approval matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: String,
    pub title: String,
    pub owned_files: Vec<String>,
    /// 0-based wave index this task lands in.
    pub wave: u32,
}

/// The plan matrix presented to the operator at the approval gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanView {
    pub run_id: String,
    pub goal: String,
    pub wave_count: u32,
    pub tasks: Vec<PlanTask>,
}

/// Build a [`PlanView`] from `run_id` + `goal` + parsed `tasks.json`.
///
/// Computes wave assignment via [`waves::plan`] and joins it back to the
/// human-facing task metadata (title, owned_files). Tasks are returned sorted
/// by wave, then by their original order.
pub fn build_plan_view(run_id: &str, goal: &str, parsed: &serde_json::Value) -> PlanView {
    let raw_tasks = parsed
        .get("tasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // wave index per task UUID
    let wave_of: std::collections::HashMap<Uuid, u32> = parse_task_defs(parsed)
        .and_then(|defs| waves::plan(&defs).ok())
        .map(|plan| {
            plan.waves
                .iter()
                .flat_map(|w| w.tasks.iter().map(move |t| (t.task_id, w.index)))
                .collect()
        })
        .unwrap_or_default();

    let mut tasks: Vec<PlanTask> = raw_tasks
        .iter()
        .map(|t| {
            let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let owned_files = t
                .get("owned_files")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|f| f.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let wave = *wave_of.get(&task_uuid(&id)).unwrap_or(&0);
            PlanTask { id, title, owned_files, wave }
        })
        .collect();

    tasks.sort_by_key(|t| t.wave);
    let wave_count = tasks.iter().map(|t| t.wave + 1).max().unwrap_or(0);

    PlanView {
        run_id: run_id.to_string(),
        goal: goal.to_string(),
        wave_count,
        tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tasks() -> serde_json::Value {
        serde_json::json!({
            "tasks": [
                { "id": "t1", "title": "scaffold", "owned_files": ["a.rs"], "depends_on": [] },
                { "id": "t2", "title": "build on t1", "owned_files": ["b.rs"], "depends_on": ["t1"] },
                { "id": "t3", "title": "independent", "owned_files": ["c.rs"], "depends_on": [] }
            ]
        })
    }

    #[test]
    fn depends_on_resolves_to_real_waves() {
        // t1,t3 in wave 0; t2 depends on t1 → wave 1.
        let view = build_plan_view("run1", "do thing", &sample_tasks());
        assert_eq!(view.wave_count, 2);
        let t2 = view.tasks.iter().find(|t| t.id == "t2").unwrap();
        assert_eq!(t2.wave, 1);
        let t1 = view.tasks.iter().find(|t| t.id == "t1").unwrap();
        assert_eq!(t1.wave, 0);
    }

    #[test]
    fn owned_files_preserved() {
        let view = build_plan_view("run1", "do thing", &sample_tasks());
        let t1 = view.tasks.iter().find(|t| t.id == "t1").unwrap();
        assert_eq!(t1.owned_files, vec!["a.rs".to_string()]);
    }

    #[test]
    fn parse_task_defs_links_dependencies() {
        let defs = parse_task_defs(&sample_tasks()).unwrap();
        assert_eq!(defs.len(), 3);
        // t2's dependency must equal t1's derived uuid.
        let t1_uuid = task_uuid("t1");
        let t2 = defs.iter().find(|d| d.task_id == task_uuid("t2")).unwrap();
        assert_eq!(t2.dependencies, vec![t1_uuid]);
    }

    #[test]
    fn missing_tasks_array_is_none() {
        assert!(parse_task_defs(&serde_json::json!({})).is_none());
    }
}
