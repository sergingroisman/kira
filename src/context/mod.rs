//! Context packer — what the model is allowed to see besides the work item
//! (ADR-0013 wiki retrieval, ADR-0015 retrieval-first, PR31).
//!
//! The KIRA does the retrieval (`FTS5` on the wiki index + the last `scan`
//! brief) and hands the model a **bounded** set of excerpts, each one stamped
//! with its path and age so a conclusion can be audited later. The model never
//! chooses what to read: that is layer 1–3 work (ADR-0016), and it is what keeps
//! a 4B model on a 6 GB card useful.
//!
//! Every failure here is **fail-open**: a missing store, an unindexed wiki or a
//! stale index yields an empty pack with a note, never a failed `triage`.

use std::collections::HashSet;
use std::path::Path;

use crate::scan::Finding;
use crate::store::Store;
use crate::wiki::Wiki;
use crate::work_item::WorkItem;

pub mod gitignore;
pub mod ignore;
pub mod index;
mod repo;

pub use index::{ContextError, IndexStats, reindex};
pub use repo::{CodeExcerpt, RepoGrep, grep_text};

/// Characters reserved per context token. Conservative on purpose: Portuguese
/// prose runs ~3.5 chars/token, and under-filling the window is cheaper than a
/// truncated prompt.
pub const CHARS_PER_TOKEN: usize = 3;

/// Floor for the budget, so a silly `KIRA_NUM_CTX=16` still sends something.
const MIN_BUDGET_CHARS: usize = 1_500;

/// Hard cap for one excerpt, before the global budget is applied.
const MAX_EXCERPT_CHARS: usize = 600;

/// How many wiki pages the pack may carry.
const MAX_PAGES: usize = 3;

/// How many repository excerpts the pack may carry (PR5).
const MAX_CODE_EXCERPTS: usize = 8;

/// How many individual findings are spelled out in the prompt.
const MAX_FINDING_LINES: usize = 3;

/// Cap for the `## Perfil` block the pack may carry from `.kira/digest.md`
/// (onda M item 4). The real profile runs 20–40 lines; anything longer means
/// the digest itself was truncated — the tail is noise for the prompt.
const MAX_PACK_PROFILE_CHARS: usize = 1_500;

/// Cap for the `## Árvore` lines the pack may carry (onda M item 4). Forty
/// lines already outline a monorepo; beyond that the tree costs tokens the
/// excerpts could spend.
const MAX_PACK_TREE_LINES: usize = 40;

/// Shortest keyword worth searching (drops "de", "erro", "não").
const MIN_KEYWORD_CHARS: usize = 5;

/// Where the repository excerpts of the pack came from (ADR-0021): the FTS5
/// index or the deterministic grep walk (fail-open fallback). Serialized in
/// lowercase so old consumers and the `--json` report stay stable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoSource {
    /// The grep walk produced the excerpts (no index, or empty index).
    #[default]
    Grep,
    /// The FTS5 retrieval index produced the excerpts.
    Index,
}

impl RepoSource {
    /// `"index"` or `"grep"` — the value printed by `kira context`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grep => "grep",
            Self::Index => "index",
        }
    }
}

/// One wiki excerpt carried into the prompt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextSection {
    /// Wiki page id (`bugs/4821`).
    pub page: String,
    /// Path shown to the model and to the user (`.kira/wiki/bugs/4821.md`).
    pub path: String,
    /// Age in days taken from the page header, when parseable.
    pub age_days: Option<i64>,
    /// Bounded excerpt around the first keyword hit.
    pub excerpt: String,
}

/// The project's own outline, extracted from the `.kira/digest.md` that
/// `kira ingest` wrote: `## Perfil` (counts, stacks, extensions) and
/// `## Árvore` (the directory tree). Pure extraction with caps applied by
/// [`digest_brief`] — no LLM, no timestamps (the digest is a deterministic
/// artifact, ADR-0038 decisão 9).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DigestBrief {
    /// `## Perfil` as written, cut at [`MAX_PACK_PROFILE_CHARS`].
    pub profile: String,
    /// `## Árvore` without the fence, at most [`MAX_PACK_TREE_LINES`] lines.
    pub tree: Vec<String>,
}

/// What the prompt will carry beyond the work item.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextPack {
    /// Wiki excerpts, most relevant first.
    pub sections: Vec<ContextSection>,
    /// Repository excerpts, each one `caminho:linha` (PR5).
    pub code: Vec<CodeExcerpt>,
    /// `kira scan` brief, e.g. `high 2, moderate 1`.
    pub findings_brief: Option<String>,
    /// Individual findings worth spelling out (severity, package, project file).
    pub findings: Vec<String>,
    /// Characters the pack occupies in the prompt.
    pub used_chars: usize,
    /// The budget the pack was allowed to use.
    pub budget_chars: usize,
    /// Why something is missing (surfaced as a warning by the caller).
    pub notes: Vec<String>,
    /// Where the repository excerpts came from: the FTS5 index or the grep
    /// walk. New field (ADR-0021); serde default keeps old payloads valid.
    #[serde(default)]
    pub repo_source: RepoSource,
    /// Profile and tree from `.kira/digest.md` when the project has been
    /// ingested (onda M item 4). Serde default keeps old payloads valid.
    #[serde(default)]
    pub digest: Option<DigestBrief>,
}

