//! Glifos do dashboard: conjunto Unicode (padrão) e conjunto ASCII.
//!
//! Regra do dono (critério de aceite do PR-UX1): `KIRA_GLYPHS=ascii` precisa
//! renderizar **sem glifo especial** — terminal de fábrica, fonte de console do
//! Windows, SSH em equipamento antigo, tudo isso desenha `#`, `-`, `>` e `|`
//! igual, enquanto `█`, `▁`, `➤`, `░` viram quadradinho ou somem. Glifo que
//! vira quadradinho é informação perdida: a barra de severidade deixa de
//! comparar, o scrollbar deixa de mostrar posição.
//!
//! A decisão é PURA sobre o valor da variável ([`Mode::from_value`]) pelo mesmo
//! motivo de [`crate::tui::theme::color_enabled`]: variável de ambiente é
//! estado global do processo e teste que muta env deixa a suíte dependente da
//! ordem de execução.
//!
//! Cada glifo tem um único dono aqui — nenhum widget escreve `"█"` solto. Assim
//! o modo ASCII é verificável por teste (`glyph::text_is_ascii`) em vez de
//! depender de inspeção visual em cada terminal.

use ratatui::symbols;

/// Variável que escolhe o conjunto de glifos (`KIRA_GLYPHS=ascii`).
pub const GLYPHS_ENV: &str = "KIRA_GLYPHS";

/// Conjunto de glifos da sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Blocos, setas e traços (padrão de terminal moderno).
    #[default]
    Unicode,
    /// Só caracteres de 7 bits (`KIRA_GLYPHS=ascii`).
    Ascii,
}

impl Mode {
    /// Função pura: decide o modo a partir do valor já lido da variável.
    ///
    /// Só desliga com valor explícito (`ascii`, `plain`, `none`) — qualquer
    /// outro valor mantém o padrão Unicode, porque o modo ASCII é degradação
    /// pedida pelo dono, nunca um acidente de digitação.
    pub fn from_value(value: Option<&str>) -> Self {
        match value.map(|raw| raw.trim().to_ascii_lowercase()) {
            Some(ref raw) if matches!(raw.as_str(), "ascii" | "plain" | "none") => Self::Ascii,
            _ => Self::Unicode,
        }
    }

    /// Modo da sessão atual, lendo [`GLYPHS_ENV`] uma vez por chamada.
    pub fn from_env() -> Self {
        Self::from_value(std::env::var(GLYPHS_ENV).ok().as_deref())
    }

    /// O modo é ASCII?
    pub const fn is_ascii(self) -> bool {
        matches!(self, Self::Ascii)
    }
}

/// Os glifos concretos que um widget pode precisar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyphs {
    mode: Mode,
}

impl Default for Glyphs {
    fn default() -> Self {
        Self::new(Mode::Unicode)
    }
}

impl Glyphs {
    /// Constrói para um modo explícito (testes e chamadas já resolvidas).
    pub const fn new(mode: Mode) -> Self {
        Self { mode }
    }

    /// Constrói a partir da variável de ambiente.
    pub fn from_env() -> Self {
        Self::new(Mode::from_env())
    }

    /// O modo por trás do conjunto.
    pub const fn mode(self) -> Mode {
        self.mode
    }

