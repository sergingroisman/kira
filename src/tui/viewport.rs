//! Viewport das listas: qual fatia da lista cabe na área e onde o cursor está.
//!
//! Antes disto, os três painéis (Achados, Sessões, Wiki) desenhavam sempre a
//! partir da linha 0 e cortavam o excedente. O efeito era um defeito real de
//! uso: com `j` além da altura visível a seleção saía da tela e o dono ficava
//! navegando às cegas — o `REVERSED` existia, mas fora do buffer visível.
//!
//! Tudo aqui é função PURA sobre números (nada de `Rect`, nada de terminal),
//! porque é isso que torna a rolagem testável sem TUI: `keep_visible` é o
//! contrato, e os widgets só aplicam o resultado.

/// Primeira linha que deve aparecer para manter `row` visível.
///
/// Regra única: o cursor nunca sai da área. Se ele já está dentro da janela, a
/// janela fica parada (rolar sem necessidade desorienta); se ele passou por
/// baixo, a janela desce o mínimo; se passou por cima, sobe o mínimo. A janela
/// também nunca começa depois do fim da lista, para não abrir um vazio no rodapé.
pub fn keep_visible(row: usize, visible: usize, total: usize) -> usize {
    if visible == 0 || total == 0 {
        return 0;
    }
    let max_offset = total.saturating_sub(visible);
    if row < visible {
        // Cabe na primeira janela: nada a rolar.
        return 0;
    }
    let offset = row + 1 - visible;
    offset.min(max_offset)
}

/// Fatia visível `(início, fim exclusivo)` para desenhar.
pub fn slice(offset: usize, visible: usize, total: usize) -> (usize, usize) {
    let start = offset.min(total);
    let end = start.saturating_add(visible).min(total);
    (start, end)
}

/// Rótulo de posição (`12/87`) quando a lista NÃO cabe inteira.
///
/// `None` quando tudo já está na tela: mostrar `1/4` numa lista de quatro
/// linhas é ruído — o rodapé e o título já dizem quantos itens existem.
pub fn position_label(offset: usize, visible: usize, total: usize) -> Option<String> {
    if total == 0 || visible == 0 || total <= visible {
        return None;
    }
    Some(format!("{}/{}", offset.min(total - 1) + 1, total))
}

/// Coloca `label` alinhado à direita de `line`, sem apagar conteúdo.
///
/// Devolve `line` intacta quando não há folga (o rótulo é acessório; a linha de
/// dados não é): perder a contagem de ferramentas para mostrar `12/87` seria
/// trocar informação por enfeite. A conta é em CHARS (nunca bytes) para um
/// acento não desalinhar a coluna.
pub fn overlay_right(line: &str, label: &str, width: usize) -> String {
    // As linhas de tabela chegam PREENCHIDAS até a largura (célula com padding),
    // então medir o fim dos espaços diria "não cabe" em toda linha que ocupa a
    // largura toda — e o rótulo nunca apareceria justamente na tabela.
    let data = line.trim_end();
    let line_len = data.chars().count();
    let label_len = label.chars().count();
    // Pelo menos um espaço entre o dado e o rótulo.
    if line_len + 1 + label_len > width {
        return line.to_string();
    }
    let pad = width - line_len - label_len;
    format!("{data}{}{label}", " ".repeat(pad))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_inside_the_first_window_does_not_scroll() {
        assert_eq!(keep_visible(0, 10, 100), 0);
        assert_eq!(keep_visible(9, 10, 100), 0);
    }

    #[test]
    fn cursor_below_the_window_scrolls_just_enough() {
        // Linha 10 com 10 visíveis ⇒ janela 1..11 (o cursor fica no rodapé).
        assert_eq!(keep_visible(10, 10, 100), 1);
        assert_eq!(keep_visible(15, 10, 100), 6);
    }

    #[test]
    fn cursor_above_the_window_puts_it_back_on_top() {
        // `k` depois de rolar: a janela volta para a linha do cursor, que fica
        // na PRIMEIRA posição (movimento mínimo, sem salto para o meio).
        assert_eq!(keep_visible(3, 10, 100), 0);
    }

    #[test]
    fn the_window_never_opens_past_the_end() {
        // Último item de uma lista quebrada (7 itens, 10 visíveis): janela 0.
        assert_eq!(keep_visible(6, 10, 7), 0);
        // Lista de 100 com 10 visíveis e cursor no fim: janela 90..100.
        assert_eq!(keep_visible(99, 10, 100), 90);
    }

    #[test]
    fn degenerate_geometry_never_panics() {
        assert_eq!(keep_visible(5, 0, 10), 0);
        assert_eq!(keep_visible(5, 10, 0), 0);
        assert_eq!(slice(5, 0, 10), (5, 5));
        assert_eq!(slice(50, 10, 10), (10, 10));
        assert_eq!(position_label(0, 0, 10), None);
        assert_eq!(position_label(0, 10, 0), None);
    }

    #[test]
    fn slice_is_always_inside_the_list() {
        let (start, end) = slice(90, 10, 100);
        assert_eq!((start, end), (90, 100));
        let (start, end) = slice(95, 10, 100);
        assert_eq!((start, end), (95, 100), "fim nunca passa do total");
        assert!(start <= end);
    }

    #[test]
    fn position_label_only_appears_when_there_is_something_to_scroll() {
        assert_eq!(position_label(0, 10, 10), None, "lista inteira visível");
        assert_eq!(position_label(0, 10, 4), None);
        assert_eq!(position_label(0, 10, 87), Some("1/87".to_string()));
        assert_eq!(position_label(11, 10, 87), Some("12/87".to_string()));
        // Offset no fim não inventa posição acima do total.
        assert_eq!(position_label(200, 10, 87), Some("87/87".to_string()));
    }

    #[test]
    fn overlay_keeps_the_data_and_only_fills_available_room() {
        let line = "ferramentas: audit 2";
        let overlaid = overlay_right(line, "12/87", 40);
        assert!(overlaid.starts_with(line), "{overlaid}");
        assert!(overlaid.ends_with("12/87"), "{overlaid}");
        assert_eq!(overlaid.chars().count(), 40, "ocupa a largura toda");
        // Sem folga: a linha de dados volta intacta (nada é apagado).
        assert_eq!(overlay_right(line, "12/87", line.len()), line);
        // Uma coluna de folga já não basta (precisa de separador).
        assert_eq!(overlay_right(line, "12/87", line.len() + 1), line);
    }

    #[test]
    fn overlay_reuses_the_padding_of_a_full_width_table_row() {
        // Cabeçalho de colunas chega preenchido até a borda; o rótulo entra no
        // espaço que era padding, sem empurrar nada.
        let header = format!("{:<30}", "TIPO TÍTULO ATUALIZAÇÃO");
        let overlaid = overlay_right(&header, "3/10", 30);
        assert!(overlaid.ends_with("3/10"), "{overlaid}");
        assert_eq!(overlaid.chars().count(), 30);
        assert!(overlaid.starts_with("TIPO"), "{overlaid}");
    }

    #[test]
    fn overlay_counts_chars_not_bytes() {
        // Nome acentuado: a conta em bytes desalinharia a borda do painel.
        let line = "ritmo (últimas 50): ▁▂▃";
        let width = 34;
        let overlaid = overlay_right(line, "1/50", width);
        assert_eq!(overlaid.chars().count(), width, "{overlaid}");
    }
}
