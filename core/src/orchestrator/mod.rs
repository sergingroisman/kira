//! Orquestrador / Watari (P2 — Coordenação baixa) — executes `Intent`s.
//!
//! Worker layer. Takes an `Intent` + run state, runs the actual work
//! (spawn agents, write artifacts, advance phase), returns a `StepResult`.
//! NEVER decides — only executes what Pilot decided.
//!
//! Two execution paths coexist during the P1→P2 migration:
//! - `execute()` — legacy SDD path using `SddRun`.
//! - `execute_workflow()` — generic path using `WorkflowRun` + `WorkflowDef`.

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::instrument;
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::observability::pillar::{self, PillarTx};
use crate::proto::agent::{AgentKind, AgentStatus, SpawnAgentRequest};
use crate::proto::intent::{Intent, StepOutcome, StepResult};
use crate::proto::pillar::Pillar;
use crate::proto::run::{PhaseVerdict as RunPhaseVerdict, WorkflowRun, PHASE_DONE, PHASE_FAILED};
use crate::proto::sdd::{PhaseVerdict, SddPhase, SddRun};
use crate::router::Router;
use crate::workflow::def::WorkflowDef;

const POLL_INTERVAL_MS: u64 = 500;
const PHASE_TIMEOUT_SECS: u64 = 600;
const DEFAULT_MAX_ITERS: u32 = 12;

/// Phase prompt loader — re-exports from `crate::sdd::prompts` to avoid cycles.
fn phase_prompt(phase: SddPhase) -> Result<(&'static str, &'static str)> {
    match phase {
        SddPhase::Specify => Ok(("spec.md", crate::sdd::prompts::SPECIFY)),
        SddPhase::Design => Ok(("design.md", crate::sdd::prompts::DESIGN)),
        SddPhase::Tasks => Ok(("tasks.json", crate::sdd::prompts::TASKS)),
        _ => anyhow::bail!("phase {phase:?} is not writer-phase"),
    }
}

#[derive(Clone)]
pub struct Orchestrator {
    pub agents: AgentManager,
    pub pillar_tx: PillarTx,
    pub router: Router,
}

impl Orchestrator {
    pub const fn new(agents: AgentManager, pillar_tx: PillarTx, router: Router) -> Self {
        Self { agents, pillar_tx, router }
    }

