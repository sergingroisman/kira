//! Spec-Driven Execution (P1) — 4-phase pipeline with auto-sizing.
//!
//! ```text
//!     ┌──────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
//!     │ SPECIFY  │ → │ DESIGN  │ → │ TASKS   │ → │ EXECUTE │
//!     │ spec.md  │   │ design  │   │ tasks   │   │ per-task│
//!     │          │   │ .md     │   │ .json   │   │ agents  │
//!     └─┬────────┘   └─┬───────┘   └─┬───────┘   └────┬────┘
//!       │ judge        │ judge       │ judge          │ gates per task
//!       └─ auto-skip if trivial ─────┘                │
//!                                                     ↓
//!                                              execution.jsonl
//! ```
//!
//! Auto-sizing (v0 heuristic):
//! - `BugFix` + `estimated_loc ≤ 5` + `estimated_files ≤ 1` → `SkipSpecifyDesign`
//! - Else → Full
//!
//! Each phase writes its artifact via a spawned `Writer` agent with a
//! phase-specific prompt. Phase advances only if judge gate passes. Failure
//! halts and persists artifacts for inspection.

pub mod prompts;
pub mod clarify;
pub mod plan_view;
pub mod store;
pub mod wave_executor;
pub mod wave_spawner;
pub mod waves;

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
use crate::proto::pillar::Pillar;
use crate::proto::sdd::{AnswerQuestionsRequest, SddKind, SddPhase, SddRun, SizingVerdict, StartSddRequest};
use crate::proto::worktree::SpawnRequest as WorktreeSpawnRequest;
use crate::router::Router;
use crate::state::{KiraDb, TBL_SDD};
use crate::worktree::WorktreeManager;

/// Auto-sizing thresholds (v0 defaults; tokens.toml override later).
const TRIVIAL_LOC: u32 = 5;
const TRIVIAL_FILES: u32 = 1;

pub fn auto_size(req: &StartSddRequest) -> SizingVerdict {
    if req.force_full_pipeline.unwrap_or(false) {
        return SizingVerdict::Full;
    }
    let kind = req.kind.unwrap_or(SddKind::Unknown);
    let loc = req.estimated_loc.unwrap_or(u32::MAX);
    let files = req.estimated_files.unwrap_or(u32::MAX);
    if matches!(kind, SddKind::BugFix) && loc <= TRIVIAL_LOC && files <= TRIVIAL_FILES {
        SizingVerdict::SkipSpecifyDesign
    } else {
        SizingVerdict::Full
    }
}

#[derive(Clone)]
pub struct SddManager {
    pub agents: AgentManager,
    pub worktrees: WorktreeManager,
    pub router: Router,
    pub embed_cache: EmbedCache,
    pub pillar_tx: PillarTx,
    pub db: Option<KiraDb>,
    /// In-memory snapshot for fast list/get; mirror of redb.
    runs: Arc<Mutex<std::collections::HashMap<String, SddRun>>>,
    /// Per-run notifier — signalled when `question_answers` are submitted,
    /// unblocking the drive loop from its `AwaitUserInput` wait.
    notifiers: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
}

