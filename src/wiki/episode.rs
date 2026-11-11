//! Episodic capture after `kira triage` (ADR-0013, PR30).
//!
//! Every triage leaves an *episode* on `docs/wiki/bugs/<item-id>.md`: predicted
//! root cause, likely files, confidence and the last `kira scan` brief. Nothing
//! is asked to the user — this is the "capture the work silently" of ADR-0013
//! reduced to the minimum, and it is **deterministic** (the model already ran in
//! `triage`; here we only format what it answered — ADR-0016).
//!
//! Two rules shape this module:
//!
//! - **Committed vs staged**: a human-run `kira triage` writes to the committed
//!   wiki ([`EpisodeTarget::Committed`]); an unattended cycle must use
//!   [`EpisodeTarget::Staging`] (`.kira/wiki-staging/`), which only the future
//!   `kira loop` does — promotion stays a human command (PR32).
//! - **Decay**: episodes older than [`EPISODE_MAX_AGE_DAYS`] collapse into one
//!   `- <date> triage #<id>: <cause> (colapsado)` line, so the page keeps the
//!   knowledge diff and drops the prose.
//!
//! Everything here is pure formatting over text, except [`record`], which goes
//! through [`Wiki`] (redaction + path guard apply inside it).

use std::path::PathBuf;

use super::{Wiki, WikiError};

/// Episodes older than this are collapsed (ADR-0013: "decay de 30 dias").
pub const EPISODE_MAX_AGE_DAYS: i64 = 30;

/// Heading of one episode block.
const EPISODE_HEADING: &str = "## Episódio ";

/// Heading of the block that keeps the collapsed one-liners.
const COLLAPSED_HEADING: &str = "## Histórico colapsado";

/// Field label carrying the predicted root cause (also what survives the decay).
const CAUSE_LABEL: &str = "- Causa raiz prevista:";

/// Placeholder used when the model answered nothing for a section.
const EMPTY_FIELD: &str = "—";

/// One episode captured after a triage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// Work item id (also the page slug: `bugs/<id>`).
    pub item_id: i32,
    /// Predicted root cause (triage section 1).
    pub root_cause: String,
    /// Likely files (triage section 2).
    pub files: String,
    /// Confidence (triage section 3).
    pub confidence: String,
    /// `kira scan` brief (`high 2, moderate 1`), when a scan file exists.
    pub findings_brief: Option<String>,
}

/// Where the episode lands (ADR-0013).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeTarget {
    /// `docs/wiki/` — the committed wiki of a human-invoked command.
    Committed,
    /// `.kira/wiki-staging/` — the local, uncommitted area of an unattended
    /// cycle (`kira wiki promote` moves it, PR32).
    Staging,
}

/// Page id of an item's episode page (`bugs/<item-id>`).
#[must_use]
pub fn page_for(item_id: i32) -> String {
    format!("bugs/{item_id}")
}

impl Episode {
    /// Markdown block of this episode.
    ///
    /// `at` is an RFC 3339 UTC stamp; every field is flattened to one line so
    /// the decay can always reduce a block to a single bullet.
    #[must_use]
    pub fn render(&self, at: &str) -> String {
        let mut block = format!("{EPISODE_HEADING}{at} · item {}\n", self.item_id);
        block.push_str(&format!("{CAUSE_LABEL} {}\n", one_line(&self.root_cause)));
        block.push_str(&format!(
            "- Arquivos prováveis: {}\n",
            one_line(&self.files)
        ));
        block.push_str(&format!("- Confiança: {}\n", one_line(&self.confidence)));
        if let Some(brief) = self
            .findings_brief
            .as_deref()
            .map(str::trim)
            .filter(|brief| !brief.is_empty() && *brief != "nenhum")
        {
            block.push_str(&format!("- Achados do scan: {}\n", one_line(brief)));
        }
        block
    }
}

