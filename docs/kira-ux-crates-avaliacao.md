# KIRA — levantamento de crates para o visual único (um a um)

- **Status**: em levantamento (12/Nov/2026) · conduzido com o dono, **crate por crate**
- **Objetivo**: definir o conjunto mínimo de crates que dá ao dashboard do KIRA um visual
  próprio, sem virar framework de TUI — e transformar as decisões num `.md` para o Kombai executar
- **Notion**: página de decisões (link no chat) — cada linha tem "sua decisão" para marcar
- **Fonte da verdade**: este arquivo. O `.md` entregue ao Kombai é derivado dele no final.

## Critérios de decisão (aplicados a TODOS, sem exceção)

1. **Resolve dor real do dashboard** (ler 4 painéis, achar o que importa em 2s) — não é enfeite.
2. **Orçamento de dependência**: nova só se economizar 100+ linhas **ou** entregar capacidade
   impossível de escrever (imagem em terminal, por exemplo). Regra do dono.
3. **Peso**: KB do crate + transitivas; compilar o KIRA rápido é requisito (dev roda `cargo install`).
4. **Licença** MIT/Apache-2.0 compatível.
5. **Manutenção**: release nos últimos 6 meses e mantenedor ativo.
6. **Windows first** (a máquina do dono) e Linux/macOS sem quebrar.
7. **Degradação obrigatória**: sem Nerd Font, sem suporte a imagem, sem truecolor ⇒ continua legível.
8. **Read-only**: nada de crate que exija escrita no mundo ou daemon.

## Fila do levantamento

| # | Crate | Estado |
|---|---|---|
| 1 | [`tui-piechart`](https://crates.io/crates/tui-piechart) | **em avaliação** (pedido do dono) |
| 2 | [`tui-big-text`](https://crates.io/crates/tui-big-text) | na fila |
| 3 | [`tachyonfx`](https://crates.io/crates/tachyonfx) | na fila |
| 4 | [`tui-tree-widget`](https://crates.io/crates/tui-tree-widget) | na fila |
| 5 | [`tui-markdown`](https://crates.io/crates/tui-markdown) | na fila |
| 6 | [`ratatui-image`](https://crates.io/crates/ratatui-image) | na fila |
| 7 | [`arboard`](https://crates.io/crates/arboard) (clipboard) | na fila |
| 8 | [`notify-rust`](https://crates.io/crates/notify-rust) (notificação) | na fila |
| 9 | [`open`](https://crates.io/crates/open) (abrir URL) | na fila |
| 10 | [`burn`](https://github.com/tracel-ai/burn) (ML) | **avaliado: NÃO** (ver abaixo) |

## Dados coletados (crates.io, 12/Nov/2026)

| Crate | Versão | Último release | Downloads | Licença | KB |
|---|---|---|---|---|---|
| tui-piechart | 1.4.2 | 2026-10-28 | 26.624 | MIT | 135 |
| tui-big-text | 0.8.10 | 2026-09-24 | 582.656 | MIT OR Apache-2.0 | 38 |
| tachyonfx | 0.25.2 | 2026-09-06 | 559.152 | MIT | 533 |
| tui-tree-widget | 0.24.1 | 2026-08-09 | 2.263.300 | MIT | 20 |
| tui-markdown | 0.3.13 | 2026-11-01 | 646.168 | MIT OR Apache-2.0 | 56 |
| ratatui-image | 12.0.0-rc.3 | 2026-10-18 | 1.446.817 | MIT | 5.875 |
| arboard | 3.7.0 | 2026-10-15 | 66.266.225 | MIT OR Apache-2.0 | 54 |
| notify-rust | 4.19.0 | 2026-11-08 | 19.517.702 | MIT OR Apache-2.0 | 71 |
| open | 5.4.5 | 2026-10-22 | 70.933.939 | MIT | 42 |
| burn | 0.23.0-pre.2 | 2026-11-09 | 2.252.419 | MIT OR Apache-2.0 | 106 |

## Decisões

| # | Crate | Decisão do dono | O que entra |
|---|---|---|---|
| 10 | burn | **NÃO** (recomendação aceita) | nada |

### Por que `burn` não entra
`burn` é framework de **treino/inferência de redes neurais**. O KIRA não treina nem roda modelo
local: quem responde é Ollama/Copilot/Foundry, e a busca é FTS5 por decisão registrada
(ADR-0021, "retrieval-first" — grep dirigido vence vetor em repo pequeno/médio).
Custo seria brutal: compilação de minutos, GBs de transitivas, em troca de nada que o dashboard
precise. Se um dia houver embedding local, será decisão própria (pós-gate M2) com ADR novo.

## Decisões tomadas (ordem do README)

| # | Crate | Seção | Decisão | Motivo |
|---|---|---|---|---|
| 1 | `tui-piechart` | Widgets | **ENTRA** | rosca de mix de severidade (pedido do dono) |
| 2 | `tui-bar-graph` | Widgets | **ENTRA** | comparação honesta; é do org ratatui; 24 KB |
| 3 | `hyperrat` | Widgets | **ENTRA** | 6 KB; link clicável OSC-8, degrada para texto |
| 4 | `boxmaid` | Widgets | **ENTRA (observado)** | Mermaid no terminal; v0.1 e 529 dl ⇒ risco de manutenção, vigiar |
| 5 | `ratatui-image` | Widgets | **NÃO agora** | mascote fica pós-MVP; 5,9 MB |
| 6 | `burn` | Frameworks | NÃO | ML; KIRA não treina (ADR-0021) |
| 7 | `ratatui-stacked-bar` | Widgets | NÃO | 413 dl, último release Abr/2026 |
| 8 | `ratatui-markdown` | Widgets | NÃO | 8,1 MB; nosso renderer basta |
| 9 | `tui-additions`, `tui-dialog`, `tui-globe`, `tui-comfy-*` | Widgets | NÃO | **licença** (GPL/AGPL/non-standard) |

### Critérios novos que a varredura criou
- **Licença é veto, não peso**: GPL/AGPL obrigariam o KIRA (distribuído como MIT) a virar GPL. `tui-additions` (GPL), `tui-dialog` (AGPL), `tui-globe` (GPL) e `tui-comfy-*` (non-standard) saem por isso, independente da qualidade.
- **Org `ratatui` é sinal forte**: crates do monorepo `ratatui/tui-widgets` (release comum em 24/Set/2026) têm o menor risco de manutenção da lista.
- **Pouco usado + pequeno ⇒ candidato a fork interno** (regra do dono): `ratatui-widgettable` (40 downloads, 39 KB) e `boxmaid` (529 dl) entram nessa categoria antes de virarem dependência.
- **Teto por peso, não por número**: soma de código novo ≤ 400 KB e `cargo install` sem degradar.
