//! Incremental FTS5 index of the repository's source files (ADR-0021).
//!
//! The old grep walked an alphabetically sorted tree and stopped at
//! `MAX_FILES_SCANNED = 400` — every file after the 400th was silently never
//! read. This indexer replaces that ceiling with [`DEFAULT_MAX_FILES`] (env
//! `KIRA_INDEX_MAX_FILES`), stores one FTS5 row per redacted source line and
//! reprocesses only files whose `(mtime, size)` changed on disk.
//!
//! Three invariants:
//!
//! 1. **Redacted on write**: every line passes [`crate::redact`] (via
//!    [`repo::clean_line`]) *before* entering the store — the store never
//!    keeps a secret, even in a derived index (AGENTS.md).
//! 2. **Deterministic**: paths are sorted before indexing and always POSIX;
//!    the same tree yields the same index state, byte-identical on a second
//!    unchanged run (no writes at all when nothing changed).
//! 3. **Fail-open at the consumer**: `search` returns an empty result instead
//!    of an error, and the packer falls back to [`repo::grep`]; `reindex`
//!    errors become a note, never a failed `triage`.

use std::collections::HashSet;
use std::path::Path;

use thiserror::Error;
use walkdir::WalkDir;

use super::repo::{self, CodeExcerpt, RepoGrep};
use crate::store::Store;

/// Env var replacing the old 400-file alphabetical ceiling (ADR-0021).
pub const INDEX_MAX_FILES_ENV: &str = "KIRA_INDEX_MAX_FILES";

/// Default ceiling of files indexed per project (validated parse: absent,
/// empty, zero or garbage falls back here).
pub const DEFAULT_MAX_FILES: usize = 5000;

/// Errors of the indexer. Only the store can fail hard here — filesystem
/// problems follow the grep contract and skip the file silently.
#[derive(Debug, Error)]
pub enum ContextError {
    /// A store read/write failed while indexing.
    #[error("falha no store durante a indexação do repositório: {0}")]
    Store(#[from] crate::store::StoreError),
}

/// What one `reindex` run did — printed by `kira context` and asserted by the
/// idempotency tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexStats {
    /// Files whose content was (re)indexed this run.
    pub files_indexed: usize,
    /// Files skipped because `(mtime, size)` is unchanged.
    pub files_unchanged: usize,
    /// Files deleted from the index (gone from the disk).
    pub files_removed: usize,
    /// FTS5 rows written this run (one per non-empty redacted line).
    pub lines_indexed: usize,
    /// True when [`DEFAULT_MAX_FILES`] (or the env override) cut the walk.
    pub truncated: bool,
    /// The ceiling this run used.
    pub max_files: usize,
}

/// [`max_files_from_env`] with an injectable value (pure, testable).
fn max_files_from_value(value: Option<&str>) -> usize {
    value
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .and_then(|trimmed| trimmed.parse::<usize>().ok())
        .filter(|parsed| *parsed > 0)
        .unwrap_or(DEFAULT_MAX_FILES)
}

/// Validated ceiling: `KIRA_INDEX_MAX_FILES` wins over [`DEFAULT_MAX_FILES`];
/// an unusable value never fails the index — it falls back to the default.
#[must_use]
pub fn max_files_from_env() -> usize {
    max_files_from_value(std::env::var(INDEX_MAX_FILES_ENV).ok().as_deref())
}

/// Rebuilds/refreshes the retrieval index for `project_id` from the tree under
/// `root`. Incremental: only files whose `(mtime, size)` changed are read and
/// re-tokenized; files that left the disk are dropped from the index.
pub fn reindex(root: &Path, store: &Store, project_id: &str) -> Result<IndexStats, ContextError> {
    reindex_limited(root, store, project_id, max_files_from_env())
}

