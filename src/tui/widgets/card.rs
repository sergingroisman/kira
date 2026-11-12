//! Moldura de cartão do Resumo (PR-UX2) — borda + título + uma linha por fato.
//!
//! O cartão **não** sabe desenhar dado: ele entrega a área útil e duas
//! ferramentas (`put` e `crop`). Quem pinta barras, gauge ou chip é o módulo do
//! cartão (`widgets::cards`) usando os widgets que o PR-UX1 já testou. Assim o
//! "cartão" é uma gramática só (borda, título com o nome do dado, coluna
//! alinhada) e o conteúdo varia sem duplicar código de moldura.
//!
//! Duas regras herdadas da casca:
//! 1. o título é o NOME DO DADO ("Risco", "Sessões"), nunca uma frase — título
//!    que explica vira comentário, e comentário envelhece;
//! 2. nada de wrap escondido: linha que não cabe é CORTADA com `…`, porque
//!    quebra de linha em painel de altura fixa empurra o resto do cartão para
//!    fora da borda.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;

use crate::tui::glyph::Glyphs;
use crate::tui::grid::CardKind;
use crate::tui::theme::Palette;
use crate::tui::widgets::panel::block_with;

/// Moldura do cartão, com o título do dado e o foco da tela.
pub fn frame(kind: CardKind, focused: bool, palette: Palette, glyphs: Glyphs) -> Block<'static> {
    block_with(kind.title(), focused, palette, glyphs)
}

/// Moldura com título próprio — usada quando o número cabe no título
/// (`Risco · total 10`) e não sobra linha dentro do cartão.
pub fn frame_titled(
    title: &str,
    focused: bool,
    palette: Palette,
    glyphs: Glyphs,
) -> Block<'static> {
    block_with(title, focused, palette, glyphs)
}

/// Corta o texto na largura, marcando o corte. Determinístico e sem wrap.
pub fn crop(text: &str, width: u16) -> String {
    let max = usize::from(width);
    if max == 0 {
        return String::new();
    }
    let length = text.chars().count();
    if length <= max {
        return text.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Escreve uma linha de conteúdo na `row` (0 = primeira linha útil).
///
/// Fora da área é no-op: cartão pequeno não estoura a borda nem panica.
pub fn put(buf: &mut Buffer, inner: Rect, row: u16, text: &str, style: Style) {
    if row >= inner.height || inner.width == 0 {
        return;
    }
    let line = crop(text, inner.width);
    buf.set_string(inner.x, inner.y.saturating_add(row), &line, style);
}

/// Retângulo de uma linha interna, para widgets que desenham sozinhos
/// (barras, gauge, sparkline).
pub fn row_rect(inner: Rect, row: u16) -> Rect {
    Rect {
        x: inner.x,
        y: inner.y.saturating_add(row),
        width: inner.width,
        height: u16::from(row < inner.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    fn render(width: u16, height: u16, kind: CardKind, glyphs: Glyphs, text: &str) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|f| {
                let area = f.area();
                let block = frame(kind, true, theme::mono(), glyphs);
                let inner = block.inner(area);
                let buf = f.buffer_mut();
                block.render(area, buf);
                put(buf, inner, 0, text, Style::default());
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
    fn card_keeps_title_and_border() {
        let text = render(40, 5, CardKind::Risco, Glyphs::default(), "total 10");
        assert!(text.contains("| Risco |"), "{text}");
        assert!(text.contains("total 10"), "{text}");
    }

    #[test]
    fn card_crops_long_text_instead_of_wrapping_out_of_the_border() {
        let long = "x".repeat(200);
        let text = render(20, 4, CardKind::Projeto, Glyphs::default(), &long);
        // Uma linha só: nada de segunda linha empurrando a borda de baixo.
        for line in text.lines().take(1) {
            assert!(line.chars().count() <= 20, "{line:?}");
        }
        assert!(text.contains('…'), "corte marcado: {text}");
    }

    #[test]
    fn ascii_mode_card_uses_plain_border() {
        let glyphs = Glyphs::new(Mode::Ascii);
        let text = render(30, 4, CardKind::Sessoes, glyphs, "scan");
        assert!(!has_special_glyphs(&text), "ascii: {text}");
        assert!(text.contains("| Sessões |"), "{text}");
    }

    #[test]
    fn crop_never_returns_more_than_the_width() {
        for width in 0..12 {
            let out = crop("abcdefghij", width);
            assert!(
                out.chars().count() <= usize::from(width),
                "width {width}: {out:?}"
            );
        }
        assert_eq!(crop("abc", 0), "");
        assert_eq!(crop("abc", 1), "…");
        assert_eq!(crop("abc", 5), "abc");
    }

    #[test]
    fn put_outside_the_inner_area_is_a_no_op() {
        let backend = TestBackend::new(10, 3);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                let area = frame.area();
                put(frame.buffer_mut(), area, 99, "fora", Style::default());
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
    }

    #[test]
    fn row_rect_collapses_when_the_row_does_not_exist() {
        let inner = Rect {
            x: 2,
            y: 3,
            width: 10,
            height: 2,
        };
        assert_eq!(row_rect(inner, 1).height, 1);
        assert_eq!(row_rect(inner, 2).height, 0);
        assert_eq!(row_rect(inner, 2).y, 5);
    }
}
