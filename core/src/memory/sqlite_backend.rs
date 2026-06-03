//! SQLite-vec backend for the Memory Store.
//!
//! Persists memory fragments in a local SQLite database with vector search
//! via the sqlite-vec extension. Zero external infra required.
//!
//! NOTE: This module requires `unsafe` for loading the sqlite-vec extension
//! via FFI. This is the ONLY exception to `#![forbid(unsafe_code)]` in the
//! project, scoped to the extension loading call.
#![allow(unsafe_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::store::{MemoryFragment, MemoryStore, MetadataUpdate};
use super::tier::MemoryTier;

/// SQLite-vec backed memory store.
pub struct SqliteVecStore {
    conn: Mutex<Connection>,
    max_per_thread: usize,
}

impl SqliteVecStore {
    /// Open or create the SQLite database at the given path.
    /// Initializes the sqlite-vec extension and creates tables if needed.
    pub fn open(path: &Path, max_per_thread: usize) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }

        // Register the sqlite-vec extension BEFORE opening the connection.
        // `sqlite3_auto_extension` only applies to connections opened *after*
        // registration — doing it post-open left `conn` without the `vec0`
        // module ("no such module: vec0" when creating the virtual table).
        unsafe {
            #[allow(clippy::missing_transmute_annotations)]
            let rc = rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
            if rc != 0 {
                anyhow::bail!("failed to register sqlite-vec extension: rc={rc}");
            }
        }

        let conn = Connection::open(path)
            .with_context(|| format!("opening sqlite at {}", path.display()))?;

        // Create tables
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_fragments (
                fragment_id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                score REAL NOT NULL DEFAULT 0.0
            );
            CREATE INDEX IF NOT EXISTS idx_fragments_thread ON memory_fragments(thread_id);
            CREATE INDEX IF NOT EXISTS idx_fragments_run ON memory_fragments(run_id);
            CREATE INDEX IF NOT EXISTS idx_fragments_created ON memory_fragments(created_at);",
        )
        .context("creating memory_fragments table")?;

        // Create virtual table for vector search (768 dims for nomic-embed-text)
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_fragments USING vec0(
                fragment_id TEXT PRIMARY KEY,
                embedding float[768]
            );",
        )
        .context("creating vec_fragments virtual table")?;

        // Run schema migration for maturation columns
        Self::migrate_maturation_columns(&conn)?;

        info!(path = %path.display(), "sqlite-vec memory store opened");

        Ok(Self {
            conn: Mutex::new(conn),
            max_per_thread,
        })
    }

    /// Migrate the `memory_fragments` table to add maturation columns if they don't exist.
    /// This is idempotent — safe to call on every startup.
    fn migrate_maturation_columns(conn: &Connection) -> Result<()> {
        // Query existing columns via PRAGMA
        let existing_columns: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(memory_fragments)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            rows.filter_map(Result::ok).collect()
        };

        let migrations: &[(&str, &str)] = &[
            ("tier", "ALTER TABLE memory_fragments ADD COLUMN tier TEXT DEFAULT 'working'"),
            ("access_count", "ALTER TABLE memory_fragments ADD COLUMN access_count INTEGER DEFAULT 0"),
            ("last_accessed", "ALTER TABLE memory_fragments ADD COLUMN last_accessed TEXT"),
            ("consolidated_into", "ALTER TABLE memory_fragments ADD COLUMN consolidated_into TEXT"),
            ("promoted_at", "ALTER TABLE memory_fragments ADD COLUMN promoted_at TEXT"),
            ("session_id", "ALTER TABLE memory_fragments ADD COLUMN session_id TEXT"),
        ];

        for (col_name, alter_sql) in migrations {
            if !existing_columns.iter().any(|c| c == col_name) {
                conn.execute(alter_sql, [])
                    .with_context(|| format!("migrating column '{col_name}'"))?;
                info!(column = col_name, "added maturation column to memory_fragments");
            }
        }

        // Add indexes for efficient tier and session queries
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_fragments_session ON memory_fragments(session_id);
             CREATE INDEX IF NOT EXISTS idx_fragments_tier ON memory_fragments(tier);",
        )
        .context("creating maturation indexes")?;

        debug!("maturation schema migration complete");
        Ok(())
    }
}

