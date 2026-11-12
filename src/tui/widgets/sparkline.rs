//! Sparkline de ritmo — 7 níveis, técnica do statui (`symbols::bar`).
//!
//! Adaptado de statui (MIT) — `ui/widgets/table.rs` (`generate_sparkline_string`,
//! barras de 7 níveis e escala pelo máximo da janela). Ver
//! `third_party/statui/NOTICE.md`.
//!
//! Regra do KIRA: a barra representa uma contagem REAL por dia; dia sem sessão
//! aparece como `·` (não como barra mínima, que fingiria atividade).

use crate::tui::glyph::Glyphs;

/// Níveis de barra, do menor ao maior (conjunto Unicode, o padrão).
pub const LEVELS: [&str; 7] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇"];

/// Dia sem nenhuma sessão.
pub const EMPTY: &str = "·";

/// Desenha uma barra por valor, adjacentes (15 dias cabem em 15 colunas).
pub fn bars(values: &[u64]) -> String {
    bars_with(values, Glyphs::default())
}

/// [`bars`] com o conjunto de glifos da sessão (`KIRA_GLYPHS=ascii`).
///
/// A escala é a MESMA nos dois modos: só o desenho muda, nunca a comparação —
/// trocar de glifo não pode mudar o que a sparkline diz sobre o ritmo.
pub fn bars_with(values: &[u64], glyphs: Glyphs) -> String {
    if values.is_empty() {
        return String::new();
    }
    let levels = glyphs.spark_levels();
    let empty = glyphs.spark_empty();
    let max = values.iter().copied().max().unwrap_or(0);
    if max == 0 {
        // Janela inteira sem sessão: um ponto por dia, sem barra falsa.
        return empty.repeat(values.len());
    }
    values
        .iter()
        .map(|value| {
            if *value == 0 {
                return empty.to_string();
            }
            let ratio = *value as f64 / max as f64;
            let index = (ratio * (levels.len() - 1) as f64).round() as usize;
            levels[index.min(levels.len() - 1)].to_string()
        })
        .collect()
}

/// Pico da janela (`0` quando não há atividade) — o denominador da escala.
pub fn peak(values: &[u64]) -> u64 {
    values.iter().copied().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_days_are_dots_not_fake_bars() {
        assert_eq!(bars(&[0, 0, 0]), "···");
    }

    #[test]
    fn peak_is_the_top_level_and_others_scale() {
        let line = bars(&[0, 1, 2, 4]);
        assert_eq!(line.chars().count(), 4, "{line}");
        assert!(line.ends_with(LEVELS[6]), "pico = nível máximo: {line}");
        assert!(line.starts_with(EMPTY), "{line}");
        assert_eq!(peak(&[0, 1, 2, 4]), 4);
    }

    #[test]
    fn empty_window_is_an_empty_line() {
        assert_eq!(bars(&[]), "");
        assert_eq!(peak(&[]), 0);
    }

    #[test]
    fn ascii_mode_keeps_the_scale_but_drops_the_block_glyphs() {
        use crate::tui::glyph::{Glyphs, Mode};
        let ascii = Glyphs::new(Mode::Ascii);
        let line = bars_with(&[0, 1, 2, 4], ascii);
        assert!(line.is_ascii(), "só 7 bits: {line:?}");
        assert_eq!(line.chars().count(), 4);
        assert!(line.starts_with('.'), "dia vazio vira ponto: {line:?}");
        assert!(line.ends_with('#'), "pico vira `#`: {line:?}");
        // A mesma série no modo Unicode: mesma forma, glifo diferente.
        let unicode = bars_with(&[0, 1, 2, 4], Glyphs::new(Mode::Unicode));
        assert_eq!(unicode, bars(&[0, 1, 2, 4]));
    }
}
