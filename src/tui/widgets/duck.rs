//! Widget do pato (TUI-01): pinta a arte do mascote, cor por classe.
//!
//! As LINHAS saem de [`crate::tui::duck::Duck::art`] (pura, 7 bits); a
//! cor é semântica da casa, nunca decoração: o `!` do alerta usa a cor
//! de falha, a bolha `?` do pensamento usa a cor de ajuda e a água usa a
//! cor de régua — em `mono()` tudo vira `Reset` e o desenho continua
//! completo (a informação nunca mora só na cor, regra da paleta).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::tui::duck::Duck;
use crate::tui::theme::Palette;

/// Pinta o pato no canto de `area` (o que couber de altura é o que
/// desenha — arte maior que o espaço nunca vira erro).
pub fn render(buf: &mut Buffer, area: Rect, duck: Duck, tick: u64, palette: Palette) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    for (row, line) in duck.art(tick).into_iter().enumerate() {
        if row as u32 >= u32::from(area.height) {
            break;
        }
        let y = area.y + row as u16;
        for (col, ch) in line.chars().enumerate() {
            let x = area.x + col as u16;
            if col as u16 >= area.width {
                break;
            }
            buf[(x, y)].set_symbol(&ch.to_string());
            buf[(x, y)].set_style(Style::default().fg(class_color(ch, palette)));
        }
    }
}

/// Cor por classe de caractere da arte: `!` é alerta (cor de falha), `?`
/// é a bolha do pensamento (cor de ajuda), `~` é água (cor de régua) e o
/// resto é o pato (cor dele).
fn class_color(ch: char, palette: Palette) -> Color {
    match ch {
        '!' => palette.failed,
        '?' => palette.help,
        '~' => palette.rule,
        _ => palette.duck,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::duck::DuckState;
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(duck: Duck, tick: u64, width: u16, height: u16, palette: Palette) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(frame.buffer_mut(), area, duck, tick, palette);
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn the_duck_is_drawn_deterministically() {
        let duck = Duck::idle(7);
        assert_eq!(
            render_text(duck, 0, 20, 8, theme::mono()),
            render_text(duck, 0, 20, 8, theme::mono())
        );
    }

    /// A arte vai INTEIRA para o buffer: bico, corpo e água presentes.
    #[test]
    fn the_full_art_lands_in_the_buffer() {
        let text = render_text(Duck::idle(7), 0, 20, 8, theme::mono());
        assert!(text.contains("<("), "bico: {text}");
        assert!(text.contains("___"), "corpo: {text}");
        assert!(text.contains('~'), "água: {text}");
    }

    #[test]
    fn alert_state_draws_the_mark() {
        let text = render_text(Duck::new(DuckState::Alerta, 7), 0, 20, 8, theme::mono());
        assert!(text.contains('!'), "{text}");
    }

    /// Área pequena demais: desenha o que couber, nunca pânico.
    #[test]
    fn tiny_area_never_panics() {
        for (width, height) in [(1_u16, 1_u16), (0, 0), (4, 2), (12, 5)] {
            let _ = render_text(Duck::idle(7), 3, width, height, theme::mono());
        }
    }
}
