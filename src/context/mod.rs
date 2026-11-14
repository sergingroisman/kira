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

pub mod dedup;
pub mod evidence;
pub mod gitignore;
pub mod ignore;
pub mod index;
mod repo;

pub use dedup::{DEDUP_MIN_CUT_PCT, omitted_marker};
pub use evidence::{EvidenceState, state_for_anchor};
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

/// How many skills the pack may carry (SK-3).
const MAX_SKILLS: usize = 3;

/// How many owner-knowledge excerpts the pack may carry (SK-3).
const MAX_KNOWLEDGE: usize = 3;

/// How many repository excerpts the pack may carry (PR5).
const MAX_CODE_EXCERPTS: usize = 8;

/// How many individual findings are spelled out in the prompt.
const MAX_FINDING_LINES: usize = 3;

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

/// One skill hit carried into the prompt (SK-3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextSkill {
    /// Skill name (`react`) — the anchor is `skill:<name>@<version>`.
    pub name: String,
    /// `bundled` | `user` (cadeia de custódia, igual ao `kira skills list`).
    pub origin: String,
    /// Semver from the frontmatter.
    pub version: String,
    /// Bounded excerpt around the first keyword hit.
    pub snippet: String,
}

impl ContextSkill {
    /// `skill:react@1.0.0 [bundled]` — âncora citável na conclusão. O skill
    /// bundled não tem caminho no disco: a âncora é o caminho dele no prompt.
    #[must_use]
    pub fn anchor(&self) -> String {
        format!("skill:{}@{} [{}]", self.name, self.version, self.origin)
    }
}

/// One owner-knowledge excerpt carried into the prompt (SK-3, índice do SK-2).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextKnowledge {
    /// File stem of the source markdown (`pbm`).
    pub id: String,
    /// Path shown to the model (`<store>/knowledge/pbm.md`), `/` normalized.
    pub path: String,
    /// Age in days from the file mtime (`None` when the file left the disk).
    pub age_days: Option<i64>,
    /// First markdown heading of the source document.
    pub title: String,
    /// Bounded excerpt around the first keyword hit.
    pub snippet: String,
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
    /// Skill hits, most relevant first (SK-3; serde default keeps old
    /// payloads valid).
    #[serde(default)]
    pub skills: Vec<ContextSkill>,
    /// Owner knowledge excerpts, most relevant first (SK-3).
    #[serde(default)]
    pub knowledge: Vec<ContextKnowledge>,
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
    /// PR38: chars de prompt que o dedup contíguo economizou na montagem dos
    /// trechos (medido na lista ANTES do teto — o teto corta por prioridade;
    /// o dedup não é teto). 0 = nada a deduplicar. O ledger de economia
    /// (`session_economy.dedup_chars_saved`, schema v14) registra este número.
    #[serde(default)]
    pub dedup_chars_saved: usize,
}

/// Limiar de **pack ocioso** (PR34, decisão aberta D9 — proposta default da
/// PR34 do plano TE): pack abaixo de 25% do teto é suspeita de furo de coleta
/// (índice ausente/vazio?). Const e não env: ajustável em PR, nunca em runtime.
pub const PACK_IDLE_MIN_PCT: u64 = 25;

