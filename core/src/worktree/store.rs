//! In-memory worktree state store.
//!
//! v0: `Arc<Mutex<HashMap<Uuid, Worktree>>>` — wiped on daemon restart.
//! v0.5: redb-backed (`worktrees` table) for durability.

use crate::proto::worktree::Worktree;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Worktree>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, id: Uuid, wt: Worktree) {
        let mut m = self.inner.lock().expect("worktree store poisoned");
        m.insert(id, wt);
    }

    pub fn get(&self, id: Uuid) -> Option<Worktree> {
        let m = self.inner.lock().expect("worktree store poisoned");
        m.get(&id).cloned()
    }

    pub fn remove(&self, id: Uuid) -> Option<Worktree> {
        let mut m = self.inner.lock().expect("worktree store poisoned");
        m.remove(&id)
    }

    pub fn list(&self) -> Vec<Worktree> {
        let m = self.inner.lock().expect("worktree store poisoned");
        m.values().cloned().collect()
    }

    pub fn update<F: FnOnce(&mut Worktree)>(&self, id: Uuid, f: F) -> bool {
        let mut m = self.inner.lock().expect("worktree store poisoned");
        if let Some(wt) = m.get_mut(&id) {
            f(wt);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("worktree store poisoned").len()
    }
}
