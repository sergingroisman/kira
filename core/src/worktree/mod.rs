//! Worktree manager — git worktree lifecycle.
//!
//! Layout: `<repo_root>/.worktrees/wt-<short-uuid>/`
//! Branch: `kira/wt-<short-uuid>`
//!
//! v0 capabilities:
//!   - spawn  → semaphore acquire, git worktree add, register in store
//!   - destroy → git worktree remove --force, optional branch -D, store remove
//!   - list / get → from store
//!
//! v0.5 adds: heartbeat scanner, stale detection, auto-cleanup timer,
//! reconciliation on startup, checkpoint/merge/diff endpoints.

pub mod git;
pub mod store;

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::proto::worktree::{DestroyResponse, SpawnRequest, Worktree, WorktreeStatus};
use self::store::Store;

const WORKTREES_SUBDIR: &str = ".worktrees";
const BRANCH_PREFIX: &str = "kira/";
const WT_PREFIX: &str = "wt-";

/// Short UUID v7 (12 hex chars) — keeps directory + branch names readable.
fn short_id() -> String {
    let u = Uuid::now_v7();
    u.simple().to_string()[..12].to_string()
}

#[derive(Clone)]
pub struct WorktreeManager {
    /// Root of the host git repository (e.g. `~/Workspace/kira`).
    repo_root: PathBuf,
    store: Store,
    semaphore: Arc<Semaphore>,
}

impl WorktreeManager {
    /// Build a manager rooted at the toplevel of `start` (commonly `cwd`).
    /// Reconciles existing `kira/*` worktrees found on disk into the store as `Stale`.
    /// `db` is optional — when present, store is durable.
    /// `max_parallel` controls concurrent worktree operations (default 2).
    pub async fn new(
        start: PathBuf,
        db: Option<crate::state::KiraDb>,
        max_parallel: usize,
    ) -> Result<Self> {
        let repo_root = git::toplevel(&start)
            .await
            .with_context(|| format!("not inside a git repo: {}", start.display()))?;
        info!(repo_root = %repo_root.display(), durable = db.is_some(), "WorktreeManager rooted");

        let store = match db {
            Some(d) => Store::with_db(d),
            None => Store::new(),
        };
        let mgr = Self {
            repo_root: repo_root.clone(),
            store,
            semaphore: Arc::new(Semaphore::new(max_parallel.max(1))),
        };
        mgr.reconcile().await?;
        Ok(mgr)
    }

    /// Scan `git worktree list` for `kira/*` worktrees the daemon doesn't know
    /// about (e.g. survived a crash) and import them as `Stale`. Each consumes
    /// a semaphore permit so we don't overcommit concurrency.
    ///
    /// On startup with a persistent store, also accounts for known worktrees
    /// (so concurrent permits stay accurate). Skips paths the store already tracks.
    async fn reconcile(&self) -> Result<()> {
        let rows = git::worktree_list(&self.repo_root).await?;
        let known_paths: std::collections::HashSet<String> = self
            .store
            .list()
            .into_iter()
            .map(|w| w.path)
            .collect();

        // First, account for permits already held by known worktrees so
        // total in-flight permits match reality.
        for _ in 0..known_paths.len() {
            let permit = self.semaphore.clone().try_acquire_owned();
            if permit.is_err() {
                break;
            }
            std::mem::forget(permit);
        }

        let mut imported = 0_usize;
        for row in rows {
            let Some(branch) = row.branch.as_deref() else {
                continue;
            };
            let branch_short = branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .to_string();
            if !branch_short.starts_with(BRANCH_PREFIX) {
                continue;
            }
            // Dedup: skip if redb store already knows this path.
            if known_paths.contains(&row.path) {
                continue;
            }
            let permit = self.semaphore.clone().try_acquire_owned();
            if permit.is_err() {
                warn!(branch = %branch_short, "reconcile: semaphore full, skipping orphan");
                continue;
            }
            std::mem::forget(permit);

            let id = Uuid::now_v7();
            let wt = Worktree {
                id: id.to_string(),
                branch: branch_short,
                path: row.path.clone(),
                base_branch: "unknown".to_string(),
                status: WorktreeStatus::Stale,
                agent_kind: None,
                created_at: Utc::now().to_rfc3339(),
            };
            self.store.insert(id, wt);
            imported += 1;
        }
        if imported > 0 {
            info!(imported, "reconciled orphan kira/* worktrees as Stale");
        }
        Ok(())
    }