/// Appends the episode to `docs/wiki/bugs/<id>.md` (or to the staging area),
/// collapsing stale episodes of that page first. Returns the written file.
pub fn record(
    wiki: &Wiki,
    episode: &Episode,
    target: EpisodeTarget,
    now: &str,
) -> Result<PathBuf, WikiError> {
    let page = page_for(episode.item_id);
    match target {
        EpisodeTarget::Staging => {
            // Staging files are named after the page (slashes are not allowed in
            // a flat staging directory) so a promotion can map them back.
            wiki.stage(&page.replace('/', "-"), &episode.render(now))
        }
        EpisodeTarget::Committed => {
            let existing = wiki.read(&page)?.unwrap_or_default();
            let without_header = strip_page_header(&existing);
            let (collapsed, decayed) =
                collapse_old_episodes(without_header, now, EPISODE_MAX_AGE_DAYS);
            let rendered = episode.render(now);
            // Um triage repetido no mesmo estado não vira diff: as linhas de
            // campo do bloco (sem o carimbo) já estão na página. O decay, se
            // houve, ainda assim é gravado — encolher a página é trabalho.
            let fields = rendered.lines().skip(1).collect::<Vec<_>>().join("\n");
            let is_new = fields.trim().is_empty() || !collapsed.contains(fields.trim_end());
            if !is_new && decayed == 0 {
                return wiki.page_path(&page);
            }
            let mut body = if collapsed.trim().is_empty() {
                // Título da página nasce com o binário: o índice mostra
                // "Bug 4821" em vez do slug, e a primeira linha é estável.
                format!("# Bug {}", episode.item_id)
            } else {
                collapsed.trim_end().to_string()
            };
            if is_new {
                body.push_str("\n\n");
                body.push_str(&rendered);
            }
            // `Wiki::write` re-adds the `updated_at` header and masks secrets.
            wiki.write(&page, &body)
        }
    }
}

/// Collapses every episode block older than `max_age_days` into one summary
/// line, keeping the predicted cause and dropping the prose.
///
/// Returns the new content and how many blocks were collapsed. Idempotent: a
/// collapsed line is not an episode block, so a second pass changes nothing.
/// An unparsable `now` (or a negative window) means "no decay", never "collapse
/// everything".
#[must_use]
pub fn collapse_old_episodes(content: &str, now: &str, max_age_days: i64) -> (String, usize) {
    let Some(now_seconds) = crate::store::rfc3339_utc_to_unix_seconds(now) else {
        return (content.to_string(), 0);
    };
    if max_age_days < 0 {
        return (content.to_string(), 0);
    }
    let cutoff = now_seconds - max_age_days * 86_400;

    let lines: Vec<&str> = content.lines().collect();
    let mut keep = vec![true; lines.len()];
    let mut collapsed: Vec<String> = Vec::new();
    for block in episode_blocks(&lines) {
        let Some(at_seconds) = crate::store::rfc3339_utc_to_unix_seconds(&block.at) else {
            // Sem carimbo legível não há como medir idade: preserva o bloco.
            continue;
        };
        if at_seconds >= cutoff {
            continue;
        }
        collapsed.push(format!(
            "- {} triage #{}: {} (colapsado)",
            date_of(&block.at),
            block.item,
            if block.cause.is_empty() {
                EMPTY_FIELD
            } else {
                block.cause.as_str()
            }
        ));
        for flag in keep.iter_mut().take(block.end).skip(block.start) {
            *flag = false;
        }
    }
    if collapsed.is_empty() {
        return (content.to_string(), 0);
    }
    collapsed.sort();
    collapsed.dedup();

    let mut out: Vec<String> = Vec::with_capacity(lines.len() + collapsed.len() + 2);
    let mut inserted = false;
    for (index, line) in lines.iter().enumerate() {
        if !keep[index] {
            continue;
        }
        out.push((*line).to_string());
        if !inserted && line.trim() == COLLAPSED_HEADING {
            out.extend(collapsed.iter().cloned());
            inserted = true;
        }
    }
    if !inserted {
        while out.last().is_some_and(|line| line.trim().is_empty()) {
            out.pop();
        }
        out.push(String::new());
        out.push(COLLAPSED_HEADING.to_string());
        out.extend(collapsed.iter().cloned());
    }
    let mut text = out.join("\n");
    text.push('\n');
    (text, collapsed.len())
}

