//! Local SQLite store per project — the persistent trail of KIRA runs (M1.1).
//!
//! One single central DB (`{home}/.config/kira/store.db`, overridable with the
//! `KIRA_STORE_DIR` env var) in WAL mode, with three tables:
//!
//! - `projects`: one row per repository root (id = FNV-1a 64 of the canonical
//!   root path, hand-implemented below — no extra dependency);
//! - `sessions`: one row per `kira inspect` / `kira triage` / `kira doctor`
//!   run (command, args, model, timing, ok/failure, summary);
//! - `facts`: free-form profile facts of the project (`kira note`).
//!
//! Rules enforced by the callers: the store is fail-open (a store error never
//! breaks the main command) and it never stores secrets (no PAT/tokens/env)
//! nor the full work-item description — at most the truncated command output.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use thiserror::Error;

use crate::credentials;

/// Name of the single central DB file inside the store directory.
pub const DB_FILE: &str = "store.db";

/// Override for the store directory, mainly used by tests and portable runs.
pub const STORE_DIR_ENV: &str = "KIRA_STORE_DIR";

/// Current schema version, recorded in `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

/// Stored session summaries are capped at this many characters.
pub const SUMMARY_MAX_CHARS: usize = 2000;

/// Errors of the local store. Every message says what happened and points
/// somewhere; no path of this message ever contains a secret.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Neither `KIRA_STORE_DIR` nor HOME/USERPROFILE is usable.
    #[error(
        "sem diretório para o store — defina HOME (ou USERPROFILE no Windows) ou a variável KIRA_STORE_DIR"
    )]
    NoStoreDir,

    /// The store directory could not be created (`KIRA_STORE_DIR` typo,
    /// a file where a directory is expected, read-only fs, ...).
    #[error("não foi possível criar o diretório do store em '{path}': {source}")]
    CreateDir {
        path: String,
        source: std::io::Error,
    },

    /// SQLite failed to open/create the DB file.
    #[error("não foi possível abrir o store em '{path}': confira o caminho e o arquivo ({source})")]
    Open {
        path: String,
        source: rusqlite::Error,
    },

    /// The schema migration (PRAGMA user_version) failed.
    #[error("não foi possível preparar o esquema do store em '{path}': {source}")]
    Schema {
        path: String,
        source: rusqlite::Error,
    },

    /// A write/read inside the store failed.
    #[error("falha de leitura/escrita no store: {0}")]
    Sql(#[from] rusqlite::Error),

    /// The project root (cwd) could not be canonicalized.
    #[error("não foi possível determinar o diretório do projeto a partir de '{path}': {source}")]
    ProjectRoot {
        path: String,
        source: std::io::Error,
    },

    /// A `kira note` usage error (argument form or unknown note number).
    #[error("{message}")]
    InvalidNote { message: String },
}

