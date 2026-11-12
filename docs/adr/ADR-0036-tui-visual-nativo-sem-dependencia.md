# ADR-0036 — Camada visual do TUI: nativo do ratatui primeiro, dependência depois

> Renumerado em 12/Nov/2026: era `ADR-0032` na árvore local até a PR #64 reservar `0029`
> (vocabulário de comandos) e `0030` (local-first). Nenhum merge existiu com o número velho.

- **Status**: implementado — aguarda aceite do dono · **Data**: 12/Nov/2026
- **Relaciona**: ADR-0014 (casca do dashboard) · ADR-0033 (execução confirmada) ·
  AGENTS.md ("sem dependência nova sem justificar no PR") · `docs/kira-w6-dashboard.md`

## Contexto

O levantamento do `awesome-ratatui` (591 entradas) mostrou duas coisas ao mesmo tempo:

1. a lista oferece dezenas de crates de widget (`tui-bar-graph`, `tui-scrollbar`,
   `throbber-widgets-tui`, `tui-tree-widget`, `tui-piechart`, `opaline`, `tachyonfx`…);
2. **o `ratatui 0.29`, que o KIRA já usa, traz `BarChart`, `Sparkline`, `Gauge`,
   `Scrollbar`, `Tabs`, `Chart`, `Canvas`, `Calendar`, `Table` e `List` nativos** — e o
   dashboard usava só `Block`, `Borders`, `Paragraph` e `Clear`.

A conclusão que reorganiza o plano: antes de abrir dependência, usar o binário. E havia um
defeito de uso real esperando solução nesse mesmo pacote: **nenhuma das três listas tinha offset
de viewport**. O `j` descia a seleção para fora da área visível e o dono navegava às cegas — o
`REVERSED` existia, mas fora da tela.

## Decisão

1. **PR-UX1 é zero dependência.** Barras de severidade usam o `BarChart` nativo e a tendência
   usa o `Sparkline` nativo, ambos com `bar_set` próprio. O que o nativo não faz bem é feito
   aqui, em módulo puro e testado (`glyph`, `viewport`), não comprado.
2. **`KIRA_GLYPHS=ascii` é contrato de leiaute, não enfeite.** O modo troca o conjunto de
   glifos de TUDO que a tela desenha — não só do PR-UX1: borda (`+ - |`), marcador de seleção
   (`>`), barra (`#`), gauge (`#`/`-`), sparkline (`. : - = + * #`), scrollbar, os popups
   (`?`, confirmação, folha de alvo) e o cabeçalho de stream do overlay `o`. A decisão é pura
   sobre o valor da variável (`glyph::Mode::from_value`), como já era `theme::color_enabled`.
   **Diferente de `text_is_ascii`, acento de palavra não conta** — `Sessões` é texto, não glifo
   quebrado; o gate é `glyph::has_special_glyphs` (box-drawing, bloco, seta).
   Em modo ASCII a linha de ajuda ensina o nome da tecla ("setas") em vez de desenhar `↑ / ↓`.
3. **O `Gauge` nativo foi rejeitado** para o modo ASCII: `use_unicode(false)` só troca a célula
   fracionária e continua pintando `█` nas cheias (`ratatui-0.29.0/src/widgets/gauge.rs:197`).
   A barra de proporção é uma linha de texto com o glifo do modo.
4. **Scrollbar nativo com trilho fino** (`│` + `█`). O `symbols::scrollbar::DOUBLE_VERTICAL` do
   ratatui usa `║`, que ao lado da borda dupla do painel vira `║║` e some na casca.
5. **O offset é matemática pura** (`viewport::keep_visible`): cursor dentro da janela não rola,
   abaixo desce o mínimo, acima volta ao topo. Achados, Sessões e Wiki usam o mesmo contrato;
   Sessões e Wiki contam LINHAS de exibição (o separador de dia e o cabeçalho de tipo ocupam
   linha), senão o cursor voltaria a sair da tela em lista agrupada.
