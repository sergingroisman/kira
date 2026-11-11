//! Paleta do dashboard, com fallback mono obrigatório.
//!
//! ADR-0014 pede paleta própria ("fofinho/colorido") com **a mesma informação
//! em texto** quando `NO_COLOR=1` ou o terminal não tem cor: aqui a troca é
//! feita apenas trocando as cores por `Color::Reset` (padrão do terminal), sem
//! tocar no conteúdo das linhas. Nada de degradar informação — persona §7.

use ratatui::style::Color;

/// Uma paleta: só as cores que a casca usa hoje (W6-PR1 = cabeçalho + rodapé).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Título do painel (nome do projeto).
    pub title: Color,
    /// Borda do painel em foco.
    pub border_focus: Color,
    /// Borda de painel fora de foco.
    pub border_idle: Color,
    /// Valores presentes (dado real do disco).
    pub value: Color,
    /// Valores ausentes (`—` + causa): destaque honesto, não erro gritado.
    pub missing: Color,
    /// Rodapé de atalhos.
    pub footer: Color,
    /// Overlay de ajuda.
    pub help: Color,
    /// Severidade critical (painel de achados, W6-PR2).
    pub critical: Color,
    /// Severidade high.
    pub high: Color,
    /// Severidade medium.
    pub medium: Color,
    /// Severidade low.
    pub low: Color,
}

/// Paleta colorida do KIRA (verde/roxo, tom fofinho do mascote futuro).
pub const fn colorful() -> Palette {
    Palette {
        title: Color::Cyan,
        border_focus: Color::Green,
        border_idle: Color::DarkGray,
        value: Color::Green,
        missing: Color::Yellow,
        footer: Color::DarkGray,
        help: Color::Magenta,
        // Semântica de severidade (semáforo): quanto pior, mais quente.
        critical: Color::Red,
        high: Color::Yellow,
        medium: Color::Cyan,
        low: Color::DarkGray,
    }
}

/// Paleta mono: `Color::Reset` em tudo = a cor que o terminal já tem. A
/// informação é a mesma linha por linha.
pub const fn mono() -> Palette {
    Palette {
        title: Color::Reset,
        border_focus: Color::Reset,
        border_idle: Color::Reset,
        value: Color::Reset,
        missing: Color::Reset,
        footer: Color::Reset,
        help: Color::Reset,
        // Mono não perde informação: a severidade segue legível no texto da
        // célula (a cor era só semântica visual, nunca dado).
        critical: Color::Reset,
        high: Color::Reset,
        medium: Color::Reset,
        low: Color::Reset,
    }
}

/// Função pura: decide cor dado o `NO_COLOR` e o `TERM` já lidos.
///
/// Regra (https://no-color.org + honestidade da casca): `NO_COLOR` definido
/// mesmo vazio vence tudo; `TERM=dumb` é o segundo sinal de terminal sem cor.
/// Receber os valores como parâmetros é o que torna o fallback unit-testável
/// sem mutar env no teste (em Rust 2024, `set_var` é `unsafe`).
pub fn color_enabled(no_color: Option<&str>, term: Option<&str>) -> bool {
    if no_color.is_some() {
        return false;
    }
    !term.is_some_and(|term| term.trim().eq_ignore_ascii_case("dumb"))
}

/// Paleta para a sessão atual: lê `NO_COLOR`/`TERM` e decide uma vez.
pub fn palette_from_env() -> Palette {
    let no_color = std::env::var("NO_COLOR").ok();
    let term = std::env::var("TERM").ok();
    if color_enabled(no_color.as_deref(), term.as_deref()) {
        colorful()
    } else {
        mono()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_always_wins_even_when_empty() {
        // "definida" vale mesmo sem valor: é o contrato no-color.org.
        assert!(!color_enabled(Some(""), Some("xterm-256color")));
        assert!(!color_enabled(Some("1"), None));
    }

    #[test]
    fn dumb_terminal_falls_back_to_mono() {
        assert!(!color_enabled(None, Some("dumb")));
        assert!(!color_enabled(None, Some("DUMB")), "case-insensitive");
    }

    #[test]
    fn normal_terminal_is_colored() {
        assert!(color_enabled(None, Some("xterm-256color")));
        assert!(color_enabled(None, None));
    }

    #[test]
    fn mono_palette_uses_reset_for_everything() {
        let mono = mono();
        assert_eq!(mono.title, Color::Reset);
        assert_eq!(mono.missing, Color::Reset);
        assert_eq!(mono.footer, Color::Reset);
        // E a colorida tem cor de verdade, para o par nunca virar mono-mudo.
        assert_ne!(colorful().title, Color::Reset);
    }
}