impl ContextPack {
    /// True when there is nothing to add to the prompt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
            && self.code.is_empty()
            && self.findings_brief.is_none()
            && self.findings.is_empty()
            && self.digest.is_none()
    }

    /// Whether `.kira/digest.md` contributed to this pack (onda M item 4) —
    /// the honest answer `kira context` prints in its one-line summary.
    #[must_use]
    pub fn digest_used(&self) -> bool {
        self.digest.is_some()
    }

    /// Markdown block to append to the user prompt; empty when the pack is empty.
    #[must_use]
    pub fn render(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut out = String::from(
            "\nContexto do projeto (conhecimento já registrado — use como evidência e \
             cite o caminho quando concluir; idade indica se ainda vale):\n",
        );
        if let Some(brief) = &self.findings_brief {
            out.push_str(&format!("- findings do último `kira scan`: {brief}\n"));
        }
        for finding in &self.findings {
            out.push_str(&format!("- achado: {finding}\n"));
        }
        if let Some(digest) = &self.digest {
            out.push_str("\n### Perfil do projeto (`kira ingest`)\n");
            out.push_str(&digest.profile);
            out.push('\n');
            if !digest.tree.is_empty() {
                out.push_str("\n### Árvore do repositório\n```text\n");
                for line in &digest.tree {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push_str("```\n");
            }
        }
        for section in &self.sections {
            let age = section.age_days.map_or_else(
                || "idade desconhecida".to_string(),
                |days| format!("{days} dia(s) atrás"),
            );
            out.push_str(&format!("\n### {path} ({age})\n", path = section.path));
            out.push_str(&section.excerpt);
            out.push('\n');
        }
        if !self.code.is_empty() {
            out.push_str("\n### Trechos do repositório (caminho:linha)\n");
            for excerpt in &self.code {
                out.push_str(&format!("{} — {}\n", excerpt.anchor(), excerpt.text));
            }
        }
        out
    }

    /// One auditable line about what was sent — printed by `kira triage` so the
    /// user can see exactly which evidence the model had (ADR-0015).
    #[must_use]
    pub fn summary(&self) -> String {
        let mut sources: Vec<String> = Vec::new();
        if !self.sections.is_empty() {
            sources.push(format!("{} página(s) da wiki", self.sections.len()));
        }
        if !self.code.is_empty() {
            sources.push(format!("{} trecho(s) de repositório", self.code.len()));
        }
        if self.findings_brief.is_some() {
            sources.push("findings do scan".to_string());
        }
        if self.digest_used() {
            sources.push("perfil do digest".to_string());
        }
        if sources.is_empty() {
            sources.push("nenhum contexto".to_string());
        }
        format!(
            "contexto: {} — {}/{} caracteres",
            sources.join(", "),
            self.used_chars,
            self.budget_chars
        )
    }
}

/// Budget in characters derived from `KIRA_NUM_CTX` (tokens).
#[must_use]
pub fn budget_chars() -> usize {
    let tokens = usize::try_from(crate::config::num_ctx_from_env()).unwrap_or(4096);
    (tokens * CHARS_PER_TOKEN).max(MIN_BUDGET_CHARS)
}

/// `kira context --json` payload (ADR-0021): the pack plus the constants the
/// summary used, so a future recall evaluation can replay exactly what the
/// prompt carried. No LLM, no network — pure serialization.
#[derive(Debug, serde::Serialize)]
pub struct ContextReport<'a> {
    /// Work item id the pack was built for.
    pub item_id: i32,
    /// `used_chars / CHARS_PER_TOKEN`, floored — the same conservative
    /// conversion the budget uses.
    pub tokens_estimate: usize,
    /// The chars-per-token constant used (3).
    pub chars_per_token: usize,
    /// The pack that would reach the prompt.
    pub pack: &'a ContextPack,
}

/// Human report of `kira context` (ADR-0021): the source, the sections, every
/// excerpt as `path:linha — trecho truncado` and the one-line summary. With
/// `explain`, each item also shows why it entered (rank bm25, prioridade).
#[must_use]
pub fn explain_report(pack: &ContextPack, item_id: i32, explain: bool) -> String {
    let mut out = format!(
        "kira context — item {item_id} — fonte do repositório: {}\n",
        pack.repo_source.as_str()
    );
    if let Some(brief) = &pack.findings_brief {
        out.push_str(&format!("findings do último `kira scan`: {brief}\n"));
    }
    for finding in &pack.findings {
        out.push_str(&format!("- achado: {finding}\n"));
    }
    if let Some(digest) = &pack.digest {
        out.push_str(&format!(
            "- perfil do digest: {} caractere(s), árvore com {} linha(s)\n",
            digest.profile.chars().count(),
            digest.tree.len()
        ));
    }
    for section in &pack.sections {
        let age = section.age_days.map_or_else(
            || "idade desconhecida".to_string(),
            |days| format!("{days} dia(s) atrás"),
        );
        out.push_str(&format!(
            "- seção {} ({age}): {}\n",
            section.path, section.excerpt
        ));
    }
    if !pack.code.is_empty() {
        out.push_str(&format!("trechos do repositório ({}):\n", pack.code.len()));
        for excerpt in &pack.code {
            out.push_str(&format!("  - {} — {}\n", excerpt.anchor(), excerpt.text));
            if explain {
                let rank = excerpt
                    .rank
                    .map_or_else(|| "—".to_string(), |value| format!("{value:.4}"));
                out.push_str(&format!(
                    "      [fonte={}, rank={}, prioridade={}]\n",
                    pack.repo_source.as_str(),
                    rank,
                    if excerpt.priority { "sim" } else { "não" }
                ));
            }
        }
    }
    for note in &pack.notes {
        out.push_str(&format!("nota: {note}\n"));
    }
    out.push_str(&context_summary(pack));
    out
}

/// One-line summary demanded by ADR-0021:
/// `contexto: N trechos — X/Y chars (~Z tokens, 3 chars/token)`.
#[must_use]
pub fn context_summary(pack: &ContextPack) -> String {
    let tokens = pack.used_chars / CHARS_PER_TOKEN;
    format!(
        "contexto: {} trecho(s) — {}/{} chars (~{} tokens, {} chars/token)\n",
        pack.code.len(),
        pack.used_chars,
        pack.budget_chars,
        tokens,
        CHARS_PER_TOKEN
    )
}

