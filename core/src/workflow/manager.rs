//! WorkflowManager — drive loop for any WorkflowDef.
//!
//! Counterpart to `SddManager`. Loads a `WorkflowDef` from the registry,
//! applies sizing, and drives the Pilot→Orchestrator loop until terminal.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tracing::{instrument, warn};
use uuid::Uuid;

use crate::agents::AgentManager;
use crate::llm::embed_cache::EmbedCache;
use crate::observability::pillar::{self, PillarTx};
use crate::proto::intent::Intent;
use crate::proto::pillar::Pillar;
use crate::proto::run::{
    StartWorkflowRequest, WorkflowRun, PHASE_DONE, PHASE_FAILED,
};
use crate::proto::sdd::AnswerQuestionsRequest;
use crate::router::Router;
use crate::state::{KiraDb, TBL_WORKFLOW};
use crate::worktree::WorktreeManager;
use crate::workflow::def::WorkflowDef;
use crate::workflow::loader;
use crate::workflow::pilot::WorkflowPilot;
use crate::workflow::registry::{ServiceRegistry, SizingHint};

#[derive(Clone)]
pub struct WorkflowManager {
    agents: AgentManager,
    worktrees: WorktreeManager,
    router: Router,
    embed_cache: EmbedCache,
    pillar_tx: PillarTx,
    db: Option<KiraDb>,
    registry: Arc<ServiceRegistry>,
    /// Loaded workflow definitions (id → def). Loaded once at startup.
    defs: Arc<HashMap<String, Arc<WorkflowDef>>>,
    runs: Arc<Mutex<HashMap<String, WorkflowRun>>>,
    notifiers: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
}

