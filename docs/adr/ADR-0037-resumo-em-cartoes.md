# ADR-0037 — Resumo em cartões: chip vem da trilha, spinner só com filho vivo

- **Status**: proposto — aguarda aval do dono · **Data**: 12/Nov/2026
- **Relaciona**: ADR-0036 (nativo primeiro, ordem dos PRs) · ADR-0033 (execução
  confirmada) · ADR-0021 (índice de retrieval) · `docs/kira-w6-dashboard.md`

## Contexto

O PR-UX1 deu número visual ao Resumo (barras, sparkline, gauge), mas a tela
continuava sendo **uma coluna de seções de texto**: o dono lê de cima para baixo
e ainda faz a conta de cabeça para comparar. Em 80×24 essa coluna já gastava
linhas com cabeçalho de tabela (`SEVERIDADE FERRAMENTA LOCAL RESUMO`) que o
terminal não alinha melhor do que um cartão.

Ao mesmo tempo, o dashboard tinha dois estados que ele nunca mostrava: a
execução viva (o overlay `o` só abre depois) e o resultado por sessão (`ok` da
trilha). E havia a tentação clássica de widget de terminal: spinner permanente
"pensando" e chip de estado deduzido — os dois mentem quando não há processo.

## Decisão

1. **O Resumo é um grid de cartões, não uma coluna de seções.** Cada cartão tem
   moldura, título (o NOME do dado) e 2–6 linhas de fato. Ordem por prioridade
   de quem opera: ação primeiro (`Risco`, `Sessões`), contexto depois (`Projeto`,
   `Atividade`), registro no fim (`Índice e wiki`, `Pendências`).
2. **A geometria é aritmética pura (`tui/grid`), não `Layout` percentual.** Em
   80×24 a coluna única fecha **exatamente** as 21 linhas do workspace
   (6+7+4+4), sem buraco e sem overflow; sobra vira altura dos últimos cartões e
   falta corta a partir da primeira linha que não cabe INTEIRA — cartão pela
   metade é pior que cartão ausente, e o que saiu é reportado (`omitted`).
3. **`KIRA_GLYPHS=ascii` continua sendo contrato de tela inteira.** Cartão,
   chip, spinner e gauge saem do mesmo `Glyphs`; o gate é o frame completo.
4. **Chip vem da trilha.** `[ok]`/`[falhou]` é `SessionLine::ok` — a única
   memória de resultado que existe. Não há `[aguardando]`, `[pausado]` nem
   percentual: a store não grava isso para comando, e chip inventado é estado de
   processo que ninguém observou.
5. **`[exec]` é o único chip vivo, e ele morre com o processo.** Spinner existe
   só sob `ExecStatus::Running` (`spinner::for_status`); o tick vem do loop
   (`AppState.tick`), então parado não pisca nada.
6. **A série de tendência sai do cartão em 80×24 e NÃO some.** Ela é listada no
   `?`, uma linha por scan, porque a decisão do dono é que dado tirado da tela
   continua alcançável — esconder na tela não é perder o dado.
7. **Nada de gauge de budget de LLM.** Continua valendo o ADR-0036 §6: não há
   contador no schema, e número inventado é proibido em qualquer tela.

## Alternativas consideradas e recusadas

- **Crate de grid/cartão** (`tui-widget-list`, `tui-bar-graph`, `throbber`):
  a regra do ADR-0036 é nativo primeiro; o que falta vira módulo puro nosso e
  testável sem terminal.
- **`Layout` com `Constraint::Percentage`** para os cartões: resolve a sobra por
  arredondamento e esconde o caso "não coube" — o critério do dono é sobre
  NÚMEROS, então a conta fica explícita.
- **Chip de estado derivado de heurística** (idade + presença de summary): daria
  estado bonito e falso. O `ok` da trilha é o único fato.
- **Spinner permanente no rodapé** ("pensando"): finge atividade; num dashboard
  de diagnóstico isso é mentira sobre o processo.
- **Diff `novo`/`ainda`/`sumiu` entre scans** e **heatmap de scans por dia**:
  exigem schema novo (fingerprint do scan anterior; janela completa de sessões,
  hoje 50 em memória). Ficam para PR próprio com ADR, não de carona.

## Consequências

- Positivas: a tela responde "como estamos" em dois segundos e em qualquer
  largura; o estado de execução aparece onde o dono está olhando; o modo ASCII
  segue legível sem box-drawing; zero dependência nova e zero campo novo na
  store.
- Negativas: um widget novo por cartão (`widgets::cards`) e a obrigação de todo
  cartão novo passar pelo `Glyphs` e pelo grid — quem desenhar borda por conta
  própria fura o contrato, e quem pega isso é o teste de frame inteiro.
- Riscos: o cartão de Risco troca a sparkline por uma barra em 80×24; se o dono
  quiser tendência ali, o caminho é reduzir barras, não reintroduzir rolagem
  horizontal.

## Testes que fixam a decisão

- `tui::grid::tests::eighty_by_twenty_four_stacks_four_cards_without_overflow`
- `tui::grid::tests::wide_terminal_uses_two_columns_and_fills_the_leftover`
- `tui::grid::tests::cards_that_do_not_fit_are_reported_not_dropped`
- `tui::spinner::tests::ascii_mode_uses_four_frames_and_leaks_no_unicode`
- `tui::spinner::tests::spinner_is_none_unless_the_child_is_running`
- `tui::widgets::chips::tests::chip_state_comes_from_the_session_ok_flag`
- `tui::widgets::chips::tests::running_chip_carries_the_spinner_and_the_elapsed_label`
- `tui::widgets::cards::tests::risco_card_shows_the_same_numbers_as_the_text_section`
- `tui::widgets::cards::tests::pendencias_card_truncates_to_three_and_points_at_help`
- `tui::widgets::cards::tests::ascii_grid_has_no_special_glyph_in_either_layout`