    /// Execute an Intent against the run state. Mutates `run` (artifact paths,
    /// verdicts, phase transition). Returns `StepResult` describing the outcome.
    #[instrument(skip(self, run, intent), fields(run_id = %run.id))]
    pub async fn execute(&self, run: &mut SddRun, intent: Intent) -> Result<StepResult> {
        let started = Instant::now();
        pillar::emit_full(
            &self.pillar_tx,
            Pillar::P2Coordination,
            "step",
            format!("Orq executing: {}", intent_label(&intent)),
            Some(run.id.clone()),
            None,
            serde_json::Map::new(),
        );

        match intent {
            // Generic workflow intents should not reach the SDD execute() path.
            Intent::RunPhase { phase_id } | Intent::RunExecute { phase_id } => Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: format!("generic intent RunPhase/RunExecute({phase_id}) reached legacy execute() — use execute_workflow()"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::RunSddPhase { phase } => self.run_phase(run, phase, started).await,
            Intent::RunSddExecute => {
                // Parallel wave execution: decompose tasks.json by dependencies
                // and run independent tasks concurrently. Falls back to serial
                // internally if tasks can't be parsed.
                self.run_execute_waves(run, started, &crate::config::WavesConfig::default())
                    .await
            }
            Intent::EscalateTier { tier: _, reason } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: format!("escalation requested: {reason} (v0 no-op)"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::RequestHumanReview { question } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: format!("awaiting human: {question}"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            // AwaitUserInput is handled by the drive loop before reaching the
            // orchestrator (see sdd/mod.rs drive()). This arm is a safety net.
            Intent::AwaitUserInput { .. } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: "awaiting user input (questions pending)".into(),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::Done => {
                run.current_phase = SddPhase::Done;
                run.finished_at = Some(Utc::now().to_rfc3339());
                Ok(StepResult {
                    outcome: StepOutcome::Ok,
                    message: "terminal: done".into(),
                    duration_ms: ms(started),
                    cost_usd: 0.0,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                })
            }
            Intent::Halt { reason } => {
                run.current_phase = SddPhase::Failed;
                run.finished_at = Some(Utc::now().to_rfc3339());
                if run.error.is_none() {
                    run.error = Some(reason.clone());
                }
                Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: format!("halt: {reason}"),
                    duration_ms: ms(started),
                    cost_usd: 0.0,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                })
            }
        }
    }

    /// Run a SPECIFY/DESIGN/TASKS phase. Spawns Writer w/ phase prompt,
    /// polls to completion, verifies artifact, advances phase.
    ///
    /// SPECIFY clarification (P1, 2026-05-25): on first entry to SPECIFY,
    /// before spawning the Writer, run a lightweight LLM probe that may
    /// return clarifying questions. If it does, mutate `run.pending_questions`
    /// and return `Pending` without advancing the phase — the SddManager's
    /// drive loop will see the questions and block until the user answers
    /// via `/v1/sdd/{id}/questions`. Workshop SPEKTOR P1 invariant:
    /// "Pior — o juiz não tem critério de aceitação porque a spec nunca foi escrita."
    async fn run_phase(
        &self,
        run: &mut SddRun,
        phase: SddPhase,
        phase_started: Instant,
    ) -> Result<StepResult> {
        // SPECIFY clarification gate — only on the first pass (no prior
        // answers yet, no spec produced) and only if the goal looks rich
        // enough to warrant questions. The clarifier itself returns an
        // empty list for trivial goals, so auto-sizing happens model-side.
        if phase == SddPhase::Specify
            && run.clarifications.is_empty()
            && run.spec_path.is_none()
            && !run.phases_run.contains(&SddPhase::Specify)
        {
            match crate::sdd::clarify::generate_clarifying_questions(
                &self.router,
                &run.goal,
                run.kind,
            )
            .await
            {
                Ok(questions) if !questions.is_empty() => {
                    pillar::emit_full(
                        &self.pillar_tx,
                        Pillar::P1Sdd,
                        "clarify",
                        format!("{} clarifying question(s) emitted", questions.len()),
                        Some(run.id.clone()),
                        None,
                        serde_json::Map::new(),
                    );
                    run.pending_questions = questions;
                    return Ok(StepResult {
                        outcome: StepOutcome::Pending,
                        message: "awaiting user clarifications before SPECIFY".into(),
                        duration_ms: ms(phase_started),
                        cost_usd: 0.0,
                        artifact_path: None,
                        fields: serde_json::Map::new(),
                    });
                }
                Ok(_) => {
                    // Empty list = model judged goal unambiguous. Proceed.
                    pillar::emit_full(
                        &self.pillar_tx,
                        Pillar::P1Sdd,
                        "clarify",
                        "no clarifying questions needed (goal unambiguous)".to_string(),
                        Some(run.id.clone()),
                        None,
                        serde_json::Map::new(),
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        run_id = %run.id,
                        error = %e,
                        "clarifier failed — proceeding to SPECIFY without questions"
                    );
                    pillar::emit_full(
                        &self.pillar_tx,
                        Pillar::P1Sdd,
                        "clarify",
                        format!("clarifier errored, skipping: {e}"),
                        Some(run.id.clone()),
                        None,
                        serde_json::Map::new(),
                    );
                }
            }
        }

        let (filename, prompt_extra) = phase_prompt(phase)?;
        let worktree_path = PathBuf::from(&run.worktree_path);
        let run_dir = worktree_path.join(".kira").join("runs").join(&run.id);
        std::fs::create_dir_all(&run_dir).context("create run dir")?;
        let artifact_abs = run_dir.join(filename);
        let artifact_rel = artifact_abs
            .strip_prefix(&worktree_path).map_or_else(|_| artifact_abs.display().to_string(), |p| p.display().to_string());

        let mut ctx = format!(
            "GOAL: {}\n\nRUN DIR (worktree-relative): .kira/runs/{}/\n\nARTIFACT TO PRODUCE: write to `{}` via file_write.\n",
            run.goal, run.id, artifact_rel
        );

        // Fold clarification answers into the task context when present.
        // Without this the Writer would never see the user's choices and
        // would re-decide them on its own — defeating the whole point of
        // asking. Format as Q → A pairs so the model can scan fast.
        if !run.clarifications.is_empty() {
            ctx.push_str("\nCLARIFICATIONS (user answers to SPECIFY questions):\n");
            for c in &run.clarifications {
                ctx.push_str(&format!("- Q: {}\n  A: {}\n", c.question, c.answers.join(", ")));
            }
        }
        if let Some(p) = &run.spec_path
            && let Ok(rel) = PathBuf::from(p)
                .strip_prefix(&worktree_path)
                .map(|x| x.display().to_string())
            {
                ctx.push_str(&format!("\nSPEC reference: `{rel}` (read first)\n"));
            }
        if let Some(p) = &run.design_path
            && let Ok(rel) = PathBuf::from(p)
                .strip_prefix(&worktree_path)
                .map(|x| x.display().to_string())
            {
                ctx.push_str(&format!("DESIGN reference: `{rel}` (read first)\n"));
            }
        let task = format!("{prompt_extra}\n\n---\n\n{ctx}");

        let agent = self
            .agents
            .spawn(SpawnAgentRequest {
                kind: AgentKind::Writer,
                task,
                tier: None,
                worktree_id: Some(run.worktree_id.clone()),
                base: None,
                budget_usd: None,
                max_iters: Some(DEFAULT_MAX_ITERS),
                // SDD phase agents MUST NOT be YOLO. YOLO injects a "Chat
                // Mode" system prompt that tells the model to return its
                // answer via done("...") instead of writing files — which
                // breaks SPECIFY/DESIGN/TASKS, all of which require
                // file_write(spec.md|design.md|tasks.json) as the deliverable.
                // Bug observed 2026-05-26: writer returned empty done("")
                // after one iter, gates failed, 3 retries exhausted, halt.
                yolo_mode: Some(false),
                background: None,
                timeout_secs: Some(PHASE_TIMEOUT_SECS),
            })
            .await
            .context("phase agent spawn failed")?;
        let agent_uuid = Uuid::parse_str(&agent.id).context("invalid agent uuid")?;

        let deadline = Instant::now() + Duration::from_secs(PHASE_TIMEOUT_SECS);
        let mut cost = 0.0_f64;
        loop {
            if Instant::now() > deadline {
                return Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: format!("phase {phase:?} timeout"),
                    duration_ms: ms(phase_started),
                    cost_usd: cost,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                });
            }
            let Some(a) = self.agents.get(agent_uuid) else {
                return Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: "phase agent vanished".into(),
                    duration_ms: ms(phase_started),
                    cost_usd: cost,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                });
            };
            if matches!(
                a.status,
                AgentStatus::Completed
                    | AgentStatus::Failed
                    | AgentStatus::Stopped
                    | AgentStatus::Cancelled
                    | AgentStatus::Stale
            ) {
                cost = a.usd_spent;
                run.usd_spent += cost;
                break;
            }
            sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        }

        if !artifact_abs.exists() {
            return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: format!("phase {phase:?} produced no artifact at {filename}"),
                duration_ms: ms(phase_started),
                cost_usd: cost,
                artifact_path: None,
                fields: serde_json::Map::new(),
            });
        }

        let abs_str = artifact_abs.display().to_string();
        match phase {
            SddPhase::Specify => run.spec_path = Some(abs_str.clone()),
            SddPhase::Design => run.design_path = Some(abs_str.clone()),
            SddPhase::Tasks => run.tasks_path = Some(abs_str.clone()),
            _ => {}
        }

        let duration = ms(phase_started);
        run.phase_verdicts.push(PhaseVerdict {
            phase,
            judge_score: None,
            gate_overall: "pass".into(),
            artifact_path: Some(abs_str.clone()),
            duration_ms: duration,
        });
        run.phases_run.push(phase);
        run.updated_at = Utc::now().to_rfc3339();
        run.current_phase = match phase {
            SddPhase::Specify => SddPhase::Design,
            SddPhase::Design => SddPhase::Tasks,
            SddPhase::Tasks => SddPhase::Execute,
            _ => SddPhase::Failed,
        };

        Ok(StepResult {
            outcome: StepOutcome::Ok,
            message: format!("{phase:?} → {filename}"),
            duration_ms: duration,
            cost_usd: cost,
            artifact_path: Some(abs_str),
            fields: serde_json::Map::new(),
        })
    }


    /// EXECUTE phase with wave-based parallel execution.
    ///
    /// Parses tasks.json, builds a dependency graph, plans waves, and executes
    /// them in parallel using the WaveExecutor. Falls back to serial execution
    /// if tasks.json doesn't contain dependency information.
    #[allow(dead_code)]
    pub async fn run_execute_waves(
        &self,
        run: &mut SddRun,
        started: Instant,
        config: &crate::config::WavesConfig,
    ) -> Result<StepResult> {
        use crate::sdd::wave_executor::{self, WaveExecutorConfig};
        use crate::sdd::wave_spawner::AgentTaskSpawner;
        use crate::sdd::waves::{self, FailurePolicy, TaskDef};

        let Some(tasks_path) = run.tasks_path.clone() else {
            return Ok(fail("EXECUTE without tasks_path", started));
        };
        let raw = match std::fs::read_to_string(&tasks_path) {
            Ok(s) => s,
            Err(e) => return Ok(fail(&format!("read tasks.json: {e}"), started)),
        };
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => return Ok(fail(&format!("tasks.json invalid JSON: {e}"), started)),
        };
        let Some(tasks) = parsed.get("tasks").and_then(|v| v.as_array()) else {
            return Ok(fail("tasks.json missing `tasks` array", started));
        };

        // Parse tasks into TaskDefs with dependencies
        let task_defs: Vec<TaskDef> = tasks
            .iter()
            .map(|t| {
                let id_str = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let task_id = Uuid::parse_str(id_str).unwrap_or_else(|_| Uuid::now_v7());
                let description = format!(
                    "{}\n\n{}",
                    t.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                );
                let dependencies: Vec<Uuid> = t
                    .get("dependencies")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|d| d.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                            .collect()
                    })
                    .unwrap_or_default();

                TaskDef {
                    task_id,
                    description,
                    dependencies,
                }
            })
            .collect();

        // Plan waves
        let wave_plan = match waves::plan(&task_defs) {
            Ok(p) => p,
            Err(e) => return Ok(fail(&format!("wave planning failed: {e}"), started)),
        };

        pillar::emit_full(
            &self.pillar_tx,
            Pillar::P2Coordination,
            "step",
            format!(
                "EXECUTE (waves): {} tasks in {} waves",
                task_defs.len(),
                wave_plan.waves.len()
            ),
            Some(run.id.clone()),
            None,
            serde_json::Map::new(),
        );

        // Execute waves
        let spawner = AgentTaskSpawner::new(self.agents.clone(), run.worktree_id.clone());
        let exec_config = WaveExecutorConfig {
            max_parallel_agents: config.max_parallel_agents,
            failure_policy: FailurePolicy::from_str(&config.failure_policy),
            retry_max_attempts: config.retry_max_attempts,
        };

        let result_plan = wave_executor::execute(wave_plan, &spawner, &exec_config, Some(&self.pillar_tx)).await;

        // Summarize results
        let total_tasks = result_plan
            .waves
            .iter()
            .flat_map(|w| &w.tasks)
            .count();
        let completed_tasks = result_plan
            .waves
            .iter()
            .flat_map(|w| &w.tasks)
            .filter(|t| t.status == crate::sdd::waves::TaskStatus::Completed)
            .count();
        let all_ok = completed_tasks == total_tasks;

        run.phase_verdicts.push(PhaseVerdict {
            phase: SddPhase::Execute,
            judge_score: None,
            gate_overall: if all_ok { "pass" } else { "fail" }.into(),
            artifact_path: run.execution_log_path.clone(),
            duration_ms: ms(started),
        });
        run.phases_run.push(SddPhase::Execute);
        run.current_phase = if all_ok { SddPhase::Done } else { SddPhase::Failed };
        run.finished_at = Some(Utc::now().to_rfc3339());
        run.updated_at = run.finished_at.clone().unwrap_or_default();

        let mut fields = serde_json::Map::new();
        fields.insert("waves".into(), serde_json::json!(result_plan.waves.len()));
        fields.insert("total_tasks".into(), serde_json::json!(total_tasks));
        fields.insert("completed_tasks".into(), serde_json::json!(completed_tasks));

        Ok(StepResult {
            outcome: if all_ok { StepOutcome::Ok } else { StepOutcome::Failed },
            message: format!(
                "EXECUTE (waves): {completed_tasks}/{total_tasks} tasks completed across {} waves",
                result_plan.waves.len()
            ),
            duration_ms: ms(started),
            cost_usd: 0.0, // TODO: aggregate from agent costs
            artifact_path: run.execution_log_path.clone(),
            fields,
        })
    }

    // ── Generic workflow execution (WorkflowPilot path) ───────────────────

    /// Execute a generic `Intent` against `WorkflowRun`.
    /// Called by the new `WorkflowManager` drive loop.
    #[instrument(skip(self, run, intent, def), fields(run_id = %run.id))]
    pub async fn execute_workflow(
        &self,
        run: &mut WorkflowRun,
        intent: Intent,
        def: &Arc<WorkflowDef>,
    ) -> Result<StepResult> {
        let started = Instant::now();
        pillar::emit_full(
            &self.pillar_tx,
            Pillar::P2Coordination,
            "step",
            format!("Watari executing: {}", intent_label(&intent)),
            Some(run.id.clone()),
            None,
            serde_json::Map::new(),
        );

        match intent {
            Intent::RunPhase { ref phase_id } => {
                let Some(phase_def) = def.phase(phase_id) else {
                    return Ok(StepResult {
                        outcome: StepOutcome::Failed,
                        message: format!("phase not found in workflow: {phase_id}"),
                        duration_ms: ms(started),
                        cost_usd: 0.0,
                        artifact_path: None,
                        fields: serde_json::Map::new(),
                    });
                };
                self.run_workflow_phase(run, phase_def.id.clone(), &phase_def.prompt, phase_def.produces.clone(), phase_def.clarify, started).await
            }
            Intent::RunExecute { phase_id: _ } => {
                self.run_workflow_execute(run, started).await
            }
            Intent::EscalateTier { tier: _, reason } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: format!("escalation requested: {reason} (v0 no-op)"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::RequestHumanReview { question } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: format!("awaiting human: {question}"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::AwaitUserInput { .. } => Ok(StepResult {
                outcome: StepOutcome::Pending,
                message: "awaiting user input".into(),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
            Intent::Done => {
                run.current_phase_id = PHASE_DONE.into();
                run.finished_at = Some(Utc::now().to_rfc3339());
                Ok(StepResult {
                    outcome: StepOutcome::Ok,
                    message: "terminal: done".into(),
                    duration_ms: ms(started),
                    cost_usd: 0.0,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                })
            }
            Intent::Halt { reason } => {
                run.current_phase_id = PHASE_FAILED.into();
                run.finished_at = Some(Utc::now().to_rfc3339());
                if run.error.is_none() {
                    run.error = Some(reason.clone());
                }
                Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: format!("halt: {reason}"),
                    duration_ms: ms(started),
                    cost_usd: 0.0,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                })
            }
            // Legacy SDD intents should not reach the workflow path.
            Intent::RunSddPhase { .. } | Intent::RunSddExecute => Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: "legacy SDD intent reached execute_workflow() — use execute()".into(),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
        }
    }

    /// Run a Writer-kind phase for a WorkflowRun.
    async fn run_workflow_phase(
        &self,
        run: &mut WorkflowRun,
        phase_id: String,
        prompt_key: &str,
        produces: Option<String>,
        clarify: bool,
        phase_started: Instant,
    ) -> Result<StepResult> {
        // Clarify gate — only on first entry (no prior clarifications) for phases
        // that opt-in via `clarify = true`.
        if clarify && run.clarifications.is_empty() && !run.phases_run.contains(&phase_id) {
            let kind_hint = if run.kind.is_empty() { crate::proto::sdd::SddKind::Unknown }
                else { serde_json::from_value(serde_json::Value::String(run.kind.clone()))
                    .unwrap_or(crate::proto::sdd::SddKind::Unknown) };
            match crate::sdd::clarify::generate_clarifying_questions(
                &self.router,
                &run.goal,
                kind_hint,
            ).await {
                Ok(questions) if !questions.is_empty() => {
                    run.pending_questions = questions;
                    return Ok(StepResult {
                        outcome: StepOutcome::Pending,
                        message: format!("awaiting user clarifications before {phase_id}"),
                        duration_ms: ms(phase_started),
                        cost_usd: 0.0,
                        artifact_path: None,
                        fields: serde_json::Map::new(),
                    });
                }
                _ => {}
            }
        }

        // Load the prompt text.
        let prompt_text = load_prompt_by_key(prompt_key)?;
        let artifact_name = produces.clone().unwrap_or_else(|| format!("{phase_id}.out"));
        let worktree_path = PathBuf::from(&run.worktree_path);
        let run_dir = worktree_path.join(".kira").join("runs").join(&run.id);
        std::fs::create_dir_all(&run_dir).context("create run dir")?;
        let artifact_abs = run_dir.join(&artifact_name);
        let artifact_rel = artifact_abs
            .strip_prefix(&worktree_path)
            .map_or_else(|_| artifact_abs.display().to_string(), |p| p.display().to_string());

        let mut ctx = format!(
            "GOAL: {}\n\nRUN DIR (worktree-relative): .kira/runs/{}/\n\nARTIFACT TO PRODUCE: write to `{}` via file_write.\n",
            run.goal, run.id, artifact_rel
        );

        if !run.clarifications.is_empty() {
            ctx.push_str("\nCLARIFICATIONS:\n");
            for c in &run.clarifications {
                ctx.push_str(&format!("- Q: {}\n  A: {}\n", c.question, c.answers.join(", ")));
            }
        }
        for (name, path) in &run.artifacts {
            if let Ok(rel) = PathBuf::from(path)
                .strip_prefix(&worktree_path)
                .map(|x| x.display().to_string())
            {
                ctx.push_str(&format!("\nArtifact `{name}`: `{rel}` (read first)\n"));
            }
        }

        let task = format!("{prompt_text}\n\n---\n\n{ctx}");
        let agent = self.agents.spawn(SpawnAgentRequest {
            kind: AgentKind::Writer,
            task,
            tier: None,
            worktree_id: Some(run.worktree_id.clone()),
            base: None,
            budget_usd: None,
            max_iters: Some(DEFAULT_MAX_ITERS),
            yolo_mode: Some(false),
            background: None,
            timeout_secs: Some(PHASE_TIMEOUT_SECS),
        }).await.context("phase agent spawn failed")?;

        let agent_uuid = Uuid::parse_str(&agent.id).context("invalid agent uuid")?;
        let deadline = Instant::now() + Duration::from_secs(PHASE_TIMEOUT_SECS);
        let mut cost = 0.0_f64;
        loop {
            if Instant::now() > deadline {
                return Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: format!("phase {phase_id} timeout"),
                    duration_ms: ms(phase_started),
                    cost_usd: cost,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                });
            }
            let Some(a) = self.agents.get(agent_uuid) else {
                return Ok(StepResult {
                    outcome: StepOutcome::Failed,
                    message: "phase agent vanished".into(),
                    duration_ms: ms(phase_started),
                    cost_usd: cost,
                    artifact_path: None,
                    fields: serde_json::Map::new(),
                });
            };
            if matches!(a.status, AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Stopped | AgentStatus::Cancelled | AgentStatus::Stale) {
                cost = a.usd_spent;
                run.usd_spent += cost;
                break;
            }
            sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        }

        if !artifact_abs.exists() {
            return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: format!("phase {phase_id} produced no artifact at {artifact_name}"),
                duration_ms: ms(phase_started),
                cost_usd: cost,
                artifact_path: None,
                fields: serde_json::Map::new(),
            });
        }

        let abs_str = artifact_abs.display().to_string();
        run.set_artifact(&artifact_name, &abs_str);
        run.phase_verdicts.push(RunPhaseVerdict {
            phase_id: phase_id.clone(),
            judge_score: None,
            gate_overall: "pass".into(),
            artifact_path: Some(abs_str.clone()),
            duration_ms: ms(phase_started),
        });
        run.phases_run.push(phase_id.clone());
        run.updated_at = Utc::now().to_rfc3339();

        Ok(StepResult {
            outcome: StepOutcome::Ok,
            message: format!("{phase_id} → {artifact_name}"),
            duration_ms: ms(phase_started),
            cost_usd: cost,
            artifact_path: Some(abs_str),
            fields: serde_json::Map::new(),
        })
    }

    /// EXECUTE phase for WorkflowRun: parse tasks.json, plan Waves, execute.
    async fn run_workflow_execute(
        &self,
        run: &mut WorkflowRun,
        started: Instant,
    ) -> Result<StepResult> {
        let Some(tasks_path) = run.tasks_path().map(str::to_owned) else {
            return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: "EXECUTE without tasks.json artifact".into(),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            });
        };
        let raw = match std::fs::read_to_string(&tasks_path) {
            Ok(s) => s,
            Err(e) => return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: format!("read tasks.json: {e}"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
        };
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: format!("tasks.json invalid JSON: {e}"),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            }),
        };
        let Some(tasks) = parsed.get("tasks").and_then(|v| v.as_array()) else {
            return Ok(StepResult {
                outcome: StepOutcome::Failed,
                message: "tasks.json missing `tasks` array".into(),
                duration_ms: ms(started),
                cost_usd: 0.0,
                artifact_path: None,
                fields: serde_json::Map::new(),
            });
        };

        let execution_log = run.execution_log_path.clone().unwrap_or_default();
        let execution_log_path = PathBuf::from(&execution_log);
        // Parallel wave execution: decompose tasks by their `dependencies`,
        // run independent tasks concurrently (one agent each, shared worktree),
        // barrier between waves.
        use crate::sdd::wave_executor::{self, WaveExecutorConfig};
        use crate::sdd::wave_spawner::AgentTaskSpawner;
        use crate::sdd::waves::{self, FailurePolicy, TaskDef, TaskStatus};

        // String task ids → uuids so `dependencies` resolve to real tasks.
        let mut id_map: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();
        for t in tasks {
            let sid = t.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            id_map.entry(sid).or_insert_with(Uuid::now_v7);
        }
        let task_defs: Vec<TaskDef> = tasks
            .iter()
            .map(|t| {
                let id = t.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let title = t.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let description = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let acs = t.get("acceptance_criteria").and_then(|v| v.as_array()).map_or_else(
                    || "  (none)".to_string(),
                    |arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .enumerate()
                            .map(|(i, s)| format!("  {}. {s}", i + 1))
                            .collect::<Vec<_>>()
                            .join("\n")
                    },
                );
                // Schema emits `depends_on` (string ids). Reading `dependencies`
                // here silently dropped all edges → every task collapsed to wave 0.
                let dependencies: Vec<Uuid> = t
                    .get("depends_on")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|d| d.as_str())
                            .filter_map(|d| id_map.get(d).copied())
                            .collect()
                    })
                    .unwrap_or_default();
                TaskDef {
                    task_id: id_map.get(id).copied().unwrap_or_else(Uuid::now_v7),
                    description: format!(
                        "Task {id}: {title}\n\n{description}\n\nAcceptance criteria:\n{acs}\n"
                    ),
                    dependencies,
                }
            })
            .collect();

        let wave_plan = match waves::plan(&task_defs) {
            Ok(p) => p,
            Err(e) => return Ok(fail(&format!("wave planning failed: {e}"), started)),
        };
        let wave_count = wave_plan.waves.len();
        tracing::info!(tasks = task_defs.len(), waves = wave_count, "EXECUTE: parallel waves");

        let spawner = AgentTaskSpawner::new(self.agents.clone(), run.worktree_id.clone());
        let exec_cfg = WaveExecutorConfig {
            max_parallel_agents: 3,
            failure_policy: FailurePolicy::Continue,
            retry_max_attempts: 0,
        };
        let result_plan = wave_executor::execute(wave_plan, &spawner, &exec_cfg, Some(&self.pillar_tx)).await;

        // Aggregate status + cost; log each task.
        let total = task_defs.len();
        let mut completed = 0usize;
        let mut cost = 0.0_f64;
        for w in &result_plan.waves {
            for t in &w.tasks {
                if t.status == TaskStatus::Completed {
                    completed += 1;
                }
                if let Some(aid) = t.agent_id
                    && let Some(a) = self.agents.get(aid)
                {
                    cost += a.usd_spent;
                }
                append_jsonl(
                    &execution_log_path,
                    serde_json::json!({
                        "task_id": t.task_id.to_string(),
                        "status": format!("{:?}", t.status).to_lowercase(),
                        "agent_id": t.agent_id.map(|a| a.to_string()),
                    }),
                );
            }
        }
        let all_ok = completed == total;

        run.usd_spent += cost;
        run.phase_verdicts.push(RunPhaseVerdict {
            phase_id: "execute".into(),
            judge_score: None,
            gate_overall: if all_ok { "pass" } else { "fail" }.into(),
            artifact_path: run.execution_log_path.clone(),
            duration_ms: ms(started),
        });
        run.phases_run.push("execute".into());
        run.current_phase_id = if all_ok { PHASE_DONE } else { PHASE_FAILED }.into();
        run.finished_at = Some(Utc::now().to_rfc3339());
        run.updated_at = run.finished_at.clone().unwrap_or_default();

        Ok(StepResult {
            outcome: if all_ok { StepOutcome::Ok } else { StepOutcome::Failed },
            message: format!("EXECUTE: {} tasks ({} ok)", tasks.len(), if all_ok { "all" } else { "partial" }),
            duration_ms: ms(started),
            cost_usd: cost,
            artifact_path: run.execution_log_path.clone(),
            fields: serde_json::Map::new(),
        })
    }
}

