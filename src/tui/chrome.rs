//! Chrome do dashboard: linha de status e abas de fonte.
//!
//! As duas linhas fixas acima da área de trabalho (casca do statui: contexto
//! sempre visível). São funções PURAS sobre o snapshot — a MESMA informação
//! aparece no modo degradado e no `--snapshot` de texto.
//!
//! Honestidade: `—` com causa curta aqui; a causa INTEGRAL vive no `?`
//! (`state::pending_causes`). O PAT só aparece mascarado (nunca o valor).

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::state::{self, Snapshot};
use crate::tui::theme::Palette;

/// Rótulos das cinco fontes, na ordem do ciclo de `Tab`/`1–5`.
pub const SCREEN_LABELS: [&str; 5] = ["Resumo", "Achados", "Sessões", "Wiki", "Posts"];

/// Linha de status: projeto, dev, PAT mascarado, backend e idade do scan.
pub fn status_line(snapshot: &Snapshot) -> String {
    let project = snapshot
        .project
        .as_ref()
        .map(|project| project.name.as_str())
        .unwrap_or("—");
    let dev = snapshot.dev_name.as_deref().unwrap_or("—");
    let pat = snapshot.pat_masked.as_deref().unwrap_or("—");
    let scan = snapshot
        .last_scan_age_secs
        .map(state::format_age_secs)
        .unwrap_or_else(|| "—".to_string());
    format!("kira · {project} · dev {dev} · pat {pat} · llm ollama   ·   scan {scan}")
}

/// Texto de uma aba, com a contagem real (nunca estimada).
fn tab_text(snapshot: &Snapshot, index: usize) -> String {
    match index {
        1 => match snapshot.findings {
            Some(count) => format!("2 Achados {count}"),
            None => "2 Achados —".to_string(),
        },
        2 => format!("3 Sessões {}", snapshot.session_rows.len()),
        3 => format!("4 Wiki {}", snapshot.wiki_rows.len()),
        4 => format!("5 Posts {}", snapshot.post_rows.len()),
        _ => "1 Resumo".to_string(),
    }
}

/// Linha de abas: a ativa fica entre colchetes (não depende de cor).
pub fn tabs_line(snapshot: &Snapshot, active: usize, palette: Palette) -> Line<'static> {
    let mut spans = Vec::with_capacity(SCREEN_LABELS.len() * 2);
    for index in 0..SCREEN_LABELS.len() {
        let text = tab_text(snapshot, index);
        let is_active = index == active;
        let label = if is_active {
            format!("[{text}]")
        } else {
            format!(" {text} ")
        };
        let style = if is_active {
            Style::default()
                .fg(palette.title)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette.footer)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

/// Texto puro das abas (modo degradado, sem cor).
pub fn tabs_text(snapshot: &Snapshot, active: usize) -> String {
    (0..SCREEN_LABELS.len())
        .map(|index| {
            let text = tab_text(snapshot, index);
            if index == active {
                format!("[{text}]")
            } else {
                format!(" {text} ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Project;
    use crate::tui::state::SessionLine;
    use crate::tui::theme;

    fn snapshot_with_pat() -> Snapshot {
        Snapshot {
            project: Some(Project {
                id: "abc".to_string(),
                root_path: "C:\\proj".to_string(),
                name: "proj".to_string(),
                first_seen_at: "2026-11-11T09:00:00Z".to_string(),
            }),
            dev_name: Some("Sérgio".to_string()),
            pat_masked: Some("Aa2x****".to_string()),
            last_scan_age_secs: Some(720),
            findings: Some(23),
            session_rows: vec![SessionLine {
                id: 1,
                command: "scan".to_string(),
                args: String::new(),
                model: None,
                started_unix: None,
                started_at: String::new(),
                local_started_at: None,
                duration_ms: 10,
                ok: true,
                summary_first_line: String::new(),
                summary: String::new(),
            }],
            ..Snapshot::default()
        }
    }

    #[test]
    fn status_line_never_exposes_a_full_pat() {
        let text = status_line(&snapshot_with_pat());
        assert!(text.contains("pat Aa2x****"), "{text}");
        assert!(text.contains("dev Sérgio"), "{text}");
        assert!(text.contains("scan há 12 min"), "{text}");
        assert!(!text.contains("Aa2x9g7"), "nunca o PAT inteiro: {text}");
    }

    #[test]
    fn status_line_shows_dash_when_data_is_absent() {
        let text = status_line(&Snapshot::default());
        assert!(text.contains("dev —"), "{text}");
        assert!(text.contains("pat —"), "{text}");
        assert!(text.contains("scan —"), "{text}");
    }

    #[test]
    fn tabs_mark_the_active_screen_with_brackets_and_real_counts() {
        let snapshot = snapshot_with_pat();
        let text = tabs_text(&snapshot, 1);
        assert!(text.contains("[2 Achados 23]"), "{text}");
        assert!(text.contains("3 Sessões 1"), "{text}");
        assert!(text.contains("4 Wiki 0"), "{text}");
        assert!(text.contains("1 Resumo"), "{text}");
        // A cor não é o único sinal de foco.
        let line = tabs_line(&snapshot, 1, theme::mono());
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(rendered.trim(), text.trim());
    }
}
