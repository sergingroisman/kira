//! Durable state — redb-backed key-value tables.
//!
//! v0.5 ships 3 durable tables:
//!   - `worktrees`  Uuid → Worktree JSON
//!   - `agents`     Uuid → Agent JSON
//!   - `plans`      Uuid → Plan JSON
//!
//! Stores keep their fast in-memory cache and write-through to redb on
//! mutating operations. On startup, each Store calls `load_*()` to rebuild
//! the cache from redb. A crash loses at most one in-flight write.
//!
//! Schema versioning lives at key `__schema_version` in a meta table.
//! Bump on breaking changes (none yet).

use anyhow::{Context, Result};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

const SCHEMA_VERSION: u32 = 1;
const META_VERSION_KEY: &str = "__schema_version";

pub const TBL_WORKTREES: TableDefinition<&str, &str> = TableDefinition::new("worktrees");
pub const TBL_AGENTS: TableDefinition<&str, &str> = TableDefinition::new("agents");
pub const TBL_PLANS: TableDefinition<&str, &str> = TableDefinition::new("plans");
pub const TBL_LESSONS: TableDefinition<&str, &str> = TableDefinition::new("lessons");
pub const TBL_EMBED_CACHE: TableDefinition<&str, &str> =
    TableDefinition::new("embed_cache");
pub const TBL_META: TableDefinition<&str, &str> = TableDefinition::new("meta");

#[derive(Clone)]
pub struct KiraDb {
    inner: Arc<Database>,
    /// Filesystem path (used for backup helpers + logs).
    pub path: PathBuf,
}