impl SddManager {
    pub async fn new(
        agents: AgentManager,
        worktrees: WorktreeManager,
        router: Router,
        embed_cache: EmbedCache,
        pillar_tx: PillarTx,
        db: Option<KiraDb>,
    ) -> Self {
        let runs = Arc::new(Mutex::new(std::collections::HashMap::new()));
        // Best-effort restore.
        if let Some(d) = &db
            && let Ok(list) = d.list::<SddRun>(TBL_SDD) {
                let mut map = runs.lock().await;
                for r in list {
                    map.insert(r.id.clone(), r);
                }
            }
        Self {
            agents,
            worktrees,
            router,
            embed_cache,
            pillar_tx,
            db,
            runs,
            notifiers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return pending questions for a run (empty vec if none or run not found).
    pub async fn get_questions(&self, id: &str) -> Vec<crate::proto::sdd::PlanQuestion> {
        let map = self.runs.lock().await;
        map.get(id).map(|r| r.pending_questions.clone()).unwrap_or_default()
    }

    /// Submit answers for a run's pending questions.
    /// Clears `pending_questions`, stores `question_answers`, notifies drive loop.
    /// Returns `false` if run not found.
    pub async fn answer_questions(&self, id: &str, req: AnswerQuestionsRequest) -> bool {
        {
            let mut map = self.runs.lock().await;
            let Some(run) = map.get_mut(id) else { return false; };

            // Resolve UUID → question text before we lose pending_questions.
            // The orchestrator reads `clarifications` (human-readable Q+A
            // pairs) when building the SPECIFY task; the UUID-keyed
            // `question_answers` map alone is opaque to the LLM.
            for q in &run.pending_questions {
                if let Some(answers) = req.answers.get(&q.id) {
                    run.clarifications.push(
                        crate::proto::sdd::ClarificationEntry {
                            question: q.question.clone(),
                            answers: answers.clone(),
                        },
                    );
                }
            }
            run.question_answers.extend(req.answers);
            run.pending_questions.clear();
            // Persist the updated run.
            if let Some(db) = &self.db
                && let Err(e) = db.put(TBL_SDD, &run.id, run) {
                    warn!(error = ?e, "sdd answer_questions persist failed");
                }
        }
        // Wake the drive loop.
        let notify = {
            let map = self.notifiers.lock().await;
            map.get(id).cloned()
        };
        if let Some(n) = notify {
            n.notify_one();
        }
        true
    }

    async fn notifier_for(&self, id: &str) -> Arc<Notify> {
        let mut map = self.notifiers.lock().await;
        map.entry(id.to_string()).or_insert_with(|| Arc::new(Notify::new())).clone()
    }

    pub async fn list(&self) -> Vec<SddRun> {
        let map = self.runs.lock().await;
        map.values().cloned().collect()
    }

    pub async fn get(&self, id: &str) -> Option<SddRun> {
        let map = self.runs.lock().await;
        map.get(id).cloned()
    }

    async fn persist(&self, run: &SddRun) {
        if let Some(db) = &self.db
            && let Err(e) = db.put(TBL_SDD, &run.id, run) {
                warn!(error = ?e, "sdd persist failed");
            }
        let mut map = self.runs.lock().await;
        map.insert(run.id.clone(), run.clone());
    }

    /// Kick off an SDD run. Returns the initial snapshot immediately;
    /// drives phases on a detached `tokio::spawn`.
    #[instrument(skip(self, req), fields(goal_len = req.goal.len()))]
    pub async fn start(&self, req: StartSddRequest) -> Result<SddRun> {
        let id = Uuid::now_v7().to_string();
        let sizing = auto_size(&req);
        let kind = req.kind.unwrap_or(SddKind::Unknown);

        // Resolve worktree.
        let (worktree_id, worktree_path) = if let Some(s) = req.worktree_id.as_deref() {
            let uid = Uuid::parse_str(s).context("invalid worktree_id")?;
            let wt = self
                .worktrees
                .get(uid)
                .with_context(|| format!("worktree not found: {uid}"))?;
            (uid.to_string(), PathBuf::from(wt.path))
        } else {
            let wt = self
                .worktrees
                .spawn(WorktreeSpawnRequest {
                    base: req.base.clone().unwrap_or_else(|| "main".to_string()),
                    agent_kind: Some("sdd".to_string()),
                })
                .await
                .context("auto-spawn worktree failed")?;
            (wt.id.clone(), PathBuf::from(wt.path))
        };

        let (phases_run, phases_skipped, first_phase) = match sizing {
            SizingVerdict::Full => (Vec::new(), Vec::new(), SddPhase::Specify),
            SizingVerdict::SkipSpecifyDesign | SizingVerdict::SingleShot => (
                Vec::new(),
                vec![SddPhase::Specify, SddPhase::Design],
                SddPhase::Tasks,
            ),
        };

        let now = Utc::now().to_rfc3339();
        let run_dir = run_dir_for(&worktree_path, &id);
        std::fs::create_dir_all(&run_dir).context("create run dir")?;

        let run = SddRun {
            id: id.clone(),
            goal: req.goal.clone(),
            kind,
            sizing,
            current_phase: first_phase,
            phases_run,
            phases_skipped,
            spec_path: None,
            design_path: None,
            tasks_path: None,
            execution_log_path: Some(run_dir.join("execution.jsonl").display().to_string()),
            worktree_id,
            worktree_path: worktree_path.display().to_string(),
            phase_verdicts: Vec::new(),
            usd_spent: 0.0,
            created_at: now.clone(),
            updated_at: now,
            finished_at: None,
            error: None,
            phase_retries: HashMap::default(),
            pending_questions: Vec::new(),
            clarifications: Vec::new(),
            question_answers: HashMap::default(),
        };
        self.persist(&run).await;

        pillar::emit(
            &self.pillar_tx,
            Pillar::P1Sdd,
            "start",
            format!(
                "SDD run {} · sizing={:?} · first_phase={:?}",
                &id[..8.min(id.len())],
                sizing,
                first_phase,
            ),
        );

        // Detach the driver. Survives client disconnects.
        let driver = self.clone();
        let initial_snapshot = run.clone();
        tokio::spawn(async move {
            if let Err(e) = driver.drive(initial_snapshot).await {
                warn!(error = ?e, "sdd drive failed");
            }
        });

        Ok(run)
    }

    /// Phase driver — Pilot decides Intent, Orquestrador executes (P2).
    ///
    /// Pure loop: ask Pilot → execute Intent → observe → persist → repeat
    /// until Pilot emits `Done` or `Halt`. Pilot/Orq isolation makes future
    /// refactors (workflow YAML, generic state machines) trivial.
    async fn drive(&self, mut run: SddRun) -> Result<()> {
        use crate::orchestrator::Orchestrator;
        use crate::pilot::{Pilot, SddPilot};
        use crate::proto::intent::Intent;

        let pilot = SddPilot;
        let orch = Orchestrator::new(
            self.agents.clone(),
            self.pillar_tx.clone(),
            self.router.clone(),
        );
        let notifier = self.notifier_for(&run.id).await;

        loop {
            // If questions are pending, wait for user answers (up to 30 min timeout).
            if !run.pending_questions.is_empty() {
                use tokio::time::{timeout, Duration};
                pillar::emit(
                    &self.pillar_tx,
                    Pillar::P1Sdd,
                    "await_input",
                    format!("SDD {} awaiting {} user questions", &run.id[..8.min(run.id.len())], run.pending_questions.len()),
                );
                let _ = timeout(Duration::from_mins(30), notifier.notified()).await;
                // Reload run from in-memory store to pick up submitted answers.
                if let Some(fresh) = self.get(&run.id).await {
                    run = fresh;
                }
                continue;
            }

            let intent = pilot.next_intent(&run).await;

            // AwaitUserInput: store questions and re-enter wait at top of loop.
            if let Intent::AwaitUserInput { questions } = intent {
                run.pending_questions = questions;
                self.persist(&run).await;
                continue;
            }

            // Terminal intents short-circuit (Orq just records).
            let terminal = matches!(intent, Intent::Done | Intent::Halt { .. });
            let result = match orch.execute(&mut run, intent).await {
                Ok(r) => r,
                Err(e) => {
                    warn!(error = ?e, "orchestrator execute errored");
                    run.error = Some(format!("orq: {e}"));
                    run.current_phase = SddPhase::Failed;
                    self.persist(&run).await;
                    break;
                }
            };
            pilot.observe(&mut run, &result).await;
            self.persist(&run).await;
            if terminal {
                break;
            }
            if matches!(run.current_phase, SddPhase::Done | SddPhase::Failed) {
                break;
            }
        }

        pillar::emit(
            &self.pillar_tx,
            Pillar::P1Sdd,
            if matches!(run.current_phase, SddPhase::Done) {
                "done"
            } else {
                "fail"
            },
            format!(
                "SDD {} terminal: {:?}",
                &run.id[..8.min(run.id.len())],
                run.current_phase
            ),
        );
        Ok(())
    }

}

fn run_dir_for(worktree_path: &std::path::Path, run_id: &str) -> PathBuf {
    worktree_path.join(".kira").join("runs").join(run_id)
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
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_size_trivial_bug_fix_skips() {
        let req = StartSddRequest {
            goal: "fix off-by-one".into(),
            kind: Some(SddKind::BugFix),
            estimated_loc: Some(3),
            estimated_files: Some(1),
            force_full_pipeline: None,
            worktree_id: None,
            base: None,
        };
        assert_eq!(auto_size(&req), SizingVerdict::SkipSpecifyDesign);
    }

    #[test]
    fn auto_size_feature_runs_full() {
        let req = StartSddRequest {
            goal: "add SSO".into(),
            kind: Some(SddKind::Feature),
            estimated_loc: Some(2),
            estimated_files: Some(1),
            force_full_pipeline: None,
            worktree_id: None,
            base: None,
        };
        assert_eq!(auto_size(&req), SizingVerdict::Full);
    }

    #[test]
    fn auto_size_force_full_overrides() {
        let req = StartSddRequest {
            goal: "trivial".into(),
            kind: Some(SddKind::BugFix),
            estimated_loc: Some(1),
            estimated_files: Some(1),
            force_full_pipeline: Some(true),
            worktree_id: None,
            base: None,
        };
        assert_eq!(auto_size(&req), SizingVerdict::Full);
    }

    #[test]
    fn auto_size_no_hints_defaults_full() {
        let req = StartSddRequest {
            goal: "do thing".into(),
            kind: None,
            estimated_loc: None,
            estimated_files: None,
            force_full_pipeline: None,
            worktree_id: None,
            base: None,
        };
        assert_eq!(auto_size(&req), SizingVerdict::Full);
    }

    #[test]
    fn run_dir_path_layout() {
        let p = run_dir_for(std::path::Path::new("/tmp/wt"), "abc");
        assert_eq!(p, PathBuf::from("/tmp/wt/.kira/runs/abc"));
    }
}