    /// Number of currently registered worktrees (cheap).
    pub fn count(&self) -> usize {
        self.store.len()
    }

    /// Spawn a fresh worktree forked from `req.base`.
    /// Acquires a permit from the semaphore — caller is blocked while max_parallel is reached.
    #[instrument(skip(self, req), fields(base = %req.base))]
    pub async fn spawn(&self, req: SpawnRequest) -> Result<Worktree> {
        // 1. Concurrency permit.
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .context("worktree semaphore closed")?;
        // We deliberately *forget* the permit — it lives for the lifetime of the
        // worktree, released only on destroy. v0.5 stores it in a per-worktree
        // task handle when agent loops attach. For now: forget == "until destroy".
        std::mem::forget(permit);

        // 2. Names + path.
        let short = short_id();
        let id = Uuid::now_v7();
        let dir_name = format!("{WT_PREFIX}{short}");
        let branch = format!("{BRANCH_PREFIX}{dir_name}");
        let rel_path = format!("{WORKTREES_SUBDIR}/{dir_name}");
        let abs_path = self.repo_root.join(&rel_path);

        // 3. Mark Spawning in store.
        let now = Utc::now().to_rfc3339();
        let wt = Worktree {
            id: id.to_string(),
            branch: branch.clone(),
            path: abs_path.display().to_string(),
            base_branch: req.base.clone(),
            status: WorktreeStatus::Spawning,
            agent_kind: req.agent_kind.clone(),
            created_at: now,
        };
        self.store.insert(id, wt.clone());

        // 4. git worktree add.
        match git::worktree_add(&self.repo_root, &rel_path, &branch, &req.base).await {
            Ok(()) => {}
            Err(e) => {
                // Rollback the store entry; release permit by removing.
                self.store.remove(id);
                self.release_permit();
                return Err(e).context("git worktree add failed");
            }
        }

        // 5. Flip to Idle (ready for agent attach).
        self.store
            .update(id, |w| w.status = WorktreeStatus::Idle);
        let mut updated = wt;
        updated.status = WorktreeStatus::Idle;
        info!(id = %id, path = %abs_path.display(), branch = %branch, "worktree spawned");
        Ok(updated)
    }

    /// Destroy a worktree by id. Removes the git worktree, optionally deletes the branch.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn destroy(&self, id: Uuid, delete_branch: bool) -> Result<DestroyResponse> {
        let wt = self
            .store
            .get(id)
            .with_context(|| format!("worktree not found: {id}"))?;

        // Mark Cleaning.
        self.store.update(id, |w| w.status = WorktreeStatus::Cleaning);

        // git worktree remove --force <path>
        // Path is stored absolute; pass it relative to repo_root when possible for `git`.
        let rel = match PathBuf::from(&wt.path).strip_prefix(&self.repo_root) {
            Ok(p) => p.display().to_string(),
            Err(_) => wt.path.clone(),
        };
        match git::worktree_remove(&self.repo_root, &rel).await {
            Ok(()) => {}
            Err(e) => {
                warn!(error = ?e, "worktree remove failed; continuing");
            }
        }

        let mut branch_deleted = false;
        if delete_branch {
            match git::branch_delete(&self.repo_root, &wt.branch).await {
                Ok(b) => branch_deleted = b,
                Err(e) => warn!(error = ?e, "branch -D failed"),
            }
        }

        self.store.remove(id);
        self.release_permit();

        info!(id = %id, branch_deleted, "worktree destroyed");
        Ok(DestroyResponse {
            destroyed: true,
            branch_deleted,
            id: id.to_string(),
        })
    }

    pub fn list(&self) -> Vec<Worktree> {
        self.store.list()
    }

    pub fn get(&self, id: Uuid) -> Option<Worktree> {
        self.store.get(id)
    }

    /// Release one permit back to the semaphore (counterpart of `mem::forget` in spawn).
    fn release_permit(&self) {
        self.semaphore.add_permits(1);
    }
}
