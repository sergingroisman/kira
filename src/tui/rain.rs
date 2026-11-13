//! Chuva de fundo do dashboard — decoração PURA, nunca dado (ADR-0036).
//!
//! Conceito: `tui-rain` (MIT, Levilutz) — nenhum trecho de código copiado, e
//! nenhuma dependência nova: o que o nativo do ratatui não faz bem vira módulo
//! nosso, a mesma regra que já criou `tui/glyph` e `tui/viewport`. Chuva são
//! ~200 linhas de escrita em buffer; trazer `rand` + `rand_pcg` para isso
//! seria trocar determinismo por peso.
//!
//! Porque é módulo e não widget: chuva é ESTADO de desenho (gotas com posição
//! e velocidade) que anda um passo por volta do loop, igual ao spinner — o
//! `tick` no [`crate::tui::app::AppState`] garante "mesma semente + mesmo
//! número de passos = mesmo frame", e é isso que deixa o efeito testável sem
//! terminal de verdade.
//!
//! Dois contratos inegociáveis, herdados do plano da onda M:
//! - **dado na tela ⇒ zero decoração**: a chuva espera os painéis desenharem
//!   e PINTA SÓ CÉLULA EM BRANCO. Uma borda, um `—` ou um byte de texto nunca
//!   são cobertos; quando o painel tem conteúdo, o modo nem chega ligado.
//! - **nunca em artefato**: o `--snapshot` de texto não conhece este módulo,
//!   e o modo degradado (<80×24) força [`RainMode::Off`]. A prova de hash
//!   byte-idêntico do dashboard continua valendo exatamente como estava.
//!
//! A matemática é inteira (mili-linhas, nunca `f64`): o `AppState` deriva
//! `Eq`, e ponto flutuante em estado de tela é convite para teste instável.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::tui::glyph::Glyphs;
use crate::tui::theme::Palette;

/// Compasso do avanço: o MESMO `poll` de 150 ms do loop de eventos — nenhum
/// timer novo, nenhum custo de CPU ocioso além do que o dashboard já tinha.
pub const TICK_MS: u64 = 150;

/// Quando a chuva aparece. A escolha é uma função pura do estado da tela,
/// feita no loop — nunca aqui dentro (módulo não conhece `Snapshot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RainMode {
    /// Painel com dado na tela, ou snapshot/degradado: zero decoração.
    #[default]
    Off,
    /// Painel em foco vazio: "KIRA esperando dados" com chuva finíssima.
    Waiting,
    /// Fundo sutil global pedido na tecla `~` (default desligado, só em
    /// memória — nada vai para `config.toml`).
    Ambient,
}

/// Unidades de posição: linhas em MILI (1 linha = 1000) — inteira, exata,
/// reproduzível byte a byte em qualquer plataforma.
const MILLI: i64 = 1000;

/// Uma gota: coluna fixa, cabeça caindo a velocidade própria, rabo atrás.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Drop {
    /// Coluna dentro da área (0-based).
    column: u16,
    /// Posição da cabeça em mili-linhas; negativa = ainda entrando pela borda.
    head: i64,
    /// Velocidade em mili-linhas por segundo (5–15 linhas/s).
    speed: i64,
    /// Comprimento do rabo em células (1–4).
    tail: i64,
}

/// Estado de desenho da chuva. Vive no `AppState`, muda só no loop (igual ao
/// `tick`), e é `Clone + PartialEq + Eq` para o `AppState` continuar derivável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rain {
    /// Semente da sessão: trocar o modo reinicia o gerador a partir dela, e
    /// assim o frame N depois de um `set_mode` é sempre o mesmo frame N.
    seed: u64,
    /// Estado do splitmix64 (PRNG próprio, sem dependência — o mesmo estilo
    /// do FNV-1a64 que já batiza posts e fingerprints).
    rng: u64,
    mode: RainMode,
    drops: Vec<Drop>,
}

impl Default for Rain {
    fn default() -> Self {
        Self::seeded(0)
    }
}