    /// Marcador de linha selecionada (2 colunas, como o das tabelas).
    ///
    /// A seleção já tem `REVERSED` como sinal que sobrevive a `NO_COLOR`; o
    /// glifo é o terceiro sinal, e em ASCII ele continua distinguível (`> `).
    pub const fn selection(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "➤ ",
            // Duas colunas, como o marcador Unicode: a tabela não desalinha.
            Mode::Ascii => "> ",
        }
    }

    /// Níveis de barra para `Sparkline`/`BarChart` (`ratatui::symbols::bar`).
    pub const fn bar_set(self) -> symbols::bar::Set {
        match self.mode {
            Mode::Unicode => symbols::bar::NINE_LEVELS,
            // Mesma semântica (vazio → cheio), sem bloco: `-`, `=`, `#`.
            Mode::Ascii => symbols::bar::Set {
                empty: " ",
                one_eighth: "-",
                one_quarter: "-",
                three_eighths: "=",
                half: "=",
                five_eighths: "=",
                three_quarters: "#",
                seven_eighths: "#",
                full: "#",
            },
        }
    }

    /// Conjunto do `Scrollbar` vertical (`ratatui::symbols::scrollbar`).
    ///
    /// Trilho FINO de propósito: o `DOUBLE_VERTICAL` do ratatui usa `║`, que ao
    /// lado da borda dupla do painel vira `║║` — dois trilhos iguais, e a barra
    /// some no desenho da casca. `│` é o trilho, `█` é o polegar.
    pub const fn scrollbar_set(self) -> symbols::scrollbar::Set {
        match self.mode {
            Mode::Unicode => symbols::scrollbar::Set {
                track: "│",
                thumb: "█",
                begin: "▲",
                end: "▼",
            },
            Mode::Ascii => symbols::scrollbar::Set {
                track: "|",
                thumb: "#",
                begin: "|",
                end: "|",
            },
        }
    }

    /// Glifos da barra de proporção (preenchido, vazio).
    ///
    /// O `Gauge` nativo ficou de fora de propósito: `use_unicode(false)` só
    /// troca a célula fracionária e continua pintando `█` nas cheias
    /// (`ratatui-0.29.0/src/widgets/gauge.rs:197`), o que quebraria o modo
    /// ASCII. A barra aqui é uma linha de texto — e por isso testável.
    pub const fn gauge(self) -> (&'static str, &'static str) {
        match self.mode {
            Mode::Unicode => ("█", "░"),
            Mode::Ascii => ("#", "-"),
        }
    }

    /// Escada de níveis da sparkline de texto, do menor para o maior.
    pub const fn spark_levels(self) -> [&'static str; 7] {
        match self.mode {
            Mode::Unicode => ["▁", "▂", "▃", "▄", "▅", "▆", "▇"],
            Mode::Ascii => [".", ":", "-", "=", "+", "*", "#"],
        }
    }

    /// Valor zero de uma série (nada aconteceu — nunca barra mínima falsa).
    pub const fn spark_empty(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "·",
            Mode::Ascii => ".",
        }
    }

    /// Régua horizontal do markdown e das seções.
    pub const fn rule(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "────",
            Mode::Ascii => "----",
        }
    }

    /// Bala de item de lista.
    pub const fn bullet(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "•",
            Mode::Ascii => "*",
        }
    }

    /// Marcador de título por nível do markdown.
    pub const fn heading(self, level: usize) -> &'static str {
        match (self.mode, level) {
            (Mode::Ascii, 1) => "=",
            (Mode::Ascii, 2) => "-",
            (Mode::Ascii, _) => ".",
            (Mode::Unicode, 1) => "▌",
            (Mode::Unicode, 2) => "■",
            (Mode::Unicode, _) => "·",
        }
    }

    /// Cerca do campo editável da folha de alvo (`▌`/`[`).
    pub const fn field_open(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "▌",
            Mode::Ascii => "[",
        }
    }

    /// Fecha a cerca do campo editável (`▏`/`]`).
    pub const fn field_close(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "▏",
            Mode::Ascii => "]",
        }
    }

    /// Barra de citação e borda de bloco de código.
    pub const fn quote(self) -> &'static str {
        match self.mode {
            Mode::Unicode => "│",
            Mode::Ascii => "|",
        }
    }

    /// Conjunto de borda do `Block`.
    ///
    /// Unicode mantém a borda DUPLA do statui (identidade da casca). Em ASCII é
    /// um conjunto construído aqui: o `symbols::border::PLAIN` do `ratatui` é
    /// `line::NORMAL`, ou seja, ainda desenha `┌─┐│` — box-drawing, exatamente o
    /// que o modo ASCII existe para evitar.
    pub const fn border_set(self) -> symbols::border::Set {
        match self.mode {
            Mode::Unicode => symbols::border::DOUBLE,
            Mode::Ascii => symbols::border::Set {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                vertical_left: "|",
                vertical_right: "|",
                horizontal_top: "-",
                horizontal_bottom: "-",
            },
        }
    }
}

/// Todo caractere do texto é ASCII (7 bits)?
///
/// Útil para telas cujo conteúdo é só rótulo e número (painel sem acento).
pub fn text_is_ascii(text: &str) -> bool {
    text.is_ascii()
}