6. **Nada de budget de LLM nesta rodada.** Não existe contador: o schema do store (v3) não tem
   tabela de chamadas, `src/config.rs` não tem `max_llm_calls_per_day` e o único registro do
   número 50 é texto em documentação (PR17 não implementado). Um `Gauge` 0/50 seria invenção.
   O gauge mede o que EXISTE hoje: **uso do índice de retrieval**
   (`repo_file_count` / `KIRA_INDEX_MAX_FILES`), que é justamente o teto que corta arquivos em
   silêncio.
7. **Tendência vem da trilha, não de estado inventado.** `findings.json` é sobrescrito a cada
   execução; a série histórica só existe no `summary` da sessão `scan`. O parser
   (`scan::findings_count_in_summary`) mora ao lado de quem ESCREVE o formato, e summary fora do
   formato é DESCARTADO — nunca vira `0`, que desenharia uma queda que não houve.
8. **Overlay modal limpa a área antes de desenhar.** A ajuda já cobria a tela inteira, mas não
   emitia `Clear`: as células que o `Paragraph` não pintava mostravam o painel de trás — título e
   barras vazando por dentro do texto da ajuda. Em Unicode passava desapercebido (fundo escuro);
   no modo ASCII os `#` das barras apareceram no meio das linhas. Agora o `Clear` vem antes,
   como já era na confirmação, na folha de alvo, no overlay `o` e na leitura de wiki. Quem
   encontrou o defeito foi o teste de frame inteiro, não uma revisão visual.
9. **Ordem dos PRs** (cada um com merge humano, `cargo build` medido no corpo):
   UX1 nativo (este) · UX2 status (cartões/spinner/chips, zero dep) · UX3 árvore
   (`tui-tree-widget`) · UX4 tema (`opaline`/`termprofile`) · UX5 marca (`tui-big-text`) ·
   UX6 rosca (`tui-piechart`) · UX7 interação (`hyperrat`/`boxmaid`) · UX8 movimento
   (`tachyonfx`). Dependência fora dessa lista exige este ADR atualizado ANTES do código.

## Consequências

- Positivas: comparação de severidade e tendência em uma leitura; a seleção nunca sai da tela;
  o terminal sem box-drawing continua legível; zero crate nova, zero risco de licença.
- Negativas: `KIRA_GLYPHS=ascii` depende de cada widget receber o `Glyphs` — widget novo que
  desenhe borda por conta própria fura o contrato, e quem pega isso é o teste de frame inteiro
  (abaixo), não a revisão. O `…` de truncamento é igual nos dois modos: é pontuação, não glifo
  de desenho; se incomodar em console legado, vira `...` no PR-UX1b.
- Riscos: o parser da tendência depende do texto do `run`; o teste
  `findings_count_is_read_back_from_the_real_summary_format` usa a linha real e quebra junto se
  o formato mudar — que é o alarme desejado.

## Testes que fixam a decisão

- `tui::glyph::tests::ascii_mode_is_opt_in_and_case_insensitive`
- `tui::glyph::tests::special_glyph_gate_separates_accent_from_broken_glyph`
- `tui::viewport::tests::cursor_below_the_window_scrolls_just_enough`
- `tui::viewport::tests::the_window_never_opens_past_the_end`
- `tui::widgets::findings::tests::selection_scrolls_into_view_instead_of_leaving_the_screen`
- `tui::widgets::sessions::tests::selection_scrolls_into_view_in_the_grouped_list`
- `tui::widgets::wiki::tests::selection_scrolls_into_view_with_the_group_headers`
- `tui::widgets::severity::tests::the_tallest_bar_is_the_worst_group_not_a_fixed_scale`
- `tui::widgets::resumo::tests::ascii_mode_resumo_has_no_box_drawing_or_block_glyph`
- `tui::app::tests::ascii_mode_renders_the_whole_frame_without_special_glyph` (as 4 telas, a
  ajuda, a folha de alvo e o overlay `o` renderizados a 80×24 em modo ASCII)
- `scan::tests::findings_count_is_read_back_from_the_real_summary_format`
- `tui::state::tests::scan_series_reads_the_trail_oldest_first_and_skips_foreign_summaries`
- `tui::state::tests::index_line_shows_the_real_ratio_or_the_actionable_cause`
