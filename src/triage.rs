//! `kira triage` — read-only analysis of a work item by a local LLM.
//!
//! The system prompt and the prompt builder are pure functions so they are
//! unit-tested without any model. Triage never writes: the system prompt
//! forbids suggesting code changes and the command never touches Azure DevOps.

use std::path::Path;

use crate::board::BoardProvider;
use crate::error::Error;
use crate::format::DESCRIPTION_MAX_CHARS;
use crate::html::html_to_text;
use crate::llm::LlmBackend;
use crate::llm::LlmKind;
use crate::llm::copilot::CopilotBackend;
use crate::llm::ollama::OllamaBackend;
// Single source for the prompt contract: the system text, the section headers
// and the user preamble all live in `llm::prompts` (versioned, golden-tested).
use crate::llm::prompts::{TRIAGE_SECTIONS as SECTION_NAMES, TRIAGE_SYSTEM_PROMPT};
use crate::work_item::WorkItem;

/// Placeholder used when the model left a section empty.
const SECTION_EMPTY: &str = "—";

/// Builds the user prompt for a work item: title, type, state, area and the
/// description with the raw HTML cleaned by [`html_to_text`].
///
/// Without a [`ContextPack`] this is only the work item — the shape the model
/// answered before PR31.
pub fn build_prompt(item: &WorkItem) -> String {
    build_prompt_with_context(item, &crate::context::ContextPack::default())
}

/// Builds the user prompt and appends the retrieved project context (wiki
/// excerpts with path and age, plus the last `kira scan` brief).
///
/// The pack is already bounded by [`crate::context::budget_chars`]; this function
/// only decides where it sits in the prompt.
pub fn build_prompt_with_context(item: &WorkItem, pack: &crate::context::ContextPack) -> String {
    let mut prompt = String::new();
    // O preâmbulo e o prompt do sistema vivem juntos em `llm::prompts`
    // (versionados); aqui só a ordem das evidências.
    prompt.push_str(crate::llm::prompts::TRIAGE_USER_PREAMBLE);
    prompt.push_str(&format!("ID: {}\n", item.id));
    prompt.push_str(&format!("Tipo: {}\n", item.work_item_type));
    // Privacy boundary: the work item is author-controlled text and can carry a
    // pasted token, e-mail or CPF. Masked before it can reach the model.
    prompt.push_str(&format!("Título: {}\n", crate::redact::redact(&item.title)));
    prompt.push_str(&format!("Estado: {}\n", item.state));
    prompt.push_str(&format!(
        "Área: {}\n",
        item.area_path.as_deref().unwrap_or(SECTION_EMPTY)
    ));
    prompt.push_str(&format!(
        "Responsável: {}\n",
        item.assigned_to.as_deref().unwrap_or(SECTION_EMPTY)
    ));
    prompt.push_str("Descrição:\n");
    prompt.push_str(&description_for_prompt(item.description.as_deref()));
    // Retrieved context goes after the item: the model reads the bug first, then
    // the evidence. Excerpts are already redacted (they come from the wiki, which
    // masks on write) and already bounded by the packer.
    prompt.push_str(&pack.render());
    prompt
}

/// Cleans the raw HTML description and truncates it to
/// [`DESCRIPTION_MAX_CHARS`] characters; missing or empty yields
/// "(sem descrição)".
fn description_for_prompt(description: Option<&str>) -> String {
    let Some(description) = description else {
        return "(sem descrição)".to_string();
    };
    let plain = html_to_text(description);
    if plain.is_empty() {
        return "(sem descrição)".to_string();
    }
    let total_chars = plain.chars().count();
    let mut text: String = plain.chars().take(DESCRIPTION_MAX_CHARS).collect();
    if total_chars > DESCRIPTION_MAX_CHARS {
        text.push('…');
    }
    // Same privacy boundary as the title: descriptions routinely carry pasted
    // PATs, connection strings and customer data.
    crate::redact::redact(&text)
}

