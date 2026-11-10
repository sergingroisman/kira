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
use crate::llm::ollama::OllamaBackend;
use crate::work_item::WorkItem;

/// Section names of the fixed triage template.
const SECTION_NAMES: [&str; 3] = ["Causa raiz provável", "Arquivos prováveis", "Confiança"];

/// Placeholder used when the model left a section empty.
const SECTION_EMPTY: &str = "—";

/// The read-only triage system prompt lives in [`crate::llm::prompts`] so it
/// is versioned in one place; this alias keeps the command code readable.
pub use crate::llm::prompts::TRIAGE_SYSTEM_PROMPT as SYSTEM_PROMPT;

/// Builds the user prompt for a work item: title, type, state, area and the
/// description with the raw HTML cleaned by [`html_to_text`].
pub fn build_prompt(item: &WorkItem) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "Analise o work item do Azure DevOps abaixo e responda no formato de três seções definido.\n\n",
    );
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
/// Sections the model did not produce show "—"; a model answer without any of
/// the three headers falls entirely under "Causa raiz provável:".
pub fn format_triage(model_text: &str) -> String {
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

/// Runs triage for a work item read from a local JSON file — the offline path
/// that needs no PAT and no Azure DevOps access.
pub async fn triage_file(base_url: &str, model: &str, path: &Path) -> Result<String, Error> {
    let body = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })?;
    let item = WorkItem::from_api_json(&body)?;
    triage_item(base_url, model, &item).await
}

/// Runs triage for a work item fetched live through the board provider
/// (read-only).
pub async fn triage_live(
    board: &dyn BoardProvider,
    id: i32,
    base_url: &str,
    model: &str,
) -> Result<String, Error> {
    let item = board.work_item(id).await?;
    triage_item(base_url, model, &item).await
}

async fn triage_item(base_url: &str, model: &str, item: &WorkItem) -> Result<String, Error> {
    // LLM porque: hipótese de causa raiz é julgamento sobre texto — nenhuma CLI
    // nem parser responde "por que este bug acontece". Todo o resto do comando
    // (campos, HTML, fingerprint, store) é determinístico (ADR-0016).
    let backend = OllamaBackend::new(base_url, model);
    let text = backend.complete(SYSTEM_PROMPT, &build_prompt(item)).await?;
    Ok(format_triage(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_FULL: &str = include_str!("../tests/fixtures/work_item_4821.json");

    fn parse(body: &str) -> WorkItem {
        WorkItem::from_api_json(body).unwrap_or_else(|err| panic!("fixture inválida: {err}"))
    }

    #[test]
    fn system_prompt_is_read_only_and_demands_three_sections() {
        assert!(
            SYSTEM_PROMPT.contains("nunca sugira escrever, editar ou corrigir código"),
            "prompt do sistema deve proibir escrita de código: {SYSTEM_PROMPT}"
        );
        assert!(
            SYSTEM_PROMPT.contains("nunca proponha ou execute alterações no Azure DevOps"),
            "prompt do sistema deve proibir escrita no Azure: {SYSTEM_PROMPT}"
        );
        for name in SECTION_NAMES {
            assert!(
                SYSTEM_PROMPT.contains(&format!("{name}:")),
                "prompt do sistema deve exigir a seção {name}: {SYSTEM_PROMPT}"
            );
        }
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
            format_triage(model_text),
            "Causa raiz provável:\nTTL do token muito curto.\n\n\
             Arquivos prováveis:\nsrc/auth/session.rs\n\n\
             Confiança:\nmédia\n"
        );
    }

    #[test]
    fn format_triage_tolerates_markdown_decorated_headers() {
        let model_text = "## Causa raiz provável\n_ttl_ curto\n**Arquivos prováveis:**\nsrc/x.rs\n- **Confiança:** alta";
        assert_eq!(
            format_triage(model_text),
            "Causa raiz provável:\n_ttl_ curto\n\n\
             Arquivos prováveis:\nsrc/x.rs\n\n\
             Confiança:\nalta\n"
        );
    }

    #[test]
    fn format_triage_without_headers_falls_back_to_the_first_section() {
        let out = format_triage("texto solto do modelo");
        assert_eq!(
            out,
            "Causa raiz provável:\ntexto solto do modelo\n\n\
             Arquivos prováveis:\n—\n\n\
             Confiança:\n—\n"
        );
    }

    #[test]
    fn format_triage_shows_placeholder_for_empty_sections() {
        let out = format_triage("Causa raiz provável:\n\nArquivos prováveis:\n\nConfiança:\n");
        assert!(out.contains("Causa raiz provável:\n—\n"), "{out}");
        assert!(out.contains("Arquivos prováveis:\n—\n"), "{out}");
        assert!(out.contains("Confiança:\n—\n"), "{out}");
    }
}
