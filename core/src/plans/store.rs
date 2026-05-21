//! Plan store — in-memory cache backed (optionally) by redb.

use crate::proto::plan::Plan;
use crate::state::{KiraDb, TBL_PLANS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Plan>>>,
    db: Option<KiraDb>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_db(db: KiraDb) -> Self {
        let mut map: HashMap<Uuid, Plan> = HashMap::new();
        match db.list::<Plan>(TBL_PLANS) {
            Ok(rows) => {
                for p in rows {
                    if let Ok(uuid) = Uuid::parse_str(&p.id) {
                        map.insert(uuid, p);
                    }
                }
            }
            Err(e) => warn!(error = ?e, "load plans from redb failed"),
        }
        Self {
            inner: Arc::new(Mutex::new(map)),
            db: Some(db),
        }
    }

    fn db_save(&self, p: &Plan) {
        if let Some(db) = &self.db {
            if let Err(e) = db.put(TBL_PLANS, &p.id, p) {
                warn!(error = ?e, id = %p.id, "persist plan failed");
            }
        }
    }

    pub fn insert(&self, id: Uuid, p: Plan) {
        self.db_save(&p);
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
        let snapshot = {
            let mut m = self.inner.lock().expect("plan store poisoned");
            if let Some(p) = m.get_mut(&id) {
                f(p);
                Some(p.clone())
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

    pub fn list(&self) -> Vec<Plan> {
        self.inner
            .lock()
            .expect("plan store poisoned")
            .values()
            .cloned()
            .collect()
    }
}
