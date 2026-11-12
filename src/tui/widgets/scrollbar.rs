//! Scrollbar vertical das listas — a posição do viewport, nunca do cursor.
//!
//! A lista longa tinha um buraco de feedback: 87 achados e nenhum sinal de que
//! existia mais coisa abaixo. O [`Scrollbar`] nativo do `ratatui` resolve o
//! desenho, mas ele não sabe de onde a lista começou — o `offset` vem de
//! [`crate::tui::viewport::keep_visible`], que é quem rola. Aqui só se pinta,
//! com os símbolos do modo de glifo (`KIRA_GLYPHS=ascii` troca `▐` por `#`).
//!
//! `content_length` é o total REAL de itens do snapshot: barra que mente sobre
//! o tamanho da lista é pior que barra nenhuma.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Desenha o scrollbar numa faixa de 1 coluna à direita da lista.
///
/// Área vazia é no-op (terminal minúsculo não pode explodir), e `total` zero
/// não desenha nada — lista vazia já tem o estado vazio honesto no corpo.
pub fn render(
    buf: &mut Buffer,
    area: Rect,
    offset: usize,
    total: usize,
    palette: Palette,
    glyphs: Glyphs,
) {
    if area.width == 0 || area.height == 0 || total == 0 {
        return;
    }
    // `viewport_content_length` é o que dá ESPESSURA proporcional ao thumb: sem
    // ele o ratatui usa o tamanho do trilho e a barra de 100 itens pareceria
    // quase vazia (ou cheia) dependendo só da altura do painel.
    let mut state = ScrollbarState::new(total)
        .position(offset.min(total.saturating_sub(1)))
        .viewport_content_length(usize::from(area.height));
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .symbols(glyphs.scrollbar_set())
        .style(Style::default().fg(palette.rule))
        .render(area, buf, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Texto de uma coluna do buffer de teste.
    fn column_text(offset: usize, total: usize, height: u16, glyphs: Glyphs) -> String {
        let backend = TestBackend::new(1, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                let area = frame.area();
                render(
                    frame.buffer_mut(),
                    area,
                    offset,
                    total,
                    theme::mono(),
                    glyphs,
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| buffer[(0, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn scrollbar_paints_a_thumb_and_a_track_in_unicode_mode() {
        let text = column_text(0, 100, 10, Glyphs::new(Mode::Unicode));
        // Trilho fino (`│`) para não se confundir com a borda dupla do painel.
        assert!(text.contains('█'), "thumb: {text:?}");
        assert!(text.contains('│'), "trilho fino: {text:?}");
        assert!(
            !text.contains('║'),
            "nunca o trilho duplo da borda: {text:?}"
        );
    }

    #[test]
    fn ascii_mode_never_emits_a_special_glyph() {
        let text = column_text(0, 100, 10, Glyphs::new(Mode::Ascii));
        assert!(text.is_ascii(), "modo ascii só com 7 bits: {text:?}");
        assert!(text.contains('#'), "thumb em ascii: {text:?}");
    }

    #[test]
    fn position_moves_the_thumb_down_the_track() {
        let top = column_text(0, 100, 10, Glyphs::new(Mode::Ascii));
        let bottom = column_text(90, 100, 10, Glyphs::new(Mode::Ascii));
        assert_ne!(top, bottom, "o thumb acompanha o offset");
        let top_thumb = top.find('#').unwrap_or(usize::MAX);
        let bottom_thumb = bottom.find('#').unwrap_or(usize::MAX);
        assert!(top_thumb < bottom_thumb, "{top:?} vs {bottom:?}");
        assert!(top.contains('#') && bottom.contains('#'), "{top:?}");
    }

    #[test]
    fn degenerate_areas_and_empty_lists_are_no_ops() {
        let backend = TestBackend::new(4, 3);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                // Área zero e lista vazia: nada desenhado, nada de pânico.
                render(
                    frame.buffer_mut(),
                    Rect::new(0, 0, 0, 0),
                    0,
                    10,
                    theme::mono(),
                    Glyphs::default(),
                );
                render(
                    frame.buffer_mut(),
                    Rect::new(0, 0, 1, 1),
                    0,
                    0,
                    theme::mono(),
                    Glyphs::default(),
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
    }
}