/// `kira context --json`: the serializable report (pretty-printed).
pub fn report_json(pack: &ContextPack, item_id: i32) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&ContextReport {
        item_id,
        tokens_estimate: pack.used_chars / CHARS_PER_TOKEN,
        chars_per_token: CHARS_PER_TOKEN,
        pack,
    })
}

/// Distinct, meaningful terms of the work item — the retrieval query.
///
/// Deterministic and dumb on purpose: no stemming, no model. Lowercase, drop
/// words shorter than [`MIN_KEYWORD_CHARS`], drop a small Portuguese/English
/// stop list, keep first appearance order.
#[must_use]
pub fn keywords(text: &str, max: usize) -> Vec<String> {
    const STOPWORDS: [&str; 24] = [
        "porque", "quando", "depois", "antes", "sempre", "nunca", "sobre", "entre", "ainda",
        "todos", "todas", "outro", "outra", "onde", "errado", "deveria", "favor", "obrigado",
        "which", "there", "these", "those", "should", "would",
    ];
    let mut found: Vec<String> = Vec::new();
    for raw in text.split(|c: char| !c.is_alphanumeric() && c != '-') {
        let word = raw.trim().to_lowercase();
        if word.chars().count() < MIN_KEYWORD_CHARS || word.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if STOPWORDS.contains(&word.as_str()) || found.contains(&word) {
            continue;
        }
        found.push(word);
        if found.len() == max {
            break;
        }
    }
    found
}

/// Builds the pack from an already-opened store and wiki, with an explicit
/// budget. This is the testable core: no environment, no implicit I/O beyond the
/// two handles the caller already owns plus the project files it greps.
#[must_use]
pub fn pack_with(
    root: &Path,
    store: &Store,
    wiki: &Wiki,
    item: &WorkItem,
    budget_chars: usize,
) -> ContextPack {
    pack_partial(root, Some(store), Some(wiki), item, budget_chars, &[])
}

/// Lines of one `## Heading` section of the digest, until the next `## `
/// heading. Pure and exact-match on the heading: `###` sub-headings belong to
/// the section, and the last section runs to the end of the file.
fn digest_section(markdown: &str, heading: &str) -> Option<Vec<String>> {
    let mut wanted: Vec<String> = Vec::new();
    let mut inside = false;
    for line in markdown.lines() {
        if line.starts_with("## ") {
            if inside {
                return Some(wanted);
            }
            inside = line.trim_end() == heading;
            continue;
        }
        if inside {
            wanted.push(line.to_string());
        }
    }
    if inside { Some(wanted) } else { None }
}

/// Extracts `## Perfil` and `## Árvore` from the markdown `kira ingest`
/// rendered (onda M item 4). None when the profile is missing or empty — a
/// digest without our headings is not our digest, and the pack simply carries
/// nothing. The tree loses the ` ```text ` fence and is capped; the remainder
/// is stated, never silently dropped.
fn digest_brief(markdown: &str) -> Option<DigestBrief> {
    let profile_raw = digest_section(markdown, "## Perfil")?;
    let profile_text = profile_raw.join("\n").trim().to_string();
    if profile_text.is_empty() {
        return None;
    }
    let profile = if profile_text.chars().count() > MAX_PACK_PROFILE_CHARS {
        let mut cut: String = profile_text.chars().take(MAX_PACK_PROFILE_CHARS).collect();
        cut.push_str("\n_perfil cortado no teto de 1 500 caracteres_");
        cut
    } else {
        profile_text
    };
    let mut tree: Vec<String> = digest_section(markdown, "## Árvore")
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !line.trim().starts_with("```") && !line.trim().is_empty())
        .collect();
    if tree.len() > MAX_PACK_TREE_LINES {
        let hidden = tree.len() - MAX_PACK_TREE_LINES;
        tree.truncate(MAX_PACK_TREE_LINES);
        tree.push(format!("… (+{hidden} linha(s) fora do teto)"));
    }
    Some(DigestBrief { profile, tree })
}