/// Formats the model answer into the fixed three-section template:
///
/// ```text
/// Causa raiz provável:
/// <model text for this section>
///
/// Arquivos prováveis:
/// <model text for this section>
///
/// Confiança:
/// <model text for this section>
/// ```
///
/// Sections the model did not produce show "—" — but only for an answer that
/// **did** carry at least one of the three headers. An answer with none of them
/// is not a filled template with holes: it is a model that ignored the format,
/// and rendering it as three `—` was the silent failure of W2-fix PR-F2. That
/// case returns [`Error::LlmFormat`] so the command fails loudly instead of
/// recording an empty triage (and an empty wiki episode).
pub fn format_triage(model_text: &str) -> Result<String, Error> {
    if section_headers_found(model_text) == 0 {
        return Err(Error::LlmFormat);
    }
    Ok(render_sections(model_text))
}

/// Number of distinct fixed headers the model answer actually carries.
///
/// Zero means the answer is out of format; one or two mean the template was
/// followed partially (a section that is missing is `—`, a section that is
/// empty is also `—` — both are data, not a format error).
fn section_headers_found(model_text: &str) -> usize {
    let mut found = [false; 3];
    for line in model_text.lines() {
        if let Some((index, _)) = section_header_index(line) {
            found[index] = true;
        }
    }
    found.iter().filter(|seen| **seen).count()
}

/// Renders the three fixed blocks from a model answer that already passed the
/// format check.
fn render_sections(model_text: &str) -> String {
    let sections = split_sections(model_text);
    let blocks: Vec<String> = SECTION_NAMES
        .iter()
        .zip(sections.iter())
        .map(|(name, lines)| {
            let content = lines.trim();
            let content = if content.is_empty() {
                SECTION_EMPTY
            } else {
                content
            };
            format!("{name}:\n{content}")
        })
        .collect();
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

/// Splits the model text into the three section bodies, in the fixed order.
fn split_sections(model_text: &str) -> [String; 3] {
    let mut sections: [String; 3] = [String::new(), String::new(), String::new()];
    let mut current: Option<usize> = None;
    for line in model_text.lines() {
        match section_header_index(line) {
            Some((index, remainder)) => {
                current = Some(index);
                if !remainder.is_empty() {
                    sections[index].push_str(&remainder);
                    sections[index].push('\n');
                }
            }
            None => {
                if let Some(index) = current {
                    sections[index].push_str(line.trim_end());
                    sections[index].push('\n');
                } else {
                    // No header seen yet: everything belongs to the first one.
                    sections[0].push_str(line.trim_end());
                    sections[0].push('\n');
                }
            }
        }
    }
    sections
}

/// Recognizes a section header line (tolerating markdown decorations like
/// `#`, `-`, `**` and a trailing `:`), returning its index plus any content
/// written on the same line.
fn section_header_index(line: &str) -> Option<(usize, String)> {
    let cleaned = line
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('-')
        .trim_start()
        .replace('*', "");
    for (index, name) in SECTION_NAMES.iter().enumerate() {
        if let Some(remainder) = cleaned.strip_prefix(name) {
            let remainder = remainder.trim_start_matches(':').trim();
            return Some((index, remainder.to_string()));
        }
    }
    None
}

/// Result of one triage run: the work item that was analyzed and the formatted
/// three-section answer.
///
/// The item id travels with the text so the caller can anchor the episodic
/// capture (PR30) without reading the work item twice. The [`ContextPack`]
/// travels too, so `kira triage` can print which evidence was sent (ADR-0015).
///
/// [`ContextPack`]: crate::context::ContextPack
#[derive(Debug, Clone, PartialEq)]
pub struct TriageReport {
    /// Id of the analyzed work item.
    pub item_id: i32,
    /// Formatted answer (`Causa raiz provável` / `Arquivos prováveis` /
    /// `Confiança`).
    pub output: String,
    /// Context that was retrieved and sent with the prompt.
    pub context: crate::context::ContextPack,
}

/// The three sections of an already formatted triage, trimmed — in the fixed
/// order (causa raiz, arquivos, confiança). `—` when a section was empty.
#[must_use]
pub fn sections_of(formatted: &str) -> [String; 3] {
    split_sections(formatted).map(|section| section.trim().to_string())
}

/// Runs triage for a work item read from a local JSON file — the offline path
/// that needs no PAT and no Azure DevOps access.
///
/// Kept with the Ollama default for existing callers; the backend is chosen
/// with [`triage_file_llm`] (ADR-0019: the costly backend is never implicit).
pub async fn triage_file(base_url: &str, model: &str, path: &Path) -> Result<TriageReport, Error> {
    triage_file_llm(LlmKind::Ollama, base_url, model, path).await
}

/// Same as [`triage_file`] with an explicit backend choice.
pub async fn triage_file_llm(
    kind: LlmKind,
    base_url: &str,
    model: &str,
    path: &Path,
) -> Result<TriageReport, Error> {
    let body = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })?;
    let item = WorkItem::from_api_json(&body)?;
    triage_item(kind, base_url, model, &item, Some(path)).await
}