/// One parsed episode block.
struct EpisodeBlock {
    /// First line index (the heading).
    start: usize,
    /// One past the last line index of the block.
    end: usize,
    /// RFC 3339 stamp from the heading, when parseable.
    at: String,
    /// Item id from the heading (`—` when missing).
    item: String,
    /// Predicted root cause, flattened to one line.
    cause: String,
}

/// Splits the content into episode blocks: a block starts at an episode heading
/// and ends at the next heading (`#`/`##`) or at the end of the page.
fn episode_blocks(lines: &[&str]) -> Vec<EpisodeBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(head) = lines[index].trim().strip_prefix(EPISODE_HEADING) else {
            index += 1;
            continue;
        };
        let (at, item) = heading_parts(head);
        let start = index;
        index += 1;
        let mut cause = String::new();
        while index < lines.len() {
            let line = lines[index].trim();
            if line.starts_with(EPISODE_HEADING)
                || line.starts_with("## ")
                || line.starts_with("# ")
            {
                break;
            }
            if cause.is_empty()
                && let Some(value) = line.strip_prefix(CAUSE_LABEL)
            {
                cause = one_line(value);
            }
            index += 1;
        }
        blocks.push(EpisodeBlock {
            start,
            end: index,
            at,
            item,
            cause,
        });
    }
    blocks
}

/// `2026-11-10T22:18:00Z · item 4821` → (`2026-11-10T22:18:00Z`, `4821`).
fn heading_parts(head: &str) -> (String, String) {
    let mut parts = head.split('·');
    let at = parts.next().unwrap_or_default().trim().to_string();
    let item = parts
        .next()
        .unwrap_or_default()
        .trim()
        .strip_prefix("item ")
        .unwrap_or(EMPTY_FIELD)
        .trim()
        .to_string();
    (at, item)
}

/// `YYYY-MM-DD` part of a stamp (the whole string when it is shorter/odd).
fn date_of(at: &str) -> &str {
    at.get(..10).unwrap_or(at)
}

/// Flattens any text to a single trimmed line, so a block can always be reduced
/// to one bullet (and a bullet can never break the markdown structure).
fn one_line(text: &str) -> String {
    let flattened: String = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let flattened = flattened.replace(['*', '`'], "");
    if flattened.is_empty() {
        EMPTY_FIELD.to_string()
    } else {
        flattened
    }
}

