//! Project wiki — knowledge that survives between runs (ADR-0013).
//!
//! Markdown is the source of truth (`docs/wiki/*.md`, committed); any index is a
//! derived, disposable artefact (PR29). This module owns only the files.
//!
//! Two invariants hold for every write:
//!
//! 1. **Privacy boundary**: the body goes through [`crate::redact`] before it
//!    touches the disk. A wiki page is a log destination like any other.
//! 2. **Write guard**: the destination is checked by [`crate::guard::path`], so a
//!    page id can never escape `docs/wiki/` (no `..`, no absolute path, no
//!    denied name).
//!
//! Autonomy rule (ADR-0013): an unattended cycle writes to
//! `.kira/wiki-staging/`, never to `docs/wiki/`. Promotion to the committed wiki
//! is a human act (`kira wiki promote`, PR32).

use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::guard::path::{Guard, GuardError};

pub mod episode;

/// Committed wiki root, relative to the project root.
pub const WIKI_DIR: &str = "docs/wiki";

/// Where an unattended cycle stages pages before a human promotes them.
pub const STAGING_DIR: &str = ".kira/wiki-staging";

/// Fixed pages of the wiki (ADR-0013) plus the `bugs/` collection.
const FIXED_PAGES: [&str; 4] = ["contexto", "decisoes", "padroes", "glossario"];

/// Collection whose pages are `bugs/<slug>.md`.
const COLLECTION: &str = "bugs";