impl Rain {
    /// Chuva desligada, gerador ancorado na semente dada (o loop semeia com
    /// FNV sobre o `now` do snapshot; testes usam um número fixo).
    pub const fn seeded(seed: u64) -> Self {
        Self {
            seed,
            rng: seed,
            mode: RainMode::Off,
            drops: Vec::new(),
        }
    }

    /// Modo atual (testes e o render usam como única fonte de verdade).
    pub const fn mode(&self) -> RainMode {
        self.mode
    }

    /// Liga/desliga; TROCA de modo reinicia gotas e gerador — a sequência
    /// "mesma semente, mesmos passos" vale inclusive através de um `~`.
    pub fn set_mode(&mut self, mode: RainMode) {
        if self.mode != mode {
            self.mode = mode;
            self.rng = self.seed;
            self.drops.clear();
        }
    }

    /// Número de gotas para a densidade do modo numa área — puramente
    /// derivado da largura, para o teste poder comparar Waiting < Ambient.
    fn target_count(mode: RainMode, area: Rect) -> usize {
        if area.width == 0 || area.height == 0 {
            return 0;
        }
        match mode {
            RainMode::Off => 0,
            // Finíssima: uma gota a cada ~40 colunas.
            RainMode::Waiting => usize::from(area.width / 40) + 1,
            // Sutil: uma a cada ~14 colunas, no máximo duas.
            RainMode::Ambient => (usize::from(area.width / 14)).max(2),
        }
    }

    /// Avança um passo de `dt_ms`. Fora de área nunca faz nada: degradado e
    /// snapshot continuam provando que aqui não há efeito nenhum.
    pub fn advance(&mut self, area: Rect, dt_ms: u64) {
        if self.mode == RainMode::Off || area.width == 0 || area.height == 0 {
            return;
        }
        let target = Self::target_count(self.mode, area);
        for _ in self.drops.len()..target {
            let fresh = self.spawn(area);
            self.drops.push(fresh);
        }
        self.drops.truncate(target);
        // A reciclagem pede o gerador (`self`) no meio do laço sobre as gotas
        // (`self.drops`): tira o vec, mexe, devolve — o `Drop` é `Copy`.
        let mut drops = std::mem::take(&mut self.drops);
        for drop in &mut drops {
            drop.head += drop.speed * i64::try_from(dt_ms).unwrap_or(150) / 1000;
            if drop.head - drop.tail * MILLI > i64::from(area.height) * MILLI {
                *drop = self.spawn(area);
            }
        }
        self.drops = drops;
    }

    /// Cria uma gota acima da borda, com coluna/velocidade/rabo do gerador.
    fn spawn(&mut self, area: Rect) -> Drop {
        let width = u64::from(area.width.max(1));
        Drop {
            column: (self.next_u64() % width) as u16,
            head: -i64::try_from(self.next_u64() % (2 * MILLI as u64)).unwrap_or(1) - MILLI,
            speed: 5_000 + (self.next_u64() % 10_001) as i64,
            tail: 1 + (self.next_u64() % 4) as i64,
        }
    }

