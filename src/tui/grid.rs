//! Geometria dos cartões do Resumo (PR-UX2) — aritmética PURA sobre `Rect`.
//!
//! Por que módulo e não conta dentro do widget: o critério do dono é "em 80×24
//! o grid fecha exatamente as 21 linhas do workspace, sem buraco e sem
//! overflow". Isso é afirmação sobre NÚMEROS, não sobre desenho — aqui a conta
//! é testável sem terminal, e o widget só pinta o que sai daqui. A divisão do
//! chrome continua em [`crate::tui::layout::shell`]: este módulo recebe o
//! workspace já recortado.
//!
//! Nada de `Layout` do ratatui de propósito: constraint percentual resolve
//! sobra por arredondamento e esconde o caso "não coube". Aqui a sobra é
//! EXPLÍCITA — ou vira altura do último cartão, ou o cartão entra em `omitted`.

use ratatui::layout::Rect;

/// Largura a partir da qual o Resumo usa duas colunas de cartão.
///
/// 100 = 2 × 50, o mínimo em que duas barras de severidade ainda comparam lado
/// a lado sem espremer o rótulo.
pub const TWO_COLUMNS_MIN_WIDTH: u16 = 100;

/// Cartão do Resumo, na ordem de prioridade do dono: ação primeiro (Risco,
/// Sessões), contexto depois (Projeto, Atividade), registro no fim
/// (Índice/Wiki, Pendências).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Risco,
    Sessoes,
    Projeto,
    Atividade,
    IndiceWiki,
    Pendencias,
}

impl CardKind {
    /// Título desenhado na borda do cartão.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Risco => "Risco",
            Self::Sessoes => "Sessões",
            Self::Projeto => "Projeto",
            Self::Atividade => "Atividade",
            Self::IndiceWiki => "Índice e wiki",
            Self::Pendencias => "Pendências",
        }
    }

    /// Altura mínima legível, CONTANDO as duas bordas (topo + conteúdo + base).
    ///
    /// Na coluna única a soma tem de dar exatamente as 21 linhas do workspace
    /// de 80×24: 6 + 7 + 4 + 4. É o critério de aceite do PR-UX2 — mudar
    /// qualquer número aqui quebra o teste do fit, que é o alarme desejado.
    pub const fn min_rows(self) -> u16 {
        match self {
            Self::Risco => 6,
            Self::Sessoes => 7,
            Self::Projeto => 4,
            Self::Atividade => 8,
            Self::IndiceWiki => 6,
            Self::Pendencias => 4,
        }
    }

    /// Altura mínima da LINHA que contém este cartão (com dois por linha, vale
    /// o mais alto: a borda de baixo é a mesma).
    const fn row_rows(self) -> u16 {
        self.min_rows()
    }
}

/// Coluna única (abaixo de [`TWO_COLUMNS_MIN_WIDTH`]): quatro cartões.
pub const NARROW: [CardKind; 4] = [
    CardKind::Risco,
    CardKind::Sessoes,
    CardKind::Projeto,
    CardKind::Pendencias,
];

/// Duas colunas, em ordem de LEITURA (linha 1, depois 2, depois 3).
pub const WIDE: [CardKind; 6] = [
    CardKind::Risco,
    CardKind::Sessoes,
    CardKind::Projeto,
    CardKind::Atividade,
    CardKind::IndiceWiki,
    CardKind::Pendencias,
];

/// Um cartão posicionado no workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub kind: CardKind,
    pub area: Rect,
}

/// Resultado do grid: o que coube (ordem de leitura) e o que saiu por falta de
/// espaço. `omitted` existe para nada sumir em silêncio — quem chama pode dizer
/// na tela onde o dado continua (`?`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Planned {
    pub slots: Vec<Slot>,
    pub omitted: Vec<CardKind>,
}

/// Quantas colunas cabem nesta largura.
pub fn columns(area: Rect) -> usize {
    if area.width >= TWO_COLUMNS_MIN_WIDTH {
        2
    } else {
        1
    }
}

/// Cartões da largura: quatro na coluna única, seis no layout largo.
pub fn kinds_for(area: Rect) -> &'static [CardKind] {
    if columns(area) == 2 { &WIDE } else { &NARROW }
}