/// Directory of the local store: `KIRA_STORE_DIR` wins over the same folder
/// that holds `config.toml` (`{home}/.config/kira`, `USERPROFILE` fallback).
pub fn store_dir() -> Option<PathBuf> {
    let override_dir = std::env::var(STORE_DIR_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    override_dir.map_or_else(credentials::config_dir, |dir| Some(PathBuf::from(dir)))
}

/// Full path of the local store DB (`{store_dir}/store.db`).
pub fn db_path() -> Option<PathBuf> {
    store_dir().map(|dir| dir.join(DB_FILE))
}

/// Connection-backed handle over the central store.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the default central store: `{store_dir}/store.db`.
    pub fn open_default() -> Result<Store, StoreError> {
        let dir = store_dir().ok_or(StoreError::NoStoreDir)?;
        Store::open_in_dir(&dir)
    }

    /// Opens (or creates) `store.db` inside `dir`, creating `dir` first.
    pub fn open_in_dir(dir: &Path) -> Result<Store, StoreError> {
        let path = dir.join(DB_FILE);
        if let Err(source) = std::fs::create_dir_all(dir) {
            return Err(StoreError::CreateDir {
                path: dir.display().to_string(),
                source,
            });
        }
        Store::open_at(&path)
    }

    /// Opens (or creates) the DB exactly at `path`.
    pub fn open_at(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path).map_err(|source| StoreError::Open {
            path: path.display().to_string(),
            source,
        })?;
        // WAL: concurrent readers (e.g. `kira history`) never block the writer.
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|source| StoreError::Schema {
                path: path.display().to_string(),
                source,
            })?;
        migrate(&conn, path)?;
        Ok(Store { conn })
    }

    /// Ensures this project root has a row in `projects` and returns it.
    ///
    /// The root is canonicalized first, so two calls on the same repo (plain
    /// path vs. symlinked path) converge to the same project id.
    pub fn ensure_project(&self, root: &Path) -> Result<Project, StoreError> {
        let canonical = canonical_root(root)?;
        let root_path = canonical.to_string_lossy().to_string();
        let id = project_id_for_root_str(&canonical);
        let name = project_name_of(&canonical);
        self.conn
            .execute(
                "INSERT OR IGNORE INTO projects (id, root_path, name, first_seen_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, root_path, name, now_rfc3339_utc()],
            )
            .map_err(StoreError::Sql)?;
        self.conn
            .query_row(
                "SELECT root_path, name, first_seen_at FROM projects WHERE id = ?1",
                [&id],
                |row| {
                    Ok(Project {
                        id: id.clone(),
                        root_path: row.get(0)?,
                        name: row.get(1)?,
                        first_seen_at: row.get(2)?,
                    })
                },
            )
            .map_err(StoreError::Sql)
    }

    /// Looks the project up by root path without creating it.
    pub fn find_project(&self, root: &Path) -> Result<Option<Project>, StoreError> {
        let canonical = canonical_root(root)?;
        let id = project_id_for_root_str(&canonical);
        let project = self
            .conn
            .query_row(
                "SELECT root_path, name, first_seen_at FROM projects WHERE id = ?1",
                [&id],
                |row| {
                    Ok(Project {
                        id: id.clone(),
                        root_path: row.get(0)?,
                        name: row.get(1)?,
                        first_seen_at: row.get(2)?,
                    })
                },
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(StoreError::Sql(other)),
            })?;
        Ok(project)
    }

    /// Records one finished session (ok or failed) for the project.
    ///
    /// `summary` is capped at [`SUMMARY_MAX_CHARS`] and is always caller-side
    /// command output — never env vars, never the PAT, never the full work
    /// item description.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        project: &Project,
        command: &str,
        args: &str,
        model: Option<&str>,
        started_at: &str,
        duration_ms: i64,
        ok: bool,
        summary: &str,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO sessions (project_id, command, args, model, started_at, \
                 duration_ms, ok, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    project.id,
                    command,
                    args,
                    model,
                    started_at,
                    duration_ms.max(0),
                    i64::from(ok),
                    cap_summary(summary),
                ],
            )
            .map_err(StoreError::Sql)?;
        Ok(())
    }

    /// Local-time display convention, one SQL line: converts a stored RFC 3339
    /// UTC timestamp to the machine's local time via SQLite
    /// (`datetime(x, 'localtime')`) — no extra dependency and no panic on NULL.
    pub fn local_display(&self, rfc3339: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row("SELECT datetime(?1, 'localtime')", [rfc3339], |row| {
                row.get::<_, Option<String>>(0)
            })
            .map_err(StoreError::Sql)
    }

    /// Last `limit` sessions of the project, most recent first.
    pub fn last_sessions(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<SessionRow>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, command, args, model, started_at, duration_ms, ok, summary \
                 FROM sessions WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(StoreError::Sql)?;
        let rows = statement
            .query_map(rusqlite::params![project_id, limit as i64], |row| {
                let summary: String = row.get(7)?;
                Ok(SessionRow {
                    id: row.get(0)?,
                    command: row.get(1)?,
                    args: row.get(2)?,
                    model: row.get(3)?,
                    started_at: row.get(4)?,
                    local_started_at: None,
                    duration_ms: row.get(5)?,
                    ok: row.get::<_, i64>(6)? != 0,
                    summary,
                })
            })
            .map_err(StoreError::Sql)?;
        let mut sessions = Vec::new();
        for row in rows {
            let mut session = row.map_err(StoreError::Sql)?;
            session.local_started_at = self.local_display(&session.started_at)?;
            sessions.push(session);
        }
        Ok(sessions)
    }

    /// Appends one fact as `key = note-N` (N incremental per project — facts
    /// are append-only, no UPSERT) and returns the assigned key.
    pub fn add_fact(&self, project_id: &str, value: &str) -> Result<String, StoreError> {
        let number = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(CAST(substr(key, 6) AS INTEGER)), 0) FROM facts \
                 WHERE project_id = ?1 AND key LIKE 'note-%'",
                [project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::Sql)?;
        let key = format!("note-{}", number + 1);
        self.conn
            .execute(
                "INSERT INTO facts (project_id, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![project_id, key, value, now_rfc3339_utc()],
            )
            .map_err(StoreError::Sql)?;
        Ok(key)
    }

    /// Removes the fact with the given key; `true` when a row was deleted.
    pub fn remove_fact(&self, project_id: &str, key: &str) -> Result<bool, StoreError> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM facts WHERE project_id = ?1 AND key = ?2",
                rusqlite::params![project_id, key],
            )
            .map_err(StoreError::Sql)?;
        Ok(deleted > 0)
    }

    /// All facts of the project, ordered by numeric note number.
    pub fn facts(&self, project_id: &str) -> Result<Vec<FactRow>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT key, value, updated_at, datetime(updated_at, 'localtime') \
                 FROM facts WHERE project_id = ?1 \
                 ORDER BY CAST(substr(key, 6) AS INTEGER)",
            )
            .map_err(StoreError::Sql)?;
        let rows = statement
            .query_map([project_id], |row| {
                Ok(FactRow {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    updated_at: row.get(2)?,
                    local_updated_at: row.get::<_, Option<String>>(3)?,
                })
            })
            .map_err(StoreError::Sql)?;
        let mut facts = Vec::new();
        for row in rows {
            facts.push(row.map_err(StoreError::Sql)?);
        }
        Ok(facts)
    }
}

