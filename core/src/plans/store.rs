//! In-memory plan store.

use crate::proto::plan::Plan;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Plan>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, id: Uuid, p: Plan) {
        self.inner.lock().expect("plan store poisoned").insert(id, p);
    }

    pub fn get(&self, id: Uuid) -> Option<Plan> {
        self.inner
            .lock()
            .expect("plan store poisoned")
            .get(&id)
            .cloned()
    }

    pub fn update<F: FnOnce(&mut Plan)>(&self, id: Uuid, f: F) -> bool {
        let mut m = self.inner.lock().expect("plan store poisoned");
        if let Some(p) = m.get_mut(&id) {
            f(p);
            true
        } else {
            false
        }
    }

    pub fn list(&self) -> Vec<Plan> {
        self.inner
            .lock()
            .expect("plan store poisoned")
            .values()
            .cloned()
            .collect()
    }
}