/// Runs triage for a work item fetched live through the board provider
/// (read-only), on the default Ollama backend.
pub async fn triage_live(
    board: &dyn BoardProvider,
    id: i32,
    base_url: &str,
    model: &str,
) -> Result<TriageReport, Error> {
    triage_live_llm(LlmKind::Ollama, board, id, base_url, model).await
}

/// Same as [`triage_live`] with an explicit backend choice.
pub async fn triage_live_llm(
    kind: LlmKind,
    board: &dyn BoardProvider,
    id: i32,
    base_url: &str,
    model: &str,
) -> Result<TriageReport, Error> {
    let item = board.work_item(id).await?;
    triage_item(kind, base_url, model, &item, None).await
}

/// A backend built for one triage call: wiring only — which backend runs is
/// the caller's explicit choice ([`LlmKind`]), never decided here.
enum TriageBackend {
    Ollama(OllamaBackend),
    Copilot(CopilotBackend),
}

async fn triage_item(
    kind: LlmKind,
    base_url: &str,
    model: &str,
    item: &WorkItem,
    exclude_from_grep: Option<&Path>,
) -> Result<TriageReport, Error> {
    // LLM porque: hipótese de causa raiz é julgamento sobre texto — nenhuma CLI
    // nem parser responde "por que este bug acontece". Todo o resto do comando
    // (campos, HTML, fingerprint, store, grep do repositório) é determinístico
    // (ADR-0016).
    let backend = match kind {
        LlmKind::Ollama => TriageBackend::Ollama(OllamaBackend::new(base_url, model)),
        // The costly path resolves its own binary/auth here and FAILS LOUDLY
        // (CopilotMissing/CopilotAuth) — never a silent fallback to Ollama.
        LlmKind::Copilot => TriageBackend::Copilot(CopilotBackend::from_env()?),
    };
    let pack = context_pack(item, exclude_from_grep);
    let prompt = build_prompt_with_context(item, &pack);
    let text = match &backend {
        TriageBackend::Ollama(backend) => backend.complete(TRIAGE_SYSTEM_PROMPT, &prompt).await?,
        TriageBackend::Copilot(backend) => backend.complete(TRIAGE_SYSTEM_PROMPT, &prompt).await?,
    };
    Ok(TriageReport {
        item_id: item.id,
        output: format_triage(&text)?,
        context: pack,
    })
}

