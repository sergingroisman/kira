//! Barras horizontais de severidade — `BarChart` nativo do `ratatui`.
//!
//! Por que nativo: `ratatui 0.29` já traz o widget, e a alternativa seria
//! comprar um crate (`tui-bar-graph`) para desenhar a mesma coisa. A regra do
//! PR-UX1 é usar o que está no binário antes de abrir dependência.
//!
//! Por que barra e não só o texto: `critical 1 · high 7 · medium 3` exige
//! converter número em grandeza na cabeça. A barra faz a comparação na leitura
//! — e continua legível em `mono()` porque o rótulo e o valor estão escritos.
//!
//! Escala: `max` é o MAIOR valor da tela (barra cheia = o pior grupo). Escala
//! fixa em 100% mentiria quando o projeto tem 40 achados de um tipo só.

use ratatui::buffer::Buffer;
use ratatui::layout::{Direction, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Bar, BarChart, BarGroup, Widget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Cor semântica de uma severidade (mesma tabela do painel de achados).
pub fn severity_color(severity: &str, palette: Palette) -> ratatui::style::Color {
    match severity {
        "critical" => palette.critical,
        "high" => palette.high,
        "medium" => palette.medium,
        "low" => palette.low,
        _ => palette.value,
    }
}

/// Desenha uma barra por severidade presente, na ordem recebida.
///
/// `area.height` é o orçamento de linhas: cada barra ocupa 1 linha e o que não
/// cabe é cortado (o painel nunca cresce além do espaço pedido). Lista vazia
/// não desenha nada — quem chama já tem o estado vazio honesto.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    counts: &[(String, usize)],
    palette: Palette,
    glyphs: Glyphs,
) {
    if counts.is_empty() || area.width < 8 || area.height == 0 {
        return;
    }
    let visible = usize::from(area.height).min(counts.len());
    let visible_counts = &counts[..visible];
    let max = visible_counts
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let bars: Vec<Bar> = visible_counts
        .iter()
        .map(|(severity, count)| {
            let value = u64::try_from(*count).unwrap_or(u64::MAX);
            Bar::default()
                .label(severity.as_str().into())
                .value(value)
                // O número escrito ao lado da barra é o dado; a barra é a
                // comparação. Em mono, é o único sinal que sobra — por isso
                // ele NUNCA é omitido.
                .text_value(format!(" {count}"))
                .style(Style::default().fg(severity_color(severity, palette)))
        })
        .collect();
    BarChart::default()
        .direction(Direction::Horizontal)
        .data(BarGroup::default().bars(&bars))
        .bar_set(glyphs.bar_set())
        .bar_width(1)
        .bar_gap(0)
        .group_gap(0)
        .max(max)
        .label_style(Style::default().fg(palette.footer))
        .value_style(Style::default().fg(palette.missing))
        .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode};
    use crate::tui::theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(counts: &[(String, usize)], width: u16, height: u16, glyphs: Glyphs) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                render(
                    frame.area(),
                    frame.buffer_mut(),
                    counts,
                    theme::mono(),
                    glyphs,
                );
            })
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

    fn counts() -> Vec<(String, usize)> {
        vec![
            ("critical".to_string(), 1),
            ("high".to_string(), 7),
            ("medium".to_string(), 3),
        ]
    }

    #[test]
    fn every_severity_gets_a_label_a_bar_and_the_real_number() {
        let text = render_text(&counts(), 60, 3, Glyphs::default());
        assert!(text.contains("critical"), "{text}");
        assert!(text.contains("high"), "{text}");
        assert!(text.contains(" 7"), "valor escrito: {text}");
        assert!(text.contains('█'), "barra do ratatui: {text}");
    }

    #[test]
    fn the_tallest_bar_is_the_worst_group_not_a_fixed_scale() {
        // `high 7` é o máximo ⇒ sua barra é a mais longa da tela.
        let text = render_text(&counts(), 60, 3, Glyphs::default());
        let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        let high = lines
            .iter()
            .find(|line| line.contains("high"))
            .copied()
            .unwrap_or_default();
        let critical = lines
            .iter()
            .find(|line| line.contains("critical"))
            .copied()
            .unwrap_or_default();
        assert!(
            high.matches('█').count() > critical.matches('█').count(),
            "escala pelo máximo real: high={high:?} critical={critical:?}"
        );
    }

    #[test]
    fn ascii_mode_draws_bars_without_block_glyphs() {
        let text = render_text(&counts(), 60, 3, Glyphs::new(Mode::Ascii));
        assert!(text.is_ascii(), "modo ascii sem █: {text}");
        assert!(text.contains('#'), "barra em ascii: {text}");
    }

    #[test]
    fn the_area_budget_cuts_the_extra_rows_instead_of_growing() {
        // Área de 1 linha com 3 severidades: só a primeira cabe, sem pânico.
        let text = render_text(&counts(), 60, 1, Glyphs::default());
        assert!(text.contains("critical"), "{text}");
        assert_eq!(text.lines().filter(|line| !line.is_empty()).count(), 1);
    }

    #[test]
    fn empty_counts_draw_nothing() {
        let text = render_text(&[], 40, 3, Glyphs::default());
        assert!(text.trim().is_empty(), "{text:?}");
    }
}