/// Why a wiki operation failed. Every message says what to do next.
#[derive(Debug, Error)]
pub enum WikiError {
    /// The page id is not one of the known pages/collections.
    #[error(
        "página '{page}' não existe na wiki — use um destes: {known}; ou {collection}/<slug> \
         (slug com letras minúsculas, números, '-' e '_')"
    )]
    UnknownPage {
        page: String,
        known: String,
        collection: &'static str,
    },

    /// A `bugs/<slug>` page has a slug that is not safe.
    #[error(
        "slug '{slug}' inválido — use letras minúsculas, números, '-' e '_' (ex.: bugs/4821-sessao-expirada)"
    )]
    InvalidSlug { slug: String },

    /// The write guard refused the destination (deny-list, traversal, outside root).
    #[error("{0}")]
    Guard(#[from] GuardError),

    /// The derived index could not be read or written.
    #[error("falha no índice da wiki: {0} — rode `kira doctor` para checar o store")]
    Store(#[from] crate::store::StoreError),

    /// Filesystem failure while creating the directory, writing or reading.
    #[error("falha ao {action} a wiki em '{path}': {source} — confira permissões do diretório")]
    Io {
        action: &'static str,
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// A wiki rooted at a project.
#[derive(Debug, Clone)]
pub struct Wiki {
    root: PathBuf,
    guard: Guard,
}

impl Wiki {
    /// Opens the wiki of `root` (the directory must exist; `docs/wiki` itself is
    /// created on the first write).
    pub fn open(root: &Path) -> Result<Self, WikiError> {
        let guard = Guard::new(root)?;
        Ok(Self {
            root: guard.root().to_path_buf(),
            guard,
        })
    }

    /// Canonical project root this wiki is confined to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of a page, validating the id.
    pub fn page_path(&self, page: &str) -> Result<PathBuf, WikiError> {
        let relative = self.relative_page_path(page)?;
        Ok(self.root.join(WIKI_DIR).join(relative))
    }

    /// Writes (or replaces) a page and returns its path.
    ///
    /// The body is masked by [`crate::redact`] and the destination is validated
    /// by the guard before anything touches the disk.
    pub fn write(&self, page: &str, body: &str) -> Result<PathBuf, WikiError> {
        let path = self.page_path(page)?;
        self.ensure_parent(&path)?;
        let destination = self.guard.check_write(&path)?;
        let content = format!(
            "<!-- kira-wiki page={page} updated_at={} -->\n\n{}\n",
            crate::store::now_rfc3339_utc(),
            crate::redact::redact(body)
        );
        fs::write(&destination, content).map_err(|source| WikiError::Io {
            action: "gravar",
            path: destination.display().to_string(),
            source,
        })?;
        Ok(destination)
    }

    /// Appends a timestamped section to a page, keeping what was already there.
    pub fn append(&self, page: &str, body: &str) -> Result<PathBuf, WikiError> {
        let path = self.page_path(page)?;
        self.ensure_parent(&path)?;
        let destination = self.guard.check_write(&path)?;
        let now = crate::store::now_rfc3339_utc();
        let masked = crate::redact::redact(body);
        let existing = fs::read_to_string(&destination).unwrap_or_default();
        let content = if existing.trim().is_empty() {
            format!("<!-- kira-wiki page={page} updated_at={now} -->\n\n## {now}\n{masked}\n")
        } else {
            format!("{existing}\n## {now}\n{masked}\n")
        };
        fs::write(&destination, content).map_err(|source| WikiError::Io {
            action: "gravar",
            path: destination.display().to_string(),
            source,
        })?;
        Ok(destination)
    }

    /// Reads a page; `None` when it does not exist yet.
    pub fn read(&self, page: &str) -> Result<Option<String>, WikiError> {
        let path = self.page_path(page)?;
        match fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(WikiError::Io {
                action: "ler",
                path: path.display().to_string(),
                source,
            }),
        }
    }

    /// Pages that exist, ordered (`contexto`, `decisoes`, ..., then `bugs/*`).
    pub fn list(&self) -> Result<Vec<String>, WikiError> {
        let base = self.root.join(WIKI_DIR);
        if !base.is_dir() {
            return Ok(Vec::new());
        }
        let mut pages: Vec<String> = Vec::new();
        for page in FIXED_PAGES {
            if base.join(format!("{page}.md")).is_file() {
                pages.push(page.to_string());
            }
        }
        let bugs = base.join(COLLECTION);
        if bugs.is_dir() {
            let mut found: Vec<String> = Vec::new();
            let entries = fs::read_dir(&bugs).map_err(|source| WikiError::Io {
                action: "listar",
                path: bugs.display().to_string(),
                source,
            })?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md")
                    && let Some(stem) = path.file_stem()
                {
                    found.push(format!("{COLLECTION}/{}", stem.to_string_lossy()));
                }
            }
            found.sort();
            pages.extend(found);
        }
        Ok(pages)
    }

    /// Stages a page for an unattended cycle (`.kira/wiki-staging/`).
    ///
    /// Nothing here ever reaches `docs/wiki/`: promotion is a human command.
    pub fn stage(&self, name: &str, body: &str) -> Result<PathBuf, WikiError> {
        let file_name = if std::path::Path::new(name)
            .extension()
            .is_some_and(|ext| ext == "md")
        {
            name.to_string()
        } else {
            format!("{name}.md")
        };
        let path = self.root.join(STAGING_DIR).join(file_name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| WikiError::Io {
                action: "criar",
                path: parent.display().to_string(),
                source,
            })?;
        }
        let destination = self.guard.check_write(&path)?;
        let content = format!(
            "<!-- kira-wiki staged updated_at={} -->\n\n{}\n",
            crate::store::now_rfc3339_utc(),
            crate::redact::redact(body)
        );
        fs::write(&destination, content).map_err(|source| WikiError::Io {
            action: "gravar",
            path: destination.display().to_string(),
            source,
        })?;
        Ok(destination)
    }

    /// `docs/wiki/<page>.md` or `docs/wiki/bugs/<slug>.md`, validated.
    fn relative_page_path(&self, page: &str) -> Result<PathBuf, WikiError> {
        let trimmed = page.trim().trim_start_matches('/');
        if trimmed.is_empty() {
            return Err(WikiError::UnknownPage {
                page: page.to_string(),
                known: FIXED_PAGES.join(", "),
                collection: COLLECTION,
            });
        }

        // Reject traversal/absolute shapes before anything else.
        if Path::new(trimmed)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(WikiError::InvalidSlug {
                slug: page.to_string(),
            });
        }

        if FIXED_PAGES.contains(&trimmed) {
            return Ok(PathBuf::from(format!("{trimmed}.md")));
        }

        if let Some(slug) = trimmed.strip_prefix(&format!("{COLLECTION}/")) {
            if slug.is_empty() || !slug.chars().all(is_slug_char) {
                return Err(WikiError::InvalidSlug {
                    slug: slug.to_string(),
                });
            }
            return Ok(PathBuf::from(COLLECTION).join(format!("{slug}.md")));
        }

        Err(WikiError::UnknownPage {
            page: page.to_string(),
            known: FIXED_PAGES.join(", "),
            collection: COLLECTION,
        })
    }

    /// Rebuilds the derived index (`store.db`) from the markdown on disk.
    ///
    /// Idempotent by design: the index is disposable, so running this twice
    /// yields the same rows. Returns how many pages were indexed.
    pub fn reindex(&self, store: &crate::store::Store) -> Result<usize, WikiError> {
        let pages = self.list()?;
        let mut rows = Vec::with_capacity(pages.len());
        for page in pages {
            let Some(content) = self.read(&page)? else {
                continue;
            };
            let path = self.page_path(&page)?;
            rows.push(crate::store::WikiPageRow {
                kind: kind_of(&page).to_string(),
                title: title_of(&content, &page),
                updated_at: updated_at_of(&content),
                content_hash: format!("{:016x}", crate::store::fnv1a64(content.as_bytes())),
                path: path.display().to_string(),
                slug: page,
                body: content,
            });
        }
        Ok(store.wiki_replace(&rows)?)
    }

    /// Searches the derived index (FTS5, falling back to `LIKE`).
    ///
    /// The index must have been built first (`kira wiki reindex`); searching an
    /// empty index returns no hits instead of an error.
    pub fn search(
        &self,
        store: &crate::store::Store,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::store::WikiHit>, WikiError> {
        Ok(store.wiki_search(query, limit)?)
    }

    /// Deletes a page from the committed wiki (`kira wiki forget`).
    ///
    /// The id is validated first (no `..`, no absolute path, known page or
    /// `bugs/<slug>`) and the destination goes through the write guard, so a
    /// forget can never remove anything outside `docs/wiki/`. `Ok(false)` means
    /// the page did not exist — nothing to do, not an error.
    pub fn forget(&self, page: &str) -> Result<bool, WikiError> {
        let path = self.page_path(page)?;
        if !path.is_file() {
            return Ok(false);
        }
        let destination = self.guard.check_write(&path)?;
        fs::remove_file(&destination).map_err(|source| WikiError::Io {
            action: "apagar",
            path: destination.display().to_string(),
            source,
        })?;
        Ok(true)
    }

    /// Promotes staged pages into the committed wiki (ADR-0013).
    ///
    /// An unattended cycle writes to `.kira/wiki-staging/`; this is the human act
    /// that moves knowledge into `docs/wiki/`. Staged files are named after the
    /// page with `/` replaced by `-` (`.kira` stays flat), so `bugs-4821.md` maps
    /// back to `bugs/4821`.
    ///
    /// Nothing is lost when the committed page already exists: the staged text is
    /// **appended** as a new section, not written over the top. Returns the pages
    /// promoted, in order.
    pub fn promote(&self, store: &crate::store::Store) -> Result<Vec<String>, WikiError> {
        let staging = self.root.join(STAGING_DIR);
        if !staging.is_dir() {
            return Ok(Vec::new());
        }
        let mut staged: Vec<PathBuf> = Vec::new();
        let entries = fs::read_dir(&staging).map_err(|source| WikiError::Io {
            action: "listar",
            path: staging.display().to_string(),
            source,
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                staged.push(path);
            }
        }
        staged.sort();

        let mut promoted = Vec::new();
        for path in staged {
            let Some(stem) = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
            else {
                continue;
            };
            let page = page_id_from_staged(&stem);
            // Validate the id before reading anything: a hand-planted file in
            // staging must not be able to name a destination outside the wiki.
            self.page_path(&page)?;
            let content = fs::read_to_string(&path).map_err(|source| WikiError::Io {
                action: "ler",
                path: path.display().to_string(),
                source,
            })?;
            let body = episode::strip_page_header(&content);
            if body.trim().is_empty() {
                continue;
            }
            if self.read(&page)?.is_some() {
                self.append(&page, body)?;
            } else {
                self.write(&page, body)?;
            }
            fs::remove_file(&path).map_err(|source| WikiError::Io {
                action: "remover",
                path: path.display().to_string(),
                source,
            })?;
            promoted.push(page);
        }
        if !promoted.is_empty() {
            // The index is derived: rebuild it so `search` sees the promoted text.
            let _ = self.reindex(store);
        }
        Ok(promoted)
    }

    /// Renders the whole wiki as one markdown bundle (portable, git-friendly).
    ///
    /// Returns `(bundle, pages)`; an empty wiki yields an empty bundle. Nothing is
    /// written here — the CLI decides whether to print it or save it inside the
    /// project.
    pub fn export(&self) -> Result<(String, usize), WikiError> {
        let pages = self.list()?;
        let mut bundle = String::new();
        bundle.push_str("# KIRA wiki\n\n");
        bundle.push_str(&format!(
            "Projeto: `{}`\nPáginas: {}\n",
            self.root.display().to_string().replace('\\', "/"),
            pages.len()
        ));
        for page in &pages {
            let Some(content) = self.read(page)? else {
                continue;
            };
            bundle.push_str(&format!("\n---\n\n## {} (`{}`)\n\n", page, page));
            bundle.push_str(episode::strip_page_header(&content).trim());
            bundle.push('\n');
        }
        Ok((bundle, pages.len()))
    }

    /// Writes the exported bundle to `dest`, confined to the project root.
    ///
    /// The destination goes through the write guard: `--out wiki.md` lands at the
    /// project root, `--out ../x.md` is refused. Export is a convenience, not a
    /// hole in the confinement.
    pub fn export_to(&self, dest: &Path) -> Result<(PathBuf, usize), WikiError> {
        let (bundle, pages) = self.export()?;
        let destination = self.guard.check_write(dest)?;
        fs::write(&destination, bundle).map_err(|source| WikiError::Io {
            action: "gravar",
            path: destination.display().to_string(),
            source,
        })?;
        Ok((destination, pages))
    }

    /// Creates the page's parent directory inside `docs/wiki`.
    ///
    /// Only fixed, code-owned names are created here, so a typo in a page id can
    /// never produce a new tree: unknown ids are rejected before this point.
    fn ensure_parent(&self, path: &Path) -> Result<(), WikiError> {
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        if parent.is_dir() {
            return Ok(());
        }
        fs::create_dir_all(parent).map_err(|source| WikiError::Io {
            action: "criar",
            path: parent.display().to_string(),
            source,
        })
    }
}

