# statui — inspiração da casca do `kira dashboard` (ADR-0014)

- Projeto: **statui** — https://github.com/Mohamed-Badry/statui
- Autor: Mohamed Badry · Licença: **MIT** · versão consultada: 0.3.0
- Papel no KIRA: o ADR-0014 escolheu o statui como a forma da casca TUI do
  `kira dashboard` (loop de eventos, keymap, paleta, painéis em Ratatui).

## Status desta PR (W6-PR1)

**Nenhum trecho de código do statui foi copiado.** Os módulos de `src/tui/`
(`app.rs`, `keymap.rs`, `theme.rs`, `state.rs`, `widgets/header.rs`) foram
escritos do zero seguindo a estrutura planejada no ADR-0014. Como não houve
cópia, a licença MIT do statui ainda não precisa ser redistribuída aqui — ela
passa a ser obrigatória (`third_party/statui/LICENSE` + header de atribuição em
cada arquivo derivado) no momento em que o primeiro trecho do statui entrar,
nos PRs seguintes da W6 (tabela, inspector, footer, fx).
