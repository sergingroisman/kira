use crate::html::html_to_text;
use crate::work_item::WorkItem;

/// Maximum number of characters shown for the description.
pub const DESCRIPTION_MAX_CHARS: usize = 400;

/// Formats a work item as a human-readable terminal summary:
///
/// ```text
/// #4821  Bug  Active  [Backend]
/// Título: Sessão expirada com token válido
/// Responsável: —
/// Descrição: (primeiras 400 chars, quebra de linha preservada, HTML removido)
/// ```
pub fn format_work_item(item: &WorkItem) -> String {
    let mut out = format!(
        "#{item_id}  {item_kind}  {item_state}",
        item_id = item.id,
        item_kind = item.work_item_type,
        item_state = item.state
    );
    if let Some(area) = &item.area_path {
        out.push_str(&format!("  [{area}]"));
    }
    out.push('\n');
    out.push_str(&format!("Título: {title}\n", title = item.title));
    out.push_str(&format!(
        "Responsável: {owner}\n",
        owner = item.assigned_to.as_deref().unwrap_or("—")
    ));
    out.push_str("Descrição: ");
    out.push_str(&description_text(item.description.as_deref()));
    out.push('\n');
    out
}

/// Cleans the raw HTML description and truncates it to
/// [`DESCRIPTION_MAX_CHARS`] characters; missing or empty yields "—".
fn description_text(description: Option<&str>) -> String {
    let Some(description) = description else {
        return "—".to_string();
    };
    let plain = html_to_text(description);
    if plain.is_empty() {
        return "—".to_string();
    }
    let total_chars = plain.chars().count();
    let mut text: String = plain.chars().take(DESCRIPTION_MAX_CHARS).collect();
    if total_chars > DESCRIPTION_MAX_CHARS {
        text.push('…');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work_item::WorkItem;

    const FIXTURE_FULL: &str = include_str!("../tests/fixtures/work_item_4821.json");
    const FIXTURE_MINIMAL: &str = include_str!("../tests/fixtures/work_item_minimal.json");

    fn parse(body: &str) -> WorkItem {
        WorkItem::from_api_json(body).unwrap_or_else(|err| panic!("fixture inválida: {err}"))
    }

    #[test]
    fn formats_header_and_fields() {
        let out = format_work_item(&parse(FIXTURE_FULL));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.first(), Some(&"#4821  Bug  Active  [Kira\\Backend]"));
        assert_eq!(
            lines.get(1),
            Some(&"Título: Sessão expirada com token válido")
        );
        assert_eq!(lines.get(2), Some(&"Responsável: Maria Souza"));
    }

    #[test]
    fn cleans_html_and_preserves_line_breaks() {
        let out = format_work_item(&parse(FIXTURE_FULL));
        let expected = "Usuário relata sessão expirada após 5 minutos.\nLogin & logout não resolvem.\nVer log anexo";
        assert!(out.trim_end().ends_with(expected), "saída: {out}");
        assert!(!out.contains('<'), "não pode sobrar tag HTML: {out}");
        assert!(
            !out.contains("&amp;"),
            "entidade deve ser decodificada: {out}"
        );
    }

    #[test]
    fn missing_optional_fields_use_placeholder() {
        let out = format_work_item(&parse(FIXTURE_MINIMAL));
        let expected =
            "#42  Task  New\nTítulo: Somente o essencial\nResponsável: —\nDescrição: —\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn truncates_description_to_400_chars() {
        let item = WorkItem {
            id: 1,
            title: "t".to_string(),
            state: "New".to_string(),
            work_item_type: "Task".to_string(),
            assigned_to: None,
            description: Some("a".repeat(500)),
            area_path: None,
        };
        let out = format_work_item(&item);
        let Some(line) = out.lines().nth(3) else {
            panic!("saída deve ter 4 linhas");
        };
        let content = line.strip_prefix("Descrição: ").unwrap_or(line);
        assert_eq!(content.chars().count(), DESCRIPTION_MAX_CHARS + 1); // 400 'a' + '…'
        assert!(content.ends_with('…'));
    }
}
