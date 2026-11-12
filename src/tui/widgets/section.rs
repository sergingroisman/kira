//! Régua de seção com título — a gramática interna do inspetor do statui.
//!
//! Adaptado de statui (MIT) — `ui/widgets/inspector/mod.rs` (separadores com
//! `Borders::TOP` e título). Ver `third_party/statui/NOTICE.md`.
//!
//! No KIRA a régua é o que separa "Achados", "Sessões recentes" e "Pendências"
//! no Resumo sem transformar cada bloco num cartão.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::widgets::{Block, Borders, Widget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;
use crate::tui::widgets::panel::title_line;

/// Bloco de 1 linha: `═| Título |══════`.
pub fn rule(title: &str, palette: Palette) -> Block<'static> {
    Block::default()
        .borders(Borders::TOP)
        .border_set(border::DOUBLE)
        .border_style(Style::default().fg(palette.rule))
        .title(title_line(title, palette))
}

/// Desenha a régua numa área de UMA linha. Altura 0 é no-op.
pub fn render(area: Rect, buf: &mut Buffer, title: &str, palette: Palette) {
    render_with(area, buf, title, palette, Glyphs::default());
}

/// [`render`] com o conjunto de glifos da sessão: em ASCII a régua dupla vira
/// `-` e o painel continua legível em console sem box-drawing.
pub fn render_with(area: Rect, buf: &mut Buffer, title: &str, palette: Palette, glyphs: Glyphs) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    Block::default()
        .borders(Borders::TOP)
        .border_set(glyphs.border_set())
        .border_style(Style::default().fg(palette.rule))
        .title(title_line(title, palette))
        .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(title: &str, width: u16) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| render(frame.area(), frame.buffer_mut(), title, theme::mono()))
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for x in 0..width {
            text.push(buffer[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        text
    }

    #[test]
    fn rule_renders_title_between_brackets() {
        let text = render_text("Sessões recentes", 50);
        assert!(text.contains("| Sessões recentes |"), "{text}");
        assert!(text.contains('═'), "régua dupla: {text}");
    }

    #[test]
    fn zero_height_is_a_no_op() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        render(Rect::new(0, 0, 0, 0), &mut buf, "x", theme::mono());
    }
}