impl MemoryStore for SqliteVecStore {
    async fn persist(&self, fragment: &MemoryFragment) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");

        // Check thread limit and evict oldest if needed
        let count: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_fragments WHERE thread_id = ?1",
                [fragment.thread_id.to_string()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if count >= self.max_per_thread {
            // Delete oldest for this thread
            conn.execute(
                "DELETE FROM memory_fragments WHERE fragment_id = (
                    SELECT fragment_id FROM memory_fragments
                    WHERE thread_id = ?1
                    ORDER BY created_at ASC LIMIT 1
                )",
                [fragment.thread_id.to_string()],
            )?;
            // Also remove from vec table
            conn.execute(
                "DELETE FROM vec_fragments WHERE fragment_id = (
                    SELECT fragment_id FROM memory_fragments
                    WHERE thread_id = ?1
                    ORDER BY created_at ASC LIMIT 1
                )",
                [fragment.thread_id.to_string()],
            )
            .ok(); // best-effort
        }

        // Insert fragment
        conn.execute(
            "INSERT OR REPLACE INTO memory_fragments (fragment_id, content, thread_id, run_id, created_at, score, tier, access_count, last_accessed, consolidated_into, promoted_at, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                fragment.fragment_id.to_string(),
                fragment.content,
                fragment.thread_id.to_string(),
                fragment.run_id.to_string(),
                fragment.created_at.to_rfc3339(),
                fragment.score,
                fragment.tier.as_str(),
                fragment.access_count as i64,
                fragment.last_accessed.map(|dt| dt.to_rfc3339()),
                fragment.consolidated_into.map(|id| id.to_string()),
                fragment.promoted_at.map(|dt| dt.to_rfc3339()),
                fragment.session_id.map(|id| id.to_string()),
            ],
        )
        .context("inserting fragment")?;

        // Insert embedding into vec table
        let embedding_bytes: Vec<u8> = fragment
            .embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        conn.execute(
            "INSERT OR REPLACE INTO vec_fragments (fragment_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![fragment.fragment_id.to_string(), embedding_bytes],
        )
        .context("inserting embedding")?;

        debug!(
            fragment_id = %fragment.fragment_id,
            thread_id = %fragment.thread_id,
            "fragment persisted to sqlite-vec"
        );

        Ok(())
    }

    async fn search(
        &self,
        embedding: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<MemoryFragment>> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");

        let query_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // sqlite-vec uses distance (lower = more similar), we need to convert to score
        let mut stmt = conn.prepare(
            "SELECT v.fragment_id, v.distance, f.content, f.thread_id, f.run_id, f.created_at, f.score,
                    f.tier, f.access_count, f.last_accessed, f.consolidated_into, f.promoted_at, f.session_id
             FROM vec_fragments v
             JOIN memory_fragments f ON v.fragment_id = f.fragment_id
             WHERE v.embedding MATCH ?1
             ORDER BY v.distance ASC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(rusqlite::params![query_bytes, top_k as i64], |row| {
            let fid: String = row.get(0)?;
            let distance: f64 = row.get(1)?;
            let content: String = row.get(2)?;
            let tid: String = row.get(3)?;
            let rid: String = row.get(4)?;
            let created: String = row.get(5)?;
            let _score: f64 = row.get(6)?;
            let tier_str: Option<String> = row.get(7)?;
            let access_count: Option<i64> = row.get(8)?;
            let last_accessed_str: Option<String> = row.get(9)?;
            let consolidated_into_str: Option<String> = row.get(10)?;
            let promoted_at_str: Option<String> = row.get(11)?;
            let session_id_str: Option<String> = row.get(12)?;
            Ok((fid, distance, content, tid, rid, created, tier_str, access_count,
                last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (fid, distance, content, tid, rid, created, tier_str, access_count,
                 last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str) = row?;
            // Convert distance to similarity score (1.0 - distance for cosine)
            let score = (1.0 - distance as f32).max(0.0);
            if score < min_score {
                continue;
            }
            results.push(MemoryFragment {
                fragment_id: Uuid::parse_str(&fid).unwrap_or_else(|_| Uuid::now_v7()),
                content,
                embedding: Vec::new(), // Don't load full embedding in search results
                thread_id: Uuid::parse_str(&tid).unwrap_or_else(|_| Uuid::now_v7()),
                run_id: Uuid::parse_str(&rid).unwrap_or_else(|_| Uuid::now_v7()),
                created_at: chrono::DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                score,
                tier: parse_tier(tier_str.as_deref()),
                access_count: access_count.unwrap_or(0) as u64,
                last_accessed: parse_optional_datetime(last_accessed_str.as_deref()),
                consolidated_into: parse_optional_uuid(consolidated_into_str.as_deref()),
                promoted_at: parse_optional_datetime(promoted_at_str.as_deref()),
                session_id: parse_optional_uuid(session_id_str.as_deref()),
            });
        }

        Ok(results)
    }

    async fn get_by_run(&self, run_id: Uuid) -> Result<Vec<MemoryFragment>> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");

        let mut stmt = conn.prepare(
            "SELECT fragment_id, content, thread_id, run_id, created_at, score,
                    tier, access_count, last_accessed, consolidated_into, promoted_at, session_id
             FROM memory_fragments WHERE run_id = ?1 ORDER BY score DESC",
        )?;

        let rows = stmt.query_map([run_id.to_string()], |row| {
            let fid: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tid: String = row.get(2)?;
            let rid: String = row.get(3)?;
            let created: String = row.get(4)?;
            let score: f64 = row.get(5)?;
            let tier_str: Option<String> = row.get(6)?;
            let access_count: Option<i64> = row.get(7)?;
            let last_accessed_str: Option<String> = row.get(8)?;
            let consolidated_into_str: Option<String> = row.get(9)?;
            let promoted_at_str: Option<String> = row.get(10)?;
            let session_id_str: Option<String> = row.get(11)?;
            Ok((fid, content, tid, rid, created, score, tier_str, access_count,
                last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (fid, content, tid, rid, created, score, tier_str, access_count,
                 last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str) = row?;
            results.push(MemoryFragment {
                fragment_id: Uuid::parse_str(&fid).unwrap_or_else(|_| Uuid::now_v7()),
                content,
                embedding: Vec::new(),
                thread_id: Uuid::parse_str(&tid).unwrap_or_else(|_| Uuid::now_v7()),
                run_id: Uuid::parse_str(&rid).unwrap_or_else(|_| Uuid::now_v7()),
                created_at: chrono::DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                score: score as f32,
                tier: parse_tier(tier_str.as_deref()),
                access_count: access_count.unwrap_or(0) as u64,
                last_accessed: parse_optional_datetime(last_accessed_str.as_deref()),
                consolidated_into: parse_optional_uuid(consolidated_into_str.as_deref()),
                promoted_at: parse_optional_datetime(promoted_at_str.as_deref()),
                session_id: parse_optional_uuid(session_id_str.as_deref()),
            });
        }

        Ok(results)
    }

    async fn delete(&self, fragment_id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");
        let id_str = fragment_id.to_string();

        let deleted = conn.execute(
            "DELETE FROM memory_fragments WHERE fragment_id = ?1",
            [&id_str],
        )?;

        conn.execute(
            "DELETE FROM vec_fragments WHERE fragment_id = ?1",
            [&id_str],
        )
        .ok(); // best-effort

        Ok(deleted > 0)
    }

    async fn count_by_thread(&self, thread_id: Uuid) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM memory_fragments WHERE thread_id = ?1",
            [thread_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    async fn get_by_id(&self, fragment_id: Uuid) -> Result<Option<MemoryFragment>> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");
        let mut stmt = conn.prepare(
            "SELECT fragment_id, content, thread_id, run_id, created_at, score,
                    tier, access_count, last_accessed, consolidated_into, promoted_at, session_id
             FROM memory_fragments WHERE fragment_id = ?1",
        )?;

        let mut rows = stmt.query_map([fragment_id.to_string()], |row| {
            let fid: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tid: String = row.get(2)?;
            let rid: String = row.get(3)?;
            let created: String = row.get(4)?;
            let score: f64 = row.get(5)?;
            let tier_str: Option<String> = row.get(6)?;
            let access_count: Option<i64> = row.get(7)?;
            let last_accessed_str: Option<String> = row.get(8)?;
            let consolidated_into_str: Option<String> = row.get(9)?;
            let promoted_at_str: Option<String> = row.get(10)?;
            let session_id_str: Option<String> = row.get(11)?;
            Ok((fid, content, tid, rid, created, score, tier_str, access_count,
                last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))
        })?;

        match rows.next() {
            Some(Ok((fid, content, tid, rid, created, score, tier_str, access_count,
                     last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))) => {
                Ok(Some(MemoryFragment {
                    fragment_id: Uuid::parse_str(&fid).unwrap_or_else(|_| Uuid::now_v7()),
                    content,
                    embedding: Vec::new(),
                    thread_id: Uuid::parse_str(&tid).unwrap_or_else(|_| Uuid::now_v7()),
                    run_id: Uuid::parse_str(&rid).unwrap_or_else(|_| Uuid::now_v7()),
                    created_at: chrono::DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    score: score as f32,
                    tier: parse_tier(tier_str.as_deref()),
                    access_count: access_count.unwrap_or(0) as u64,
                    last_accessed: parse_optional_datetime(last_accessed_str.as_deref()),
                    consolidated_into: parse_optional_uuid(consolidated_into_str.as_deref()),
                    promoted_at: parse_optional_datetime(promoted_at_str.as_deref()),
                    session_id: parse_optional_uuid(session_id_str.as_deref()),
                }))
            }
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn get_by_session(&self, session_id: Uuid) -> Result<Vec<MemoryFragment>> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");
        let mut stmt = conn.prepare(
            "SELECT fragment_id, content, thread_id, run_id, created_at, score,
                    tier, access_count, last_accessed, consolidated_into, promoted_at, session_id
             FROM memory_fragments WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([session_id.to_string()], |row| {
            let fid: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tid: String = row.get(2)?;
            let rid: String = row.get(3)?;
            let created: String = row.get(4)?;
            let score: f64 = row.get(5)?;
            let tier_str: Option<String> = row.get(6)?;
            let access_count: Option<i64> = row.get(7)?;
            let last_accessed_str: Option<String> = row.get(8)?;
            let consolidated_into_str: Option<String> = row.get(9)?;
            let promoted_at_str: Option<String> = row.get(10)?;
            let session_id_str: Option<String> = row.get(11)?;
            Ok((fid, content, tid, rid, created, score, tier_str, access_count,
                last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (fid, content, tid, rid, created, score, tier_str, access_count,
                 last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str) = row?;
            results.push(MemoryFragment {
                fragment_id: Uuid::parse_str(&fid).unwrap_or_else(|_| Uuid::now_v7()),
                content,
                embedding: Vec::new(),
                thread_id: Uuid::parse_str(&tid).unwrap_or_else(|_| Uuid::now_v7()),
                run_id: Uuid::parse_str(&rid).unwrap_or_else(|_| Uuid::now_v7()),
                created_at: chrono::DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                score: score as f32,
                tier: parse_tier(tier_str.as_deref()),
                access_count: access_count.unwrap_or(0) as u64,
                last_accessed: parse_optional_datetime(last_accessed_str.as_deref()),
                consolidated_into: parse_optional_uuid(consolidated_into_str.as_deref()),
                promoted_at: parse_optional_datetime(promoted_at_str.as_deref()),
                session_id: parse_optional_uuid(session_id_str.as_deref()),
            });
        }

        Ok(results)
    }

    async fn get_by_tier(&self, tier: MemoryTier) -> Result<Vec<MemoryFragment>> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");
        let mut stmt = conn.prepare(
            "SELECT fragment_id, content, thread_id, run_id, created_at, score,
                    tier, access_count, last_accessed, consolidated_into, promoted_at, session_id
             FROM memory_fragments WHERE tier = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([tier.as_str()], |row| {
            let fid: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tid: String = row.get(2)?;
            let rid: String = row.get(3)?;
            let created: String = row.get(4)?;
            let score: f64 = row.get(5)?;
            let tier_str: Option<String> = row.get(6)?;
            let access_count: Option<i64> = row.get(7)?;
            let last_accessed_str: Option<String> = row.get(8)?;
            let consolidated_into_str: Option<String> = row.get(9)?;
            let promoted_at_str: Option<String> = row.get(10)?;
            let session_id_str: Option<String> = row.get(11)?;
            Ok((fid, content, tid, rid, created, score, tier_str, access_count,
                last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str))
        })?;

        let mut results = Vec::new();
        for row in rows {
            let (fid, content, tid, rid, created, score, tier_str, access_count,
                 last_accessed_str, consolidated_into_str, promoted_at_str, session_id_str) = row?;
            results.push(MemoryFragment {
                fragment_id: Uuid::parse_str(&fid).unwrap_or_else(|_| Uuid::now_v7()),
                content,
                embedding: Vec::new(),
                thread_id: Uuid::parse_str(&tid).unwrap_or_else(|_| Uuid::now_v7()),
                run_id: Uuid::parse_str(&rid).unwrap_or_else(|_| Uuid::now_v7()),
                created_at: chrono::DateTime::parse_from_rfc3339(&created)
                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                score: score as f32,
                tier: parse_tier(tier_str.as_deref()),
                access_count: access_count.unwrap_or(0) as u64,
                last_accessed: parse_optional_datetime(last_accessed_str.as_deref()),
                consolidated_into: parse_optional_uuid(consolidated_into_str.as_deref()),
                promoted_at: parse_optional_datetime(promoted_at_str.as_deref()),
                session_id: parse_optional_uuid(session_id_str.as_deref()),
            });
        }

        Ok(results)
    }

    async fn update_metadata(&self, fragment_id: Uuid, updates: MetadataUpdate) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite conn poisoned");

        // Verify fragment exists
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM memory_fragments WHERE fragment_id = ?1",
            [fragment_id.to_string()],
            |row| row.get(0),
        )?;

        if !exists {
            anyhow::bail!("fragment not found: {fragment_id}");
        }

        // Apply updates individually (the schema migration in task 1.6 will add these columns)
        if let Some(tier) = updates.tier {
            conn.execute(
                "UPDATE memory_fragments SET tier = ?1 WHERE fragment_id = ?2",
                rusqlite::params![tier.as_str(), fragment_id.to_string()],
            )?;
        }
        if let Some(access_count) = updates.access_count {
            conn.execute(
                "UPDATE memory_fragments SET access_count = ?1 WHERE fragment_id = ?2",
                rusqlite::params![access_count as i64, fragment_id.to_string()],
            )?;
        }
        if let Some(last_accessed) = updates.last_accessed {
            conn.execute(
                "UPDATE memory_fragments SET last_accessed = ?1 WHERE fragment_id = ?2",
                rusqlite::params![last_accessed.to_rfc3339(), fragment_id.to_string()],
            )?;
        }
        if let Some(consolidated_into) = updates.consolidated_into {
            let val = consolidated_into.map(|id| id.to_string());
            conn.execute(
                "UPDATE memory_fragments SET consolidated_into = ?1 WHERE fragment_id = ?2",
                rusqlite::params![val, fragment_id.to_string()],
            )?;
        }
        if let Some(promoted_at) = updates.promoted_at {
            let val = promoted_at.map(|dt| dt.to_rfc3339());
            conn.execute(
                "UPDATE memory_fragments SET promoted_at = ?1 WHERE fragment_id = ?2",
                rusqlite::params![val, fragment_id.to_string()],
            )?;
        }

        Ok(())
    }
}

// ── Helper functions for parsing maturation fields from SQLite rows ────────────

/// Parse a tier string from the database, defaulting to Working for NULL or unknown values.
fn parse_tier(tier_str: Option<&str>) -> MemoryTier {
    match tier_str {
        Some("working") => MemoryTier::Working,
        Some("episodic") => MemoryTier::Episodic,
        Some("semantic") => MemoryTier::Semantic,
        Some("procedural") => MemoryTier::Procedural,
        Some(unknown) => {
            warn!(tier = unknown, "unknown tier value in database, defaulting to working");
            MemoryTier::Working
        }
        None => MemoryTier::Working,
    }
}

/// Parse an optional RFC3339 datetime string from the database.
fn parse_optional_datetime(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|v| {
        chrono::DateTime::parse_from_rfc3339(v)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    })
}

/// Parse an optional UUID string from the database.
fn parse_optional_uuid(s: Option<&str>) -> Option<Uuid> {
    s.and_then(|v| Uuid::parse_str(v).ok())
}