impl WorkflowManager {
    pub async fn new(
        agents: AgentManager,
        worktrees: WorktreeManager,
        router: Router,
        embed_cache: EmbedCache,
        pillar_tx: PillarTx,
        db: Option<KiraDb>,
        registry: ServiceRegistry,
        workflow_dir: Option<&std::path::Path>,
    ) -> Self {
        // Load all bundled workflows.
        let mut defs: HashMap<String, Arc<WorkflowDef>> = HashMap::new();
        let bundled = loader::bundled_workflow_dir();
        if let Some(dir) = workflow_dir.or_else(|| bundled.as_deref()) {
            match loader::load_all(dir) {
                Ok(loaded) => {
                    for def in loaded {
                        tracing::info!(workflow = %def.id, "workflow loaded");
                        defs.insert(def.id.clone(), Arc::new(def));
                    }
                }
                Err(e) => warn!(error = ?e, "workflow dir load failed"),
            }
        }

        let runs = Arc::new(Mutex::new(HashMap::new()));
        if let Some(d) = &db {
            if let Ok(list) = d.list::<WorkflowRun>(TBL_WORKFLOW) {
                let mut map = runs.lock().await;
                for r in list {
                    map.insert(r.id.clone(), r);
                }
            }
        }

        Self {
            agents,
            worktrees,
            router,
            embed_cache,
            pillar_tx,
            db,
            registry: Arc::new(registry),
            defs: Arc::new(defs),
            runs,
            notifiers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn list_workflow_ids(&self) -> Vec<String> {
        self.defs.keys().cloned().collect()
    }

    pub async fn list(&self) -> Vec<WorkflowRun> {
        self.runs.lock().await.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<WorkflowRun> {
        self.runs.lock().await.get(id).cloned()
    }

    pub async fn get_questions(&self, id: &str) -> Vec<crate::proto::sdd::PlanQuestion> {
        self.runs.lock().await.get(id).map(|r| r.pending_questions.clone()).unwrap_or_default()
    }

    pub async fn answer_questions(&self, id: &str, req: AnswerQuestionsRequest) -> bool {
        {
            let mut map = self.runs.lock().await;
            let Some(run) = map.get_mut(id) else { return false };
            for q in &run.pending_questions {
                if let Some(answers) = req.answers.get(&q.id) {
                    run.clarifications.push(crate::proto::run::ClarificationEntry {
                        question: q.question.clone(),
                        answers: answers.clone(),
                    });
                }
            }
            run.question_answers.extend(req.answers);
            run.pending_questions.clear();
            if let Some(db) = &self.db {
                if let Err(e) = db.put(TBL_WORKFLOW, &run.id, run) {
                    warn!(error = ?e, "workflow answer_questions persist failed");
                }
            }
        }
        if let Some(n) = self.notifiers.lock().await.get(id).cloned() {
            n.notify_one();
        }
        true
    }

    /// Build the plan-approval matrix (tasks + waves + files) from the run's
    /// `tasks.json` artifact. Empty tasks if it hasn't been produced yet.
    pub async fn get_plan(&self, id: &str) -> Option<crate::sdd::plan_view::PlanView> {
        let run = self.runs.lock().await.get(id).cloned()?;
        let parsed = run
            .tasks_json_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .unwrap_or_else(|| serde_json::json!({ "tasks": [] }));
        Some(crate::sdd::plan_view::build_plan_view(&run.id, &run.goal, &parsed))
    }

    /// Resolve a pending plan-approval gate. `approved=true` clears the pause
    /// and lets EXECUTE proceed; `approved=false` halts the run (failed).
    /// Returns `false` if the run is not found or wasn't awaiting approval.
    pub async fn approve_plan(&self, id: &str, approved: bool) -> bool {
        {
            let mut map = self.runs.lock().await;
            let Some(run) = map.get_mut(id) else { return false };
            if !run.awaiting_plan_approval {
                return false;
            }
            run.awaiting_plan_approval = false;
            if !approved {
                run.current_phase_id = PHASE_FAILED.into();
                run.error = Some("plan rejected by operator".into());
            }
            run.updated_at = Utc::now().to_rfc3339();
            if let Some(db) = &self.db {
                if let Err(e) = db.put(TBL_WORKFLOW, &run.id, run) {
                    warn!(error = ?e, "workflow approve_plan persist failed");
                }
            }
        }
        if let Some(n) = self.notifiers.lock().await.get(id).cloned() {
            n.notify_one();
        }
        true
    }

    async fn persist(&self, run: &WorkflowRun) {
        if let Some(db) = &self.db {
            if let Err(e) = db.put(TBL_WORKFLOW, &run.id, run) {
                warn!(error = ?e, "workflow persist failed");
            }
        }
        self.runs.lock().await.insert(run.id.clone(), run.clone());
    }

    async fn notifier_for(&self, id: &str) -> Arc<Notify> {
        let mut map = self.notifiers.lock().await;
        map.entry(id.to_string()).or_insert_with(|| Arc::new(Notify::new())).clone()
    }

    /// Start a new workflow run. Returns the initial snapshot immediately;
    /// drives phases on a detached `tokio::spawn`.
    #[instrument(skip(self, req), fields(goal_len = req.goal.len()))]
    pub async fn start(&self, req: StartWorkflowRequest) -> Result<WorkflowRun> {
        let def = self.defs.get(&req.workflow)
            .cloned()
            .with_context(|| format!("unknown workflow: {} (available: {})",
                req.workflow,
                self.defs.keys().cloned().collect::<Vec<_>>().join(", ")))?;

        // Apply sizing.
        let skipped_phases = if let Some(sizing_id) = &def.sizing {
            if let Some(sizer) = self.registry.sizer(sizing_id) {
                let hint = SizingHint {
                    estimated_loc: req.estimated_loc,
                    estimated_files: req.estimated_files,
                    force_full: req.force_full_pipeline.unwrap_or(false),
                    kind: req.kind.clone(),
                };
                sizer.skippable_phases(&hint)
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Find the first non-skipped phase.
        let first_phase = def.phases.iter()
            .find(|p| !skipped_phases.contains(&p.id))
            .map(|p| p.id.clone())
            .unwrap_or_else(|| PHASE_DONE.into());

        // Resolve worktree.
        let (worktree_id, worktree_path) = if let Some(s) = req.worktree_id.as_deref() {
            let uid = Uuid::parse_str(s).context("invalid worktree_id")?;
            let wt = self.worktrees.get(uid)
                .with_context(|| format!("worktree not found: {uid}"))?;
            (uid.to_string(), PathBuf::from(wt.path))
        } else {
            let wt = self.worktrees.spawn(crate::proto::worktree::SpawnRequest {
                base: req.base.clone().unwrap_or_else(|| "main".to_string()),
                agent_kind: Some("workflow".to_string()),
            }).await.context("auto-spawn worktree failed")?;
            (wt.id.clone(), PathBuf::from(wt.path))
        };

        let id = Uuid::now_v7().to_string();
        let now = Utc::now().to_rfc3339();
        let run_dir = worktree_path.join(".kira").join("runs").join(&id);
        std::fs::create_dir_all(&run_dir).context("create run dir")?;

        let run = WorkflowRun {
            id: id.clone(),
            goal: req.goal.clone(),
            workflow_id: req.workflow.clone(),
            kind: req.kind.clone().unwrap_or_default(),
            current_phase_id: first_phase,
            phases_run: vec![],
            skipped_phases,
            artifacts: HashMap::new(),
            execution_log_path: Some(run_dir.join("execution.jsonl").display().to_string()),
            worktree_id,
            worktree_path: worktree_path.display().to_string(),
            phase_verdicts: vec![],
            usd_spent: 0.0,
            created_at: now.clone(),
            updated_at: now,
            finished_at: None,
            error: None,
            phase_retries: HashMap::new(),
            pending_questions: vec![],
            clarifications: vec![],
            question_answers: HashMap::new(),
            require_plan_approval: req.require_plan_approval.unwrap_or(false),
            awaiting_plan_approval: false,
            research_notes: None,
        };
        self.persist(&run).await;

        pillar::emit(&self.pillar_tx, Pillar::P1Sdd, "start",
            format!("Workflow run {} · wf={} · first={}", &id[..8.min(id.len())], req.workflow, run.current_phase_id));

        let driver = self.clone();
        let snapshot = run.clone();
        tokio::spawn(async move {
            if let Err(e) = driver.drive(snapshot, def).await {
                warn!(error = ?e, "workflow drive failed");
            }
        });

        Ok(run)
    }

    /// Drive loop — Pilot→Watari→observe until terminal.
    async fn drive(&self, mut run: WorkflowRun, def: Arc<WorkflowDef>) -> Result<()> {
        use crate::orchestrator::Orchestrator;

        let pilot = WorkflowPilot::new(def.clone());
        let orch = Orchestrator::new(self.agents.clone(), self.pillar_tx.clone(), self.router.clone());
        let notifier = self.notifier_for(&run.id).await;

        loop {
            if !run.pending_questions.is_empty() {
                use tokio::time::{timeout, Duration};
                let _ = timeout(Duration::from_mins(30), notifier.notified()).await;
                if let Some(fresh) = self.get(&run.id).await {
                    run = fresh;
                }
                continue;
            }

            // Plan-approval gate: pause before EXECUTE fans out waves.
            // Mirrors the pending_questions wait — blocks until `approve_plan`
            // clears the flag (or rejects → current_phase_id=failed).
            if run.awaiting_plan_approval {
                use tokio::time::{timeout, Duration};
                pillar::emit(&self.pillar_tx, Pillar::P1Sdd, "await_approval",
                    format!("Workflow {} awaiting plan approval", &run.id[..8.min(run.id.len())]));
                let _ = timeout(Duration::from_mins(30), notifier.notified()).await;
                if let Some(fresh) = self.get(&run.id).await {
                    run = fresh;
                }
                continue;
            }

            let intent = pilot.next_intent(&run).await;

            if let Intent::AwaitUserInput { questions } = intent {
                run.pending_questions = questions;
                self.persist(&run).await;
                continue;
            }

            let terminal = matches!(intent, Intent::Done | Intent::Halt { .. });
            let result = match orch.execute_workflow(&mut run, intent, &def).await {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = ?e, "watari execute_workflow errored");
                    run.error = Some(format!("watari: {e}"));
                    run.current_phase_id = PHASE_FAILED.into();
                    self.persist(&run).await;
                    break;
                }
            };
            pilot.observe(&mut run, &result).await;
            self.persist(&run).await;
            if terminal || run.is_terminal() {
                break;
            }
        }

        pillar::emit(&self.pillar_tx, Pillar::P1Sdd,
            if run.current_phase_id == PHASE_DONE { "done" } else { "fail" },
            format!("Workflow {} terminal: {}", &run.id[..8.min(run.id.len())], run.current_phase_id));
        Ok(())
    }
}
