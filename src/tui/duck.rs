//! O pato de borracha (TUI-01): mascote determinístico do dashboard.
//!
//! O que ele É: feedback de atividade real. Os estados saem de eventos que
//! o KIRA observou ([`crate::tui::activity`] — spawn, linhas e exit do
//! filho), e a animação é função pura do tick: o mesmo tick desenha o
//! mesmo frame, a mesma regra do [`crate::tui::spinner`]. O que ele NUNCA
//! é (persona §7): barra de progresso, contador ou spinner eterno — o pato
//! não sabe "quanto falta", porque ninguém sabe.
//!
//! Seed = FNV-1a 64 da raiz do projeto (o MESMO algoritmo do fingerprint
//! do scan, [`crate::store::fnv1a64`]), padrão do companheiro do
//! openclaude (apêndice 10.3 do plano). No MVP o seed decide a expressão
//! do olho e a fase inicial da animação — o `pick(EYES)`/`rollRarity`
//! completo fica como evolução, e o ponto de encaixe é [`Duck::eye`].
//!
//! Arte: um pato só, em caracteres de 7 bits — o MESMO critério do modo
//! ASCII de [`crate::tui::glyph`]: desenho que vira quadradinho em console
//! de fábrica é informação perdida. Por isso existe UM conjunto (e não um
//! por modo): o clássico pato de borracha desenha igual em terminal
//! moderno, SSH antigo e `KIRA_GLYPHS=ascii`; o que muda é a cor
//! ([`crate::tui::theme::Palette::duck`]), nunca o glifo.

/// O estado do pato — SEMPRE dirigido por evento real (ver [`crate::tui::activity`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuckState {
    /// Nenhum filho rodando (ou o último terminou bem).
    #[default]
    Idle,
    /// Julgamento em curso (o `triage` fala com o modelo local).
    Pensando,
    /// O `scan` está interrogando a stack.
    Pesquisando,
    /// Trabalho de montagem (o `pr draft` juntando evidência).
    Trabalhando,
    /// Algo pede atenção (exit ≠ 0 ou critical novo na saída real).
    Alerta,
}

impl DuckState {
    /// Rótulo humano do estado (o mesmo texto do feed — uma só voz).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "em repouso",
            Self::Pensando => "pensando…",
            Self::Pesquisando => "pesquisando stack…",
            Self::Trabalhando => "trabalhando…",
            Self::Alerta => "alerta!",
        }
    }
}

/// FNV-1a 64 da raiz do projeto — o seed do companheiro. Mesmo algoritmo
/// da casa ([`crate::store::fnv1a64`]): mesma entrada, mesmo pato.
#[must_use]
pub fn seed_from_root(root: &str) -> u64 {
    crate::store::fnv1a64(root.as_bytes())
}

/// Olhos possíveis, escolhidos pelo seed (o `pick(EYES)` mínimo).
const EYES: [char; 3] = ['o', 'O', '*'];

/// Olho de piscar (frame com a pálpebra fechada, literal em todo seed).
/// O literal vive na arte; a constante é o nome do glifo nos testes.
#[cfg(test)]
const BLINK_EYE: char = '-';

/// O pato: estado + seed do projeto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duck {
    /// Estado dirigido por evento real.
    pub state: DuckState,
    /// FNV-1a da raiz do projeto (o mesmo para sempre, por projeto).
    pub seed: u64,
}

impl Duck {
    /// Pato em repouso para o projeto dado.
    #[must_use]
    pub fn idle(seed: u64) -> Self {
        Self {
            state: DuckState::Idle,
            seed,
        }
    }

    /// Pato num estado dado.
    #[must_use]
    pub const fn new(state: DuckState, seed: u64) -> Self {
        Self { state, seed }
    }

    /// O olho deste pato — a variante que o seed escolheu.
    #[must_use]
    pub fn eye(self) -> char {
        EYES[(self.seed % EYES.len() as u64) as usize]
    }

    /// Fase inicial da animação (também do seed — o mesmo projeto
    /// começa o piscar no mesmo ponto, para sempre).
    fn phase(self) -> u64 {
        (self.seed >> 32) % 4
    }