/// Same walk with an explicit ceiling (tests inject it; runtime reads the env).
fn reindex_limited(
    root: &Path,
    store: &Store,
    project_id: &str,
    max_files: usize,
) -> Result<IndexStats, ContextError> {
    let mut stats = IndexStats {
        max_files,
        ..IndexStats::default()
    };

    // 1. Collect the candidate paths first, then sort: the same tree always
    //    indexes in the same order, independent of filesystem reply order.
    let walker = WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            if entry.file_type().is_dir() {
                !repo::is_skipped_dir(entry.path())
            } else {
                true
            }
        });
    let mut candidates: Vec<String> = Vec::new();
    for entry in walker {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(rel) = repo::relative_path(root, entry.path()) else {
            continue;
        };
        if repo::is_source_file(&rel) {
            candidates.push(rel);
        }
    }
    candidates.sort();
    if candidates.len() > max_files {
        candidates.truncate(max_files);
        stats.truncated = true;
    }

    // 2. Index what changed; skip what did not.
    let mut seen: HashSet<String> = HashSet::new();
    for rel in &candidates {
        seen.insert(rel.clone());
        let full = root.join(rel);
        let Ok(metadata) = std::fs::metadata(&full) else {
            continue; // unreadable now: skip silently, like the grep does
        };
        let mtime = mtime_secs(&metadata);
        let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        if store.repo_file_get(project_id, rel)? == Some((mtime, size)) {
            stats.files_unchanged += 1;
            continue;
        }
        let Some(content) = repo::read_text_file(&full) else {
            // Turned binary/oversized/unreadable: it must not linger in the
            // index — drop both the identity row and the lines.
            if store.repo_file_get(project_id, rel)?.is_some() {
                store.repo_fts_remove_file(project_id, rel)?;
                store.repo_file_remove(project_id, rel)?;
            }
            continue;
        };
        store.repo_fts_remove_file(project_id, rel)?;
        let mut lines = 0usize;
        for (index, line) in content.lines().enumerate() {
            // REDAÇÃO NA ESCRITA: clean_line já passa por `crate::redact`; o
            // store não pode guardar segredo nem em dado derivado (AGENTS.md).
            let cleaned = repo::clean_line(line);
            if cleaned.is_empty() {
                continue; // whitespace-only lines can never match
            }
            store.repo_fts_insert_line(project_id, rel, index + 1, &cleaned)?;
            lines += 1;
        }
        store.repo_file_upsert(project_id, rel, mtime, size)?;
        stats.files_indexed += 1;
        stats.lines_indexed += lines;
    }

    // 3. Files that left the disk leave the index too.
    for path in store.repo_file_paths(project_id)? {
        if !seen.contains(&path) {
            store.repo_fts_remove_file(project_id, &path)?;
            store.repo_file_remove(project_id, &path)?;
            stats.files_removed += 1;
        }
    }
    Ok(stats)
}

/// mtime as Unix seconds; 0 means "unknown" and forces a re-index (safe side).
fn mtime_secs(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

/// Ranked search over the FTS5 index (ADR-0021).
///
/// Uses FTS5 `MATCH` with the bm25 `rank`; files named by `priority_paths`
/// (the findings) get the same bonus the grep gives them, so the pack keeps
/// the shape `path:line`. Fail-open: an absent or empty index returns an
/// empty [`RepoGrep`] and the packer falls back to [`repo::grep`].
#[must_use]
pub fn search(
    _root: &Path,
    store: &Store,
    project_id: &str,
    terms: &[String],
    priority_paths: &[String],
    limit: usize,
) -> RepoGrep {
    let mut out = RepoGrep::default();
    if terms.is_empty() || limit == 0 {
        return out;
    }
    let Ok(total) = store.repo_file_count(project_id) else {
        return out; // no index (or broken store): fail-open, grep takes over
    };
    out.files_scanned = usize::try_from(total).unwrap_or(0);
    if out.files_scanned == 0 {
        return out; // empty index: the caller must fall back to the grep
    }
    let Some(query) = build_query(terms) else {
        return out;
    };
    // Fetch a pool larger than `limit`: the priority bonus re-orders hits, so
    // the best grep-style answer may not be the FTS5 top-N.
    let fetch = limit.saturating_mul(4).max(32);
    let Ok(rows) = store.repo_fts_search(project_id, &query, fetch) else {
        return out;
    };
    let priority: HashSet<String> = priority_paths
        .iter()
        .filter_map(|raw| repo::normalize_rel(raw))
        .collect();
    let mut hits = rows
        .into_iter()
        .map(|row| {
            let line = usize::try_from(row.line.max(0)).unwrap_or(1);
            let is_priority = priority.contains(&row.path);
            (
                is_priority,
                row.rank,
                CodeExcerpt {
                    path: row.path,
                    line,
                    text: row.text,
                    rank: Some(row.rank),
                    priority: is_priority,
                },
            )
        })
        .collect::<Vec<_>>();
    // priority first (finding bonus), then bm25 (smaller = better), then the
    // path/line tie-break that keeps the pack reproducible.
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.total_cmp(&b.1))
            .then_with(|| a.2.path.cmp(&b.2.path))
            .then_with(|| a.2.line.cmp(&b.2.line))
    });
    let mut per_file: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (_, _, excerpt) in hits {
        let count = per_file.entry(excerpt.path.clone()).or_insert(0);
        if *count >= repo::MAX_HITS_PER_FILE {
            continue;
        }
        *count += 1;
        out.excerpts.push(excerpt);
        if out.excerpts.len() == limit {
            break;
        }
    }
    out.files_with_hits = out
        .excerpts
        .iter()
        .map(|excerpt| excerpt.path.as_str())
        .collect::<HashSet<&str>>()
        .len();
    out
}

