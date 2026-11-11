# W6 — `kira dashboard`: TUI Ratatui read-only sobre o store (especificação)

- **Status**: proposta para o dono aprovar antes do primeiro PR do W6
- **Base**: ADR-0014 (statui MIT como shell) · ADR-0013 (wiki) · AGENTS.md (contrato)
- **Por que vem antes do W5**: dashboard é **read-only** sobre dados que já existem
  (store, `findings.json`, wiki). Não escreve, não decide, não chama LLM. O gate M2
  protege escrita/autonomia — nada disso está aqui. W5 (`kira loop`/agendado) continua
  bloqueado até a medição do M2.

## Objetivo

`kira dashboard` = **modo interativo do mesmo binário** (nunca daemon, nunca segundo
programa). Mostra o estado vivo do projeto atual: o que o `scan` achou, o que o
`triage` concluiu, o que a wiki sabe, se o ambiente está saudável. Só leitura.

## Painéis (v1 — 4 painéis, nada além)

| Painel | Fonte | Conteúdo |
|---|---|---|
| **Cabeçalho** | `projects` + `kira doctor` (sem rede) | nome, root, id FNV, PAT `Aa2x****`, modelo LLM, se Ollama respondeu por último e quando |
| **Achados** | `.kira/findings.json` | agrupado por severidade (critical→low), contagem por ferramenta, fingerprint curto; `Enter` abre o item |
| **Sessões** | `sessions` | últimas N (default 50): `#id data comando modelo duração` + 1ª linha do summary; `Enter` expande o summary (≤2000 chars já gravados) |
| **Wiki** | `wiki_page` + FTS | páginas por tipo (bugs/decisões/padrões/glossário), `updated_at`, busca incremental com `/` |

**Regra de honestidade**: dado ausente aparece como `—` com a causa
("nunca rodou `kira scan`", "store sem sessões"), nunca inventado ou estimado.

## Interação (mínima, sem mouse)

`q` sair · `Tab`/`Shift+Tab` painel · `j`/`k` linha · `g`/`G` topo/fim · `Enter`
detalhe · `Esc` volta · `/` busca na wiki · `r` recarrega (relê disco; sem rede) ·
`?` ajuda. **Nenhuma tecla escreve** — nem `d`, nem `x`, nem confirmação.

## Estrutura técnica

- `src/tui/{app,state,theme,layout,widgets}.rs`; `app` = loop de eventos
  (`crossterm`) + `ratatui`; `state` = snapshot imutável lido do store/findings no
  início e no `r` (sem query por frame).
- Base: shell do **statui** (MIT) adaptado — atribuição obrigatória em
  `third_party/statui/{LICENSE,NOTICE.md}` (ADR-0014). Não entra `backend.rs`
  (probes HTTP) nem o modelo de config deles.
- Tema: paleta própria (KIRA fofinho/colorido), com **fallback mono** quando
  `NO_COLOR=1` ou terminal sem 256 cores — a mesma informação em texto.
- Mascote: sprite pixel art em asset versionado (`assets/mascot/*.ans`), desenhado no
  canto do cabeçalho; `kira --about` imprime a mesma arte. É arte, não PR de código.
- Terminal menor que 80×24: layout degrada para lista única, com mensagem clara.

## Contratos e testes (não negociáveis)

1. **Read-only verificado**: teste e2e que roda o dashboard com um store fixture e
   compara o hash do `store.db` antes/depois — tem que ser idêntico.
2. **Smith determinístico**: `TestBackend` do Ratatui + snapshot de buffer ASCII para
   cada painel com store fixture (`tests/fixtures/store/`), sem depender de terminal.
3. **Sem rede**: teste que aponta `KIRA_OLLAMA_URL` para porta morta e
   `AZURE_DEVOPS_PAT` inválido; o dashboard abre e mostra `—`.