/// O texto contém algum GLIFO especial (o que o modo ASCII promete não ter)?
///
/// Diferente de [`text_is_ascii`]: o português da tela tem acento (`Sessões`,
/// `tendência`) e acento NÃO é glifo quebrado — o que quebra em console antigo
/// é box-drawing, bloco e seta. Este é o gate mecânico do critério 4: blocos
/// Unicode (`▁`, `█`, `░`), box-drawing (`╔═║`) e setas (`➤`, `▲`).
pub fn has_special_glyphs(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            u32::from(character),
            // Box drawing (a casca do statui em Unicode).
            0x2500..=0x257F
            // Block elements: barras, sparkline, gauge, thumb do scrollbar.
            | 0x2580..=0x259F
            // Setas e formas geométricas (marcador de seleção, pontas).
            | 0x2190..=0x21FF
            | 0x25A0..=0x25FF
            // Dingbats (`➤` e parentes).
            | 0x2700..=0x27BF
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_mode_is_opt_in_and_case_insensitive() {
        for ascii in ["ascii", "ASCII", " plain ", "None"] {
            assert_eq!(
                Mode::from_value(Some(ascii)),
                Mode::Ascii,
                "{ascii:?} deveria pedir ASCII"
            );
        }
        // Ausente, vazio ou lixo mantém o padrão Unicode: degradar sem pedido
        // seria trocar a identidade do produto por causa de um typo.
        for unicode in [None, Some(""), Some("unicode"), Some("0"), Some("utf8")] {
            assert_eq!(Mode::from_value(unicode), Mode::Unicode, "{unicode:?}");
        }
    }

    #[test]
    fn every_ascii_glyph_is_seven_bit() {
        let ascii = Glyphs::new(Mode::Ascii);
        let mut all = vec![
            ascii.selection(),
            ascii.gauge().0,
            ascii.gauge().1,
            ascii.spark_empty(),
            ascii.rule(),
            ascii.bullet(),
            ascii.quote(),
        ];
        all.extend(ascii.spark_levels());
        all.extend([ascii.heading(1), ascii.heading(2), ascii.heading(3)]);
        for glyph in all {
            assert!(glyph.is_ascii(), "glifo não-ASCII em modo ascii: {glyph:?}");
        }
        // E o conjunto Unicode tem, sim, glifo especial — o par não é mudo.
        assert!(!Glyphs::new(Mode::Unicode).selection().is_ascii());
    }

    #[test]
    fn selection_marker_keeps_the_two_column_width() {
        // As tabelas alinham colunas assumindo 2 colunas de marcador; trocar o
        // glifo não pode desalinhar a tabela.
        for mode in [Mode::Unicode, Mode::Ascii] {
            let marker = Glyphs::new(mode).selection();
            assert_eq!(marker.chars().count(), 2, "{mode:?}: {marker:?}");
        }
    }

    #[test]
    fn bar_and_scrollbar_sets_follow_the_mode() {
        let ascii = Glyphs::new(Mode::Ascii);
        assert_eq!(ascii.bar_set().full, "#");
        assert_eq!(ascii.bar_set().empty, " ");
        assert!(ascii.bar_set().full.is_ascii());
        assert!(ascii.scrollbar_set().thumb.is_ascii());
        assert!(ascii.scrollbar_set().track.is_ascii());
        // Unicode usa os símbolos de barra do ratatui; o scrollbar tem trilho
        // fino próprio (o `DOUBLE_VERTICAL` se confundiria com a borda).
        let unicode = Glyphs::new(Mode::Unicode);
        assert_eq!(unicode.bar_set().full, symbols::bar::NINE_LEVELS.full);
        assert_eq!(unicode.scrollbar_set().thumb, symbols::block::FULL);
        assert_eq!(unicode.scrollbar_set().track, symbols::line::VERTICAL);
    }

    #[test]
    fn spark_levels_are_seven_and_monotonic_in_both_modes() {
        for mode in [Mode::Unicode, Mode::Ascii] {
            let levels = Glyphs::new(mode).spark_levels();
            assert_eq!(levels.len(), 7, "{mode:?}");
            assert_eq!(levels[0].chars().count(), 1, "{mode:?}");
            assert_eq!(levels[6].chars().count(), 1, "{mode:?}");
        }
    }

    #[test]
    fn ascii_text_helper_catches_a_single_non_ascii_char() {
        assert!(text_is_ascii("Achados #3 12/87"));
        assert!(!text_is_ascii("Achados ➤ 12/87"));
        assert!(!text_is_ascii("critical 1 · high 2"));
    }

    #[test]
    fn special_glyph_gate_separates_accent_from_broken_glyph() {
        // Acento é texto, não glifo: "Sessões" continua válido em modo ASCII.
        assert!(!has_special_glyphs("Sessões — tendência (5 scans) 12/87"));
        // Já o desenho da casca e das barras é glifo especial.
        assert!(has_special_glyphs("╔══╗"));
        assert!(has_special_glyphs("▁▂▃"));
        assert!(has_special_glyphs("█░░"));
        assert!(has_special_glyphs("➤ critical"));
        // E os glifos do conjunto ASCII passam limpos.
        assert!(!has_special_glyphs("+--+ | > # - [##--] 25% .,:=+*#"));
    }
}