    /// As linhas do frame deste tick. Função pura: mesmo
    /// `(estado, seed, tick)` = mesmas linhas, byte a byte.
    #[must_use]
    pub fn art(self, tick: u64) -> Vec<String> {
        let frames = frames(self.state);
        let period = period(self.state);
        // A fase do seed entra no índice: dois projetos de patos
        // diferentes não piscam sincronizados (mesma entrada de tick,
        // porém, dá o mesmo frame — o determinismo é do tick, não do
        // relógio).
        let index = ((tick / period) + self.phase()) % frames.len() as u64;
        let frame = frames[(index as usize) % frames.len()];
        let eye = self.eye().to_string();
        frame.iter().map(|line| line.replace('@', &eye)).collect()
    }
}

/// Frames por estado. `@` = olho (trocado pela variante do seed); frame
/// com `-` literal é pálpebra fechada (o piscar). Toda linha é de 7 bits.
const IDLE_FRAMES: [&[&str]; 2] = [
    &[
        "    __   ",
        " <(@ )___",
        "  ( ._\\  ",
        "   `---' ",
        " ~  ~~~ ~",
    ],
    &[
        "    __   ",
        " <(- )___",
        "  ( ._\\  ",
        "   `---' ",
        "~~~ ~ ~ ~~",
    ],
];

const PENSANDO_FRAMES: [&[&str]; 2] = [
    &[
        " o  __   ",
        " <(@ )___",
        "  ( ._\\  ",
        "   `---' ",
        " ~  ~~~ ~",
    ],
    &[
        "  ? __   ",
        " <(@ )___",
        "  ( ._\\  ",
        "   `---' ",
        " ~~ ~~~ ~",
    ],
];

const PESQUISANDO_FRAMES: [&[&str]; 2] = [
    &[
        "    __   ",
        " <(@ )___",
        "  ( ._\\  ",
        "   `---' ",
        "~~  ~~~~~",
    ],
    &[
        "    __   ",
        " <(O )___",
        "  ( ._\\  ",
        "   `---' ",
        "~~~  ~~~~",
    ],
];

const TRABALHANDO_FRAMES: [&[&str]; 2] = [
    &[
        "    __   ",
        " <(@ )___",
        "  ( /_\\  ",
        "   `---' ",
        " ~~~ ~~ ~",
    ],
    &[
        "    __   ",
        " <(@ )___",
        "  ( \\_/  ",
        "   `---' ",
        " ~~ ~~~ ~",
    ],
];

const ALERTA_FRAMES: [&[&str]; 2] = [
    &[
        " !  __   ",
        " <(O )___",
        "  ( ._\\  ",
        "   `---' ",
        "~~ ~~~~~~",
    ],
    &[
        "    __   ",
        " <(O )___",
        "  ( ._\\  ",
        "   `---' ",
        " ~~~~~~~~",
    ],
];

/// Frames do estado.
const fn frames(state: DuckState) -> &'static [&'static [&'static str]] {
    match state {
        DuckState::Idle => &IDLE_FRAMES,
        DuckState::Pensando => &PENSANDO_FRAMES,
        DuckState::Pesquisando => &PESQUISANDO_FRAMES,
        DuckState::Trabalhando => &TRABALHANDO_FRAMES,
        DuckState::Alerta => &ALERTA_FRAMES,
    }
}