impl KiraDb {
    /// Open or create the database at `path`. Creates parent dirs.
    /// Initializes empty tables if missing.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir parent failed: {}", parent.display()))?;
        }
        let db = Database::create(path)
            .with_context(|| format!("open redb failed: {}", path.display()))?;

        // Initialize tables + schema version.
        {
            let txn = db.begin_write()?;
            // Touch each table so opens in subsequent read txns don't fail.
            txn.open_table(TBL_WORKTREES)?;
            txn.open_table(TBL_AGENTS)?;
            txn.open_table(TBL_PLANS)?;
            txn.open_table(TBL_LESSONS)?;
            txn.open_table(TBL_EMBED_CACHE)?;
            {
                let mut meta = txn.open_table(TBL_META)?;
                let current_version: Option<u32> = {
                    let cur = meta.get(META_VERSION_KEY)?;
                    cur.map(|guard| guard.value().parse().unwrap_or(0_u32))
                    // `cur` AccessGuard dropped at end of scope here.
                };
                match current_version {
                    Some(v) if v != SCHEMA_VERSION => {
                        warn!(
                            stored = v,
                            expected = SCHEMA_VERSION,
                            "schema version mismatch — v0.5 has no migrations, proceeding"
                        );
                    }
                    Some(_) => {}
                    None => {
                        let value_string = SCHEMA_VERSION.to_string();
                        meta.insert(META_VERSION_KEY, value_string.as_str())?;
                    }
                }
            }
            txn.commit()?;
        }

        info!(path = %path.display(), version = SCHEMA_VERSION, "KiraDb opened");
        Ok(Self {
            inner: Arc::new(db),
            path: path.to_path_buf(),
        })
    }

    /// In-memory variant for tests (writes go to a temp file).
    pub fn open_temp() -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("kira-db-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir)?;
        Self::open(&dir.join("state.redb"))
    }

    /// Insert or overwrite a row. Value is JSON-serialized.
    pub fn put<T: Serialize>(
        &self,
        table: TableDefinition<&str, &str>,
        key: &str,
        value: &T,
    ) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.put_raw(table, key, &json)
    }

    /// Insert or overwrite a row with a pre-serialized string value.
    /// Use this when the caller already has the canonical string form (e.g.
    /// cache writes where double-JSON encoding would corrupt lookups).
    pub fn put_raw(
        &self,
        table: TableDefinition<&str, &str>,
        key: &str,
        value: &str,
    ) -> Result<()> {
        let txn = self.inner.begin_write()?;
        {
            let mut t = txn.open_table(table)?;
            t.insert(key, value)?;
        }
        txn.commit()?;
        Ok(())
    }

    /// Delete a row. Returns true if it existed.
    pub fn delete(
        &self,
        table: TableDefinition<&str, &str>,
        key: &str,
    ) -> Result<bool> {
        let txn = self.inner.begin_write()?;
        let existed = {
            let mut t = txn.open_table(table)?;
            t.remove(key)?.is_some()
        };
        txn.commit()?;
        Ok(existed)
    }

    /// Read a single row by key. Returns the raw string value if present.
    pub fn list_one(
        &self,
        table: TableDefinition<&str, &str>,
        key: &str,
    ) -> Result<Option<String>> {
        let txn = self.inner.begin_read()?;
        let t = txn.open_table(table)?;
        Ok(t.get(key)?.map(|g| g.value().to_string()))
    }

    /// Count rows in a table (used for cache eviction triggers).
    pub fn count(&self, table: TableDefinition<&str, &str>) -> Result<u64> {
        let txn = self.inner.begin_read()?;
        let t = txn.open_table(table)?;
        Ok(t.len()?)
    }

    /// Delete up to `n` rows from the table by iter order (no LRU). Returns
    /// the actual count removed. Used for reactive cache eviction.
    pub fn delete_n(
        &self,
        table: TableDefinition<&str, &str>,
        n: usize,
    ) -> Result<usize> {
        // Collect keys first to release the read borrow before write.
        let keys: Vec<String> = {
            let txn = self.inner.begin_read()?;
            let t = txn.open_table(table)?;
            t.iter()?
                .take(n)
                .filter_map(|entry| entry.ok().map(|(k, _)| k.value().to_string()))
                .collect()
        };
        let removed = keys.len();
        let txn = self.inner.begin_write()?;
        {
            let mut t = txn.open_table(table)?;
            for k in &keys {
                let _ = t.remove(k.as_str());
            }
        }
        txn.commit()?;
        Ok(removed)
    }

    /// Read all rows from a table, deserializing values as `T`.
    pub fn list<T: DeserializeOwned>(
        &self,
        table: TableDefinition<&str, &str>,
    ) -> Result<Vec<T>> {
        let txn = self.inner.begin_read()?;
        let t = txn.open_table(table)?;
        let mut out: Vec<T> = Vec::new();
        for entry in t.iter()? {
            let (_, v) = entry?;
            match serde_json::from_str::<T>(v.value()) {
                Ok(item) => out.push(item),
                Err(e) => warn!(error = ?e, "skipping unparseable row"),
            }
        }
        Ok(out)
    }
}

/// Resolve the default state path: `~/.local/share/kira/state.redb` (Mac/Linux).
pub fn default_state_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".local/share/kira/state.redb"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Sample {
        id: String,
        n: u32,
    }

    #[test]
    fn put_list_delete_roundtrip() {
        let db = KiraDb::open_temp().unwrap();
        let s = Sample { id: "a".into(), n: 1 };
        db.put(TBL_WORKTREES, "a", &s).unwrap();
        let all: Vec<Sample> = db.list(TBL_WORKTREES).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0], s);
        assert!(db.delete(TBL_WORKTREES, "a").unwrap());
        let all: Vec<Sample> = db.list(TBL_WORKTREES).unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn missing_key_delete_returns_false() {
        let db = KiraDb::open_temp().unwrap();
        assert!(!db.delete(TBL_WORKTREES, "ghost").unwrap());
    }

    #[test]
    fn open_twice_keeps_data() {
        let dir = std::env::temp_dir().join(format!("kira-db-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.redb");
        {
            let db = KiraDb::open(&path).unwrap();
            db.put(TBL_AGENTS, "x", &Sample { id: "x".into(), n: 7 }).unwrap();
        }
        let db = KiraDb::open(&path).unwrap();
        let all: Vec<Sample> = db.list(TBL_AGENTS).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].n, 7);
    }
}
