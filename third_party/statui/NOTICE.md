# statui — casca do `kira dashboard` (ADR-0014)

- Projeto: **statui** — https://github.com/Mohamed-Badry/statui
- Autor: Mohamed Badry · Licença: **MIT** · versão consultada: 0.3.0
- Commit de referência: `8a5edab282632443`
- Licença redistribuída em [`LICENSE`](LICENSE) (exigência da MIT a partir da primeira adaptação).

## O que foi adaptado

Nenhum arquivo do statui foi copiado inteiro. As **primitivas de layout e pintura** abaixo foram
reescritas em `src/tui/`, com nomes, dados e contratos do KIRA:

| Primitiva do statui | Origem (src/) | Adaptação no KIRA |
|---|---|---|
| Root em duas faixas (`Min(0)` + `Length(1)`) | `ui/mod.rs` | `tui::layout::shell` — status, abas, trabalho e rodapé de 1 linha |
| Alternância 100% / 50–50 para o inspetor | `ui/mod.rs` | `tui::layout::workspace` + `AppState::inspecting` |
| Borda `border::DOUBLE` + título entre `\| \|` | `ui/theme.rs`, `ui/util.rs` | `tui::widgets::panel` |
| Seções com `Borders::TOP` + título | `ui/widgets/inspector/mod.rs` | `tui::widgets::section` |
| Rodapé com teclas em chip (fg/bg) | `ui/widgets/footer.rs`, `ui/theme.rs` | `tui::widgets::footer` |
| `centered_rect`/`centered_area` para popup | `ui/util.rs` | `tui::layout::centered_rect` |
| `➤ ` como símbolo de seleção | `ui/theme.rs` | `widgets::{findings,sessions,wiki}` (já existia desde o W6) |

## O que NÃO foi trazido

- `fx.rs` / `tachyonfx` (animação): dependência nova + persona §7 proíbe efeito enjoativo.
- `backend.rs`, `config.rs`, `actions.rs`, `shellexpand.rs`: o KIRA tem store/config/CLI próprios e o
  dashboard não faz rede.
- Loop a 60 FPS: o KIRA usa poll de 150 ms (CPU baixa, sem redraw contínuo).

## Regra de licença para os próximos PRs

Qualquer trecho novo derivado do statui entra com atribuição no cabeçalho do arquivo
(`// Adaptado de statui (MIT) — ver third_party/statui/NOTICE.md`) e atualização desta tabela.