/// Ticks por frame do estado: o repouso é lento (piscar), o trabalho
/// rápido (remar). Nunca por frame de relógio — só tick do loop.
const fn period(state: DuckState) -> u64 {
    match state {
        DuckState::Idle => 12,
        DuckState::Pensando => 6,
        DuckState::Pesquisando => 6,
        DuckState::Trabalhando => 3,
        DuckState::Alerta => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::glyph::has_special_glyphs;

    fn flat(duck: Duck, tick: u64) -> String {
        duck.art(tick).join("\n")
    }

    #[test]
    fn labels_are_the_single_voice_of_the_feed() {
        assert_eq!(DuckState::Idle.label(), "em repouso");
        assert_eq!(DuckState::Pensando.label(), "pensando…");
        assert_eq!(DuckState::Pesquisando.label(), "pesquisando stack…");
        assert_eq!(DuckState::Trabalhando.label(), "trabalhando…");
        assert_eq!(DuckState::Alerta.label(), "alerta!");
    }

    /// O seed é o FNV-1a da casa — valor de referência (a base do offset
    /// do FNV-1a 64 para a string vazia).
    #[test]
    fn seed_is_the_house_fnv1a() {
        assert_eq!(seed_from_root(""), 0xcbf29ce484222325);
        assert_eq!(seed_from_root("a"), 0xaf63dc4c8601ec8c);
        assert_eq!(seed_from_root("C:\\proj"), seed_from_root("C:\\proj"));
    }

    #[test]
    fn the_seed_picks_the_eye() {
        // O `pick(EYES)` mínimo: seeds consecutivos variam o olho, e o
        // MESMO projeto tem sempre o mesmo olho.
        for (seed, expected) in [(0_u64, EYES[0]), (1, EYES[1]), (2, EYES[2])] {
            assert_eq!(Duck::idle(seed).eye(), expected, "seed {seed}");
        }
        assert_eq!(Duck::idle(9).eye(), Duck::idle(9).eye());
        // E o olho escolhido APARECE no frame (a variante não é decorativa).
        assert!(flat(Duck::idle(1), 0).contains('O'));
    }

    #[test]
    fn same_tick_same_frame_and_the_cycle_wraps() {
        let duck = Duck::idle(7);
        for tick in [0_u64, 5, 11, 12, 999] {
            assert_eq!(flat(duck, tick), flat(duck, tick), "tick {tick}");
        }
        // O ciclo fecha: o tick `n + períodos × frames` volta ao início.
        assert_eq!(flat(duck, 0), flat(duck, 12 * IDLE_FRAMES.len() as u64));
    }

    /// O piscar existe: algum tick do Idle desenha a pálpebra fechada.
    #[test]
    fn idle_blinks() {
        let duck = Duck::idle(7);
        assert!(flat(duck, 0).contains(duck.eye()), "olho aberto");
        assert!(flat(duck, 12).contains(BLINK_EYE), "piscou no frame 2");
    }

    /// O estado muda o desenho: o alerta tem o `!` (em algum frame) e o
    /// pensando tem a bolha `?`.
    #[test]
    fn states_have_their_mark() {
        let alert = flat(Duck::new(DuckState::Alerta, 7), 0);
        assert!(alert.contains('!'), "{alert}");
        // A bolha do pensamento alterna os dois frames (o primeiro frame
        // mostra a bolha pequena `o`, o segundo a `?` maior).
        let small = flat(Duck::new(DuckState::Pensando, 7), 0);
        let big = flat(
            Duck::new(DuckState::Pensando, 7),
            period(DuckState::Pensando),
        );
        assert!(small.contains('?') || big.contains('?'), "{small}");
    }

    /// Gate do modo ASCII na prática: TODO frame de TODO estado é de 7
    /// bits — o pato é o desenho que não vira quadradinho em console
    /// de fábrica.
    #[test]
    fn every_frame_is_seven_bit_art() {
        for state in [
            DuckState::Idle,
            DuckState::Pensando,
            DuckState::Pesquisando,
            DuckState::Trabalhando,
            DuckState::Alerta,
        ] {
            for seed in [0_u64, 1, 2, 0xdead_beef] {
                for tick in [0_u64, 3, 12, 45] {
                    let art = flat(Duck::new(state, seed), tick);
                    assert!(art.is_ascii(), "{state:?} seed {seed} tick {tick}: {art}");
                    assert!(
                        !has_special_glyphs(&art),
                        "{state:?} tem glifo especial: {art}"
                    );
                }
            }
        }
    }

    /// O silhueta tem cara de pato: bico `<(`, olho, corpo e a linha
    /// d'água — a arte é literal, não gerada.
    #[test]
    fn the_classic_silhouette_is_intact() {
        let art = flat(Duck::idle(7), 0);
        assert!(art.contains("<("), "bico: {art}");
        assert!(art.contains("___"), "corpo: {art}");
        assert!(art.contains('~'), "água: {art}");
    }
}
