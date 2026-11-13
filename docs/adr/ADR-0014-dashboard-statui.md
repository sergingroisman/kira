# ADR-0014 — Dashboard do KIRA: shell adaptado do statui (MIT)

- **Status**: aceito (decisão do dono, 10/Nov/2026: "cópia descarada do statui")
- **Fonte**: https://github.com/Mohamed-Badry/statui · Rust + Ratatui · **MIT** · v0.3.0
- **Decisores**: Sérgio (dono) · Hermes

## Contexto

O KIRA precisa de um dashboard no terminal com cara de jogo — não de um `println`
colorido — e já existe um TUI em Ratatui, pequeno e bem estruturado, com exatamente
a forma que queremos: `app.rs` (loop), `keymap.rs`, `ui/theme.rs`, `ui/fx.rs`
(efeitos), `ui/widgets/{table,inspector,footer,welcome}` e presets em TOML.

Ficha verificada por API em 10/Nov/2026: **MIT**, 9,3 MB de repo, ~60 KB de código
em `src/`, Ratatui **0.29** com feature `crossterm`, demo gravado com `vhs`
(`demo.tape`), teste de integração com `wiremock`.

Duas consequências imediatas:

1. **Licença compatível.** MIT → MIT permite copiar, modificar e relicenciar,
   **desde que o aviso de copyright e o texto da licença sejam preservados**.
   "Cópia descarada" é legítimo; cópia **sem atribuição** é violação de licença.
2. **Escopo do que se copia importa mais que a licença.** O statui é um monitor de
   saúde de API (probes HTTP, latência, disponibilidade). O domínio dele não serve;
   a **casca** serve.

## Decisão

Adaptar a **casca de UI** do statui dentro de `src/tui/`, mantendo o KIRA como
binário único, e descartar o domínio de monitoramento HTTP.

### O que entra (adaptado, com atribuição)

| Origem (statui) | Destino (kira) | Papel |
|---|---|---|
| `src/app.rs` (loop de eventos) | `src/tui/app.rs` | ciclo render/evento, estado da tela |
| `src/keymap.rs` | `src/tui/keymap.rs` | atalhos (`a` aprovar, `s` scan, `t` triage…) |
| `src/ui/theme.rs` | `src/tui/theme.rs` | paleta colorida (mascote-friendly) |
| `src/ui/fx.rs` | `src/tui/fx.rs` | animação de "novo achado", pulsos |
| `src/ui/widgets/table.rs` | `src/tui/widgets/table.rs` | lista de bugs com severidade |
| `src/ui/widgets/inspector/*` | `src/tui/widgets/inspector/*` | detalhe do item selecionado |
| `src/ui/widgets/footer.rs` | `src/tui/widgets/footer.rs` | barra de atalhos + erro copiável |
| `src/state.rs` (forma do estado) | `src/tui/state.rs` | estado de tela (não o de domínio) |

### O que NÃO entra

`backend.rs` (probes HTTP), `config.rs`/`shellexpand.rs` (modelo de config próprio —
o KIRA já tem `config.toml` + store), `presets/*` (presets de endpoints),
`dotenvy`/`color-eyre`/`config`/`directories` (dependências que o KIRA não precisa).
O KIRA alimenta a TUI **do store**, não de requisições.

### Dependências novas (justificativa, exigida pelo AGENTS.md)

- `ratatui = { version = "0.29", features = ["crossterm"] }` — padrão de fato para
  TUI em Rust, puro Rust sobre crossterm, sem libs de sistema: **CI-safe nos 3 SOs**.
- `crossterm` vem por feature; `unicode-width` é transitiva do ratatui.
- **Nenhuma** outra dependência do statui entra.

### Atribuição (obrigatória)

```
third_party/statui/LICENSE          texto MIT original, intacto
third_party/statui/NOTICE.md        URL, versão (0.3.0), commit copiado, lista dos
                                    arquivos adaptados e o que mudou
README.md                           seção "Credits": statui (Mohamed Badry, MIT)
```

Regra de ouro: qualquer arquivo derivado carrega no topo `// adapted from
statui 0.3.0 (MIT) — see third_party/statui/NOTICE.md`.

## Consequências

- **Positivas**: semanas de encanamento de Ratatui já resolvidas (loop de evento,
  theme, table, inspector, fx); visual comprovadamente bom; licença limpa; o
  `demo.tape` do statui dá um caminho pronto para gravar o GIF do KIRA.
- **Negativas**: código de terceiro para manter (`NOTICE` resolve a licença, não a
  manutenção); risco de herdar padrões que não são os nossos — mitigado por adaptar
  só a casca e revisar arquivo por arquivo.
- **Invariantes mantidas**: TUI é modo do binário, read-only sobre o store, não
  substitui o modo texto, erro em texto copiável no rodapé.

## Rejeitado

- **Escrever a TUI do zero**: 2–3 semanas repetindo o que já existe em MIT.
- **Copiar o repo inteiro (fork do statui)**: traz backend HTTP, config model e
  dependências que contradizem o contrato do KIRA.
- **Dashboard web**: viola "sem daemon/servidor separado" do mandato v3.
