//! Sparkline de tendência — `Sparkline` nativo do `ratatui`.
//!
//! O painel de Sessões já desenhava ritmo com barras de texto (statui); aqui a
//! série é outra: **achados por scan**, para responder "o risco está subindo?".
//! A série vem de [`crate::tui::state::scan_series_from`] (lida do summary de
//! cada scan na trilha), nunca de estimativa.
//!
//! O widget nativo não escreve rótulo, e sem rótulo a sparkline é decoração:
//! por isso o desenho é `tendência (N scans): [série] último N`, com a série
//! recebendo só a faixa do meio. Quando a faixa é estreita, o texto inteiro
//! vence a sparkline — número legível vale mais que barra espremida.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline, Widget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Desenha a linha de tendência numa área de UMA linha.
pub fn render(area: Rect, buf: &mut Buffer, series: &[u64], palette: Palette, glyphs: Glyphs) {
    if area.height == 0 || area.width == 0 || series.is_empty() {
        return;
    }
    let label = format!("tendência ({} scans):", series.len());
    let last = series.last().copied().unwrap_or(0);
    let suffix = format!("último {last} ");
    let label_w = label.chars().count() + 1;
    let suffix_w = suffix.chars().count();
    let width = usize::from(area.width);

    // Faixa mínima para a sparkline existir (4 colunas) e o texto caber.
    if width > label_w + suffix_w + 6 {
        let spark_width = u16::try_from(width - label_w - suffix_w).unwrap_or(0);
        Paragraph::new(Line::from(Span::styled(
            format!("{label} "),
            Style::default().fg(palette.footer),
        )))
        .render(
            Rect {
                width: u16::try_from(label_w).unwrap_or(area.width),
                ..area
            },
            buf,
        );
        let spark_area = Rect {
            x: area.x + u16::try_from(label_w).unwrap_or(0),
            width: spark_width,
            ..area
        };
        Sparkline::default()
            .data(series)
            .bar_set(glyphs.bar_set())
            .style(Style::default().fg(palette.value))
            .render(spark_area, buf);
        Paragraph::new(Line::from(Span::styled(
            suffix,
            Style::default().fg(palette.missing),
        )))
        .render(
            Rect {
                x: area.x
                    + area
                        .width
                        .saturating_sub(u16::try_from(suffix_w).unwrap_or(0)),
                width: u16::try_from(suffix_w).unwrap_or(0),
                ..area
            },
            buf,
        );
        return;
    }

    // Estreito: só o texto, com os valores reais (nada de barra ilegível).
    let values = series
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    Paragraph::new(Line::from(Span::styled(
        format!("tendência: {values}"),
        Style::default().fg(palette.footer),
    )))
    .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(series: &[u64], width: u16, glyphs: Glyphs) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                render(
                    frame.area(),
                    frame.buffer_mut(),
                    series,
                    theme::mono(),
                    glyphs,
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for x in 0..width {
            text.push(buffer[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        text
    }

    #[test]
    fn label_series_and_last_value_share_the_line() {
        let text = render_text(&[3, 5, 4, 7], 60, Glyphs::default());
        assert!(text.contains("tendência (4 scans):"), "{text}");
        assert!(text.contains("último 7"), "{text}");
        assert!(
            text.contains('▁') || text.contains('▂') || text.contains('█'),
            "série desenhada: {text}"
        );
    }

    #[test]
    fn ascii_mode_has_no_special_glyph() {
        use crate::tui::glyph::has_special_glyphs;
        let text = render_text(&[3, 5, 4, 7], 60, Glyphs::new(Mode::Ascii));
        // O acento de "tendência" é texto; bloco/seta não podem aparecer.
        assert!(!has_special_glyphs(&text), "modo ascii: {text}");
        assert!(text.contains('#'), "a barra em ascii é `#`: {text}");
    }

    #[test]
    fn narrow_area_prefers_the_real_numbers_over_a_squeezed_sparkline() {
        let text = render_text(&[3, 5, 4, 7], 24, Glyphs::default());
        assert!(text.contains("tendência: 3 5 4 7"), "{text}");
    }

    #[test]
    fn empty_series_is_a_no_op() {
        let text = render_text(&[], 40, Glyphs::default());
        assert!(text.trim().is_empty(), "{text:?}");
    }
}
