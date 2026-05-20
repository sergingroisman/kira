//! In-memory agent store. Mirrors `worktree/store.rs`.

use crate::proto::agent::Agent;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Agent>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, id: Uuid, a: Agent) {
        self.inner.lock().expect("agent store poisoned").insert(id, a);
    }

    pub fn get(&self, id: Uuid) -> Option<Agent> {
        self.inner
            .lock()
            .expect("agent store poisoned")
            .get(&id)
            .cloned()
    }

    pub fn list(&self) -> Vec<Agent> {
        self.inner
            .lock()
            .expect("agent store poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn update<F: FnOnce(&mut Agent)>(&self, id: Uuid, f: F) -> bool {
        let mut m = self.inner.lock().expect("agent store poisoned");
        if let Some(a) = m.get_mut(&id) {
            f(a);
            true
        } else {
            false
        }
    }
}