/// FTS5 `MATCH` expression: one quoted phrase per term, OR-ed. Quoting keeps a
/// stray `-` or `"` from turning into a syntax error.
fn build_query(terms: &[String]) -> Option<String> {
    let mut clauses: Vec<String> = Vec::new();
    for term in terms {
        let term = term.trim();
        if term.is_empty() {
            continue;
        }
        clauses.push(format!("\"{}\"", term.replace('"', " ")));
    }
    if clauses.is_empty() {
        None
    } else {
        Some(clauses.join(" OR "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::project_id_for_root;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-index-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    fn setup(tag: &str) -> (PathBuf, Store, String) {
        let root = temp_repo(tag);
        let store = Store::open_in_dir(&root.join(".store-dir"))
            .unwrap_or_else(|err| panic!("store: {err}"));
        let project_id =
            project_id_for_root(&root).unwrap_or_else(|err| panic!("project id: {err}"));
        (root, store, project_id)
    }

    fn fts_dump(store: &Store, project_id: &str) -> Vec<(String, i64, String)> {
        store
            .repo_fts_rows(project_id)
            .unwrap_or_else(|err| panic!("fts_rows: {err}"))
            .into_iter()
            .map(|hit| (hit.path, hit.line, hit.text))
            .collect()
    }

    #[test]
    fn env_parse_falls_back_to_the_default() {
        assert_eq!(max_files_from_value(None), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("")), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("  ")), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("banana")), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("0")), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("-3")), DEFAULT_MAX_FILES);
        assert_eq!(max_files_from_value(Some("12")), 12);
        assert_eq!(max_files_from_value(Some(" 12 ")), 12);
    }

    #[test]
    fn reindex_indexes_sorted_paths_and_the_cap_truncates_deterministically() {
        let (root, store, project_id) = setup("cap");
        write(&root, "src/a.rs", "token a\n");
        write(&root, "src/b.rs", "token b\n");
        write(&root, "src/c.rs", "token c\n");
        let stats =
            reindex_limited(&root, &store, &project_id, 2).unwrap_or_else(|err| panic!("{err}"));
        assert!(stats.truncated, "{stats:?}");
        assert_eq!(stats.files_indexed, 2, "o teto corta a caminhada");
        let paths: Vec<String> = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("{err}"))
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert_eq!(paths, vec!["src/a.rs", "src/b.rs"], "ordem determinística");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reindex_is_incremental_add_modify_and_delete() {
        let (root, store, project_id) = setup("incremental");
        write(&root, "src/a.rs", "token a\ncooldown no arquivo\n");
        let first = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(first.files_indexed, 1);
        assert_eq!(first.lines_indexed, 2);

        // Unchanged run: nothing is read or written.
        let second = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(second.files_unchanged, 1);
        assert_eq!(second.files_indexed, 0);
        assert_eq!(second.lines_indexed, 0);

        // Modify: only the changed file is re-tokenized.
        write(&root, "src/a.rs", "token a versão nova\n");
        write(&root, "src/b.rs", "token b\n");
        let third = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(third.files_indexed, 2);
        assert_eq!(third.lines_indexed, 2);

        // Delete: the file leaves the index too.
        fs::remove_file(root.join("src/b.rs")).unwrap_or_else(|err| panic!("{err}"));
        let fourth = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(fourth.files_removed, 1);
        let paths: Vec<String> = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("{err}"))
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert_eq!(paths, vec!["src/a.rs"]);
        assert!(
            !fts_dump(&store, &project_id)
                .iter()
                .any(|(path, _, _)| path == "src/b.rs"),
            "nenhuma linha do arquivo removido pode sobrar no índice"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reindex_twice_leaves_a_byte_identical_state() {
        let (root, store, project_id) = setup("idempotent");
        write(&root, "src/a.rs", "token a\n");
        write(&root, "src/b.ts", "const token = sessionToken;\n");
        let _ = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        let files_before = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("{err}"));
        let lines_before = fts_dump(&store, &project_id);
        let stats = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(stats.files_indexed, 0);
        assert_eq!(stats.files_unchanged, 2);
        assert_eq!(stats.lines_indexed, 0);
        let files_after = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("{err}"));
        let lines_after = fts_dump(&store, &project_id);
        assert_eq!(files_before, files_after, "repo_file idêntico");
        assert_eq!(lines_before, lines_after, "repo_fts idêntico");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_store_never_keeps_a_secret_from_the_index() {
        let (root, store, project_id) = setup("redact");
        write(
            &root,
            "src/config.rs",
            "let pat = \"AbC123XyZ789QqQ\";\ncontato serjota@proton.me\ntoken limpo aqui\n",
        );
        let _ = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        for (_, _, text) in fts_dump(&store, &project_id) {
            assert!(!text.contains("AbC123XyZ789QqQ"), "PAT vazou: {text}");
            assert!(!text.contains("serjota@proton.me"), "e-mail vazou: {text}");
        }
        let dumped = fts_dump(&store, &project_id);
        assert!(
            dumped
                .iter()
                .any(|(_, _, text)| text.contains("[REDACTED]")),
            "linha do PAT deve estar mascarada: {dumped:?}"
        );
        assert!(
            dumped.iter().any(|(_, _, text)| text.contains("[EMAIL]")),
            "linha do e-mail deve estar mascarada: {dumped:?}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reindex_respects_the_same_filters_as_the_grep() {
        let (root, store, project_id) = setup("filters");
        write(&root, "src/a.rs", "token no fonte\n");
        write(&root, "target/debug/b.rs", "token no build\n");
        write(&root, "node_modules/pkg/c.js", "token na dependência\n");
        write(&root, ".git/config", "token no git\n");
        write(&root, "big.ts", &format!("token {}\n", "y".repeat(200_000)));
        fs::write(root.join("image.rs"), [0u8, 1, 2, 3])
            .unwrap_or_else(|err| panic!("escrever binário: {err}"));
        let stats = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(stats.files_indexed, 1, "{stats:?}");
        let paths: Vec<String> = store
            .repo_file_rows(&project_id)
            .unwrap_or_else(|err| panic!("{err}"))
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert_eq!(paths, vec!["src/a.rs"]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn search_finds_the_rare_term_file_first() {
        let (root, store, project_id) = setup("rare");
        write(&root, "src/common.rs", "token comum\noutro token\n");
        write(&root, "src/rare.rs", "cooldown do reprocesso\n");
        let _ = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        let found = search(
            &root,
            &store,
            &project_id,
            &["cooldown".to_string()],
            &[],
            5,
        );
        assert_eq!(found.excerpts.len(), 1);
        assert_eq!(found.excerpts[0].path, "src/rare.rs");
        assert_eq!(found.excerpts[0].line, 1);
        assert_eq!(found.excerpts[0].anchor(), "src/rare.rs:1");
        assert!(found.excerpts[0].rank.is_some(), "rank bm25 presente");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn search_gives_the_priority_file_the_bonus() {
        let (root, store, project_id) = setup("priority");
        write(&root, "src/aaa.rs", "token aqui\n");
        write(&root, "src/App.csproj", "PackageReference token\n");
        let _ = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        let found = search(
            &root,
            &store,
            &project_id,
            &["token".to_string()],
            &["src/App.csproj".to_string()],
            5,
        );
        assert_eq!(
            found.excerpts[0].path, "src/App.csproj",
            "arquivo citado pelo finding na frente: {:?}",
            found.excerpts
        );
        assert!(found.excerpts[0].priority);
        assert!(!found.excerpts[1].priority);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn search_fails_open_without_an_index() {
        let (root, store, project_id) = setup("no-index");
        // Store vazio (nenhum arquivo indexado): resposta vazia, nunca erro.
        let found = search(&root, &store, &project_id, &["token".to_string()], &[], 5);
        assert!(found.is_empty());
        assert_eq!(found.files_scanned, 0);
        assert!(!found.truncated);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn search_respects_the_per_file_and_global_caps() {
        let (root, store, project_id) = setup("caps");
        let content = (0..10)
            .map(|index| format!("token na linha {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        write(&root, "src/a.rs", &content);
        write(&root, "src/b.rs", "token b\n");
        let _ = reindex(&root, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        let found = search(&root, &store, &project_id, &["token".to_string()], &[], 1);
        assert_eq!(found.excerpts.len(), 1, "limite global");
        assert_eq!(found.files_with_hits, 1, "só o melhor arquivo entra");
        let wide = search(&root, &store, &project_id, &["token".to_string()], &[], 10);
        let from_a = wide
            .excerpts
            .iter()
            .filter(|excerpt| excerpt.path == "src/a.rs")
            .count();
        assert_eq!(from_a, repo::MAX_HITS_PER_FILE, "limite por arquivo");
        let _ = fs::remove_dir_all(&root);
    }
}