/// Planeja o workspace inteiro: colunas pela largura, alturas por prioridade.
pub fn plan(area: Rect) -> Planned {
    plan_kinds(area, kinds_for(area))
}

/// Planeja uma lista explícita de cartões (testes e telas futuras).
///
/// Regras, nesta ordem:
/// 1. as linhas são os cartões em blocos de `columns` (ordem de leitura);
/// 2. a altura da linha é a do cartão mais alto dela;
/// 3. sobra de espaço vai da última linha para cima, uma linha por vez;
/// 4. falta de espaço corta a partir da PRIMEIRA linha que não cabe inteira —
///    cartão pela metade é pior que cartão ausente, e o resto vai para
///    `omitted`.
pub fn plan_kinds(area: Rect, kinds: &[CardKind]) -> Planned {
    let cols = columns(area);
    let mut planned = Planned::default();
    if area.height == 0 || area.width == 0 || kinds.is_empty() {
        planned.omitted.extend_from_slice(kinds);
        return planned;
    }

    // 1 + 2: linhas e suas alturas mínimas.
    let mut rows: Vec<Vec<CardKind>> = kinds.chunks(cols).map(<[CardKind]>::to_vec).collect();
    let mut heights: Vec<u16> = rows
        .iter()
        .map(|row| row.iter().map(|kind| kind.row_rows()).max().unwrap_or(0))
        .collect();

    // 4: corta a partir da primeira linha que não cabe inteira.
    let mut used: u16 = 0;
    let mut row_count = 0;
    for (index, height) in heights.iter().enumerate() {
        let fits = used.saturating_add(*height) <= area.height;
        if !fits {
            break;
        }
        used = used.saturating_add(*height);
        row_count = index + 1;
    }
    for row in rows.iter().skip(row_count) {
        planned.omitted.extend(row.iter().copied());
    }
    rows.truncate(row_count);
    heights.truncate(row_count);

    // 3: sobra vai da última linha para cima (nunca sobra buraco no fim).
    let mut leftover = area.height.saturating_sub(used);
    while leftover > 0 && !heights.is_empty() {
        for height in heights.iter_mut().rev() {
            if leftover == 0 {
                break;
            }
            *height = height.saturating_add(1);
            leftover -= 1;
        }
    }

    // Posiciona: colunas iguais, com o resto de largura ímpar na direita.
    let left_width = if cols == 2 {
        area.width / 2
    } else {
        area.width
    };
    let right_width = area.width.saturating_sub(left_width);
    let mut y = area.y;
    for (row, height) in rows.iter().zip(heights.iter()) {
        for (index, kind) in row.iter().enumerate() {
            let (x, width) = if index == 0 {
                (area.x, left_width)
            } else {
                (area.x.saturating_add(left_width), right_width)
            };
            planned.slots.push(Slot {
                kind: *kind,
                area: Rect {
                    x,
                    y,
                    width,
                    height: *height,
                },
            });
        }
        y = y.saturating_add(*height);
    }
    planned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::layout;

    fn workspace_80x24() -> Rect {
        layout::shell(Rect::new(0, 0, 80, 24)).workspace
    }

    /// Critério de aceite do PR-UX2: 80×24 fecha EXATAMENTE as 21 linhas do
    /// workspace — sem buraco no fim e sem passar da borda.
    #[test]
    fn eighty_by_twenty_four_stacks_four_cards_without_overflow() {
        let workspace = workspace_80x24();
        assert_eq!(workspace.height, 21, "workspace de 24 linhas");
        let planned = plan(workspace);

        assert!(planned.omitted.is_empty(), "nada pode sobrar em 80×24");
        assert_eq!(planned.slots.len(), 4);
        let kinds: Vec<CardKind> = planned.slots.iter().map(|slot| slot.kind).collect();
        assert_eq!(kinds, NARROW.to_vec(), "ordem de prioridade do dono");

        // Sem buraco: cada cartão começa onde o anterior terminou…
        let mut y = workspace.y;
        for slot in &planned.slots {
            assert_eq!(slot.area.y, y, "buraco antes de {:?}", slot.kind);
            assert_eq!(slot.area.x, workspace.x);
            assert_eq!(slot.area.width, workspace.width);
            assert!(slot.area.height >= slot.kind.min_rows());
            y = y.saturating_add(slot.area.height);
        }
        // …e o último fecha na borda de baixo.
        assert_eq!(y, workspace.y + 21, "o grid tem de fechar as 21 linhas");
        assert_eq!(y, workspace.bottom());
    }

    #[test]
    fn wide_terminal_uses_two_columns_and_fills_the_leftover() {
        let workspace = layout::shell(Rect::new(0, 0, 120, 30)).workspace;
        // Chrome de 3 linhas fixas (status, abas, rodapé) ⇒ 27 de workspace.
        assert_eq!(workspace.height, 27);
        let planned = plan(workspace);

        assert!(planned.omitted.is_empty());
        assert_eq!(planned.slots.len(), 6);
        let kinds: Vec<CardKind> = planned.slots.iter().map(|slot| slot.kind).collect();
        assert_eq!(kinds, WIDE.to_vec());

        // Duas colunas de 60, alinhadas por linha.
        let riscos = planned.slots[0].area;
        let sessoes = planned.slots[1].area;
        assert_eq!((riscos.x, riscos.width), (0, 60));
        assert_eq!((sessoes.x, sessoes.width), (60, 60));
        assert_eq!(riscos.y, sessoes.y, "linha 1 alinhada");
        assert_eq!(riscos.height, sessoes.height);

        // A sobra vai para as linhas, não vira buraco no fim.
        let bottom = planned.slots[1].area.y + planned.slots[1].area.height;
        let second_row = planned.slots[2].area.y;
        assert_eq!(second_row, bottom);
        let last = planned.slots[5].area;
        assert_eq!(last.y + last.height, workspace.bottom());
    }

    #[test]
    fn cards_that_do_not_fit_are_reported_not_dropped() {
        // 10 linhas: só o Risco (6) cabe inteiro; Sessões (7) não entra.
        let workspace = Rect::new(0, 0, 80, 10);
        let planned = plan(workspace);

        assert_eq!(planned.slots.len(), 1);
        assert_eq!(planned.slots[0].kind, CardKind::Risco);
        assert_eq!(
            planned.omitted,
            vec![CardKind::Sessoes, CardKind::Projeto, CardKind::Pendencias],
            "cartão pela metade é pior que cartão ausente"
        );
        // O espaço dos que saíram vira altura do que ficou: nada de buraco.
        assert_eq!(planned.slots[0].area.height, workspace.height);
        assert!(planned.slots[0].area.height <= workspace.height);
    }

    #[test]
    fn tiny_area_returns_no_slot_and_never_panics() {
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 79, 0),
            Rect::new(0, 0, 0, 24),
            Rect::new(0, 0, 1, 1),
        ] {
            let planned = plan(area);
            let drawn: u16 = planned.slots.iter().map(|slot| slot.area.height).sum();
            assert!(drawn <= area.height, "{area:?} estourou: {planned:?}");
        }
        let none = plan(Rect::new(0, 0, 80, 5));
        assert!(none.slots.is_empty(), "nada cabe em 5 linhas");
        assert_eq!(none.omitted.len(), NARROW.len());
    }

    #[test]
    fn leftover_height_grows_the_last_cards_instead_of_leaving_a_hole() {
        let workspace = Rect::new(0, 0, 80, 30);
        let planned = plan(workspace);
        let last = *planned
            .slots
            .last()
            .unwrap_or_else(|| panic!("tem cartões"));
        assert_eq!(last.area.y + last.area.height, workspace.bottom());
        assert!(
            last.area.height > last.kind.min_rows(),
            "a sobra vai para o fim: {:?}",
            last.area
        );
        assert!(planned.omitted.is_empty());
    }

    #[test]
    fn two_columns_start_at_the_documented_width() {
        assert_eq!(columns(Rect::new(0, 0, 99, 30)), 1);
        assert_eq!(columns(Rect::new(0, 0, 100, 30)), 2);
        // Largura ímpar: a coluna direita fica com o resto, nunca a esquerda.
        let planned = plan(Rect::new(0, 0, 101, 30));
        assert_eq!(planned.slots[0].area.width, 50);
        assert_eq!(planned.slots[1].area.width, 51);
        assert_eq!(planned.slots[1].area.x, 50);
    }
}
