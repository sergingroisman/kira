# awesome-ratatui → KIRA: o que aproveitar SEM dependência nova

**Resposta ao Kombai** — 12/Nov/2026 · fonte: https://github.com/ratatui/awesome-ratatui (591 entradas lidas)
Decisões do dono registradas em `/opt/data/planos/kira-visual-crates.md`.

> **Estado em 12/Nov/2026**: **PR-UX1** e **PR-UX2** entregues, os dois **sem dependência nova**
> — o nativo do `ratatui 0.29` mais módulos puros nossos cobriram o que a lista pedia de crate.
> A fila UX3–UX8 está intacta (ver §6).

## 0. O achado que reorganiza tudo

`ratatui 0.29` (a versão que o KIRA já usa) traz **nativos**: `BarChart`, `Sparkline`, `Gauge`,
`Scrollbar`, `Tabs`, `Chart`, `Canvas`, `Calendar`, `Table`, `List`. Hoje o dashboard usa apenas
`Block`, `Borders`, `Paragraph` e `Clear`.

**Consequência**: a maior parte do "visual único" que buscávamos em crates está no binário. Antes de
adicionar dependência, use o nativo. Isso corta ~6 crates da lista original.

## 1. Nativo do ratatui → onde usar no KIRA (zero dependência)

| Nativo | Uso no KIRA | Substitui |
|---|---|---|
| `BarChart` | severidade dos achados em barras horizontais (comparação honesta) | `tui-bar-graph` |
| `Sparkline` | tendência de achados por scan (últimos N scans) | `tui-bar-graph`/`malevich` |
| `Gauge` | **uso do índice de retrieval** (feito). Budget do dia fica bloqueado até existir contador de chamadas (PR17): sem número real, o gauge mentiria | `toptop`/`amtr` (padrão) |
| `Scrollbar` | posição em listas longas | `tui-scrollbar` |
| `Tabs` | indicador dos 4 painéis (se quisermos mais que o rodapé) | `tui-tabs` |
| `Canvas` | desenho de linha/pontos (ex.: DAG de commits simples) | `malevich` |
| `Table` | achados/sessões/wiki (já usado) | `Ratagrid`, `tui-widget-list` |

## 2. Dependências que SOBREVIVEM à revisão

| Crate | Versão | Por que não dá para fazer em casa com qualidade |
|---|---|---|
| `tui-piechart` | 1.4.2 | rosca de severidade (não existe nativo; Canvas exigiria ~150 linhas de trigonometria + rótulos) |
| `tui-tree-widget` | 0.24.1 | árvore com expand/collapse e navegação por teclado (20 KB, 2,26M dl) |
| `tui-big-text` | 0.8.10 | fonte 8x8 para a marca e o número de achados |
| `opaline` | 0.4.2 | motor de tema com 20 temas + tema do usuário (identidade do produto) |
| `tachyonfx` | 0.25.2 | transições — só se o dono quiser movimento; sai primeiro se o peso incomodar |
| `termprofile` | 0.2.5 | detectar cor/fonte reais e escolher glifos (degradação honesta) |
| `hyperrat` | 0.1.3 | link OSC-8 (6 KB) — Ctrl+Click abre arquivo/PR |
| `boxmaid` | 0.1.1 | Mermaid → ASCII (v0.1: vigiar upstream ou trocar por `nereid`) |

## 3. Faz-se em casa, melhor que a crate (zero dep)

