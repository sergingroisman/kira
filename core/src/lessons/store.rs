//! Lesson store — in-memory cache + optional redb write-through.

use crate::proto::lesson::Lesson;
use crate::state::{KiraDb, TBL_LESSONS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    inner: Arc<Mutex<HashMap<Uuid, Lesson>>>,
    db: Option<KiraDb>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_db(db: KiraDb) -> Self {
        let mut map: HashMap<Uuid, Lesson> = HashMap::new();
        match db.list::<Lesson>(TBL_LESSONS) {
            Ok(rows) => {
                for l in rows {
                    if let Ok(uuid) = Uuid::parse_str(&l.id) {
                        map.insert(uuid, l);
                    }
                }
            }
            Err(e) => warn!(error = ?e, "load lessons from redb failed"),
        }
        Self {
            inner: Arc::new(Mutex::new(map)),
            db: Some(db),
        }
    }

    fn db_save(&self, l: &Lesson) {
        if let Some(db) = &self.db
            && let Err(e) = db.put(TBL_LESSONS, &l.id, l) {
                warn!(error = ?e, id = %l.id, "persist lesson failed");
            }
    }

    fn db_delete(&self, id: &str) {
        if let Some(db) = &self.db
            && let Err(e) = db.delete(TBL_LESSONS, id) {
                warn!(error = ?e, id, "delete lesson failed");
            }
    }

    pub fn insert(&self, id: Uuid, l: Lesson) {
        self.db_save(&l);
        self.inner.lock().expect("lesson store poisoned").insert(id, l);
    }

    pub fn get(&self, id: Uuid) -> Option<Lesson> {
        self.inner
            .lock()
            .expect("lesson store poisoned")
            .get(&id)
            .cloned()
    }

    pub fn remove(&self, id: Uuid) -> Option<Lesson> {
        let removed = {
            let mut m = self.inner.lock().expect("lesson store poisoned");
            m.remove(&id)
        };
        if removed.is_some() {
            self.db_delete(&id.to_string());
        }
        removed
    }

    pub fn list(&self) -> Vec<Lesson> {
        self.inner
            .lock()
            .expect("lesson store poisoned")
            .values()
            .cloned()
            .collect()
    }

    pub fn update<F: FnOnce(&mut Lesson)>(&self, id: Uuid, f: F) -> bool {
        let snapshot = {
            let mut m = self.inner.lock().expect("lesson store poisoned");
            if let Some(l) = m.get_mut(&id) {
                f(l);
                Some(l.clone())
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