4. **Store ausente** ⇒ mensagem acionável ("nenhuma trilha em …, rode `kira scan` ou
   `kira doctor`"), exit 0 (é diagnóstico, não erro de uso).
5. **CI 3 SOs** roda só os testes de snapshot (sem TTY); o modo interativo é testado
   por smoke manual do dono no Windows.

## Ordem de PRs do W6

| PR | Entrega |
|---|---|
| W6-PR1 | `src/tui` + `kira dashboard` abrindo o cabeçalho (dados do `doctor`) |
| W6-PR2 | Painel Achados (findings.json) + detalhe |
| W6-PR3 | Painel Sessões (store) + detalhe do summary |
| W6-PR4 | Painel Wiki + busca `/` |
| W6-PR5 | Mascote + tema + `NO_COLOR` + `--about` |
| W6-PR6 | README: seção do dashboard com captura (arte ASCII no próprio markdown) |

### Entregue no W6-PR1 (branch `w6/pr1-dashboard-shell`)

- `src/tui/{mod,app,keymap,theme,state}.rs` + `src/tui/widgets/header.rs`:
  loop draw→poll→ação (crossterm + ratatui 0.29), keymap completo (`q`,
  `Tab`/`Shift+Tab`, `j`/`k` e `↑`/`↓`, `g`/`G`, `r`, `?`), tema com fallback
  mono (`NO_COLOR`/`TERM=dumb`), snapshot imutável lido do disco no início e no
  `r` (nunca por frame).
- `kira dashboard` em `src/main.rs`: TUI **somente com stdout TTY**; sem TTY ou
  com `--snapshot` imprime o mesmo conteúdo em texto e sai 0; NUNCA grava
  sessão na trilha (`records: false`).
- Cabeçalho: nome, root (`display_path`), id FNV, `dev:` (`dev_name` do
  config), `llm: ollama (default)`, PAT mascarado via `doctor::mask_pat`,
  idade do último `scan` pela trilha, contagem de achados de
  `.kira/findings.json`. Ausente ⇒ `— (causa)`.
- Store aberta em **read-only real**: nova `Store::open_readonly_at`
  (`SQLITE_OPEN_READ_ONLY` — sem pragma WAL, sem migração).
- Degradação < 80×24: lista única com aviso. Overlay `?` com os atalhos.
- Testes: unit de keymap/state/theme + widget com `TestBackend`;
  `tests/dashboard_e2e.rs` (snapshot com fixtures, **hash do `store.db`
  antes/depois byte-idêntico** com e sem `--snapshot`, `KIRA_OLLAMA_URL` em
  porta morta, causa "nunca rodou kira scan", store ausente ⇒ exit 0).
- **Não copiado nada do statui** nesta PR — ver `third_party/statui/NOTICE.md`
  (a licença MIT passa a ser redistribuída no primeiro PR que copiar trechos).
- Ainda não entregue desta spec: painéis Sessões/Wiki, mascote,
  `--about`, busca `/` (o `TestBackend` por painel já existe — cabeçalho no
  W6-PR1, achados no W6-PR2), smoke manual do dono no Windows.

### Entregue no W6-PR2 (branch `w6/pr2-findings-panel`)

- Painel **Achados** (`src/tui/widgets/findings.rs`): tabela com cabeçalho de
  contagem no título do bloco (`critical N · high N · medium N · low N` — zeros
  visíveis) e colunas SEVERIDADE / FERRAMENTA / LOCAL (`path:linha` quando o
  path carrega `:N`; o scan não inventa linha) / RESUMO (1ª linha da mensagem).
  Cores semânticas por severidade (critical→low na paleta; mono preserva o
  texto). Seleção com `➤` (padrão do statui) + linha `REVERSED`. **Sem scroll
  horizontal**: colunas têm largura da área e célula trunca com `…`.
- `state`: `FindingRow` (severity/tool/path/line/fingerprint_short/
  message_first_line + fingerprint e mensagem completas para o detalhe),
  agregação por severidade e por ferramenta (`severity_counts`, `tool_counts`),
  ordenação canônica critical→low, `findings.json` ausente/corrompido ⇒ lista
  vazia + causa (fail-open), estado vazio honesto
  (`achados: — (causa)` / `achados: (0 achados no último scan)`).
- `Enter` abre o detalhe (painel lateral, 60/40) com severidade, ferramenta,
  LOCAL, fingerprint completo e a mensagem inteira; `Esc` volta. Detalhe é
  modal. `Tab`/`Shift+Tab` alterna **cabeçalho ↔ achados de verdade** (borda
  em foco/idle, rodapé `painel 1/2`).
- Decisão do loop EXTRAÍDA em função pura: `app::decide(state, key) -> Outcome`
  (`Quit`, `Redraw` = Ctrl+L, `Reload`, `ToggleDetail`, `Move(isize)`,
  `CyclePanel(±1)`, `Help`, `Ignore`) + `apply_move` (saturação) +
  `next_panel` (wrap) — o loop só executa o outcome; unit-tests de todas as
  transições (tui/app.rs sai de 0% de cobertura).
- `--snapshot`/não-TTY imprimem a MESMA informação em texto (contagens, colunas,
  path:linha). Nada escreve em disco — o hash do `store.db` antes/depois segue
  byte-idêntico (`tests/dashboard_e2e.rs`, agora com fixture de 3 achados em 2
  severidades e caso de findings.json corrompido).

## Fora de escopo (rejeitado por ora)

Mouse · edição de qualquer dado · gráfico temporal além de contagem · múltiplos
projetos na mesma tela (v1 mostra o projeto atual; troca de projeto é `kira dashboard
--project <id>` opcional em PR futuro) · qualquer chamada de rede ou LLM.