/// A project row.
#[derive(Debug, Clone)]
pub struct Project {
    /// Hex id = FNV-1a 64 of the canonical root path.
    pub id: String,
    /// Canonical root path of the repository/project.
    pub root_path: String,
    /// Directory name of the root.
    pub name: String,
    /// RFC 3339 UTC timestamp of the first KIRA run in this root.
    pub first_seen_at: String,
}

/// One recorded session.
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub command: String,
    pub args: String,
    pub model: Option<String>,
    /// RFC 3339 UTC.
    pub started_at: String,
    /// `started_at` already converted to the machine's local time for display.
    pub local_started_at: Option<String>,
    pub duration_ms: i64,
    pub ok: bool,
    /// Truncated command output (first line is shown by `kira history`).
    pub summary: String,
}

/// One profile fact (`kira note`).
#[derive(Debug, Clone)]
pub struct FactRow {
    pub key: String,
    pub value: String,
    /// RFC 3339 UTC.
    pub updated_at: String,
    /// `updated_at` already converted to local time for display.
    pub local_updated_at: Option<String>,
}

/// Creates the missing tables and stamps `PRAGMA user_version`.
///
/// Migration policy: all statements are `CREATE TABLE IF NOT EXISTS`, so the
/// step is idempotent and future versions only append new statements per
/// bumped [`SCHEMA_VERSION`].
fn migrate(conn: &Connection, path: &Path) -> Result<(), StoreError> {
    let schema_error = |source| StoreError::Schema {
        path: path.display().to_string(),
        source,
    };
    let current: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(schema_error)?;
    if current >= SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            root_path TEXT NOT NULL,
            name TEXT NOT NULL,
            first_seen_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            command TEXT NOT NULL,
            args TEXT NOT NULL DEFAULT '',
            model TEXT,
            started_at TEXT NOT NULL,
            duration_ms INTEGER NOT NULL,
            ok INTEGER NOT NULL,
            summary TEXT NOT NULL DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS facts (
            project_id TEXT,
            key TEXT,
            value TEXT,
            updated_at TEXT,
            PRIMARY KEY (project_id, key)
        );
        PRAGMA user_version = 1;",
    )
    .map_err(schema_error)
}