/// Removes the `<!-- kira-wiki ... -->` header line a page may already carry, so
/// a rewrite never nests two headers.
pub(crate) fn strip_page_header(content: &str) -> &str {
    let mut rest = content.trim_start();
    if rest.starts_with("<!-- kira-wiki")
        && let Some(end) = rest.find("-->")
    {
        rest = rest[end + "-->".len()..].trim_start_matches('\n');
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kira-episode-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("criar {dir:?}: {err}"));
        dir
    }

    fn episode(cause: &str) -> Episode {
        Episode {
            item_id: 4821,
            root_cause: cause.to_string(),
            files: "src/auth/session.rs".to_string(),
            confidence: "média".to_string(),
            findings_brief: Some("high 2, moderate 1".to_string()),
        }
    }

    #[test]
    fn render_writes_one_line_per_field_under_the_episode_heading() {
        let block = episode("TTL do token\nem cache").render("2026-11-10T22:18:00Z");
        assert!(
            block.starts_with("## Episódio 2026-11-10T22:18:00Z · item 4821\n"),
            "{block}"
        );
        assert!(
            block.contains("- Causa raiz prevista: TTL do token / em cache\n"),
            "campo deve virar uma linha só: {block}"
        );
        assert!(
            block.contains("- Arquivos prováveis: src/auth/session.rs"),
            "{block}"
        );
        assert!(block.contains("- Confiança: média"), "{block}");
        assert!(
            block.contains("- Achados do scan: high 2, moderate 1"),
            "{block}"
        );
    }

    #[test]
    fn empty_sections_become_a_placeholder_instead_of_a_blank_line() {
        let mut item = episode("");
        item.findings_brief = Some("nenhum".to_string());
        let block = item.render("2026-11-10T22:18:00Z");
        assert!(block.contains("- Causa raiz prevista: —"), "{block}");
        assert!(
            !block.contains("Achados do scan"),
            "\"nenhum\" não é achado: {block}"
        );
    }

    #[test]
    fn decay_collapses_only_episodes_older_than_the_window() {
        let old = episode("TTL curto").render("2026-09-01T10:00:00Z");
        let recent = episode("cache sem invalidação").render("2026-11-09T10:00:00Z");
        let content = format!("# Bug 4821\n\n{old}\n{recent}");

        let (collapsed, count) = collapse_old_episodes(&content, "2026-11-10T22:18:00Z", 30);
        assert_eq!(count, 1, "{collapsed}");
        assert!(
            !collapsed.contains("2026-09-01T10:00:00Z"),
            "bloco antigo deve sumir: {collapsed}"
        );
        assert!(
            collapsed.contains("- 2026-09-01 triage #4821: TTL curto (colapsado)"),
            "{collapsed}"
        );
        assert!(
            collapsed.contains("- Causa raiz prevista: cache sem invalidação"),
            "episódio recente fica inteiro: {collapsed}"
        );
        assert!(collapsed.contains(COLLAPSED_HEADING), "{collapsed}");

        // Idempotente: nada mais a colapsar na segunda passada.
        let (again, count) = collapse_old_episodes(&collapsed, "2026-11-10T22:18:00Z", 30);
        assert_eq!(count, 0);
        assert_eq!(again, collapsed);
    }

    #[test]
    fn decay_appends_into_the_existing_collapsed_section() {
        let old = episode("primeira causa").render("2026-08-01T10:00:00Z");
        let newer_old = episode("segunda causa").render("2026-08-15T10:00:00Z");
        let content = format!(
            "# Bug 4821\n\n{old}\n{COLLAPSED_HEADING}\n- 2026-07-01 triage #4821: antiga (colapsado)\n"
        );
        let with_two = format!("{content}\n{newer_old}");
        let (collapsed, count) = collapse_old_episodes(&with_two, "2026-11-10T22:18:00Z", 30);
        assert_eq!(count, 2, "{collapsed}");
        assert_eq!(
            collapsed.matches(COLLAPSED_HEADING).count(),
            1,
            "não pode criar uma segunda seção de colapsados: {collapsed}"
        );
        assert!(collapsed.contains("- 2026-07-01 triage #4821: antiga (colapsado)"));
        assert!(collapsed.contains("- 2026-08-01 triage #4821: primeira causa (colapsado)"));
        assert!(collapsed.contains("- 2026-08-15 triage #4821: segunda causa (colapsado)"));
    }

    #[test]
    fn decay_does_nothing_without_a_readable_clock_or_stamp() {
        let old = episode("causa").render("2026-09-01T10:00:00Z");
        let (kept, count) = collapse_old_episodes(&old, "ontem", 30);
        assert_eq!(count, 0);
        assert_eq!(kept, old, "sem relógio confiável o conhecimento fica");

        let undated = format!(
            "# Bug\n\n{EPISODE_HEADING}sem carimbo · item 4821\n- Causa raiz prevista: x\n"
        );
        let (kept, count) = collapse_old_episodes(&undated, "2026-11-10T22:18:00Z", 30);
        assert_eq!(count, 0);
        assert_eq!(kept, undated);
    }

    #[test]
    fn record_writes_the_committed_page_masked_and_without_a_second_header() {
        let dir = temp_project("record");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));

        let first = record(
            &wiki,
            &episode("token=AbC123XyZ789QqQ vazado no log"),
            EpisodeTarget::Committed,
            "2026-11-10T22:18:00Z",
        )
        .unwrap_or_else(|err| panic!("record 1: {err}"));
        assert!(first.ends_with("bugs/4821.md"), "{first:?}");

        let second = record(
            &wiki,
            &episode("segunda hipótese"),
            EpisodeTarget::Committed,
            "2026-11-10T23:00:00Z",
        )
        .unwrap_or_else(|err| panic!("record 2: {err}"));
        assert_eq!(first, second);

        let content = fs::read_to_string(&second).unwrap_or_else(|err| panic!("ler: {err}"));
        assert_eq!(
            content.matches("<!-- kira-wiki").count(),
            1,
            "um único cabeçalho por página: {content}"
        );
        assert_eq!(
            content.matches("## Episódio ").count(),
            2,
            "dois episódios no mesmo bug: {content}"
        );
        assert!(
            content.contains("## Episódio 2026-11-10T23:00:00Z · item 4821"),
            "carimbo do último episódio: {content}"
        );
        assert!(
            content.contains("updated_at="),
            "cabeçalho com carimbo da escrita: {content}"
        );
        assert!(
            !content.contains("AbC123XyZ789QqQ"),
            "segredo foi para o disco: {content}"
        );
        assert!(content.contains("[REDACTED]"), "{content}");
        assert!(
            content.starts_with("<!-- kira-wiki"),
            "cabeçalho na primeira linha: {content}"
        );
        assert!(
            content.contains("# Bug 4821"),
            "página nova nasce com título (o índice mostra isso): {content}"
        );

        // Repetir o mesmo triage não cria outro bloco (anti-ruído).
        record(
            &wiki,
            &episode("segunda hipótese"),
            EpisodeTarget::Committed,
            "2026-11-11T00:30:00Z",
        )
        .unwrap_or_else(|err| panic!("record repetido: {err}"));
        let after = fs::read_to_string(&second).unwrap_or_else(|err| panic!("ler: {err}"));
        assert_eq!(
            after.matches("## Episódio ").count(),
            2,
            "episódio idêntico não duplica: {after}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn decay_is_written_even_when_the_episode_itself_is_a_repeat() {
        let dir = temp_project("decay-repeat");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = record(
            &wiki,
            &Episode {
                item_id: 4821,
                root_cause: "hipótese antiga".to_string(),
                files: "src/legacy.rs".to_string(),
                confidence: "baixa".to_string(),
                findings_brief: None,
            },
            EpisodeTarget::Committed,
            "2026-09-01T10:00:00Z",
        )
        .unwrap_or_else(|err| panic!("record antigo: {err}"));
        record(
            &wiki,
            &episode("TTL curto"),
            EpisodeTarget::Committed,
            "2026-11-09T10:00:00Z",
        )
        .unwrap_or_else(|err| panic!("record recente: {err}"));

        // Terceiro triage: mesmo conteúdo do segundo (dedupe) e o primeiro
        // episódio já passou dos 30 dias — o decay ainda tem que ser gravado.
        record(
            &wiki,
            &episode("TTL curto"),
            EpisodeTarget::Committed,
            "2026-11-10T22:18:00Z",
        )
        .unwrap_or_else(|err| panic!("record repetido: {err}"));

        let content = fs::read_to_string(&path).unwrap_or_else(|err| panic!("ler: {err}"));
        assert!(
            content.contains("- 2026-09-01 triage #4821: hipótese antiga (colapsado)"),
            "decay descartado: {content}"
        );
        assert_eq!(
            content.matches("## Episódio ").count(),
            1,
            "só o episódio recente fica inteiro: {content}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn staging_target_never_touches_the_committed_wiki() {
        let dir = temp_project("staging");
        let wiki = Wiki::open(&dir).unwrap_or_else(|err| panic!("wiki: {err}"));
        let path = record(
            &wiki,
            &episode("hipótese do ciclo autônomo"),
            EpisodeTarget::Staging,
            "2026-11-10T22:18:00Z",
        )
        .unwrap_or_else(|err| panic!("record staging: {err}"));
        assert!(path.ends_with("bugs-4821.md"), "{path:?}");
        assert!(path.to_string_lossy().contains("wiki-staging"), "{path:?}");
        assert!(
            !dir.join(super::super::WIKI_DIR).exists(),
            "ADR-0013: ciclo autônomo nunca escreve em docs/wiki"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