    /// splitmix64: um número por chamada, determinístico pela semente.
    fn next_u64(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Células do frame atual, na ordem das gotas (estável): posição
    /// `(coluna, linha)` e se é a cabeça. Puro — é isto que os testes
    /// reproduzem com semente fixa, sem terminal nenhum.
    #[must_use]
    pub fn drop_cells(&self, area: Rect) -> Vec<(u16, u16, bool)> {
        if self.mode == RainMode::Off {
            return Vec::new();
        }
        let mut cells = Vec::new();
        for drop in &self.drops {
            if drop.column >= area.width {
                continue;
            }
            for offset in 0..=drop.tail {
                let row = (drop.head - offset * MILLI).div_euclid(MILLI);
                if row < 0 {
                    // cabeça ainda fora da área ⇒ rabo inteiro está fora;
                    // `div_euclid` já cuida do arredondamento negativo.
                    break;
                }
                if let Ok(row) = u16::try_from(row)
                    && row < area.height
                {
                    cells.push((drop.column, row, offset == 0));
                }
            }
        }
        cells
    }

    /// Pinta no buffer DEPOIS de todos os painéis, e só onde a célula está em
    /// branco. A checagem do branco é o contrato "dado vence decoração"
    /// escrito em uma linha de código — não em uma promessa de teste.
    pub fn render(&self, glyphs: &Glyphs, palette: Palette, area: Rect, buf: &mut Buffer) {
        if self.mode == RainMode::Off {
            return;
        }
        let (head_glyph, tail_glyph) = glyphs.rain();
        let head_style = Style::default().fg(palette.border_idle);
        let tail_style = Style::default()
            .fg(palette.border_idle)
            .add_modifier(Modifier::DIM);
        for (column, row, is_head) in self.drop_cells(area) {
            // `drop_cells` é relativa à área; o buffer do frame é a tela
            // inteira — a origem da área é o que converte um no outro.
            let cell = &mut buf[(area.x + column, area.y + row)];
            if cell.symbol() != " " {
                continue;
            }
            cell.set_symbol(if is_head { head_glyph } else { tail_glyph });
            cell.set_style(if is_head { head_style } else { tail_style });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::{Glyphs, Mode, has_special_glyphs};
    use crate::tui::theme;
    use ratatui::buffer::Buffer;

    fn area() -> Rect {
        Rect::new(0, 0, 80, 20)
    }

    fn walk(seed: u64, mode: RainMode, steps: usize, dt: u64) -> Rain {
        let mut rain = Rain::seeded(seed);
        rain.set_mode(mode);
        for _ in 0..steps {
            rain.advance(area(), dt);
        }
        rain
    }

    #[test]
    fn same_seed_same_frame_sequence() {
        // O contrato de teste do plano: semente fixa + passos fixos ⇒ frames
        // idênticos, sem terminal, sem relógio, sem `rand`.
        let a = walk(7, RainMode::Ambient, 40, TICK_MS);
        let b = walk(7, RainMode::Ambient, 40, TICK_MS);
        assert_eq!(a.drop_cells(area()), b.drop_cells(area()));
        assert!(!a.drop_cells(area()).is_empty(), "40 passos já choveram");
        // Semente diferente, outra chuva — o gerador não é constante.
        let c = walk(8, RainMode::Ambient, 40, TICK_MS);
        assert_ne!(a.drop_cells(area()), c.drop_cells(area()));
    }

    #[test]
    fn mode_change_restarts_from_the_same_seeded_frame_zero() {
        // Trocar de modo zera gerador e gotas: 3 passos de `Waiting` seguidos
        // de 5 de `Ambient` valem os mesmos 5 de `Ambient` puro.
        let direct = walk(42, RainMode::Ambient, 5, TICK_MS);
        let mut indirect = Rain::seeded(42);
        indirect.set_mode(RainMode::Waiting);
        indirect.advance(area(), TICK_MS);
        indirect.advance(area(), TICK_MS);
        indirect.advance(area(), TICK_MS);
        indirect.set_mode(RainMode::Ambient);
        for _ in 0..5 {
            indirect.advance(area(), TICK_MS);
        }
        assert_eq!(direct.drop_cells(area()), indirect.drop_cells(area()));
    }

    #[test]
    fn off_mode_never_draws_anything() {
        let mut rain = Rain::seeded(7);
        for _ in 0..50 {
            rain.advance(area(), TICK_MS);
        }
        assert!(rain.drop_cells(area()).is_empty());
        assert_eq!(rain.mode(), RainMode::Off);
    }

    #[test]
    fn drops_never_leave_the_area() {
        for mode in [RainMode::Waiting, RainMode::Ambient] {
            for steps in 0..200 {
                let rain = walk(3, mode, steps, TICK_MS);
                for (column, row, _) in rain.drop_cells(area()) {
                    assert!(column < 80 && row < 20, "{mode:?} passo {steps}");
                }
            }
        }
    }

    #[test]
    fn drops_keep_falling_and_recycle_instead_of_freezing() {
        let mut rain = Rain::seeded(11);
        rain.set_mode(RainMode::Ambient);
        // Deixa chover: as primeiras gotas nascem FORA da área (caindo de
        // cima) e um único passo ainda não garantiria célula visível.
        for _ in 0..20 {
            rain.advance(area(), TICK_MS);
        }
        let first = rain.drop_cells(area());
        assert!(!first.is_empty(), "20 passos e nada na tela?");
        rain.advance(area(), TICK_MS);
        assert_ne!(first, rain.drop_cells(area()), "o frame anda");
        // Depois de cair muito, as gotas reaparecem lá em cima (reciclagem),
        // nunca somem para sempre.
        for _ in 0..600 {
            rain.advance(area(), TICK_MS);
        }
        assert!(!rain.drop_cells(area()).is_empty());
    }

    #[test]
    fn waiting_is_sparser_than_ambient_for_the_same_seed() {
        let waiting = walk(5, RainMode::Waiting, 20, TICK_MS).drop_cells(area());
        let ambient = walk(5, RainMode::Ambient, 20, TICK_MS).drop_cells(area());
        assert!(
            waiting.len() < ambient.len(),
            "finíssima < sutil: {waiting:?} vs {ambient:?}"
        );
        assert!(!waiting.is_empty(), "esperando também chove um pouco");
    }

    #[test]
    fn zero_sized_area_never_produces_cells() {
        let mut rain = Rain::seeded(9);
        rain.set_mode(RainMode::Ambient);
        rain.advance(Rect::new(0, 0, 80, 0), TICK_MS);
        assert!(rain.drop_cells(Rect::new(0, 0, 80, 0)).is_empty());
        rain.advance(Rect::new(0, 0, 0, 20), TICK_MS);
        assert!(rain.drop_cells(Rect::new(0, 0, 0, 20)).is_empty());
    }

    #[test]
    fn render_paints_only_blank_cells_and_never_touches_data() {
        // Área de UMA coluna: toda gota nasce na coluna 0, e o teste sabe
        // exatamente onde a cabeça cai.
        let area = Rect::new(0, 0, 1, 4);
        let mut buf = Buffer::empty(area);
        buf[(0, 1)].set_symbol("X");
        buf[(0, 1)].set_style(Style::default());
        let mut rain = Rain::seeded(21);
        rain.set_mode(RainMode::Ambient);
        for _ in 0..60 {
            rain.advance(area, TICK_MS);
            rain.render(&Glyphs::default(), theme::mono(), area, &mut buf);
        }
        // O dado sobreviveu a 60 frames de chuva.
        assert_eq!(buf[(0, 1)].symbol(), "X");
        // E a célula vazia molhou em algum momento.
        assert!(
            [0usize, 2, 3]
                .iter()
                .any(|y| buf[(0, *y as u16)].symbol() != " "),
            "chuva não desenhou nada em 60 frames"
        );
    }

    #[test]
    fn ascii_rain_is_seven_bit_and_unicode_rain_is_the_pair() {
        let ascii = Glyphs::new(Mode::Ascii).rain();
        assert!(ascii.0.is_ascii() && ascii.1.is_ascii(), "{ascii:?}");
        let unicode = Glyphs::new(Mode::Unicode).rain();
        assert!(
            has_special_glyphs(unicode.0),
            "o par Unicode tem glifo de casca"
        );
        // As MESMAS posições nos dois conjuntos: a ASCII não muda a física.
        for mode in [Mode::Unicode, Mode::Ascii] {
            let mut buf = Buffer::empty(area());
            let rain = walk(2, RainMode::Ambient, 30, TICK_MS);
            rain.render(&Glyphs::new(mode), theme::mono(), area(), &mut buf);
            let mut text = String::new();
            for y in 0..area().height {
                for x in 0..area().width {
                    text.push_str(buf[(x, y)].symbol());
                }
            }
            if mode == Mode::Ascii {
                assert!(!has_special_glyphs(&text), "chuva vazou glifo: {text}");
            }
        }
    }
}