impl ContextPack {
    /// True when there is nothing to add to the prompt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
            && self.code.is_empty()
            && self.findings_brief.is_none()
            && self.findings.is_empty()
            && self.skills.is_empty()
            && self.knowledge.is_empty()
    }

    /// Percentual do teto que o pack consumiu (inteiro, piso) — o número do
    /// aviso de pack ocioso e do ledger. Ordem inteira de propósito: sem
    /// float, o aviso é determinístico byte a byte.
    #[must_use]
    pub fn used_pct_of_budget(&self) -> u64 {
        if self.budget_chars == 0 {
            return 0;
        }
        u64::try_from(self.used_chars).unwrap_or(0) * 100
            / u64::try_from(self.budget_chars).unwrap_or(1)
    }

    /// Sinal de pack ocioso (PR34, D9): abaixo do limiar, o orçamento foi
    /// desperdiçado — provável furo de coleta (índice ausente/vazio?). A
    /// mensagem vai para o stderr do comando; o LEDGER registra a flag
    /// separadamente, nunca como chamada.
    #[must_use]
    pub fn idle_warning(&self) -> Option<String> {
        if self.used_pct_of_budget() >= PACK_IDLE_MIN_PCT {
            return None;
        }
        Some(format!(
            "aviso: pack ocioso ({}% do teto) — provável furo de coleta (índice ausente/vazio?)",
            self.used_pct_of_budget()
        ))
    }

    /// Markdown block to append to the user prompt; empty when the pack is empty.
    ///
    /// The section order is the budget priority (SK-3): brief → findings →
    /// skills → knowledge → wiki → repo — the same order the pack spent the
    /// budget in.
    #[must_use]
    pub fn render(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        // Header de 1 linha (PR34, onda TE): os números que a sessão vai
        // registrar no ledger de economia — derivados dos mesmos chars, então
        // mesmo estado ⇒ mesmo header (o gate M1 continua valendo).
        // Fonte: a proveniência dos trechos do repositório; sem trechos, `—`.
        let source = if self.code.is_empty() {
            "—"
        } else {
            self.repo_source.as_str()
        };
        let mut out = format!(
            "~{} tokens (chars {} / {}) dentro do teto {} (fonte: {source})\n",
            self.used_chars / CHARS_PER_TOKEN,
            self.used_chars,
            CHARS_PER_TOKEN,
            self.budget_chars
        );
        out.push_str(
            "Contexto do projeto (conhecimento já registrado — use como evidência e \
             cite o caminho quando concluir; idade indica se ainda vale):\n",
        );
        if let Some(brief) = &self.findings_brief {
            out.push_str(&format!("- findings do último `kira scan`: {brief}\n"));
        }
        for finding in &self.findings {
            out.push_str(&format!("- achado: {finding}\n"));
        }
        for skill in &self.skills {
            out.push_str(&format!("\n### {}\n{}\n", skill.anchor(), skill.snippet));
        }
        for knowledge in &self.knowledge {
            let age = knowledge.age_days.map_or_else(
                || "idade desconhecida".to_string(),
                |days| format!("{days} dia(s) atrás"),
            );
            out.push_str(&format!(
                "\n### {path} ({age})\n{title} — {snippet}\n",
                path = knowledge.path,
                title = knowledge.title,
                snippet = knowledge.snippet
            ));
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
                // PR36: cada trecho sai tagueado — (CONFIRMADA) = o arquivo
                // existe no repositório, (INFERIDA) = não pôde ser verificado.
                // O prompt v3 manda o modelo PRESERVAR a tag ao citar.
                // PR38: repetidas do dedup saem contadas no marcador — a
                // âncora continua sendo da primeira ocorrência.
                let marker = if excerpt.omitted > 0 {
                    dedup::omitted_marker(excerpt.omitted)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "{} — {} ({}){marker}\n",
                    excerpt.anchor(),
                    excerpt.text,
                    excerpt.evidence.as_str()
                ));
            }
        }
        out
    }

    /// One auditable line about what was sent — printed by `kira triage` so the
    /// user can see exactly which evidence the model had (ADR-0015). Sources
    /// listed in the pack priority (SK-3).
    #[must_use]
    pub fn summary(&self) -> String {
        let mut sources: Vec<String> = Vec::new();
        if self.findings_brief.is_some() {
            sources.push("findings do scan".to_string());
        }
        if !self.skills.is_empty() {
            sources.push(format!("{} skill(s)", self.skills.len()));
        }
        if !self.knowledge.is_empty() {
            sources.push(format!(
                "{} documento(s) de conhecimento",
                self.knowledge.len()
            ));
        }
        if !self.sections.is_empty() {
            sources.push(format!("{} página(s) da wiki", self.sections.len()));
        }
        if !self.code.is_empty() {
            sources.push(format!("{} trecho(s) de repositório", self.code.len()));
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
    /// The teto the pack was allowed (PR34: o `--json` exporta o mesmo
    /// par pack/teto que o header do pack e o ledger registram).
    pub budget_chars: usize,
    /// Where the repository excerpts came from (`index`/`grep`) — o mesmo
    /// valor do header do pack (PR34).
    pub repo_source: RepoSource,
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
    for skill in &pack.skills {
        out.push_str(&format!("- skill {}: {}\n", skill.anchor(), skill.snippet));
    }
    for knowledge in &pack.knowledge {
        let age = knowledge.age_days.map_or_else(
            || "idade desconhecida".to_string(),
            |days| format!("{days} dia(s) atrás"),
        );
        out.push_str(&format!(
            "- conhecimento {} ({age}): {}\n",
            knowledge.path, knowledge.snippet
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
            // PR38: o marcador contável aparece também na leitura humana —
            // o que o dev vê é o que o prompt carrega (mesmo colapso).
            let marker = if excerpt.omitted > 0 {
                dedup::omitted_marker(excerpt.omitted)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "  - {} — {} ({}){marker}\n",
                excerpt.anchor(),
                excerpt.text,
                excerpt.evidence.as_str()
            ));
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
        budget_chars: pack.budget_chars,
        repo_source: pack.repo_source,
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
    pack_partial(
        root,
        Some(store),
        Some(wiki),
        item,
        budget_chars,
        &[],
        crate::knowledge::dir().as_deref(),
    )
}

/// Builds the pack from whatever sources the caller managed to open (PR5).
///
/// Each source is independent on purpose: a broken wiki must not cost the
/// repository excerpts, and a missing store must not cost the findings. The
/// budget is spent in a **declared** priority order — findings brief, individual
/// findings, skills, owner knowledge, wiki pages, repository excerpts — so a
/// small `KIRA_NUM_CTX` always drops the least specific evidence first (SK-3).
///
/// `skip_paths` are repository-relative files the caller already sent whole (the
/// work item file): grepping them again would only duplicate the prompt.
/// `knowledge_dir` is `<store_dir>/knowledge/` — it only feeds the path/age of
/// the knowledge excerpts; retrieval itself just needs the store (SK-2 index).
#[must_use]
pub fn pack_partial(
    root: &Path,
    store: Option<&Store>,
    wiki: Option<&Wiki>,
    item: &WorkItem,
    budget_chars: usize,
    skip_paths: &[String],
    knowledge_dir: Option<&Path>,
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

    // 2. Skills (SK-3): cartas de instrução que casam com os termos. O orçamento
    //    gasta aqui antes da wiki porque a instrução vale para qualquer
    //    evidência que vier depois — sem ela, trecho nenhum é interpretável.
    if let Some(store) = store
        && let Ok(project_id) = crate::store::project_id_for_root(root)
    {
        match skills_of(store, &project_id, &terms) {
            Ok(hits) => {
                for skill in hits {
                    let anchor = skill.anchor();
                    let cost = skill.snippet.chars().count() + anchor.chars().count() + 16;
                    if pack.used_chars + cost > budget_chars {
                        pack.notes.push(format!(
                            "orçamento de contexto esgotado em {budget_chars} caracteres; skill '{}' ficou de fora",
                            skill.name
                        ));
                        break;
                    }
                    pack.used_chars += cost;
                    pack.skills.push(skill);
                }
            }
            Err(cause) => pack.notes.push(format!("busca de skills falhou ({cause})")),
        }

        // 3. Conhecimento do dono (SK-2): o índice `knowledge_fts` é derivado;
        //    caminho e idade vêm do disco — blob anônimo no prompt é proibido.
        match knowledge_of(knowledge_dir, store, &terms) {
            Ok(hits) => {
                for entry in hits {
                    let cost = entry.snippet.chars().count()
                        + entry.title.chars().count()
                        + entry.path.chars().count()
                        + 24;
                    if pack.used_chars + cost > budget_chars {
                        pack.notes.push(format!(
                            "orçamento de contexto esgotado em {budget_chars} caracteres; conhecimento '{}' ficou de fora",
                            entry.id
                        ));
                        break;
                    }
                    pack.used_chars += cost;
                    pack.knowledge.push(entry);
                }
            }
            Err(cause) => pack
                .notes
                .push(format!("busca de conhecimento falhou ({cause})")),
        }
    }

    // 4. Wiki pages (PR31): FTS5 first, one query per term, ranked by hits.
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

    // 5. Repository excerpts (PR5): the KIRA greps, the model interprets. The
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
    // PR36: a tag de evidência é calculada AQUI — o único ponto que conhece
    // a raiz. Trecho do grep ⇒ arquivo acabou de ser lido (CONFIRMADA);
    // trecho de índice velho (arquivo saiu do disco) ⇒ INFERIDA. Função pura
    // de (raiz, âncora): mesmo estado ⇒ mesma tag.
    for excerpt in &mut grep.excerpts {
        excerpt.evidence = state_for_anchor(root, &excerpt.path);
    }
    // PR38: dedup contíguo — primeira ocorrência fica, repetidas do MESMO
    // arquivo saem contadas no marcador. Depois da ordenação final e DEPOIS
    // da tag (a economia medida usa o custo real do trecho no prompt), antes
    // do orçamento: o pack que sobra é menor e o número vai para o ledger
    // (schema v14). Função pura da lista ⇒ mesmo estado ⇒ mesmo pack (M1).
    let deduped = dedup::same_file(&grep.excerpts);
    pack.dedup_chars_saved = deduped.chars_saved;
    for excerpt in deduped.excerpts {
        // A tag entra no prompt, então entra na conta do orçamento
        // (ledger honesto: o custo é o que o prompt carrega).
        let cost = dedup::cost_of_excerpt(&excerpt);
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
    let knowledge_dir = crate::knowledge::dir();
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
        // SK-3: skills e conhecimento também têm índice derivado (SK-1/SK-2);
        // o pack fresco exige índice fresco, e falha é nota, nunca erro do
        // triage.
        if let Err(cause) = crate::skills::reindex(root, store, &project_id) {
            source_notes.push(format!("índice de skills não atualizado ({cause})"));
        }
    }
    if let Ok(store) = &store
        && let Err(cause) = crate::knowledge::reindex(store)
    {
        source_notes.push(format!("índice de conhecimento não atualizado ({cause})"));
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
        knowledge_dir.as_deref(),
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

/// Skills that match the terms (SK-3): one query per term against the derived
/// index, score = number of matching terms, tie broken by name — the same
/// formula as the wiki, so the pack stays reproducible. Bundled and user
/// skills of the current project compete in the same ranking.
fn skills_of(
    store: &Store,
    project_id: &str,
    terms: &[String],
) -> Result<Vec<ContextSkill>, String> {
    let mut scored: Vec<(usize, ContextSkill)> = Vec::new();
    for term in terms {
        let hits = crate::skills::search(store, project_id, term, MAX_SKILLS * 2)
            .map_err(|err| err.to_string())?;
        for hit in hits {
            match scored.iter_mut().find(|(_, skill)| skill.name == hit.name) {
                Some((score, _)) => *score += 1,
                None => scored.push((
                    1,
                    ContextSkill {
                        name: hit.name,
                        origin: hit.origin,
                        version: hit.version,
                        snippet: hit.snippet,
                    },
                )),
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
    Ok(scored
        .into_iter()
        .take(MAX_SKILLS)
        .map(|(_, skill)| skill)
        .collect())
}

/// Owner knowledge that matches the terms (SK-3 over the SK-2 index): one
/// query per term against `knowledge_fts`, same scoring as the wiki. The
/// disk gives the path and the age (`None` when the file left the disk — the
/// index is derived, so a stale entry still shows, just without age).
/// Without a store dir the excerpt carries the `knowledge:<id>` anchor
/// instead of a path: an anonymous blob is forbidden (ADR-0015).
fn knowledge_of(
    knowledge_dir: Option<&Path>,
    store: &Store,
    terms: &[String],
) -> Result<Vec<ContextKnowledge>, String> {
    let mut scored: Vec<(usize, ContextKnowledge)> = Vec::new();
    for term in terms {
        let hits = crate::knowledge::search(store, term, MAX_KNOWLEDGE * 2)
            .map_err(|err| err.to_string())?;
        for hit in hits {
            match scored.iter_mut().find(|(_, entry)| entry.id == hit.id) {
                Some((score, _)) => *score += 1,
                None => {
                    // Normalised to `/` like every prompt path: the pack must
                    // be identical across Windows, macOS and Linux. The path
                    // comes from the disk only when the file still exists —
                    // pointing the model at a gone file is a lie; the anchor
                    // `knowledge:<id>` is what remains (the index is derived
                    // and the next pack refreshes it).
                    let (path, age_days) = knowledge_dir
                        .map(|dir| {
                            let file = dir.join(format!("{}.md", hit.id));
                            (
                                crate::store::display_path(&file).replace('\\', "/"),
                                age_days_from_mtime(&file),
                            )
                        })
                        .filter(|(_, age)| age.is_some())
                        .unwrap_or_else(|| (format!("knowledge:{}", hit.id), None));
                    scored.push((
                        1,
                        ContextKnowledge {
                            id: hit.id,
                            path,
                            age_days,
                            title: hit.title,
                            snippet: hit.snippet,
                        },
                    ));
                }
            }
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    Ok(scored
        .into_iter()
        .take(MAX_KNOWLEDGE)
        .map(|(_, entry)| entry)
        .collect())
}

/// Age in days from the file mtime (`None` = file gone or clock ahead —
/// never a panic, just an un-aged excerpt).
fn age_days_from_mtime(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let now = std::time::SystemTime::now();
    // mtime no futuro (relógio à frente) vale como idade 0, não como erro.
    let seconds = now.duration_since(modified).unwrap_or_default().as_secs();
    i64::try_from(seconds / 86_400).ok()
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

    /// Pack mínimo de teste: teto e uma fonte — o resto fica no default.
    /// (Nome distinto do `pack_with` público, que os outros testes usam.)
    fn ledger_pack(used_chars: usize, budget_chars: usize, code: usize) -> ContextPack {
        ContextPack {
            used_chars,
            budget_chars,
            code: (0..code)
                .map(|_| CodeExcerpt {
                    path: "src/a.rs".to_string(),
                    line: 1,
                    text: "x".to_string(),
                    rank: None,
                    priority: false,
                    evidence: EvidenceState::Confirmed,
                    omitted: 0,
                })
                .collect(),
            ..ContextPack::default()
        }
    }

    /// PR34: o header do pack é a MESMA conta que o ledger registra — chars,
    /// estimativa (chars/3) e teto, byte a byte determinístico no mesmo estado.
    #[test]
    fn render_has_the_economy_header_with_the_budget_and_the_source() {
        let pack = ledger_pack(612, 12_288, 1);
        let rendered = pack.render();
        assert!(
            rendered
                .starts_with("~204 tokens (chars 612 / 3) dentro do teto 12288 (fonte: grep)\n"),
            "header de economia ausente/diferente: {rendered}"
        );
        // Fonte = index quando os trechos vieram do FTS5.
        let indexed = ContextPack {
            repo_source: RepoSource::Index,
            ..ledger_pack(612, 12_288, 1)
        };
        assert!(
            indexed
                .render()
                .starts_with("~204 tokens (chars 612 / 3) dentro do teto 12288 (fonte: index)\n"),
            "fonte index no header: {}",
            indexed.render()
        );
        // Sem trechos de repositório, a fonte não existe — `—`, nunca mentira.
        // (Pack não vazio: um findings brief dá conteúdo sem code.)
        let no_code = ContextPack {
            findings_brief: Some("high 1".to_string()),
            ..ledger_pack(612, 12_288, 0)
        };
        assert!(
            no_code
                .render()
                .starts_with("~204 tokens (chars 612 / 3) dentro do teto 12288 (fonte: —)\n"),
            "fonte — sem trechos: {}",
            no_code.render()
        );
        // Pack vazio NÃO imprime header (não há economia para medir).
        assert_eq!(ContextPack::default().render(), String::new());
    }

    /// PR34 (D9): pack abaixo de 25% do teto é ocioso; no limiar ou acima,
    /// não. Mensagem determinística com o percentual inteiro (piso).
    #[test]
    fn idle_warning_flags_packs_below_the_quarter_threshold() {
        // 0% do teto — ocioso (furo de coleta total).
        let idle = ledger_pack(0, 12_288, 0);
        assert_eq!(
            idle.idle_warning(),
            Some(
                "aviso: pack ocioso (0% do teto) — provável furo de coleta (índice ausente/vazio?)"
                    .to_string()
            ),
            "pack vazio é ocioso"
        );
        // 24% — ocioso.
        let below = ledger_pack(2950, 12_288, 0); // 24.02% → piso 24
        assert_eq!(below.used_pct_of_budget(), 24, "piso inteiro");
        assert!(below.idle_warning().is_some(), "24% < 25%: ocioso");
        // 25% — NO limiar já não é ocioso (o desperdício aceitável).
        let edge = ledger_pack(3072, 12_288, 0); // exatamente 25%
        assert_eq!(edge.used_pct_of_budget(), 25);
        assert!(edge.idle_warning().is_none(), "25% não é ocioso");
        // 50% — longe do limiar.
        let healthy = ledger_pack(6144, 12_288, 0);
        assert!(healthy.idle_warning().is_none());
        // Teto zero não pode dividir por zero.
        assert_eq!(ledger_pack(0, 0, 0).used_pct_of_budget(), 0);
    }

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
            None,
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

    // -----------------------------------------------------------------------
    // PR36 — evidência em 3 estados: o packer marca cada trecho por arquivo.
    // -----------------------------------------------------------------------

    /// Aceite do PR36: âncora que existe no disco = CONFIRMADA, e a tag sai
    /// no render (o prompt v3 manda o modelo preservá-la).
    #[test]
    fn packer_tags_an_existing_anchor_as_confirmed() {
        let dir = temp_project("evidence-confirmed");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_file(
            &dir,
            "src/auth/session.rs",
            "pub fn check_token() -> bool {\n",
        );

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "o check_token falhou no refresh"),
            12_000,
        );
        assert_eq!(pack.repo_source, RepoSource::Grep, "{:?}", pack.notes);
        assert!(
            pack.code
                .iter()
                .all(|excerpt| excerpt.evidence == EvidenceState::Confirmed),
            "todo trecho do grep veio do disco: {:?}",
            pack.code
        );
        let rendered = pack.render();
        assert!(rendered.contains("(CONFIRMADA)"), "{rendered}");
        // Âncora intacta: a tag entra DEPOIS do texto, nunca no caminho:linha.
        assert!(
            rendered
                .contains("src/auth/session.rs:1 — pub fn check_token() -> bool { (CONFIRMADA)"),
            "{rendered}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Aceite do PR36: trecho de índice cujo arquivo saiu do disco =
    /// INFERIDA — o índice é derivado (ADR-0021) e a tag é honesta.
    #[test]
    fn packer_tags_a_stale_index_excerpt_as_inferred() {
        let dir = temp_project("evidence-inferred");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_file(
            &dir,
            "src/auth/session.rs",
            "pub fn check_token() -> bool {\n",
        );
        let project_id = crate::store::project_id_for_root(&dir)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        index::reindex(&dir, &store, &project_id).unwrap_or_else(|err| panic!("reindex: {err}"));
        // O disco manda: arquivo saiu, índice derivado ainda tem a linha.
        fs::remove_file(dir.join("src/auth/session.rs"))
            .unwrap_or_else(|err| panic!("remover: {err}"));

        let pack = pack_with(
            &dir,
            &store,
            &wiki,
            &item_with("Sessão expirada", "token inválido"),
            12_000,
        );
        assert_eq!(pack.repo_source, RepoSource::Index, "{:?}", pack.notes);
        assert!(
            pack.code
                .iter()
                .all(|excerpt| excerpt.evidence == EvidenceState::Inferred),
            "índice velho não é CONFIRMADA: {:?}",
            pack.code
        );
        assert!(pack.render().contains("(INFERIDA)"), "{}", pack.render());
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

    /// SK-3: com as seis fontes presentes, o prompt carrega na ordem
    /// contratada `brief → findings → skills → knowledge → wiki → repo` —
    /// cada trecho ancorado, conhecimento com caminho e idade, e o mesmo
    /// estado produz o mesmo pack.
    #[test]
    fn pack_order_contract_with_all_six_sources() {
        let dir = temp_project("sk3-order");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));

        write_findings(&dir, FINDINGS_HIGH);
        write_file(&dir, "src/auth/session.rs", "let token = session_token;\n");

        // Skill do usuário casando com os termos do work item (SK-1).
        write_file(
            &dir,
            ".kira/skills/sessao.md",
            "---\nname: sessao\ndescription: Use when token refresh expires. Do NOT use for cookies.\nlicense: MIT\nauthor: Ana\nversion: 1.0.0\n---\n\n- renove o token com rotação\n",
        );
        let project_id = crate::store::project_id_for_root(&dir)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        crate::skills::reindex(&dir, &store, &project_id)
            .unwrap_or_else(|err| panic!("reindex skills: {err}"));

        // Conhecimento do dono (SK-2): disco + índice derivado.
        let knowledge_dir = dir.join("knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap_or_else(|err| panic!("criar: {err}"));
        fs::write(
            knowledge_dir.join("token-refresh.md"),
            "# Token refresh\n- sessão expira: rotacionar o token\n",
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));
        crate::knowledge::reindex_from(&knowledge_dir, &store)
            .unwrap_or_else(|err| panic!("reindex knowledge: {err}"));

        // Wiki (PR31).
        wiki.write(
            "bugs/4821-sessao-expirada",
            "# Sessão expirada\ntoken renovado sem rotacionar; refresh inválido",
        )
        .unwrap_or_else(|err| panic!("{err}"));
        wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));

        let item = item_with("Sessão expirada", "token inválido no refresh");
        let pack = pack_partial(
            &dir,
            Some(&store),
            Some(&wiki),
            &item,
            12_000,
            &[],
            Some(&knowledge_dir),
        );

        assert_eq!(pack.findings_brief.as_deref(), Some("critical 1, high 1"));
        assert!(!pack.findings.is_empty(), "notas: {:?}", pack.notes);
        // Skills bundled ("token" em tailwind/shadcn) competem na mesma busca:
        // o do usuário vence por casar com mais termos (score = nº de termos).
        assert!(
            !pack.skills.is_empty()
                && pack.skills[0].name == "sessao"
                && pack.skills[0].origin == "user",
            "skills: {:?}",
            pack.skills
        );
        assert_eq!(pack.knowledge.len(), 1, "knowledge: {:?}", pack.knowledge);
        assert_eq!(pack.knowledge[0].id, "token-refresh");
        assert!(
            pack.knowledge[0]
                .path
                .ends_with("/knowledge/token-refresh.md"),
            "caminho ancorado: {}",
            pack.knowledge[0].path
        );
        assert_eq!(
            pack.knowledge[0].age_days,
            Some(0),
            "idade vem do mtime do disco"
        );
        assert_eq!(pack.sections.len(), 1, "wiki: {:?}", pack.sections);
        assert!(!pack.code.is_empty(), "repo: {:?}", pack.notes);

        let rendered = pack.render();
        let pos = |needle: &str| {
            rendered
                .find(needle)
                .unwrap_or_else(|| panic!("falta `{needle}` no render:\n{rendered}"))
        };
        let brief = pos("findings do último");
        let finding = pos("achado:");
        let skill = pos("skill:sessao@1.0.0 [user]");
        let knowledge = pos("knowledge/token-refresh.md");
        let wiki_pos = pos(".kira/wiki/bugs/4821-sessao-expirada.md");
        let repo = pos("### Trechos do repositório");
        assert!(
            brief < finding
                && finding < skill
                && skill < knowledge
                && knowledge < wiki_pos
                && wiki_pos < repo,
            "ordem contratada violada:\n{rendered}"
        );

        let summary = pack.summary();
        assert!(
            summary.contains(&format!("{} skill(s)", pack.skills.len()))
                && summary.contains("1 documento(s) de conhecimento")
                && summary.contains("1 página(s) da wiki")
                && summary.contains("3 trecho(s) de repositório"),
            "{summary}"
        );

        // Mesmo estado ⇒ mesmo pack (gate do M1), com as fontes novas dentro.
        let second = pack_partial(
            &dir,
            Some(&store),
            Some(&wiki),
            &item,
            12_000,
            &[],
            Some(&knowledge_dir),
        );
        assert_eq!(pack, second, "determinismo com skills/knowledge");
        let _ = fs::remove_dir_all(&dir);
    }

    /// SK-3: teto honrado — orçamento minúsculo derruba skills, conhecimento,
    /// wiki e repositório antes de apagar o brief, e explica o corte em nota.
    #[test]
    fn tiny_budget_drops_the_new_sources_first() {
        let dir = temp_project("sk3-budget");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        write_findings(&dir, FINDINGS_HIGH);
        write_file(
            &dir,
            ".kira/skills/sessao.md",
            "---\nname: sessao\ndescription: Use when token refresh expires. Do NOT use for cookies.\nlicense: MIT\nauthor: Ana\nversion: 1.0.0\n---\n\n- renove o token com rotação\n",
        );
        let project_id = crate::store::project_id_for_root(&dir)
            .unwrap_or_else(|err| panic!("project id: {err}"));
        crate::skills::reindex(&dir, &store, &project_id)
            .unwrap_or_else(|err| panic!("reindex skills: {err}"));
        let knowledge_dir = dir.join("knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap_or_else(|err| panic!("criar: {err}"));
        fs::write(
            knowledge_dir.join("token-refresh.md"),
            "# Token refresh\n- sessão expira: rotacionar o token\n",
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));
        crate::knowledge::reindex_from(&knowledge_dir, &store)
            .unwrap_or_else(|err| panic!("reindex knowledge: {err}"));
        wiki.write("bugs/4821-sessao-expirada", "# Sessão expirada\ntoken")
            .unwrap_or_else(|err| panic!("{err}"));
        wiki.reindex(&store).unwrap_or_else(|err| panic!("{err}"));

        let pack = pack_partial(
            &dir,
            Some(&store),
            Some(&wiki),
            &item_with("Sessão expirada", "token inválido no refresh"),
            130,
            &[],
            Some(&knowledge_dir),
        );
        assert!(pack.findings_brief.is_some(), "brief sobrevive sempre");
        assert!(
            pack.skills.is_empty() && pack.knowledge.is_empty() && pack.sections.is_empty(),
            "fontes caras não cabem: {:?} {:?} {:?}",
            pack.skills,
            pack.knowledge,
            pack.sections
        );
        assert!(
            pack.notes.iter().any(|note| note.contains("orçamento")),
            "corte explicado: {:?}",
            pack.notes
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// SK-3: o índice derivado pode ficar atrás do disco — arquivo removido
    /// ainda aparece do índice, mas sem idade (nunca inventa dado), e a saída
    /// usa a âncora em vez de apontar arquivo que não existe.
    #[test]
    fn knowledge_indexed_but_gone_from_disk_carries_no_age() {
        let dir = temp_project("sk3-stale");
        let store =
            Store::open_in_dir(&dir.join("store")).unwrap_or_else(|err| panic!("store: {err}"));
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let knowledge_dir = dir.join("knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap_or_else(|err| panic!("criar: {err}"));
        fs::write(
            knowledge_dir.join("token-refresh.md"),
            "# Token refresh\n- sessão expira: rotacionar o token\n",
        )
        .unwrap_or_else(|err| panic!("escrever: {err}"));
        crate::knowledge::reindex_from(&knowledge_dir, &store)
            .unwrap_or_else(|err| panic!("reindex knowledge: {err}"));
        // O disco manda: arquivo saiu, índice derivado ainda não foi refresco.
        fs::remove_file(knowledge_dir.join("token-refresh.md"))
            .unwrap_or_else(|err| panic!("remover: {err}"));

        let pack = pack_partial(
            &dir,
            Some(&store),
            Some(&wiki),
            &item_with("Sessão expirada", "token inválido no refresh"),
            12_000,
            &[],
            Some(&knowledge_dir),
        );
        assert_eq!(
            pack.knowledge.len(),
            1,
            "stale indexado não some: {:?}",
            pack.knowledge
        );
        assert_eq!(
            pack.knowledge[0].age_days, None,
            "idade inventada é proibido"
        );
        assert_eq!(
            pack.knowledge[0].path, "knowledge:token-refresh",
            "arquivo que não existe não recebe caminho: {:?}",
            pack.knowledge[0].path
        );
        let rendered = pack.render();
        assert!(
            rendered.contains("idade desconhecida"),
            "render honesto: {rendered}"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
