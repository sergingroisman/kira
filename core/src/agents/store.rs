//! Agent store — in-memory cache backed (optionally) by redb.

use crate::proto::agent::Agent;
use crate::state::{KiraDb, TBL_AGENTS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Agent>>>,
    db: Option<KiraDb>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_db(db: KiraDb) -> Self {
        let mut map: HashMap<Uuid, Agent> = HashMap::new();
        match db.list::<Agent>(TBL_AGENTS) {
            Ok(rows) => {
                for a in rows {
                    if let Ok(uuid) = Uuid::parse_str(&a.id) {
                        map.insert(uuid, a);
                    }
                }
            }
            Err(e) => warn!(error = ?e, "load agents from redb failed"),
        }
        Self {
            inner: Arc::new(Mutex::new(map)),
            db: Some(db),
        }
    }

    fn db_save(&self, a: &Agent) {
        if let Some(db) = &self.db {
            if let Err(e) = db.put(TBL_AGENTS, &a.id, a) {
                warn!(error = ?e, id = %a.id, "persist agent failed");
            }
        }
    }

    pub fn insert(&self, id: Uuid, a: Agent) {
        self.db_save(&a);
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
        let snapshot = {
            let mut m = self.inner.lock().expect("agent store poisoned");
            if let Some(a) = m.get_mut(&id) {
                f(a);
                Some(a.clone())
            } else {
                None
            }
        };
        match snapshot {
            Some(s) => {
                self.db_save(&s);
                true
            }
            None => false,
        }
    }
}
