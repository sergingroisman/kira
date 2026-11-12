//! Primitiva de painel: borda dupla + título entre `| |` (casca do statui).
//!
//! Adaptado de statui (MIT) — `ui/theme.rs` (`border::DOUBLE`,
//! `TITLE_BRACKETS`) e `ui/widgets/table.rs` (bloco com título). Ver
//! `third_party/statui/NOTICE.md`. O KIRA só troca a paleta e o conteúdo:
//! o título é sempre o nome do dado, nunca uma frase.

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Linha do título: `| texto |`, com os brackets na cor da régua.
pub fn title_line(title: &str, palette: Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled("| ", Style::default().fg(palette.rule)),
        Span::styled(
            format!("{title} "),
            Style::default()
                .fg(palette.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("|", Style::default().fg(palette.rule)),
    ])
}

/// Bloco com borda dupla; a cor da borda diz o estado (foco, severidade,
/// ok/falhou) e por isso é parâmetro explícito.
pub fn block_colored(title: &str, border_color: Color, palette: Palette) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::DOUBLE)
        .border_style(Style::default().fg(border_color))
        .title(title_line(title, palette))
}

/// Bloco de painel comum: borda em foco ou idle.
pub fn block(title: &str, focused: bool, palette: Palette) -> Block<'static> {
    let color = if focused {
        palette.border_focus
    } else {
        palette.border_idle
    };
    block_colored(title, color, palette)
}

/// [`block`] com o conjunto de glifos da sessão.
///
/// Em `KIRA_GLYPHS=ascii` a borda dupla do statui vira borda simples: `╔╗╚╝`
/// dependem da fonte do terminal, e num console sem box-drawing o painel
/// desaparece. A geometria (borda, título entre `| |`) é a mesma — só o glifo
/// muda, então nenhum layout quebra.
pub fn block_with(title: &str, focused: bool, palette: Palette, glyphs: Glyphs) -> Block<'static> {
    let color = if focused {
        palette.border_focus
    } else {
        palette.border_idle
    };
    Block::default()
        .borders(Borders::ALL)
        .border_set(glyphs.border_set())
        .border_style(Style::default().fg(color))
        .title(title_line(title, palette))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(title: &str, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        let block = block(title, true, theme::mono());
        terminal
            .draw(|frame| frame.render_widget(block, frame.area()))
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
    fn panel_uses_double_border_and_bracketed_title() {
        let text = render_text("Achados", 30, 4);
        assert!(text.contains('╔'), "borda dupla do statui: {text}");
        assert!(text.contains('╗'), "{text}");
        assert!(
            text.contains("| Achados |"),
            "título entre brackets: {text}"
        );
    }

    #[test]
    fn tiny_area_still_renders() {
        let text = render_text("X", 4, 2);
        assert!(!text.is_empty(), "{text}");
    }

    #[test]
    fn focus_and_idle_borders_are_visually_distinct() {
        let palette = theme::colorful();
        assert_ne!(palette.border_focus, palette.border_idle);
    }

    #[test]
    fn ascii_borders_render_without_box_drawing_glyphs() {
        use crate::tui::glyph::{Glyphs, Mode};
        let backend = TestBackend::new(30, 4);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                frame.render_widget(
                    block_with("Achados", true, theme::mono(), Glyphs::new(Mode::Ascii)),
                    frame.area(),
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..4 {
            for x in 0..30 {
                text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        assert!(text.is_ascii(), "modo ascii sem box-drawing: {text}");
        assert!(text.contains('+'), "canto da borda simples: {text}");
        assert!(text.contains("| Achados |"), "título preservado: {text}");
    }
}