/// Hand-rolled FNV-1a 64-bit (no extra dependency) — used to build stable
/// project ids from the canonical root path.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Canonical (real) path of a root: fails when the path does not exist.
fn canonical_root(root: &Path) -> Result<PathBuf, StoreError> {
    root.canonicalize()
        .map_err(|source| StoreError::ProjectRoot {
            path: root.display().to_string(),
            source,
        })
}

/// Project id: FNV-1a 64 of the canonical root path, hex, zero-padded.
fn project_id_for_root_str(canonical_root: &Path) -> String {
    let source = canonical_root.to_string_lossy();
    let bytes = source.as_bytes();
    format!("{:016x}", fnv1a64(bytes))
}

/// Directory name of the root (the full path is the fallback for `/`).
fn project_name_of(canonical_root: &Path) -> String {
    canonical_root.file_name().map_or_else(
        || canonical_root.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Canonical project id from an already-real root path (test-friendly).
pub fn project_id_for_root(root: &Path) -> Result<String, StoreError> {
    let canonical = canonical_root(root)?;
    Ok(project_id_for_root_str(&canonical))
}

/// Detects the project root: climbs from `base` to the first ancestor with a
/// `.git` marker (file or directory); falls back to the *nearest* ancestor
/// with `Cargo.toml` or `package.json`; falls back to `base` itself.
///
/// Pure over its input — never reads the environment.
pub fn detect_project_root_from(base: &Path) -> PathBuf {
    let mut profile_fallback: Option<PathBuf> = None;
    for dir in base.ancestors() {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        if profile_fallback.is_none()
            && (dir.join("Cargo.toml").exists() || dir.join("package.json").exists())
        {
            profile_fallback = Some(dir.to_path_buf());
        }
    }
    profile_fallback.unwrap_or_else(|| base.to_path_buf())
}

/// [`detect_project_root_from`] starting at the current working directory.
pub fn detect_project_root() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| detect_project_root_from(&cwd))
}

/// Current time in RFC 3339 UTC (`...Z`) — no chrono dependency.
///
/// Callers display it in local time via SQLite (`datetime(x, 'localtime')`).
pub fn now_rfc3339_utc() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| i64::try_from(duration.as_secs()).unwrap_or(0));
    rfc3339_utc_from_unix_seconds(seconds)
}

/// Formats Unix seconds as RFC 3339 UTC (testable, no clock access).
pub fn rfc3339_utc_from_unix_seconds(seconds: i64) -> String {
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60,
    );
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Days since 1970-01-01 → (year, month, day) in the proleptic Gregorian
/// calendar (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * mp + 2).div_euclid(5) + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (year + i64::from(month <= 2), month, day)
}