fn ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn fail(msg: &str, started: Instant) -> StepResult {
    StepResult {
        outcome: StepOutcome::Failed,
        message: msg.into(),
        duration_ms: ms(started),
        cost_usd: 0.0,
        artifact_path: None,
        fields: serde_json::Map::new(),
    }
}

fn intent_label(intent: &Intent) -> String {
    match intent {
        Intent::RunPhase { phase_id } => format!("RunPhase({phase_id})"),
        Intent::RunExecute { phase_id } => format!("RunExecute({phase_id})"),
        Intent::RunSddPhase { phase } => format!("RunSddPhase({phase:?})"),
        Intent::RunSddExecute => "RunSddExecute".into(),
        Intent::EscalateTier { tier, .. } => format!("EscalateTier({tier:?})"),
        Intent::RequestHumanReview { .. } => "RequestHumanReview".into(),
        Intent::AwaitUserInput { questions } => format!("AwaitUserInput({})", questions.len()),
        Intent::Done => "Done".into(),
        Intent::Halt { .. } => "Halt".into(),
    }
}

/// Load prompt text by key, e.g. `"sdd/specify.md"`.
///
/// Currently resolves keys baked into the `sdd::prompts` constants. Future:
/// look up from the bundled prompts dir for arbitrary keys.
fn load_prompt_by_key(key: &str) -> Result<&'static str> {
    match key {
        "sdd/specify.md" => Ok(crate::sdd::prompts::SPECIFY),
        "sdd/design.md"  => Ok(crate::sdd::prompts::DESIGN),
        "sdd/tasks.md"   => Ok(crate::sdd::prompts::TASKS),
        other => anyhow::bail!("unknown prompt key: {other}"),
    }
}

fn append_jsonl(path: &std::path::Path, value: serde_json::Value) {
    use std::io::Write;
    if path.as_os_str().is_empty() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = match serde_json::to_string(&value) {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}