/// Allowed characters in a `bugs/<slug>` id.
fn is_slug_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
}

/// Maps a flat staging file stem back to a wiki page id.
///
/// `.kira/wiki-staging/` is flat (no subdirectories), so a bug page is staged as
/// `bugs-4821.md` and has to come back as `bugs/4821`. Fixed pages keep their
/// name. Anything else is left as-is — [`Wiki::page_path`] then rejects it, so a
/// hand-planted file cannot name a destination the wiki does not own.
fn page_id_from_staged(stem: &str) -> String {
    match stem.strip_prefix("bugs-") {
        Some(slug) if !slug.is_empty() => format!("{COLLECTION}/{slug}"),
        _ => stem.to_string(),
    }
}

/// Index kind: `bug` for `bugs/*`, `page` for the fixed pages.
fn kind_of(page: &str) -> &'static str {
    if page.starts_with(COLLECTION) {
        "bug"
    } else {
        "page"
    }
}

/// Title for the index: the first `# ` heading, else the page id.
fn title_of(content: &str, page: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map_or_else(|| page.to_string(), ToString::to_string)
}

/// `updated_at` from the page header written by [`Wiki::write`], else now.
fn updated_at_of(content: &str) -> String {
    for line in content.lines().take(2) {
        let Some(index) = line.find("updated_at=") else {
            continue;
        };
        let rest = line[index + "updated_at=".len()..].trim();
        let value = rest.split([' ', '>']).next().unwrap_or_default().trim();
        if !value.is_empty() {
            return value.to_string();
        }
    }
    crate::store::now_rfc3339_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-wiki-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    #[test]
    fn writes_a_page_with_header_and_body() {
        let dir = temp_project("write");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = wiki
            .write("contexto", "stack: Rust + Ratatui")
            .unwrap_or_else(|err| panic!("write: {err}"));
        let content = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler: {err}"));
        assert!(
            content.contains("<!-- kira-wiki page=contexto"),
            "{content}"
        );
        assert!(content.contains("updated_at="), "{content}");
        assert!(content.contains("stack: Rust + Ratatui"), "{content}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn masks_secrets_before_writing() {
        let dir = temp_project("redact");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = wiki
            .write("decisoes", "usar token=AbC123XyZ789QqQ no CI")
            .unwrap_or_else(|err| panic!("write: {err}"));
        let content = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler: {err}"));
        assert!(
            !content.contains("AbC123XyZ789QqQ"),
            "segredo no disco: {content}"
        );
        assert!(content.contains("[REDACTED]"), "{content}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_page_ids_that_could_escape_the_wiki() {
        let dir = temp_project("escape");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        for page in ["../etc/passwd", "bugs/../../x", "/etc/passwd", "Contexto"] {
            assert!(wiki.page_path(page).is_err(), "{page} deveria ser recusado");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nests_bug_pages_under_the_collection() {
        let dir = temp_project("bugs");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = wiki
            .write(
                "bugs/4821-sessao-expirada",
                "causa raiz: token renovado sem rotação",
            )
            .unwrap_or_else(|err| panic!("write: {err}"));
        assert!(
            path.ends_with("docs/wiki/bugs/4821-sessao-expirada.md"),
            "{path:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_keeps_what_was_there() {
        let dir = temp_project("append");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write("decisoes", "primeira decisão")
            .unwrap_or_else(|err| panic!("write: {err}"));
        let path = wiki
            .append("decisoes", "segunda decisão")
            .unwrap_or_else(|err| panic!("append: {err}"));
        let content = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler: {err}"));
        assert!(content.contains("primeira decisão"), "{content}");
        assert!(content.contains("segunda decisão"), "{content}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_fixed_pages_and_bugs_in_order() {
        let dir = temp_project("list");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        assert!(
            wiki.list()
                .unwrap_or_else(|err| panic!("list: {err}"))
                .is_empty()
        );
        wiki.write("contexto", "c")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.write("bugs/2-b", "b")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.write("bugs/1-a", "a")
            .unwrap_or_else(|err| panic!("{err}"));
        let pages = wiki.list().unwrap_or_else(|err| panic!("list: {err}"));
        assert_eq!(pages, vec!["contexto", "bugs/1-a", "bugs/2-b"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_missing_page_is_none_not_an_error() {
        let dir = temp_project("missing");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        assert!(
            wiki.read("glossario")
                .unwrap_or_else(|err| panic!("{err}"))
                .is_none()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn staging_never_touches_the_committed_wiki() {
        let dir = temp_project("stage");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let staged = wiki
            .stage("episodio-4821", "causa raiz prevista: ...")
            .unwrap_or_else(|err| panic!("stage: {err}"));
        assert!(
            staged.ends_with(".kira/wiki-staging/episodio-4821.md"),
            "{staged:?}"
        );
        assert!(
            !dir.join(WIKI_DIR).exists(),
            "staging não pode criar docs/wiki"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    fn open_store(dir: &Path) -> crate::store::Store {
        let store_dir = dir.join("store");
        crate::store::Store::open_in_dir(&store_dir)
            .unwrap_or_else(|err| panic!("abrir store: {err}"))
    }

    #[test]
    fn reindex_indexes_every_page_and_search_finds_it() {
        let dir = temp_project("reindex");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let store = open_store(&dir);
        wiki.write("contexto", "# Contexto\nstack: Rust + Ratatui")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.write(
            "bugs/4821-sessao-expirada",
            "# Sessão expirada\ncausa raiz: token renovado sem rotacionar",
        )
        .unwrap_or_else(|err| panic!("{err}"));

        let indexed = wiki
            .reindex(&store)
            .unwrap_or_else(|err| panic!("reindex: {err}"));
        assert_eq!(indexed, 2, "duas páginas deveriam ser indexadas");

        let hits = wiki
            .search(&store, "token", 5)
            .unwrap_or_else(|err| panic!("search: {err}"));
        assert_eq!(hits.len(), 1, "achados: {hits:?}");
        assert_eq!(hits[0].slug, "bugs/4821-sessao-expirada");
        assert_eq!(hits[0].kind, "bug");
        assert_eq!(hits[0].title, "Sessão expirada", "título vem do heading");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reindex_is_idempotent_and_tracks_changes() {
        let dir = temp_project("idempotent");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let store = open_store(&dir);
        wiki.write("glossario", "# Glossário\ntermo: pin")
            .unwrap_or_else(|err| panic!("{err}"));
        let first = wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));
        let second = wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(first, second, "reindex deve ser idempotente");

        wiki.write("glossario", "# Glossário\ntermo: jail")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));
        let old = wiki
            .search(&store, "pin", 5)
            .unwrap_or_else(|err| panic!("{err}"));
        assert!(
            old.is_empty(),
            "índice deveria refletir a versão nova: {old:?}"
        );
        let new = wiki
            .search(&store, "jail", 5)
            .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(new.len(), 1, "achados: {new:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn promote_moves_a_staged_bug_page_into_the_committed_wiki() {
        let dir = temp_project("promote");
        let store = open_store(&dir);
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        // Unattended cycle stages; a human promotes (ADR-0013).
        wiki.stage("bugs-4821-sessao-expirada", "causa raiz: token sem rotação")
            .unwrap_or_else(|err| panic!("stage: {err}"));
        assert!(!dir.join(WIKI_DIR).join("bugs").exists());

        let promoted = wiki
            .promote(&store)
            .unwrap_or_else(|err| panic!("promote: {err}"));
        assert_eq!(promoted, vec!["bugs/4821-sessao-expirada".to_string()]);

        let page = wiki
            .read("bugs/4821-sessao-expirada")
            .unwrap_or_else(|err| panic!("{err}"))
            .expect("página promovida deveria existir");
        assert!(page.contains("token sem rotação"), "{page}");
        assert!(
            !dir.join(STAGING_DIR)
                .join("bugs-4821-sessao-expirada.md")
                .exists(),
            "staging deveria ficar vazio depois da promoção"
        );

        // O índice é derivado: promoção reindexia, então a busca enxerga a página.
        let hits = wiki
            .search(&store, "rotação", 5)
            .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(hits.len(), 1, "{hits:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn promote_appends_when_the_page_already_exists() {
        let dir = temp_project("promote-append");
        let store = open_store(&dir);
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write("bugs/1-x", "# Bug 1\nconhecimento antigo")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.stage("bugs-1-x", "conhecimento novo")
            .unwrap_or_else(|err| panic!("stage: {err}"));

        wiki.promote(&store)
            .unwrap_or_else(|err| panic!("promote: {err}"));
        let page = wiki
            .read("bugs/1-x")
            .unwrap_or_else(|err| panic!("{err}"))
            .expect("página");
        assert!(page.contains("conhecimento antigo"), "preservou: {page}");
        assert!(page.contains("conhecimento novo"), "acrescentou: {page}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn promote_on_empty_staging_is_a_no_op() {
        let dir = temp_project("promote-empty");
        let store = open_store(&dir);
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        assert!(
            wiki.promote(&store)
                .unwrap_or_else(|err| panic!("{err}"))
                .is_empty()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_bundles_every_page_in_order() {
        let dir = temp_project("export");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write("contexto", "# Contexto\nstack Rust")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.write("bugs/2-b", "# Bug 2\ntoken")
            .unwrap_or_else(|err| panic!("{err}"));

        let (bundle, pages) = wiki.export().unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(pages, 2);
        assert!(bundle.starts_with("# KIRA wiki"), "{bundle}");
        let ctx = bundle.find("## contexto").unwrap_or(usize::MAX);
        let bug = bundle.find("## bugs/2-b").unwrap_or(usize::MAX);
        assert!(ctx < bug, "ordem estável: {bundle}");
        assert!(bundle.contains("stack Rust"), "{bundle}");
        // O cabeçalho interno não vaza para o bundle.
        assert!(!bundle.contains("<!-- kira-wiki"), "{bundle}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_to_refuses_a_destination_outside_the_project() {
        let dir = temp_project("export-guard");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write("contexto", "# Contexto")
            .unwrap_or_else(|err| panic!("{err}"));
        assert!(
            wiki.export_to(Path::new("../fora.md")).is_err(),
            "export para fora da raiz deve ser recusado"
        );
        let (path, _) = wiki
            .export_to(Path::new("wiki-bundle.md"))
            .unwrap_or_else(|err| panic!("export interno: {err}"));
        assert!(path.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_on_an_empty_index_is_empty_not_an_error() {
        let dir = temp_project("emptyindex");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let store = open_store(&dir);
        let hits = wiki
            .search(&store, "qualquer", 5)
            .unwrap_or_else(|err| panic!("search: {err}"));
        assert!(hits.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forget_deletes_the_page_and_reports_when_it_was_missing() {
        let dir = temp_project("forget");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = wiki
            .write("bugs/4821", "# Bug 4821\ncausa: TTL curto")
            .unwrap_or_else(|err| panic!("{err}"));
        assert!(path.is_file());

        assert!(
            wiki.forget("bugs/4821")
                .unwrap_or_else(|err| panic!("forget: {err}")),
            "a página existia"
        );
        assert!(!path.exists(), "arquivo deveria sumir do disco");
        assert!(
            !wiki
                .forget("bugs/4821")
                .unwrap_or_else(|err| panic!("forget repetido: {err}")),
            "esquecer de novo é no-op"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn forget_refuses_unknown_and_traversing_ids_without_deleting_anything() {
        let dir = temp_project("forget-denied");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let inside = wiki
            .write("contexto", "# Contexto")
            .unwrap_or_else(|err| panic!("{err}"));
        for page in ["../../etc/passwd", "/etc/passwd", "Contexto", "bugs/../x"] {
            assert!(
                wiki.forget(page).is_err(),
                "'{page}' deveria ser recusado antes de qualquer I/O"
            );
        }
        assert!(inside.is_file(), "nada pode ser apagado por um id inválido");
        let _ = fs::remove_dir_all(&dir);
    }
}