/// Keeps the first [`SUMMARY_MAX_CHARS`] characters of the summary.
fn cap_summary(summary: &str) -> String {
    if summary.chars().count() <= SUMMARY_MAX_CHARS {
        summary.to_string()
    } else {
        summary.chars().take(SUMMARY_MAX_CHARS).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique temp directory per call (std only, as in the other e2e tests).
    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("kira-store-{}-{name}-{nanos}", std::process::id()))
    }

    #[test]
    fn fnv1a64_matches_known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    #[test]
    fn project_ids_only_depend_on_the_canonical_root() {
        let dir = temp_dir("ids");
        let inner = dir.join("repo");
        std::fs::create_dir_all(&inner).unwrap_or_else(|err| panic!("criar {inner:?}: {err}"));
        let id = project_id_for_root(&inner).unwrap_or_else(|err| panic!("id: {err}"));
        assert_eq!(id.len(), 16, "id hex de 64 bits: {id}");
        // The manual FNV over the canonical path must equal the API result...
        let canonical =
            std::fs::canonicalize(&inner).unwrap_or_else(|err| panic!("canonicalizar: {err}"));
        let manual = format!("{:016x}", fnv1a64(canonical.to_string_lossy().as_bytes()));
        assert_eq!(id, manual);
        // ...and a redundant `.` component converges to the same root.
        let dotted =
            project_id_for_root(&inner.join(".")).unwrap_or_else(|err| panic!("id com '.': {err}"));
        assert_eq!(dotted, id, "caminhos canônicos iguais => mesmo id");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamps_are_rfc3339_utc() {
        assert_eq!(rfc3339_utc_from_unix_seconds(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            rfc3339_utc_from_unix_seconds(630_72000),
            "1972-01-01T00:00:00Z"
        );
        assert_eq!(
            rfc3339_utc_from_unix_seconds(951_782_400),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            rfc3339_utc_from_unix_seconds(1_000_000_000),
            "2001-09-09T01:46:40Z"
        );
        assert!(now_rfc3339_utc().ends_with('Z'));
    }

    #[test]
    fn detect_project_root_prefers_git_then_manifests_then_cwd() {
        let dir = temp_dir("detect");
        let git_repo = dir.join("git-repo");
        std::fs::create_dir_all(git_repo.join(".git"))
            .unwrap_or_else(|err| panic!("criar .git: {err}"));
        let nested = git_repo.join("src").join("deep");
        std::fs::create_dir_all(&nested).unwrap_or_else(|err| panic!("criar nested: {err}"));
        assert_eq!(
            detect_project_root_from(&nested),
            std::fs::canonicalize(&git_repo).unwrap_or_else(|err| panic!("canonicalizar: {err}")),
            "então sobe até .git"
        );

        let manifest_repo = dir.join("manifest-repo");
        std::fs::create_dir_all(&manifest_repo)
            .unwrap_or_else(|err| panic!("criar manifest repo: {err}"));
        std::fs::write(manifest_repo.join("Cargo.toml"), "[package]\n")
            .unwrap_or_else(|err| panic!("escrever Cargo.toml: {err}"));
        let nested_manifest = manifest_repo.join("crate");
        std::fs::create_dir_all(&nested_manifest)
            .unwrap_or_else(|err| panic!("criar crate: {err}"));
        assert_eq!(
            detect_project_root_from(&nested_manifest),
            std::fs::canonicalize(&manifest_repo)
                .unwrap_or_else(|err| panic!("canonicalizar: {err}"))
        );

        // Directory without any marker: the directory itself is the root.
        assert_eq!(
            detect_project_root_from(&dir),
            std::fs::canonicalize(&dir).unwrap_or_else(|err| panic!("canonicalizar: {err}"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Opens a fresh store in a temp dir; the dir is removed on drop of the
    /// returned keep-alive path.
    fn open_temp_store(name: &str) -> (Store, PathBuf) {
        let dir = temp_dir(name);
        (
            Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("abrir store: {err}")),
            dir,
        )
    }

    #[test]
    fn roundtrip_ensure_record_and_last_sessions() {
        let (store, dir) = open_temp_store("roundtrip");
        let project = store
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project: {err}"));
        assert_eq!(
            project.name,
            dir.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        );
        store
            .record(
                &project,
                "inspect",
                "4821",
                None,
                "2026-11-10T21:00:00Z",
                150,
                true,
                "#4821  Bug  Active\nTítulo: sessão",
            )
            .unwrap_or_else(|err| panic!("record: {err}"));
        store
            .record(
                &project,
                "doctor",
                "",
                None,
                "2026-11-10T21:05:00Z",
                2_100,
                false,
                "PAT não encontrado — defina AZURE_DEVOPS_PAT",
            )
            .unwrap_or_else(|err| panic!("record: {err}"));

        let sessions = store
            .last_sessions(&project.id, 10)
            .unwrap_or_else(|err| panic!("last_sessions: {err}"));
        assert_eq!(sessions.len(), 2);
        let latest = &sessions[0];
        assert_eq!(latest.command, "doctor");
        assert!(!latest.ok);
        assert_eq!(latest.duration_ms, 2_100);
        assert_eq!(latest.started_at, "2026-11-10T21:05:00Z");
        assert!(
            latest
                .local_started_at
                .as_deref()
                .is_some_and(|text| text.contains("2026")),
            "convenção local presente: {:?}",
            latest.local_started_at
        );

        let limited = store
            .last_sessions(&project.id, 1)
            .unwrap_or_else(|err| panic!("last_sessions: {err}"));
        assert_eq!(limited.len(), 1, "limit respeitado");
        assert_eq!(limited[0].command, "doctor");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn summaries_are_capped_and_stored_round_trip() {
        let (store, dir) = open_temp_store("summary-cap");
        let project = store
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project: {err}"));
        let long = "x".repeat(SUMMARY_MAX_CHARS + 500);
        store
            .record(
                &project,
                "triage",
                "--file x.json",
                Some("modelo"),
                "2026-11-10T21:00:00Z",
                10,
                true,
                &long,
            )
            .unwrap_or_else(|err| panic!("record: {err}"));
        let sessions = store
            .last_sessions(&project.id, 1)
            .unwrap_or_else(|err| panic!("last_sessions: {err}"));
        assert_eq!(sessions[0].summary.chars().count(), SUMMARY_MAX_CHARS);
        assert_eq!(sessions[0].model.as_deref(), Some("modelo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn facts_are_append_only_with_incremental_keys_and_removable() {
        let (store, dir) = open_temp_store("facts");
        let project = store
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project: {err}"));
        let first = store
            .add_fact(&project.id, "build roda em make")
            .unwrap_or_else(|err| panic!("add_fact: {err}"));
        let second = store
            .add_fact(&project.id, "deploy = cargo build --release")
            .unwrap_or_else(|err| panic!("add_fact: {err}"));
        assert_eq!(first, "note-1");
        assert_eq!(second, "note-2");

        let facts = store
            .facts(&project.id)
            .unwrap_or_else(|err| panic!("facts: {err}"));
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].key, "note-1");
        assert_eq!(facts[1].value, "deploy = cargo build --release");
        assert!(facts[0].local_updated_at.is_some());

        assert!(
            store
                .remove_fact(&project.id, "note-1")
                .unwrap_or_else(|err| panic!("remove_fact: {err}"))
        );
        assert!(
            !store
                .remove_fact(&project.id, "note-1")
                .unwrap_or_else(|err| panic!("remove_fact: {err}")),
            "remover de novo não pode achar"
        );
        // The next note keeps the maximum, so keys never collide.
        assert_eq!(
            store
                .add_fact(&project.id, "depois da remoção")
                .unwrap_or_else(|err| panic!("add_fact: {err}")),
            "note-3"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fact_numbering_skips_runaways_beyond_stored_gaps() {
        let (store, dir) = open_temp_store("facts-gaps");
        let project = store
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project: {err}"));
        for _ in 0..12 {
            store
                .add_fact(&project.id, "fato")
                .unwrap_or_else(|err| panic!("add_fact: {err}"));
        }
        assert_eq!(store.facts(&project.id).unwrap_or_default().len(), 12);
        assert!(
            store
                .remove_fact(&project.id, "note-10")
                .unwrap_or_else(|err| panic!("remove_fact: {err}"))
        );
        assert_eq!(
            store
                .add_fact(&project.id, "reposto")
                .unwrap_or_else(|err| panic!("add_fact: {err}")),
            "note-13"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn impossible_db_path_reports_the_cause() {
        let dir = temp_dir("blocked");
        std::fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        let blocker = dir.join("not-a-dir");
        std::fs::write(&blocker, "arquivo").unwrap_or_else(|err| panic!("escrever blocker: {err}"));
        let error = match Store::open_in_dir(&blocker.join("inner")) {
            Ok(_) => panic!("não pode abrir store dentro de arquivo"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains("não foi possível criar o diretório"),
            "erro deve explicar a causa: {message}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_is_idempotent_across_reopens_and_versions() {
        let (store, dir) = open_temp_store("idempotent");
        let project = store
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project: {err}"));
        drop(store);
        let reopened =
            Store::open_in_dir(&dir).unwrap_or_else(|err| panic!("reabrir store: {err}"));
        let again = reopened
            .ensure_project(&dir)
            .unwrap_or_else(|err| panic!("ensure_project de novo: {err}"));
        assert_eq!(again.id, project.id, "id estável entre execuções");
        assert_eq!(again.first_seen_at, project.first_seen_at);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
