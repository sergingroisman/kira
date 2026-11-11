//! Painel de cabeçalho — identidade do projeto e saúde básica (W6-PR1).
//!
//! Renderiza as MESMAS linhas de [`crate::tui::state::header_lines`] num bloco
//! ratatui, colorindo `—` de ausência com a cor de "dado honesto" da paleta.
//! Nada de conteúdo aqui: o widget é só pintura, o texto vem do snapshot.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::tui::theme::Palette;

/// O widget do cabeçalho: linhas de snapshot + paleta + título.
#[derive(Debug, Clone)]
pub struct HeaderWidget<'a> {
    lines: &'a [String],
    /// Nome curto do projeto para o título do bloco (quando existe).
    title: Option<&'a str>,
    palette: Palette,
}

impl<'a> HeaderWidget<'a> {
    /// Constrói o widget a partir do snapshot pronto (nunca consulta disco).
    pub fn new(lines: &'a [String], title: Option<&'a str>, palette: Palette) -> Self {
        Self {
            lines,
            title,
            palette,
        }
    }
}

impl Widget for HeaderWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Título do bloco: nome do projeto quando existe, senão um marcador
        // honesto — nunca um nome inventado.
        let title = self
            .title
            .map(|name| format!(" kira — {name} "))
            .unwrap_or_else(|| " kira — projeto ".to_string());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(self.palette.title)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(self.palette.border_focus));

        block.clone().render(area, buf);

        let spans: Vec<Line> = self
            .lines
            .iter()
            .map(|line| {
                if line.contains(": — ") || line.ends_with(": —") {
                    // Linha com dado ausente: mantém o texto, pinta o aviso.
                    Line::from(vec![Span::styled(
                        line.clone(),
                        Style::default().fg(self.palette.missing),
                    )])
                } else {
                    Line::from(vec![Span::styled(
                        line.clone(),
                        Style::default().fg(self.palette.value),
                    )])
                }
            })
            .collect();
        Paragraph::new(spans)
            .block(block)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme;
    use ratatui::{Terminal, backend::TestBackend};

    /// Renderiza num buffer de teste e devolve o texto da área interna.
    /// `TestBackend` é o "smith determinístico" da spec W6: snapshot de buffer
    /// ASCII sem depender de terminal real.
    fn render_text(lines: &[String], width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let widget = HeaderWidget::new(lines, Some("proj"), theme::mono());
        terminal
            .draw(|frame| frame.render_widget(widget, frame.area()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..height {
            for x in 0..width {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn header_renders_lines_with_border_and_title() {
        let lines = vec!["projeto: proj".to_string(), "achados: 7".to_string()];
        let text = render_text(&lines, 40, 6);
        assert!(text.contains("projeto: proj"), "{text}");
        assert!(text.contains("achados: 7"), "{text}");
        assert!(text.contains("kira — proj"), "{text}");
        assert!(text.contains("┌"), "borda presente: {text}");
    }

    #[test]
    fn missing_dash_line_renders_with_missing_style_but_same_text() {
        let lines = vec!["achados: — (nunca rodou kira scan)".to_string()];
        let text = render_text(&lines, 50, 5);
        assert!(
            text.contains("achados: — (nunca rodou kira scan)"),
            "o texto honesto nunca desaparece: {text}"
        );
    }

    #[test]
    fn tiny_terminal_still_renders_without_panic() {
        // Degradação: terminal minúsculo não pode panicar nem sumir com o dado.
        let lines = vec!["projeto: proj".to_string()];
        let text = render_text(&lines, 20, 3);
        assert!(!text.is_empty(), "{text}");
    }
}