| Padrão | Referência (app do awesome) | O que escrever |
|---|---|---|
| **Cartões de status** [entregue] | `mirador`, `kdash` | feito no PR-UX2: `widgets/card` (moldura) + `widgets/cards` (seis cartões) + `tui/grid` (colunas e altura puras) |
| **Spinner "pensando"** [entregue] | `throbber-widgets-tui` | feito no PR-UX2: `tui/spinner` com 10 frames braille e 4 ascii, avançando pelo `AppState.tick` — e **só** com `ExecStatus::Running`, porque spinner em tela parada finge atividade |
| **Popup/overlay** | `tui-popup` | `Clear` + área calculada com clamp: já temos no `?`; extrair helper e testar em 80×24 |
| **Diff entre scans** | `hwatch` | comparar `findings` por fingerprint com o scan anterior: "novo/ainda/sumiu" |
| **Heatmap de atividade** | `sprout` | blocos `░▒▓█` em grade de 7×N (scans por dia) |
| **Mapa do context pack** | `amtr` | barra segmentada por fonte (código, wiki, findings) + `Gauge` |
| **Supervisão de sessões** [entregue] | `iris`, `savras` | feito no PR-UX2 com a verdade do dado: `[ok]`/`[falhou]` vêm de `SessionLine.ok` e `[exec]` do `ExecStatus` vivo. **`aguardando` não existe** na store para comando, então não virou chip |
| **Busca fuzzy com preview** | `television`, `atuin` | filtro incremental + preview do match (nosso `/` evolui) |
| **Árvore de JSON/TOML** | `otree` | work item cru ao lado do triage, colapsável (reusa `tui-tree-widget`) |
| **Markdown melhor** | `md-tui` | tabelas e listas aninhadas no renderer próprio (hoje ~300 linhas) |
| **Log viewer** | `therapist` | abas por severidade + salto entre matches no painel de sessões |

## 4. Fork interno (código de terceiro que vira nosso)

| Crate | Motivo do fork |
|---|---|
| `ratatui-widgettable` (40 downloads) | tabela com widget em célula: adaptar para badge de severidade + sparkline na mesma linha |
| `boxmaid` (se a v0.1 travar) | renderizador Mermaid mínimo, só com o subconjunto que nossos diagramas usam |

## 5. Proibido (não reabrir sem ADR)

- **Licença**: `tui-additions` (GPL-3.0), `tui-dialog` (AGPL), `tui-globe` (GPL), `ratatui-comfy-*` (non-standard).
- **Peso**: `ratatui-markdown` 8,1 MB · `ratatui-image` 5,9 MB (mascote pós-MVP) · `tui-logger` 6,5 MB · `tui-widget-list` 3,4 MB · `tui-shimmer` 1,7 MB.
- **Abandonado**: `ratatui-stacked-bar` (413 dl, Abr/2026), `color-to-tui` (2023), `term-rustdoc` (2024), `tui-rain` (2024), `widgetui` (2024).
- **Arquitetura**: toda a seção Frameworks (23 itens) — `tui-realm`, `tui-react`, `ratzilla`, `rlt`, `burn`.

## 6. Ordem de implementação (PRs separados, merge humano)

1. **PR-UX1 — nativo primeiro** [entregue] (ADR-0036, merge 71c20b9 via #68): `BarChart` de severidade + `Sparkline` de tendência + `Gauge` de budget + `Scrollbar`. Zero dep. Testes `TestBackend` (altura constante, sem `\n`, 80×24).
2. **PR-UX2 — status** [entregue] (ADR-0037, merge pendente — branch ux/pr-ux2-cartoes-spinner-chips): cartões caseiros + spinner caseiro + chips de sessão. Zero dep.
3. **PR-UX3 — árvore**: `tui-tree-widget` no painel de wiki (1 dep).
4. **PR-UX4 — tema**: `opaline` + `termprofile` (2 deps) com `KIRA_GLYPHS=ascii` testado.
5. **PR-UX5 — marca**: `tui-big-text` no cabeçalho (1 dep).
6. **PR-UX6 — rosca**: `tui-piechart` no resumo de severidade (1 dep).
7. **PR-UX7 — interação**: `hyperrat` + `boxmaid` (2 deps).
8. **PR-UX8 — movimento**: `tachyonfx` (opcional, último).

Cada PR mede `cargo build` antes/depois e cola no corpo. Se o tempo subir mais de 30%, a dep daquele
PR é reavaliada antes do merge.

## 7. Critérios de aceite (valem para todos)

1. `decide()` continua puro e testado; teclas atuais não mudam (`q`/`Ctrl+C` saem, `Esc` volta, `Tab` cicla 4 painéis, `j/k` só no foco).
2. Todo widget novo tem teste `TestBackend` afirmando altura/linha constante e ausência de `\n` no buffer.
3. Nenhuma cor RGB solta: tudo vem do tema.
4. `KIRA_GLYPHS=ascii` renderiza sem glifo especial; 80×24 continua legível.
5. Read-only: nenhuma tecla escreve no mundo.
6. Sem dependência fora desta lista; exceção exige atualizar este documento **antes** do código.
