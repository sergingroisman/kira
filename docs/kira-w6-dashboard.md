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

### Entregue no W6-PR3 (branch `w6/pr3-sessions-panel`)

- Painel **Sessões** (`src/tui/widgets/sessions.rs`): as últimas 50 sessões da
  trilha (`state::SESSIONS_LIMIT = 50`; a store continua consultando 500 para
  achar o último `scan` — uma query, dois consumidores). Colunas `#ID` /
  QUANDO / COMANDO / MODELO / DURAÇÃO / RESUMO (1ª linha do summary);
  seleção com `➤` + `REVERSED`; **falha destacada** (cor crítica), sucesso
  neutro; **sem scroll horizontal** (célula trunca com `…`).
- Tempo relativo **determinístico**: função pura `state::humanize_age(now,
  then)` (`há 45s` / `há 3 min` / `há 2 h` / `há 5 dias`, relógio adiantado
  satura em `há 0s`) — o `now` é capturado UMA vez por snapshot
  (`Snapshot::now_secs`); NENHUM `SystemTime::now()` dentro do widget.
  `format_duration_ms` idem (`412ms`, `1s`, `1m5s`, `1h05m`), testada com
  casos fixos.
- `state`: `SessionLine` (id/comando/modelo/duração/ok/started_unix +
  `summary_first_line` para a tabela e `summary` INTEIRO, ≤2000 chars já
  gravados pela store, para o detalhe), ordenação MAIS RECENTE PRIMEIRO
  (defensiva, por id) e truncada em 50; store legível sem sessões ⇒ lista
  vazia + causa (`sessões: — (store sem sessões)`), store ausente/ilegível ⇒
  a causa concreta da trilha — nunca inventada.
- `Enter` expande o summary da sessão selecionada num detalhe lateral (60/40,
  o mesmo `DetailWidget` dos achados) com comando, modelo, quando, duração,
  status e o summary inteiro; `Esc` volta. Detalhe segue modal.
- `Tab`/`Shift+Tab` cicla **cabeçalho → achados → sessões** (3 painéis,
  `PANEL_COUNT = 3`, rodapé `painel 1/3..3/3`, borda em foco/idle).
  `decide()` continua PURA: as transições novas (Enter no painel de sessões
  com lista vazia = `Ignore`; ciclo de 3) somam-se às existentes, todas
  unit-testadas.
- `--snapshot`/não-TTY imprimem a MESMA tabela em texto
  (`painel sessões — últimas N` + colunas). Nada escreve em disco — o hash do
  `store.db` antes/depois segue byte-idêntico (`tests/dashboard_e2e.rs`, novo
  fixture de 3 sessões com caso de store sem sessões).

### Entregue no W6-PR4 (branch `w6/pr4-wiki-panel`)

- Painel **Wiki** (`src/tui/widgets/wiki.rs`): lista de páginas lidas do
  índice derivado `wiki_page` da store pela MESMA conexão
  `Store::open_readonly_at` do resto do dashboard (read-only real), com o
  markdown REAL de cada arquivo re-lido de `.kira/wiki/` (caminho do índice,
  fallback canônico `<root>/.kira/wiki/<slug>.md`) e **limite de 256 KiB por
  página** (`WIKI_PAGE_MAX_BYTES`; leitura com `from_utf8_lossy` — byte
  quebrado nunca derruba o painel). Colunas TIPO / TÍTULO / ATUALIZAÇÃO
  (idade relativa via `humanize_age` sobre o `now` do snapshot), seleção com
  `➤` + REVERSED, **sem scroll horizontal** (célula trunca com `…`).
- **Renderizador markdown próprio e mínimo** (`src/tui/markdown.rs`), SEM
  dependência nova: título `#`→`▌`, `##`→`■`, `###`+→`·` (o `#` cru NUNCA sai
  na tela); lista `-`/`*`→`•`; bloco de código (``` → `│` sem cor de
  sintaxe); code span cru desligado; negrito/itálico desligando marcadores
  (`**x**`→`x`); link `[txt](url)`→`txt (url)`; citação `>`→`│`; régua
  `---`→`────`. HTML **nunca executado** (tags viram texto), wrap finito na
  largura dada, determinístico, e **markdown inválido não panica** (fence
  aberto, marcador solto, par vazio degradam para texto). Unit-tests de cada
  elemento + caso de lixo.
- **Busca incremental `/`** com filtro PURA `state::filter_pages(pages,
  needle)`: título E conteúdo, case-insensitive e **sem acento-sensível**
  (`fold_text`/`deaccent` — `sessao` acha `Sessão`), unit-testada. O rodapé
  vira caixa de texto (`busca wiki: <agulha>▌`) enquanto a busca está aberta;
  `Esc` cancela; `Backspace` apaga. A lista do painel é a FILTRADA (a
  seleção/clamp usam o tamanho da lista filtrada).
- **`Enter`** abre a página selecionada em tela cheia (`OpenPage` — modal, o
  painel de fundo é limpo, nada vaza) com scroll vertical `j`/`k`, `g`/`G` e
  `PgUp`/`PgDn` (passo de 10 linhas); o scroll é **clamped pelo tamanho REAL
  do conteúdo rendido dentro do widget** (nunca além do fim). `Esc` volta à
  lista. `q`/`r` continuam saindo/re-ler dentro do preview (mesmo contrato do
  modal de detalhe).
- Índice vazio com store legível ⇒ `wiki: — (wiki vazia — rode kira wiki
  reindex)`; store ausente/ilegível ⇒ a causa concreta da store (nunca
  inventada). Página ilegível no disco ⇒ preview com a nota `(conteúdo
  ausente em .kira/wiki/ — rode kira wiki reindex)`.
- `Tab`/`Shift+Tab` agora cicla **cabeçalho → achados → sessões → wiki** (4
  painéis, `PANEL_COUNT = 4`, rodapé `painel 1/4..4/4`, borda em foco/idle).
  `decide()` continua PURA: as transições novas (`OpenPage`/`ClosePage`,
  `SearchWiki`/`SearchChar`/`SearchBackspace`/`CloseSearch`,
  `ScrollPage(±n)`; `/` fora do painel de wiki = Ignore) somam-se às
  existentes (keymap ganhou `Action::SearchWiki`), todas unit-testadas.
- `--snapshot`/não-TTY imprimem a MESMA informação em texto: lista do painel
  (`painel wiki — N páginas`) + o markdown rendido de cada página (a mesma
  saída do renderizador que o `Enter` mostra na TUI). Nada escreve em disco —
  o hash do `store.db` antes/depois segue byte-idêntico
  (`tests/dashboard_e2e.rs`, fixture de 2 páginas em `.kira/wiki/` +
  `Wiki::reindex`, casos de índice vazio e hash no modo não-TTY).
- **Ainda não entregue desta spec (PR5)**: mascote + tema com `NO_COLOR`
  formalizado no sprite/`--about` (o fallback mono de paleta já existe desde
  o PR1), `kira --about` e smoke manual do dono no Windows; PR6 fica com a
  captura ASCII do dashboard no README.

## Fora de escopo (rejeitado por ora)

Mouse · edição de qualquer dado · gráfico temporal além de contagem · múltiplos
projetos na mesma tela (v1 mostra o projeto atual; troca de projeto é `kira dashboard
--project <id>` opcional em PR futuro) · qualquer chamada de rede ou LLM.
