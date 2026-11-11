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

use std::path::Path;

use crate::store::Store;
use crate::wiki::Wiki;
use crate::work_item::WorkItem;

/// Characters reserved per context token. Conservative on purpose: Portuguese
/// prose runs ~3.5 chars/token, and under-filling the window is cheaper than a
/// truncated prompt.
const CHARS_PER_TOKEN: usize = 3;

/// Floor for the budget, so a silly `KIRA_NUM_CTX=16` still sends something.
const MIN_BUDGET_CHARS: usize = 1_500;

/// Hard cap for one excerpt, before the global budget is applied.
const MAX_EXCERPT_CHARS: usize = 600;

/// How many wiki pages the pack may carry.
const MAX_PAGES: usize = 3;

/// Shortest keyword worth searching (drops "de", "erro", "não").
const MIN_KEYWORD_CHARS: usize = 5;

/// One wiki excerpt carried into the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSection {
    /// Wiki page id (`bugs/4821`).
    pub page: String,
    /// Path shown to the model and to the user (`docs/wiki/bugs/4821.md`).
    pub path: String,
    /// Age in days taken from the page header, when parseable.
    pub age_days: Option<i64>,
    /// Bounded excerpt around the first keyword hit.
    pub excerpt: String,
}

/// What the prompt will carry beyond the work item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextPack {
    /// Wiki excerpts, most relevant first.
    pub sections: Vec<ContextSection>,
    /// `kira scan` brief, e.g. `high 2, moderate 1`.
    pub findings_brief: Option<String>,
    /// Characters the pack occupies in the prompt.
    pub used_chars: usize,
    /// The budget the pack was allowed to use.
    pub budget_chars: usize,
    /// Why something is missing (surfaced as a warning by the caller).
    pub notes: Vec<String>,
}

impl ContextPack {
    /// True when there is nothing to add to the prompt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.findings_brief.is_none()
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
        for section in &self.sections {
            let age = section.age_days.map_or_else(
                || "idade desconhecida".to_string(),
                |days| format!("{days} dia(s) atrás"),
            );
            out.push_str(&format!("\n### {path} ({age})\n", path = section.path));
            out.push_str(&section.excerpt);
            out.push('\n');
        }
        out
    }
}

/// Budget in characters derived from `KIRA_NUM_CTX` (tokens).
#[must_use]
pub fn budget_chars() -> usize {
    let tokens = usize::try_from(crate::config::num_ctx_from_env()).unwrap_or(4096);
    (tokens * CHARS_PER_TOKEN).max(MIN_BUDGET_CHARS)
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
/// two handles the caller already owns.
#[must_use]
pub fn pack_with(
    root: &Path,
    store: &Store,
    wiki: &Wiki,
    item: &WorkItem,
    budget_chars: usize,
) -> ContextPack {
    let mut pack = ContextPack {
        budget_chars,
        ..ContextPack::default()
    };

    if let Some(brief) = findings_brief(root) {
        let line = format!("- findings do último `kira scan`: {brief}\n");
        pack.used_chars += line.chars().count();
        pack.findings_brief = Some(brief);
    }

    let haystack = format!(
        "{} {}",
        item.title,
        item.description.as_deref().unwrap_or_default()
    );
    let terms = keywords(&haystack, 6);
    if terms.is_empty() {
        pack.notes
            .push("work item sem termos úteis para buscar na wiki".to_string());
        return pack;
    }

    // Retrieve: one FTS5 query per term, rank pages by how many terms they hit.
    let mut scored: Vec<(usize, String)> = Vec::new();
    for term in &terms {
        let hits = match wiki.search(store, term, MAX_PAGES * 2) {
            Ok(hits) => hits,
            Err(cause) => {
                pack.notes.push(format!("busca na wiki falhou ({cause})"));
                return pack;
            }
        };
        for hit in hits {
            match scored.iter_mut().find(|(_, page)| *page == hit.slug) {
                Some((score, _)) => *score += 1,
                None => scored.push((1, hit.slug)),
            }
        }
    }
    // Stable: more matches first, then page id, so the pack is reproducible.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    for (_, page) in scored.into_iter().take(MAX_PAGES) {
        let Ok(Some(content)) = wiki.read(&page) else {
            continue;
        };
        let excerpt = excerpt_around(&content, &terms, MAX_EXCERPT_CHARS);
        if excerpt.is_empty() {
            continue;
        }
        // Normalised to `/` on purpose: the model reads this path and the pack
        // must be identical across Windows, macOS and Linux. `\` in a prompt was
        // exactly the kind of difference that made the Windows CI fail.
        let path = wiki
            .page_path(&page)
            .map(|path| path.display().to_string().replace('\\', "/"))
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
    pack
}

/// Production entry point: opens what it needs and degrades to an empty pack.
#[must_use]
pub fn pack(root: &Path, item: &WorkItem) -> ContextPack {
    let budget = budget_chars();
    let mut pack = ContextPack {
        budget_chars: budget,
        ..ContextPack::default()
    };
    let store = match Store::open_default() {
        Ok(store) => store,
        Err(cause) => {
            pack.notes
                .push(format!("sem store para buscar na wiki ({cause})"));
            return pack;
        }
    };
    let wiki = match Wiki::open(root) {
        Ok(wiki) => wiki,
        Err(cause) => {
            pack.notes.push(format!("wiki indisponível ({cause})"));
            return pack;
        }
    };
    pack_with(root, &store, &wiki, item, budget)
}

/// `kira scan` brief for `root`, when a findings file exists.
fn findings_brief(root: &Path) -> Option<String> {
    let findings = crate::scan::read_findings_file(root).ok().flatten()?;
    if findings.is_empty() {
        return None;
    }
    Some(crate::scan::severity_brief(&findings))
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
                .ends_with("docs/wiki/bugs/4821-sessao-expirada.md"),
            "{}",
            pack.sections[0].path
        );
        assert_eq!(pack.sections[0].age_days, Some(0));
        let rendered = pack.render();
        assert!(rendered.contains("docs/wiki/bugs/4821-sessao-expirada.md"));
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
}
