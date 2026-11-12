//! Barra de proporção (gauge) desenhada como linha de texto.
//!
//! O plano do PR-UX1 pedia o `Gauge` nativo; ele ficou de fora por um motivo
//! concreto: `use_unicode(false)` só troca a célula FRACIONÁRIA e continua
//! pintando `█` nas cheias (`ratatui-0.29.0/src/widgets/gauge.rs:197`), o que
//! quebra o `KIRA_GLYPHS=ascii` do critério 4. Uma linha de texto com os
//! glifos do modo resolve o mesmo problema e é testável célula a célula.
//!
//! O que a barra mede aqui é o **uso do índice de retrieval**: quantos arquivos
//! o `repo_file` tem contra o teto `KIRA_INDEX_MAX_FILES`. É um número REAL que
//! o dono precisa ver ANTES do teto cortar arquivos em silêncio — e o único
//! orçamento com dado disponível hoje (o budget de LLM ainda não tem contador:
//! não existe tabela nem `max_llm_calls_per_day` em `src/config.rs`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Largura máxima da barra (a linha inteira cabe em 80 colunas com o rótulo).
const MAX_BAR_WIDTH: usize = 16;

/// Proporção usada/limite, saturada em 100% (estouro não desenha barra maior
/// que a barra; o texto ao lado diz o número exato, inclusive acima do teto).
pub fn ratio(used: usize, cap: usize) -> f64 {
    if cap == 0 {
        return 0.0;
    }
    (used as f64 / cap as f64).clamp(0.0, 1.0)
}

/// Texto da barra: `índice 812/5000 arquivos  [████░░░░░░░░] 16%`.
///
/// Função pura sobre números — o teste afirma o desenho sem terminal.
pub fn text(label: &str, used: usize, cap: usize, bar_width: usize, glyphs: Glyphs) -> String {
    let (fill, empty) = glyphs.gauge();
    let width = bar_width.max(1);
    let filled = ((ratio(used, cap) * width as f64).round() as usize).min(width);
    let percent = (ratio(used, cap) * 100.0).round() as usize;
    format!(
        "{label} {used}/{cap}  [{}{}] {percent}%",
        fill.repeat(filled),
        empty.repeat(width - filled)
    )
}

/// Desenha a linha do gauge (1 linha de altura).
///
/// Barra some em área estreita e o texto continua: a proporção é acessório, o
/// número é o dado.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    used: usize,
    cap: usize,
    palette: Palette,
    glyphs: Glyphs,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let width = usize::from(area.width);
    // Reserva do texto ao redor da barra: rótulo + ` used/cap` + ` 100%` + `[]`.
    let text_room = format!("{label} {used}/{cap}  [] 100%").chars().count();
    let available = width.saturating_sub(text_room);
    if available < 4 {
        Paragraph::new(Line::from(Span::styled(
            format!("{label} {used}/{cap}"),
            Style::default().fg(palette.footer),
        )))
        .render(area, buf);
        return;
    }
    let bar_width = available.min(MAX_BAR_WIDTH);
    let body = text(label, used, cap, bar_width, glyphs);
    // Destaque só na barra: o número já é legível por si, e o REVERSED aqui
    // roubaria o sinal de seleção das listas.
    Paragraph::new(Line::from(Span::styled(
        body,
        Style::default()
            .fg(palette.footer)
            .add_modifier(Modifier::BOLD),
    )))
    .render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode};
    use crate::tui::theme;

    #[test]
    fn half_of_the_cap_fills_half_the_bar() {
        let body = text("índice", 50, 100, 10, Glyphs::default());
        assert!(body.contains("50/100"), "{body}");
        assert_eq!(body.matches('█').count(), 5, "{body}");
        assert_eq!(body.matches('░').count(), 5, "{body}");
        assert!(body.ends_with("50%"), "{body}");
    }

    #[test]
    fn overflow_saturates_the_bar_and_still_shows_the_real_number() {
        // Índice acima do teto: barra cheia, número honesto.
        let body = text("índice", 6200, 5000, 8, Glyphs::default());
        assert_eq!(body.matches('█').count(), 8, "{body}");
        assert!(body.contains("6200/5000"), "{body}");
        assert!(body.ends_with("100%"), "{body}");
        assert_eq!(ratio(6200, 5000), 1.0);
    }

    #[test]
    fn zero_entries_is_zero_percent_not_a_division_by_zero() {
        assert_eq!(ratio(0, 0), 0.0);
        assert_eq!(ratio(10, 0), 0.0);
        let body = text("índice", 0, 5000, 6, Glyphs::default());
        assert!(body.contains("0/5000"), "{body}");
        assert_eq!(body.matches('█').count(), 0, "{body}");
        assert!(body.contains("0%"), "{body}");
    }

    #[test]
    fn ascii_mode_uses_hash_and_dash() {
        // Rótulo sem acento neste caso: o modo ASCII é sobre GLIFO, não sobre o
        // português do texto (as palavras continuam acentuadas na tela).
        let glyphs = Glyphs::new(Mode::Ascii);
        let body = text("indice", 5, 10, 4, glyphs);
        assert!(body.is_ascii(), "{body}");
        assert_eq!(body.matches('#').count(), 2, "{body}");
        assert_eq!(body.matches('-').count(), 2, "{body}");
    }

    #[test]
    fn narrow_area_hides_the_bar_but_keeps_both_numbers() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(20, 1);
        let mut terminal = Terminal::new(backend).unwrap_or_else(|err| panic!("backend: {err}"));
        terminal
            .draw(|frame| {
                render(
                    frame.area(),
                    frame.buffer_mut(),
                    "índice",
                    812,
                    5000,
                    theme::mono(),
                    Glyphs::default(),
                );
            })
            .unwrap_or_else(|err| panic!("draw: {err}"));
        let buffer = terminal.backend().buffer();
        let mut text = String::new();
        for x in 0..20 {
            text.push(buffer[(x, 0)].symbol().chars().next().unwrap_or(' '));
        }
        assert!(text.contains("812/5000"), "{text:?}");
        assert!(!text.contains('█'), "sem barra espremida: {text:?}");
    }
}