/// Retrieval for the prompt (PR31 + PR5).
///
/// Fail-open by design: without a project root, store or wiki the pack is empty
/// and `triage` behaves exactly as before — context is an improvement, never a
/// new way to fail.
fn context_pack(item: &WorkItem, exclude: Option<&Path>) -> crate::context::ContextPack {
    let Some(root) = crate::store::detect_project_root() else {
        return crate::context::ContextPack::default();
    };
    crate::context::pack_excluding(&root, item, exclude)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_FULL: &str = include_str!("../tests/fixtures/work_item_4821.json");

    fn parse(body: &str) -> WorkItem {
        WorkItem::from_api_json(body).unwrap_or_else(|err| panic!("fixture inválida: {err}"))
    }

    /// Formats a model answer that is known to be in format (test-only sugar).
    fn format(text: &str) -> String {
        format_triage(text).unwrap_or_else(|err| panic!("formato válido não pode falhar: {err}"))
    }

    #[test]
    fn build_prompt_contains_fields_and_cleaned_html() {
        let prompt = build_prompt(&parse(FIXTURE_FULL));
        assert!(prompt.contains("ID: 4821\n"), "{prompt}");
        assert!(prompt.contains("Tipo: Bug\n"), "{prompt}");
        assert!(
            prompt.contains("Título: Sessão expirada com token válido\n"),
            "{prompt}"
        );
        assert!(prompt.contains("Estado: Active\n"), "{prompt}");
        assert!(prompt.contains("Área: Kira\\Backend\n"), "{prompt}");
        assert!(prompt.contains("Responsável: Maria Souza\n"), "{prompt}");
        assert!(
            prompt.contains("Usuário relata sessão expirada após 5 minutos."),
            "{prompt}"
        );
        assert!(prompt.contains("Login & logout não resolvem."), "{prompt}");
        assert!(
            !prompt.contains('<'),
            "prompt não pode conter tag HTML: {prompt}"
        );
        assert!(
            !prompt.contains("&amp;"),
            "prompt não pode conter entidade crua: {prompt}"
        );
    }

    /// Privacy boundary: a description with a pasted PAT, e-mail and CPF must not
    /// reach the model verbatim (W1-PR2, AGENTS.md).
    #[test]
    fn build_prompt_masks_secrets_from_title_and_description() {
        let item = WorkItem {
            id: 4821,
            title: "Erro com token=AbC123XyZ789QqQ no header".to_string(),
            state: "Active".to_string(),
            work_item_type: "Bug".to_string(),
            assigned_to: Some("Maria Souza".to_string()),
            description: Some(
                "<div>Contato serjota@proton.me CPF 123.456.789-01 \
                 Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6</div>"
                    .to_string(),
            ),
            area_path: Some("Kira\\Backend".to_string()),
        };
        let prompt = build_prompt(&item);
        assert!(!prompt.contains("AbC123XyZ789QqQ"), "PAT vazou: {prompt}");
        assert!(
            !prompt.contains("serjota@proton.me"),
            "e-mail vazou: {prompt}"
        );
        assert!(!prompt.contains("123.456.789-01"), "CPF vazou: {prompt}");
        assert!(
            !prompt.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6"),
            "token vazou: {prompt}"
        );
        assert!(
            prompt.contains("Maria Souza"),
            "campo não secreto preservado: {prompt}"
        );
    }

    /// PR31: the retrieved context must reach the prompt, and the no-context path
    /// must stay byte-identical to what shipped before (no silent behaviour change).
    #[test]
    fn context_pack_is_appended_to_the_prompt() {
        let item = parse(FIXTURE_FULL);
        let without = build_prompt(&item);
        let pack = crate::context::ContextPack {
            sections: vec![crate::context::ContextSection {
                page: "bugs/4821".to_string(),
                path: ".kira/wiki/bugs/4821.md".to_string(),
                age_days: Some(3),
                excerpt: "token renovado sem rotacionar".to_string(),
            }],
            code: vec![crate::context::CodeExcerpt {
                path: "src/auth/session.rs".to_string(),
                line: 42,
                text: "let ttl = Duration::minutes(5);".to_string(),
                rank: None,
                priority: false,
            }],
            findings_brief: Some("high 1".to_string()),
            findings: vec!["high · Newtonsoft.Json 12.0.1 (src/App.csproj)".to_string()],
            used_chars: 120,
            budget_chars: 12_288,
            notes: Vec::new(),
            repo_source: crate::context::RepoSource::Grep,
        };
        let with = build_prompt_with_context(&item, &pack);
        assert!(
            !without.contains(".kira/wiki/bugs/4821.md"),
            "sem pack não há wiki"
        );
        assert!(with.contains(".kira/wiki/bugs/4821.md"), "{with}");
        assert!(with.contains("3 dia(s) atrás"), "{with}");
        assert!(with.contains("token renovado sem rotacionar"), "{with}");
        assert!(
            with.contains("findings do último `kira scan`: high 1"),
            "{with}"
        );
        // PR5: o trecho de repositório chega com caminho:linha auditável.
        assert!(with.contains("src/auth/session.rs:42 —"), "{with}");
        assert!(
            with.contains("achado: high · Newtonsoft.Json 12.0.1"),
            "{with}"
        );
        // O work item continua antes do contexto.
        let id_at = with.find("ID: 4821").unwrap_or(0);
        let ctx_at = with.find("Contexto do projeto").unwrap_or(usize::MAX);
        assert!(id_at < ctx_at, "work item primeiro, contexto depois");
    }

    #[test]
    fn build_prompt_handles_missing_optional_fields() {
        let item = WorkItem {
            id: 7,
            title: "t".to_string(),
            state: "New".to_string(),
            work_item_type: "Task".to_string(),
            assigned_to: None,
            description: None,
            area_path: None,
        };
        let prompt = build_prompt(&item);
        assert!(prompt.contains("Área: —\n"), "{prompt}");
        assert!(prompt.contains("Responsável: —\n"), "{prompt}");
        assert!(prompt.contains("Descrição:\n(sem descrição)"), "{prompt}");
    }

    #[test]
    fn format_triage_renders_the_three_fixed_sections() {
        let model_text = "Causa raiz provável:\nTTL do token muito curto.\nArquivos prováveis:\nsrc/auth/session.rs\nConfiança:\nmédia";
        assert_eq!(
            format(model_text),
            "Causa raiz provável:\nTTL do token muito curto.\n\n\
             Arquivos prováveis:\nsrc/auth/session.rs\n\n\
             Confiança:\nmédia\n"
        );
    }

    #[test]
    fn format_triage_tolerates_markdown_decorated_headers() {
        let model_text = "## Causa raiz provável\n_ttl_ curto\n**Arquivos prováveis:**\nsrc/x.rs\n- **Confiança:** alta";
        assert_eq!(
            format(model_text),
            "Causa raiz provável:\n_ttl_ curto\n\n\
             Arquivos prováveis:\nsrc/x.rs\n\n\
             Confiança:\nalta\n"
        );
    }

    /// W2-fix PR-F2: the real failure was a model answer with none of the three
    /// headers being rendered as three `—` with exit 0. Out of format is an
    /// actionable error, never an empty result.
    #[test]
    fn format_triage_without_any_header_is_an_actionable_error() {
        for model_text in [
            "texto solto do modelo",
            "",
            "   \n\n",
            "A causa raiz provável é o TTL curto do token.",
        ] {
            match format_triage(model_text) {
                Err(Error::LlmFormat) => {
                    let message = Error::LlmFormat.to_string();
                    assert!(
                        message.contains("fora do formato das 3 seções"),
                        "{message}"
                    );
                    assert!(message.contains("--model"), "{message}");
                    assert!(message.contains("ollama ps"), "{message}");
                }
                other => panic!("{model_text:?} deveria falhar com LlmFormat, veio {other:?}"),
            }
        }
    }

    /// The complement of the rule above: a partially followed template is data.
    /// A section the model omits (or leaves empty) shows `—`; only the absence
    /// of *all* headers is an error.
    #[test]
    fn format_triage_with_two_of_three_sections_placeholders_only_the_missing_one() {
        let out = format("Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs");
        assert!(out.contains("Causa raiz provável:\nTTL curto\n"), "{out}");
        assert!(out.contains("Arquivos prováveis:\nsrc/a.rs\n"), "{out}");
        assert!(out.contains("Confiança:\n—\n"), "{out}");

        let empty_third =
            format("Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs\nConfiança:\n");
        assert!(empty_third.contains("Confiança:\n—\n"), "{empty_third}");
        assert!(
            !empty_third.contains("Arquivos prováveis:\n—"),
            "{empty_third}"
        );
    }

    #[test]
    fn format_triage_shows_placeholder_for_empty_sections() {
        let out = format("Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n");
        assert!(out.contains("Causa raiz provável:\n—\n"), "{out}");
        assert!(out.contains("Arquivos prováveis:\n—\n"), "{out}");
        assert!(out.contains("Confiança:\n—\n"), "{out}");
    }

    /// The episodic capture (PR30) reads back exactly what was printed: the
    /// three sections, in the fixed order, trimmed.
    #[test]
    fn sections_of_returns_the_three_sections_in_order() {
        let formatted = format(
            "Causa raiz provável:\nTTL curto\nArquivos prováveis:\nsrc/a.rs, src/b.rs\nConfiança:\nalta",
        );
        let sections = sections_of(&formatted);
        assert_eq!(sections[0], "TTL curto");
        assert_eq!(sections[1], "src/a.rs, src/b.rs");
        assert_eq!(sections[2], "alta");

        let empty = sections_of(&format(
            "Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n",
        ));
        assert_eq!(empty, ["—", "—", "—"], "seções vazias voltam como —");
    }
}
