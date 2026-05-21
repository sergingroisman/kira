//! Worktree store — in-memory cache backed (optionally) by redb.

use crate::proto::worktree::Worktree;
use crate::state::{KiraDb, TBL_WORKTREES};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Worktree>>>,
    db: Option<KiraDb>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a store backed by redb. Loads existing rows into the cache.
    pub fn with_db(db: KiraDb) -> Self {
        let mut map: HashMap<Uuid, Worktree> = HashMap::new();
        match db.list::<Worktree>(TBL_WORKTREES) {
            Ok(rows) => {
                for w in rows {
                    if let Ok(uuid) = Uuid::parse_str(&w.id) {
                        map.insert(uuid, w);
                    }
                }
            }
            Err(e) => warn!(error = ?e, "load worktrees from redb failed"),
        }
        Self {
            inner: Arc::new(Mutex::new(map)),
            db: Some(db),
        }
    }

    fn db_save(&self, w: &Worktree) {
        if let Some(db) = &self.db {
            if let Err(e) = db.put(TBL_WORKTREES, &w.id, w) {
                warn!(error = ?e, id = %w.id, "persist worktree failed");
            }
        }
    }

    fn db_delete(&self, id: &str) {
        if let Some(db) = &self.db {
            if let Err(e) = db.delete(TBL_WORKTREES, id) {
                warn!(error = ?e, id, "delete worktree failed");
            }
        }
    }

    pub fn insert(&self, id: Uuid, wt: Worktree) {
        self.db_save(&wt);
        let mut m = self.inner.lock().expect("worktree store poisoned");
        m.insert(id, wt);
    }

    pub fn get(&self, id: Uuid) -> Option<Worktree> {
        let m = self.inner.lock().expect("worktree store poisoned");
        m.get(&id).cloned()
    }

    pub fn remove(&self, id: Uuid) -> Option<Worktree> {
        let removed = {
            let mut m = self.inner.lock().expect("worktree store poisoned");
            m.remove(&id)
        };
        if removed.is_some() {
            self.db_delete(&id.to_string());
        }
        removed
    }

    pub fn list(&self) -> Vec<Worktree> {
        let m = self.inner.lock().expect("worktree store poisoned");
        m.values().cloned().collect()
    }

    pub fn update<F: FnOnce(&mut Worktree)>(&self, id: Uuid, f: F) -> bool {
        let mut m = self.inner.lock().expect("worktree store poisoned");
        if let Some(wt) = m.get_mut(&id) {
            f(wt);
            let snapshot = wt.clone();
            drop(m);
            self.db_save(&snapshot);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("worktree store poisoned").len()
    }
}