/// Builds the pack from whatever sources the caller managed to open (PR5).
///
/// Each source is independent on purpose: a broken wiki must not cost the
/// repository excerpts, and a missing store must not cost the findings. The
/// budget is spent in a **declared** priority order — findings brief, individual
/// findings, digest profile/tree (item 4), wiki pages, repository excerpts — so
/// a small `KIRA_NUM_CTX` always drops the least specific evidence first.
///
/// `skip_paths` are repository-relative files the caller already sent whole (the
/// work item file): grepping them again would only duplicate the prompt.
#[must_use]
pub fn pack_partial(
    root: &Path,
    store: Option<&Store>,
    wiki: Option<&Wiki>,
    item: &WorkItem,
    budget_chars: usize,
    skip_paths: &[String],
) -> ContextPack {
    let mut pack = ContextPack {
        budget_chars,
        ..ContextPack::default()
    };

    // 1. Findings of the last `kira scan`: the brief is one line and always
    //    fits; the individual findings are the ones a triage can act on.
    let findings = findings_of(root);
    if let Some(findings) = findings.as_deref().filter(|list| !list.is_empty()) {
        let brief = crate::scan::severity_brief(findings);
        let line = format!("- findings do último `kira scan`: {brief}\n");
        pack.used_chars += line.chars().count();
        pack.findings_brief = Some(brief);
        for entry in ranked_findings(findings, MAX_FINDING_LINES) {
            let cost = entry.chars().count() + 12;
            if pack.used_chars + cost > budget_chars {
                pack.notes.push(format!(
                    "orçamento de contexto esgotado em {budget_chars} caracteres; achados detalhados ficaram de fora"
                ));
                break;
            }
            pack.used_chars += cost;
            pack.findings.push(entry);
        }
    }

    // 1b. The project's outline from `kira ingest` (onda M item 4): `## Perfil`
    //     + `## Árvore` of `.kira/digest.md`, spent right after the findings
    //     and before the wiki pages — the declared budget order holds. Read
    //     before the keyword gate: the digest is project identity, it does not
    //     depend on the work item. Absent digest is normal (fail-open, no
    //     note); unreadable is a note, never an error.
    match std::fs::read_to_string(root.join(".kira").join("digest.md")) {
        Ok(markdown) => {
            if let Some(brief) = digest_brief(&markdown) {
                let cost = brief.profile.chars().count()
                    + brief
                        .tree
                        .iter()
                        .map(|line| line.chars().count() + 1)
                        .sum::<usize>()
                    + 48;
                if pack.used_chars + cost > budget_chars {
                    pack.notes.push(format!(
                        "orçamento de contexto esgotado em {budget_chars} caracteres; o perfil do digest ficou de fora"
                    ));
                } else {
                    pack.used_chars += cost;
                    pack.digest = Some(brief);
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => pack
            .notes
            .push(format!("não foi possível ler .kira/digest.md ({err})")),
    }

    let haystack = format!(
        "{} {}",
        item.title,
        item.description.as_deref().unwrap_or_default()
    );
    let terms = keywords(&haystack, 6);
    if terms.is_empty() {
        pack.notes
            .push("work item sem termos úteis para buscar no repositório".to_string());
        return pack;
    }

    // 2. Wiki pages (PR31): FTS5 first, one query per term, ranked by hits.
    if let (Some(store), Some(wiki)) = (store, wiki) {
        let mut scored: Vec<(usize, String)> = Vec::new();
        let mut search_failed = false;
        for term in &terms {
            match wiki.search(store, term, MAX_PAGES * 2) {
                Ok(hits) => {
                    for hit in hits {
                        match scored.iter_mut().find(|(_, page)| *page == hit.slug) {
                            Some((score, _)) => *score += 1,
                            None => scored.push((1, hit.slug)),
                        }
                    }
                }
                Err(cause) => {
                    pack.notes.push(format!("busca na wiki falhou ({cause})"));
                    search_failed = true;
                    break;
                }
            }
        }
        if !search_failed {
            // Stable: more matches first, then page id, so the pack is
            // reproducible.
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            for (_, page) in scored.into_iter().take(MAX_PAGES) {
                let Ok(Some(content)) = wiki.read(&page) else {
                    continue;
                };
                let excerpt = excerpt_around(&content, &terms, MAX_EXCERPT_CHARS);
                if excerpt.is_empty() {
                    continue;
                }
                // Normalised to `/` on purpose: the model reads this path and the
                // pack must be identical across Windows, macOS and Linux. `\` in
                // a prompt was exactly the kind of difference that made the
                // Windows CI fail. `display_path` first (W2-fix PR-F3): the
                // canonical root carries the `\\?\` prefix on Windows and the
                // page path inherits it — humans read this path too.
                let path = wiki
                    .page_path(&page)
                    .map(|path| crate::store::display_path(&path).replace('\\', "/"))
                    .unwrap_or_else(|_| page.clone());
                let age_days = age_days_of(&content);
                let cost = excerpt.chars().count() + path.chars().count() + 32;
                if pack.used_chars + cost > budget_chars {
                    pack.notes.push(format!(
                        "orçamento de contexto esgotado em {budget_chars} caracteres; '{page}' ficou de fora"
                    ));
                    break;
                }
                pack.used_chars += cost;
                pack.sections.push(ContextSection {
                    page,
                    path,
                    age_days,
                    excerpt,
                });
            }
        }
    }

    // 3. Repository excerpts (PR5): the KIRA greps, the model interprets. The
    //    FTS5 retrieval index (ADR-0021) is tried first — it ranks with bm25
    //    and has no 400-file alphabetical ceiling — and the grep walk is the
    //    fail-open fallback for a missing or empty index. Files named by a
    //    finding come first in either source.
    let priority = finding_paths(findings.as_deref().unwrap_or_default());
    let mut grep = RepoGrep::default();
    let mut repo_source = RepoSource::Grep;
    if let Some(store) = store
        && let Ok(project_id) = crate::store::project_id_for_root(root)
    {
        let indexed = index::search(
            root,
            store,
            &project_id,
            &terms,
            &priority,
            MAX_CODE_EXCERPTS,
        );
        if indexed.files_scanned > 0 {
            repo_source = RepoSource::Index;
            let skipped: HashSet<String> = skip_paths
                .iter()
                .filter_map(|raw| repo::normalize_rel(raw))
                .collect();
            let mut filtered = indexed;
            filtered
                .excerpts
                .retain(|excerpt| !skipped.contains(&excerpt.path));
            grep = filtered;
        }
    }
    if repo_source == RepoSource::Grep {
        grep = repo::grep(root, &terms, &priority, skip_paths, MAX_CODE_EXCERPTS);
    }
    pack.repo_source = repo_source;
    for excerpt in grep.excerpts {
        let cost = excerpt.text.chars().count() + excerpt.path.chars().count() + 12;
        if pack.used_chars + cost > budget_chars {
            pack.notes.push(format!(
                "orçamento de contexto esgotado em {budget_chars} caracteres; trechos do repositório ficaram de fora"
            ));
            break;
        }
        pack.used_chars += cost;
        pack.code.push(excerpt);
    }
    pack
}

/// Production entry point: opens what it needs and degrades to an empty pack.
#[must_use]
pub fn pack(root: &Path, item: &WorkItem) -> ContextPack {
    pack_excluding(root, item, None)
}

/// Production entry point that also refuses to grep `exclude` — the work item
/// file the caller already placed whole in the prompt (PR5).
#[must_use]
pub fn pack_excluding(root: &Path, item: &WorkItem, exclude: Option<&Path>) -> ContextPack {
    let budget = budget_chars();
    let store = Store::open_default();
    let wiki = Wiki::open(root);
    let mut source_notes: Vec<String> = Vec::new();
    if let Err(cause) = &store {
        source_notes.push(format!("sem store para buscar na wiki ({cause})"));
    }
    if let Ok(store) = &store
        && let Ok(project_id) = crate::store::project_id_for_root(root)
    {
        // ADR-0021: refresh the incremental index before packing — the
        // same walk the grep did, minus the file reads. Fail-open: a
        // broken index degrades the pack to the grep fallback, never
        // fails the caller.
        if let Err(cause) = index::reindex(root, store, &project_id) {
            source_notes.push(format!("índice do repositório não atualizado ({cause})"));
        }
    }
    if let Err(cause) = &wiki {
        source_notes.push(format!("wiki indisponível ({cause})"));
    }
    let skip: Vec<String> = exclude
        .and_then(|path| relative_to_root(root, path))
        .into_iter()
        .collect();
    let mut pack = pack_partial(
        root,
        store.as_ref().ok(),
        wiki.as_ref().ok(),
        item,
        budget,
        &skip,
    );
    source_notes.append(&mut pack.notes);
    pack.notes = source_notes;
    pack
}

/// Repository-relative form of `path` (POSIX separators), when it lives under
/// `root`. Canonicalization on both sides handles relative flags and symlinks.
fn relative_to_root(root: &Path, path: &Path) -> Option<String> {
    let file = std::fs::canonicalize(path).ok()?;
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let rel = file.strip_prefix(&root).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// Findings of the last `kira scan`, when the file exists and parses.
fn findings_of(root: &Path) -> Option<Vec<Finding>> {
    crate::scan::read_findings_file(root).ok().flatten()
}

/// Up to `max` findings, worst severity first, ties broken by fingerprint so the
/// selection is reproducible.
fn ranked_findings(findings: &[Finding], max: usize) -> Vec<String> {
    let mut ranked: Vec<&Finding> = findings.iter().collect();
    ranked.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.fingerprint.cmp(&b.fingerprint))
    });
    ranked
        .into_iter()
        .take(max)
        .map(|finding| {
            let package = finding.package.trim();
            let version = finding.version.trim();
            let path = finding.path.trim();
            let mut line = format!(
                "{} · {package} {version}",
                finding.severity.trim().to_lowercase()
            );
            if !path.is_empty() {
                line.push_str(&format!(" ({path})"));
            }
            crate::redact::redact(line.trim())
        })
        .collect()
}

/// Ordering of severities as the tools report them (`Critical`, `high`, ...).
fn severity_rank(severity: &str) -> u8 {
    match severity.trim().to_lowercase().as_str() {
        "critical" | "crítica" | "critica" => 0,
        "high" | "alta" => 1,
        "moderate" | "medium" | "média" | "media" => 2,
        "low" | "baixa" => 3,
        _ => 4,
    }
}

/// Project files named by the findings — the grep starts there.
fn finding_paths(findings: &[Finding]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for finding in findings {
        let path = finding.path.trim();
        if path.is_empty() || paths.iter().any(|seen| seen == path) {
            continue;
        }
        paths.push(path.to_string());
    }
    paths
}

/// First `± window/2` characters around a keyword hit, newlines collapsed.
fn excerpt_around(content: &str, terms: &[String], window: usize) -> String {
    let lower = content.to_lowercase();
    let hit = terms
        .iter()
        .filter_map(|term| lower.find(term.as_str()))
        .min()
        .unwrap_or(0);
    // Byte offsets from a lowercase copy are safe here: `to_lowercase` can change
    // length for some scripts, so clamp into a char boundary of the original.
    let start = clamp_boundary(content, hit.saturating_sub(window / 3));
    let end = clamp_boundary(content, (start + window).min(content.len()));
    content[start..end]
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Snaps a byte index into `text` to a char boundary, downwards.
fn clamp_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Age of the page in days, from the `updated_at` header (`None` if unparsable).
fn age_days_of(content: &str) -> Option<i64> {
    let header = content
        .lines()
        .take(2)
        .find(|line| line.contains("updated_at="))?;
    let start = header.find("updated_at=")? + "updated_at=".len();
    let stamp = header[start..]
        .split([' ', '>'])
        .next()
        .unwrap_or_default()
        .trim();
    let then = crate::store::rfc3339_utc_to_unix_seconds(stamp)?;
    let now = crate::store::rfc3339_utc_to_unix_seconds(&crate::store::now_rfc3339_utc())?;
    Some((now - then).max(0) / 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use std::fs;
    use std::path::PathBuf;

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kira-ctx-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    fn item_with(title: &str, description: &str) -> WorkItem {
        WorkItem {
            id: 4821,
            title: title.to_string(),
            state: "Active".to_string(),
            work_item_type: "Bug".to_string(),
            assigned_to: None,
            description: Some(description.to_string()),
            area_path: None,
        }
    }

    #[test]
    fn keywords_are_deterministic_and_drop_noise() {
        let terms = keywords(
            "Sessão expirada quando o token renova; token inválido no refresh",
            6,
        );
        assert_eq!(terms[0], "sessão");
        assert!(terms.contains(&"expirada".to_string()));
        assert_eq!(
            terms.iter().filter(|term| *term == "token").count(),
            1,
            "sem duplicata: {terms:?}"
        );
        assert!(
            !terms.contains(&"quando".to_string()),
            "stopword: {terms:?}"
        );
    }

    #[test]
    fn pack_carries_a_matching_page_with_path_and_age() {
        let dir = temp_project("match");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write(
            "bugs/4821-sessao-expirada",
            "# Sessão expirada\ntoken renovado sem rotacionar; refresh inválido",
        )
        .unwrap_or_else(|err| panic!("{err}"));
        wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "token inválido no refresh"),
            8000,
        );
        assert_eq!(pack.sections.len(), 1, "notas: {:?}", pack.notes);
        assert_eq!(pack.sections[0].page, "bugs/4821-sessao-expirada");
        assert!(
            pack.sections[0]
                .path
                .ends_with(".kira/wiki/bugs/4821-sessao-expirada.md"),
            "{}",
            pack.sections[0].path
        );
        assert_eq!(pack.sections[0].age_days, Some(0));
        let rendered = pack.render();
        assert!(rendered.contains(".kira/wiki/bugs/4821-sessao-expirada.md"));
        assert!(rendered.contains("0 dia(s) atrás"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tiny_budget_keeps_the_prompt_bounded() {
        let dir = temp_project("budget");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        wiki.write("bugs/1-a", "# A\ntoken problema muito comprido aqui")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));

        let pack = pack_with(&dir, &store, &wiki, &item_with("token problema", ""), 10);
        assert!(
            pack.sections.is_empty(),
            "orçamento minúsculo não carrega página"
        );
        assert!(
            !pack.notes.is_empty(),
            "precisa explicar por que ficou de fora"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_wiki_yields_an_empty_pack_not_an_error() {
        let dir = temp_project("empty");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "token"),
            8000,
        );
        assert!(pack.is_empty(), "notas: {:?}", pack.notes);
        assert!(pack.render().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn excerpt_never_splits_a_char_boundary() {
        let content = "ação ação ação token problema";
        let excerpt = excerpt_around(content, &["token".to_string()], 20);
        assert!(excerpt.contains("token"), "{excerpt}");
        assert!(std::str::from_utf8(excerpt.as_bytes()).is_ok());
    }

    fn write_findings(dir: &Path, body: &str) {
        let kira = dir.join(".kira");
        fs::create_dir_all(&kira).unwrap_or_else(|err| panic!("criar {kira:?}: {err}"));
        fs::write(kira.join("findings.json"), body).unwrap_or_else(|err| panic!("{err}"));
    }

    fn write_file(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| panic!("criar {parent:?}: {err}"));
        }
        fs::write(&path, body).unwrap_or_else(|err| panic!("escrever {path:?}: {err}"));
    }

    const FINDINGS_HIGH: &str = r#"{"findings":[{"tool":"dotnet","rule":"vulnerable","package":"Newtonsoft.Json","version":"12.0.1","severity":"High","path":"src/App.csproj","message":"advisory","fingerprint":"0000000000000002"},{"tool":"audit","rule":"vulnerable","package":"lodash","version":"4.17.20","severity":"critical","path":"web/package.json","message":"prototype pollution","fingerprint":"0000000000000001"}],"stacks":[]}"#;

    /// PR5: findings do arquivo + trechos de repositório com `caminho:linha`,
    /// e o arquivo citado pelo achado na frente dos demais.
    #[test]
    fn pack_carries_findings_and_repo_excerpts_with_anchors() {
        let dir = temp_project("pr5-code");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_findings(&dir, FINDINGS_HIGH);
        write_file(
            &dir,
            "web/package.json",
            "{\"dependencies\":{\"lodash\":\"token\"}}\n",
        );
        write_file(
            &dir,
            "src/App.csproj",
            "<Project>\n  <PackageReference Include=\"Newtonsoft.Json\" Version=\"12.0.1\" />\n</Project>\n",
        );
        write_file(
            &dir,
            "src/auth/session.rs",
            "pub fn ttl() {\n    let ttl = session_ttl_minutes;\n}\n",
        );

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with(
                "Sessão expirada",
                "token inválido no refresh do session_ttl",
            ),
            12_000,
        );

        assert_eq!(pack.findings_brief.as_deref(), Some("critical 1, high 1"));
        assert_eq!(
            pack.findings[0],
            "critical · lodash 4.17.20 (web/package.json)"
        );
        assert!(
            pack.findings.len() >= 2,
            "os dois achados entram: {:?}",
            pack.findings
        );
        assert!(!pack.code.is_empty(), "notas: {:?}", pack.notes);
        assert_eq!(
            pack.code[0].path, "web/package.json",
            "achado mais grave primeiro: {:?}",
            pack.code
        );
        let rendered = pack.render();
        assert!(rendered.contains("web/package.json:1 —"), "{rendered}");
        assert!(
            rendered.contains("### Trechos do repositório (caminho:linha)"),
            "{rendered}"
        );
        assert!(
            pack.summary().contains("trecho(s) de repositório"),
            "{}",
            pack.summary()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// O mesmo estado tem que produzir o mesmo pack (determinismo do M1).
    #[test]
    fn the_same_state_produces_the_same_pack() {
        let dir = temp_project("pr5-determinism");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_findings(&dir, FINDINGS_HIGH);
        write_file(&dir, "src/a.rs", "let token = session_token;\n");
        write_file(&dir, "src/b.ts", "const token = sessionToken;\n");
        let item = item_with("Sessão expirada", "token inválido");

        let first = pack_with(&dir, &store, &wiki, &item, 12_000);
        let second = pack_with(&dir, &store, &wiki, &item, 12_000);
        assert_eq!(first, second);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Um store/wiki que não abriu não pode custar o grep do repositório.
    #[test]
    fn a_missing_wiki_still_yields_repo_excerpts() {
        let dir = temp_project("pr5-partial");
        write_file(&dir, "src/auth/session.rs", "let token = session_token;\n");
        let pack = pack_partial(
            &dir,
            None,
            None,
            &item_with("Sessão expirada", "token inválido"),
            12_000,
            &[],
        );
        assert!(pack.sections.is_empty());
        assert!(!pack.code.is_empty(), "notas: {:?}", pack.notes);
        assert_eq!(pack.code[0].path, "src/auth/session.rs");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Prioridade declarada do orçamento: o achado (mais barato e mais
    /// específico) sobrevive, o trecho de arquivo é o primeiro a cair.
    #[test]
    fn budget_drops_the_repo_excerpts_before_the_findings() {
        let dir = temp_project("pr5-budget");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_findings(&dir, FINDINGS_HIGH);
        write_file(
            &dir,
            "src/auth/session.rs",
            &format!("let token = {};\n", "session_token ".repeat(20)),
        );

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "token inválido"),
            130,
        );
        assert!(pack.findings_brief.is_some());
        assert!(!pack.findings.is_empty(), "notas: {:?}", pack.notes);
        assert!(
            pack.code.is_empty(),
            "trecho grande não cabe: {:?}",
            pack.code
        );
        assert!(
            pack.notes.iter().any(|note| note.contains("orçamento")),
            "precisa explicar o corte: {:?}",
            pack.notes
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// ADR-0021: com índice o pack vem do FTS5 e registra a fonte; sem índice
    /// (ou vazio) cai no grep com `RepoSource::Grep` — sem erro.
    #[test]
    fn the_pack_prefers_the_index_and_records_the_source() {
        let dir = temp_project("index-source");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_file(
            &dir,
            "src/auth/session.rs",
            "let token = session_token;\nlet cooldown = 30;\n",
        );
        let item = item_with("Sessão expirada", "token inválido no cooldown");

        let without_index = pack_with(&dir, &store, &wiki, &item, 12_000);
        assert_eq!(
            without_index.repo_source,
            RepoSource::Grep,
            "índice vazio ⇒ grep"
        );
        assert!(!without_index.code.is_empty(), "{:?}", without_index.notes);

        let project_id = crate::store::project_id_for_root(&dir)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        index::reindex(&dir, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));

        let with_index = pack_with(&dir, &store, &wiki, &item, 12_000);
        assert_eq!(with_index.repo_source, RepoSource::Index);
        assert!(!with_index.code.is_empty(), "notas: {:?}", with_index.notes);
        assert!(
            with_index.code.iter().all(|excerpt| excerpt.rank.is_some()),
            "trechos do índice carregam o rank bm25: {:?}",
            with_index.code
        );
        assert!(
            with_index
                .code
                .iter()
                .any(|excerpt| excerpt.anchor() == "src/auth/session.rs:1"),
            "{:?}",
            with_index.code
        );
        let rendered = with_index.render();
        assert!(rendered.contains("src/auth/session.rs:1 —"), "{rendered}");
        assert_eq!(
            context_summary(&with_index),
            format!(
                "contexto: {} trecho(s) — {}/12000 chars (~{} tokens, 3 chars/token)\n",
                with_index.code.len(),
                with_index.used_chars,
                with_index.used_chars / CHARS_PER_TOKEN
            )
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_json_report_round_trips_and_the_source_is_lowercase() {
        let dir = temp_project("json-report");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_file(&dir, "src/a.rs", "let token = session_token;\n");
        let project_id = crate::store::project_id_for_root(&dir)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        index::reindex(&dir, &store, &project_id).unwrap_or_else(|err| panic!("{err}"));
        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "token inválido"),
            12_000,
        );
        let json = report_json(&pack, 4821).unwrap_or_else(|err| panic!("{err}"));
        assert!(json.contains("\"repo_source\": \"index\""), "{json}");
        assert!(json.contains("\"item_id\": 4821"), "{json}");
        assert!(json.contains("\"chars_per_token\": 3"), "{json}");
        // O campo novo é opcional: um pack antigo sem `repo_source` parseia.
        let legacy: ContextPack = serde_json::from_str(
            "{\"sections\":[],\"code\":[],\"findings_brief\":null,\"findings\":[],\"used_chars\":0,\"budget_chars\":0,\"notes\":[]}",
        )
        .unwrap_or_else(|err| panic!("pack legado: {err}"));
        assert_eq!(legacy.repo_source, RepoSource::Grep);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A forma exata do `kira ingest`: seções `## `, árvore cercada por fence
    /// de texto, sub-headings `###` dentro de Arquivos-chave.
    fn synthetic_digest(tree_lines: usize, profile: &str) -> String {
        let mut out = String::from("# Digest — demo\n\n<!-- interno -->\n\n## Árvore\n\n```text\n");
        for index in 0..tree_lines {
            out.push_str(&format!("src/file{index}.rs\n"));
        }
        out.push_str("```\n\n## Perfil\n\n");
        out.push_str(profile);
        out.push_str("\n\n## Arquivos-chave\n\n### `README.md`\n\nblá\n\n## Notas\n\n_nada_\n");
        out
    }

    #[test]
    fn digest_brief_extracts_profile_and_tree_without_the_fence() {
        let brief = digest_brief(&synthetic_digest(3, "- arquivos: 3\n- stacks: rust"))
            .expect("o digest do ingest sempre tem as duas seções");
        assert_eq!(brief.tree, ["src/file0.rs", "src/file1.rs", "src/file2.rs"]);
        assert_eq!(brief.profile, "- arquivos: 3\n- stacks: rust");
    }

    #[test]
    fn digest_brief_caps_the_tree_and_states_the_remainder() {
        let brief = digest_brief(&synthetic_digest(45, "- arquivos: 45\n")).expect("perfil");
        assert_eq!(brief.tree.len(), 41, "40 linhas + a nota do corte");
        assert_eq!(brief.tree[40], "… (+5 linha(s) fora do teto)");
    }

    #[test]
    fn digest_brief_caps_the_profile_at_its_ceiling() {
        let long = "x".repeat(MAX_PACK_PROFILE_CHARS + 500);
        let brief = digest_brief(&synthetic_digest(1, &long)).expect("perfil");
        assert!(
            brief
                .profile
                .ends_with("_perfil cortado no teto de 1 500 caracteres_"),
            "corte nota a própria existência"
        );
        let kept: String = brief.profile.chars().take(MAX_PACK_PROFILE_CHARS).collect();
        assert_eq!(kept, long[..MAX_PACK_PROFILE_CHARS]);
    }

    #[test]
    fn digest_brief_refuses_markdown_without_our_sections() {
        assert!(digest_brief("# Digest — outro\n\n## Resumo\n\n- nada\n").is_none());
        assert!(
            digest_brief(&synthetic_digest(1, "   \n")).is_none(),
            "perfil vazio não entra no pack"
        );
    }

    #[test]
    fn pack_carries_the_digest_and_reports_it_everywhere() {
        let dir = temp_project("pack-digest");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_file(&dir, "src/a.rs", "let cooldown = 30;\n");
        write_file(
            &dir,
            ".kira/digest.md",
            &synthetic_digest(2, "- arquivos: 2\n- stacks: rust"),
        );

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Cooldown expirou", "o cooldown da fila nunca volta"),
            12_000,
        );
        assert!(pack.digest_used(), "notas: {:?}", pack.notes);
        let rendered = pack.render();
        assert!(
            rendered.contains("### Perfil do projeto (`kira ingest`)"),
            "{rendered}"
        );
        assert!(rendered.contains("### Árvore do repositório"), "{rendered}");
        assert!(
            pack.summary().contains("perfil do digest"),
            "{}",
            pack.summary()
        );
        let report = explain_report(&pack, 4821, false);
        assert!(report.contains("- perfil do digest:"), "{report}");
        let json = report_json(&pack, 4821).unwrap_or_else(|err| panic!("{err}"));
        assert!(json.contains("\"digest\""), "{json}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_absent_digest_changes_nothing() {
        let dir = temp_project("pack-no-digest");
        write_file(&dir, "src/a.rs", "let cooldown = 30;\n");
        let pack = pack_partial(
            &dir,
            None,
            None,
            &item_with("Cooldown expirou", "o cooldown da fila"),
            12_000,
            &[],
        );
        assert!(!pack.digest_used());
        assert!(
            pack.notes.iter().all(|note| !note.contains("digest")),
            "projeto sem ingestão não gera nota: {:?}",
            pack.notes
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_digest_is_a_note_not_an_error() {
        let dir = temp_project("pack-broken-digest");
        // Um diretório no caminho do digest: a leitura falha com erro que não é
        // NotFound — fail-open com nota, e o grep do repositório continua.
        fs::create_dir_all(dir.join(".kira").join("digest.md"))
            .unwrap_or_else(|err| panic!("mkdir: {err}"));
        write_file(&dir, "src/a.rs", "let cooldown = 30;\n");
        let pack = pack_partial(
            &dir,
            None,
            None,
            &item_with("Cooldown expirou", "o cooldown da fila"),
            12_000,
            &[],
        );
        assert!(!pack.digest_used());
        assert!(
            pack.notes
                .iter()
                .any(|note| note.contains(".kira/digest.md")),
            "a causa aparece: {:?}",
            pack.notes
        );
        assert!(!pack.code.is_empty(), "fail-open: o grep aconteceu");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn budget_drops_the_digest_with_a_note_and_keeps_the_excerpts() {
        let dir = temp_project("pack-digest-budget");
        write_file(&dir, "src/a.rs", "let cooldown = 30;\n");
        write_file(
            &dir,
            ".kira/digest.md",
            &synthetic_digest(200, &"y".repeat(4_000)),
        );
        // Orçamento pequeno: o digest (cortado no teto, ~1.5 k) não cabe, e o
        // corte vira nota — os trechos, mais baratos, ainda entram.
        let pack = pack_partial(
            &dir,
            None,
            None,
            &item_with("Cooldown expirou", "o cooldown da fila"),
            100,
            &[],
        );
        assert!(!pack.digest_used());
        assert!(
            pack.notes
                .iter()
                .any(|note| note.contains("perfil do digest ficou de fora")),
            "{:?}",
            pack.notes
        );
        assert!(!pack.code.is_empty(), "trechos cabem no resto do orçamento");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn severity_ranking_puts_critical_first() {
        assert!(severity_rank("Critical") < severity_rank("High"));
        assert!(severity_rank("high") < severity_rank("moderate"));
        assert!(severity_rank("Medium") < severity_rank("low"));
        assert_eq!(severity_rank("qualquer"), 4);
    }

    /// O `--file` do work item vira caminho relativo (para o grep pular o
    /// arquivo que já está inteiro no prompt).
    #[test]
    fn relative_to_root_normalizes_the_excluded_file() {
        let dir = temp_project("pr5-exclude");
        let item = dir.join("item.json");
        write_file(&dir, "item.json", "{}");
        assert_eq!(relative_to_root(&dir, &item), Some("item.json".to_string()));
        let outside = std::env::temp_dir().join("kira-fora-do-root.json");
        assert_eq!(relative_to_root(&dir, &outside), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
