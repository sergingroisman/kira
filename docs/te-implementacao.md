# Plano de implementação — Onda TE (PR34–PR43)

**Data**: 12/Nov/2026 · **Fonte**: [`docs/rtk-gsd-rea-openclaude-vale-trazer.md`](rtk-gsd-rea-openclaude-vale-trazer.md)
(plano de análise, mergeado na PR #50) · **Estado ancorado**: `main` em
`1f9ed80` (prompt v2 na PR #56, redact W7.1 na #55, keyring hermético na #57,
probe suite na #58, índice de ADR na #59).

Este documento transforma a tabela PR34–PR43 do plano de análise em **planos de
implementação executáveis**: escopo fechado, arquivos com âncoras de código
reais, fixtures, testes de aceite (determinismo byte a byte) e decisões abertas
com proposta default. As âncoras foram verificadas no disco em 12/Nov/2026 —
podem derivar alguns números de linha quando a onda começar; o módulo e o
ponto de inserção é que são o contrato.

**Advertência honesta**: cada PR aqui só vira código após `cargo fmt --check` →
`cargo clippy --all-targets -- -D warnings` → `cargo test` → `cargo build` →
CI 3 SOs (gates do `docs/roadmap-prs.md`). Merge é sempre humano. As decisões
abertas da seção 6 precisam do selo do dono **antes** da PR correspondente
entrar na fila de execução.

---

## 1. Estado da main que a onda toca (âncoras verificadas)

| Módulo | Âncora (verificada 12/Nov) | Consumido por |
|---|---|---|
| `src/context/mod.rs` | `CHARS_PER_TOKEN=3` (L30), `MAX_EXCERPT_CHARS=600` (L36), `MAX_PAGES=3` (L39), `MAX_CODE_EXCERPTS=8` (L42), `MAX_FINDING_LINES=3` (L45); `ContextPack` (L89) com `budget_chars` (L101), `render()` (L122), `summary()` (L157); `budget_chars()` de `KIRA_NUM_CTX` (L182); `ContextReport` (L191) **já tem** `tokens_estimate`/`chars_per_token`; `report_json()` (L268); `keywords()` (L283); `pack_partial()` (L331); `pack()` (L491) | PR34, PR38–40 |
| `src/context/repo.rs` | `CodeExcerpt` (L68) com `anchor()` (L88), `grep_text()` (L129), `grep()` (L193) | PR38, PR39 |
| `src/context/index.rs` | `KIRA_INDEX_MAX_FILES` (L31), `DEFAULT_MAX_FILES=5000` (L35), `reindex()` (L84), `search()` (L201) | PR37 (fail-open), PR38 |
| `src/triage.rs` | `build_prompt_with_context()` (L40, prompt v2 delimitado), `format_triage()` (L109), `TriageReport` (L209), `sections_of()` → `[String; 3]` (L222), `triage_file_llm()` (L237), `triage_live_llm()` (L263) | PR35, PR36 |
| `src/eval.rs` | `CAUSA_LIMIAR=0.50` (L33), `GATE_LIMIAR_PCT=70.0` (L36), `content_tokens()` (L75), `causa_score()` (L90), `file_basenames()` (L107), `seed_dir()` (L342), `gate()` (L458) | PR35 (similaridade), PR36 (tags), medição |
| `src/store/mod.rs` | `SCHEMA_VERSION=3` (L34); migração idempotente por `PRAGMA user_version` + `CREATE TABLE IF NOT EXISTS` (L608–716); `sessions(id, project_id, command, args, model, started_at, duration_ms, ok, summary)`; `repo_file`/`repo_fts` (v3, ADR-0021) | PR34 (ledger) |
| `src/llm/prompts.rs` | `TRIAGE_PROMPT_VERSION="v2"` (L37), `TRIAGE_SYSTEM_PROMPT` (L45), `TRIAGE_SYSTEM_PROMPT_FNV1A64="5f1ac3ad91f003d1"` (L56), `triage_prompt_id()` (L64), `TRIAGE_SECTIONS=[3]` (L69), `TRIAGE_USER_PREAMBLE`/`TRIAGE_UNTRUSTED_END` (L82/L87) | PR36 (v3) |
| `src/wiki/mod.rs` | `Wiki::open()` (L89), `read()` (L152), `list()` (L166), `search()` FTS5 (L303), `reindex()` (L278) | PR35 (candidatos) |
| `src/wiki/episode.rs` | `Episode` (L44), `EpisodeTarget` (L59, Staging), `page_for()` (L69), `render()` (L93), `record()` (L115), `collapse_old_episodes()` (L167); seções `## Episódio ...` / `## Histórico colapsado` | PR35, PR36 |
| `src/binresolve.rs` | `bin_from_env()` (L21), `find_on_path()` (por SO, L30/66/83), `is_cmd_script()` (L92), `first_stderr_line()` (L102) | PR41 |
| `src/credentials.rs` | resolução flag > env > `config.toml` (L418+), `CONFIG_KEY_DEV_NAME` (L42), `config_dir()` (L254) | PR40 (chave nova) |
| `src/setup.rs` | padrão setup-ensina (ADR-0025): `pat_lesson()` (L696), `msg_pat_ok/401/403/rede` (L876–895), `format_summary()` (L999), `SetupOptions` (L56) | PR41 (pergunta) |
| `src/scan/{node,dotnet}.rs` | parse de JSON das ferramentas — **sem tratamento de BOM hoje** (grep confirmou ausência) | PR37 |
| `src/tui/widgets/` | `findings.rs` (`FindingsWidget` L22, `column_widths()` L68, `cell()` L81), `sessions.rs`, `wiki.rs` | PR43 |
| `src/main.rs` | subcomandos: `Doctor, Triage, Setup, History, Note, Profile, Scan, Wiki, Dashboard, Pr` — **sem** `Approve`/`Loop` ainda | PR35 (`--no-cache`) |
| `.gitignore` | só `/target` — `.kira/` está descoberto | PR37 |
| Testes de padrão | `tests/triage_prompt_fixtures.rs` (bytes do prompt), `tests/context_e2e.rs` (gate M1), `tests/dashboard_e2e.rs` (read-only provado, hash byte-idêntico), `tests/llm_allowlist.rs`, `tests/scan_e2e.rs`, `tests/store_e2e.rs`, `tests/wiki_e2e.rs`, `tests/setup_e2e.rs` | todos |
| Fixtures | `tests/fixtures/{prompts,redact,scan,triage,triage_gold_seed}` + `work_item_4821.json`/`work_item_minimal.json` | todos |

Fatos que reancoram o plano de análise:

- **Prompt v2** (PR #56): o system prompt já tem bloco delimitado anti-injeção.
  O PR36 não cria mecanismo novo — muda **texto** versionado + hash.
- **`ContextReport` já estima tokens** (`tokens_estimate = used_chars / 3`): o
  PR34 não inventa estimador, só o **propaga** para o header do pack e o `--json`.
- **`src/loop/` não existe**: PR42 é doc-only (spec W5), sem código.
- **`tests/fixtures/triage_gold/` não existe**: só a seed com 3 itens. Rotular
  os 20 bugs é pré-requisito do dono para **medir** PR35/36 (gate ≥70%), não
  para escrever o plano.

---

## 2. Regras da onda (não negociáveis)

1. **Gates por PR**: `cargo fmt --check` → `clippy --all-targets -- -D warnings`
   → `test` → `build` → CI 3 SOs. Docs-only (PR42) roda os mesmos gates.
2. **Branch naming**: `te/pr<n>-<slug>` (ex.: `te/pr35-cache-raciocinio`). Um PR
   por item, agrupados pela onda TE.
3. **Determinismo**: mesmo estado ⇒ saída byte-idêntica onde o plano indica.
   Nuância do ledger (PR34): a tabela SQLite tem `id`/`started_at` por design
   (AGENTS.md manda RFC 3339 UTC) — o gate byte a byte vale para os **artefatos
   derivados** (header do pack, campos de valor do `--json`), não para as linhas
   do DB.
4. **Dependências novas**: nenhuma. Env nova: nenhuma (PR41 propõe
   `KIRA_REA_BIN` por convenção com `KIRA_SCAN_<TOOL>_BIN` — justificada na PR;
   PR40 usa chave de `config.toml`, não env). Qualquer acréscimo precisa de
   justificativa explícita no PR.
5. **Segredo e redação**: nada cru no prompt, DB, log ou disco — toda escrita
   derivada passa por `crate::redact` (padrão ADR-0021).
6. **`.kira/` é local**: nunca comitado (PR37 fecha o mecanismo); snapshots em
   `docs/dumps/` continuam locais, fora do git (decisão do dono).
7. **CLI-first, LLM-last**: o pulo do gato (PR35) é exatamente isso — hit de
   cache **não chama LLM**. Similaridade é cálculo em Rust (nível 3 da escada).
8. **Fail-open**: wiki, índice, store ou findings que falharem viram nota — o
   comando principal continua (padrão vigente do packer).

## 3. Ordem de execução e dependências

```
PR34 (medir) ──► PR35 (cache) ──► PR36 (3 estados) ─┐
               └────────────► PR37 (raw + .kira)    ├─► PR41 (rea)
PR38 ──► PR39 ──► PR40 (perfil coordena caps) ──────┘
PR42 (doc W5) ── independente ──► PR43 (painel, W6)
```

- **PR34 primeiro** porque o ledger é quem **prova** a economia do cache
  (`llm_calls=0` no hit) — sem medidor, o pulo é fé.
- **PR35 antes do 36**: o formato do episódio ganha `fonte: cache-wiki` no 35;
  o 36 acrescenta os 3 estados nesse formato já assentado.
- **PR43 fica para W6** (pós-gate M2); PR42 pode entrar em qualquer ponto.

---

## 4. PR34 — Ledger de economia

**Objetivo**: medir antes de otimizar (RTK: `rtk gain`). Toda sessão grava o
que o pack consumiu vs o teto, quantas chamadas de LLM custou, e o `--json`
exporta os números — com sinal de **pack ocioso** (orçamento desperdiçado é
suspeita de furo de coleta, análogo ao `rtk discover`).

**Escopo fechado**: sem mudança de comportamento do pack (bytes do conteúdo
das seções intocados); sem dashboard (PR43); sem cache (PR35); gravação do
ledger é fail-open (`aviso: trilha não foi gravada (<causa>)` só no `main.rs`,
padrão vigente).

**Arquivos**:

- `src/store/mod.rs` — **schema v4**: nova tabela
  `session_economy(session_id INTEGER PRIMARY KEY REFERENCES sessions(id),
  pack_chars INTEGER, budget_chars INTEGER, tokens_est INTEGER,
  llm_calls INTEGER, route TEXT NOT NULL DEFAULT 'llm', pack_idle INTEGER)`.
  A coluna `route` nasce aqui para o PR35 preencher sem nova migração.
  Migração idempotente: bump de `SCHEMA_VERSION` (L34 → 4) + `CREATE TABLE IF
  NOT EXISTS` (padrão L608–716).
- `src/context/mod.rs` — `render()` ganha **header de 1 linha**:
  `~N tokens (chars X / 3) dentro do teto Y (fonte: index|grep|—)` — números
  derivados de `budget_chars`/chars usados (estimador já existe em
  `ContextReport`). `report_json()` (L268) serializa os mesmos campos.
- `src/triage.rs` (e demais comandos que gravam sessão: `inspect`, `doctor` —
  confirmar os call sites do `store::record_*` na execução) — preenche o
  ledger com o pack construído.
- `src/doctor.rs` / `src/format.rs` — `kira history`/`profile` exibem os
  números; **ausente ⇒ `—` + causa**, nunca inventado.

**Fixtures**: nenhum novo arquivo de ferramenta; teste de schema v3→v4 em
`tests/store_e2e.rs` (DB velho abre e ganha a tabela).

**Testes de aceite**:

- Duas execuções no mesmo estado ⇒ **header do pack byte-idêntico** e campos
  de valor do `--json` idênticos (ids/timestamps do DB fora do gate, conforme
  seção 2.3).
- `findings.json` continua byte-idêntico (PR não o toca).
- Store falho ⇒ comando principal segue com `aviso:` no stderr (fail-open).
- `context_e2e` (gate M1) continua: mesmo estado ⇒ mesmo pack — o header é
  função dos mesmos chars.

**Decisão aberta D9**: limiar de **pack ocioso** — proposta: `pack_chars <
25% do teto` ⇒ nota `aviso: pack ocioso (X% do teto) — provável furo de
coleta (índice ausente/vazio?)` no stderr. Barra ajustável por const.

---

## 5. PR35 — Cache de raciocínio (o pulo do gato)

**Objetivo**: triage de item cujo episódio anterior na wiki casa com
similaridade ≥ 0.50 **prende a causa sem chamar LLM** (custo zero de token na
rota acertada). A fonte aparece como `cache-wiki` no output; `--no-cache`
desliga (padrão `rtk bypass`: o bypass também é rastreado no gain).

**Escopo fechado**: sem embedding, sem tool-calling, sem FTS vetorial —
retrieval determinístico (`keywords()` + FTS5 + similaridade de tokens em
Rust). Sem escrita em `.kira/wiki/` fora do padrão vigente do `triage`. A
rota de hit **não importa nada de `llm::*`** (CLI-first: hit não fala com
modelo; `llm_allowlist` continua verde).

**Arquivos**:

- `src/triage.rs` — ponto de entrada: antes de `triage_live_llm()`,
  `cache_lookup(item, wiki, store)` devolve `Option<CachedDiagnosis>`
  (causa/arquivos/confiança/fonte). Hit ⇒ monta `TriageReport` sem LLM e
  imprime `fonte: cache-wiki`; miss ⇒ rota de hoje, byte a byte. Flag
  `--no-cache` no subcomando `Triage` (`src/main.rs`).
- Similaridade: reusa `eval::content_tokens` (L75) com interseção/Jaccard;
  limiar em const própria `CACHE_LIMIAR: f64 = 0.50` (comentário explica o
  **porquê** de coincidir com `CAUSA_LIMIAR`, sem importar de eval para não
  acoplar semântica de gold a cache).
- `src/wiki/episode.rs` — extração: parse de página de episódio (L93/render) →
  `Option<Episode>` com as 3 seções; `page_for()` (L69) para o lookup direto
  por id do item.
- `src/wiki/mod.rs` — candidatos: `Wiki::search()` (FTS5, L303) com as
  `context::keywords()` do item, top-`K`.
- `src/store/mod.rs` — o hit grava `route='cache-wiki'` e `llm_calls=0` no
  ledger (PR34).
- `src/triage.rs` (`--eval`) — mede as **duas rotas**: a rota LLM roda sempre
  (verdade de ouro contra o gold), e o relatório conta
  `hits de cache: N (economia estimada: M chamadas)` — o gate ≥70%
  (`GATE_LIMIAR_PCT`, L36) não muda.

**Fail-open**: wiki ausente/vazia ou índice sem resultado ⇒ rota normal de
hoje + nota `contexto: cache-wiki indisponível (<causa>)` no stderr. Item novo
fora da wiki ⇒ **comportamento idêntico ao de hoje, byte a byte** (gate).

**Fixtures**: episódio de wiki sintético em `tests/fixtures/triage/` (2
episódios: 1 casa ≥0.50, 1 não casa) + item JSON que casa e um que não casa.
Rótulos do dono em `tests/fixtures/triage_gold/` (20 itens) validam a métrica
**com número** — sem eles, o PR não promove autonomia (só adiciona a rota).

**Testes de aceite**:

- Mesmo estado (wiki + item) ⇒ decisão de cache idêntica e saída byte-idêntica
  nas duas rotas (hit e miss).
- Hit ⇒ zero chamadas ao backend LLM (backend de teste que **falha** se
  chamado, no estilo dos testes de scan).
- `--no-cache` ⇒ rota LLM sempre, byte-idêntica à main de hoje.
- `--eval` com cache ⇒ percentual do gate inalterado + contagem de hits.
- `llm_allowlist` continua passando (nenhum import novo de `llm::*`).

**Decisões abertas**:

- **D1** — Hit grava episódio novo? Proposta: **não** — página de bug não é
  log de conversa (AGENTS.md); o hit fica no output e no ledger. A página
  original já guarda a conclusão; regravar viraria log de re-execução.
  (O plano de análise diz "fonte no episódio" — este ponto precisa do selo.)
- **D2** — Confiança no hit: proposta = a confiança **da página original**,
  capada em `"média"` (nunca `"alta"` vinda de cache).
- **D3** — `K` de candidatos FTS5: proposta `K=3` (mesmo espírito de
  `MAX_PAGES`).

---

## 6. PR36 — Evidência em 3 estados (observado/inferido/desconhecido)

**Objetivo**: cada arquivo citado no output e no episódio carrega o estado da
evidência (lição do rea, AGENTS.md deles: "distinguish observations,
derivations, inferences, and unknowns") — o leitor humano (e o dono) sabe o
que foi **visto** no pack vs o que o modelo **deduziu**.

**Escopo fechado**: as 3 seções do output continuam 3 (`TRIAGE_SECTIONS`, L69)
— os estados entram como **tag por arquivo** dentro de "Arquivos prováveis",
não como seção nova (evita mexer em `sections_of()` → `[String; 3]` L222,
eval, TUI e prdraft de uma vez). Sem mudança no pack (PR38/39 cuidam dele).

**Arquivos**:

- `src/llm/prompts.rs` — regra nova no `TRIAGE_SYSTEM_PROMPT` (L45):
  "cada arquivo citado em Arquivos prováveis recebe no fim `(observado)` se o
  caminho:linha veio do contexto, `(inferido)` se você deduziu, ou responda
  `não identificado` (`TRIAGE_FILES_UNKNOWN`, L76) quando não há arquivo".
  **`TRIAGE_PROMPT_VERSION` → `"v3"`** (L37) e
  `TRIAGE_SYSTEM_PROMPT_FNV1A64` recomputado (L56) — o teste
  `triage_prompt_fixtures` recomputa FNV-1a e reprova se a const estiver
  desatualizada (padrão PR7).
- `src/triage.rs` — `format_triage()` (L109) tolera as tags (não as descarta);
  `build_prompt_with_context()` (L40) instrui com a regra do system prompt.
- `src/eval.rs` — `file_basenames()` (L107) **deve** extrair basenames de
  `src/foo.rs (observado)` sem comer a tag; ajuste defensivo + teste.
- `src/wiki/episode.rs` — o episódio grava as linhas já tagueadas (mudança
  nenhuma no formato: é conteúdo da seção).
- `tests/fixtures/prompts/` — fixtures byte a byte atualizadas para v3.

**Testes de aceite**:

- Prompt v3 byte-idêntico entre execuções; hash da const == FNV-1a do prompt
  (teste reprovando const velha).
- `--eval` **não regride**: com gold do dono, percentual medido antes/depois
  do bump (rótulos existentes continuam parseáveis).
- Saída de triage com tag `(inferido)` continua parseável por `sections_of()`
  e pelo painel.

**Decisão aberta D4**: formato da tag — proposta `(observado)`/`(inferido)`
entre parênteses no fim da linha do arquivo; arquivo desconhecido usa
`TRIAGE_FILES_UNKNOWN` (comportamento de hoje) e vale como `desconhecido`.
Alternativa recusada: 4ª seção "Evidência:" (quebra `[String; 3]` em cadeia).

---

## 7. PR37 — Digest bruto no scan + mecanismo `.kira/` + BOM-sniff

**Objetivo**: (a) guardar o **digest bruto** (redigido) do output das
ferramentas de scan com endereço de linha, para citação posterior com
proveniência (padrão do packer: nada de blob anônimo); (b) fechar o mecanismo
de "`.kira/` nunca comita" que hoje **não existe** (`.gitignore` só tem
`/target`); (c) absorver o item P1 do relatório do gitingest — BOM-sniff no
parse dos JSONs das ferramentas.

**Escopo fechado**: `findings.json` **não muda um byte** (o raw é artefato
irmão); sem upload de nada; sem ferramenta nova; o BOM-sniff é strip de BOM
antes do parse, não detecção de encoding generalizada.

**Arquivos**:

- `src/scan/mod.rs` — após parse de cada ferramenta, escreve
  `.kira/raw/<tool>.json`: linhas do output já passadas por `crate::redact`
  (redação na escrita, padrão ADR-0021), header de 3 linhas
  `# tool: <tool>` / `# exit: <N>` / `# lines: <N>`, cap de **512 KiB**
  truncado com linha marcadora `# truncado em <N> bytes`. Nome fixo por
  ferramenta ⇒ overwrite é a **rotação** (mais recente vence) — sem diretório
  crescendo, sem env nova.
- `src/scan/node.rs` / `dotnet.rs` — strip de UTF-8 BOM antes do parse
  (função pura de `String`, testável); parse que falha com BOM presente diz
  o que fazer ("output veio com BOM — strip aplicado; verifique a ferramenta").
- `.gitignore` — ganha `.kira/` (cobrindo `wiki-staging/` junto) e
- `.kira/README.md` — nota escrita pelo KIRA: "diretório local, nunca
  comitado" (mandato do AGENTS.md que não existe neste clone).
- `src/context/mod.rs` — nada (o raw não entra no pack nesta onda; consumo
  futuro cita `caminho:linha` do raw).

**Fixtures**: JSON de npm/pnpm com BOM em `tests/fixtures/scan/` (o parse
passa com strip) + fixture de output truncado (>512 KiB sintético).

**Testes de aceite**:

- Duas execuções no mesmo estado ⇒ `.kira/raw/<tool>.json` **byte-idêntico**
  (nome fixo, overwrite, sem timestamp).
- `findings.json` byte-idêntico ao de hoje (mesmo estado).
- Linha com PAT no output da ferramenta sai `[REDACTED]` no raw.
- `.gitignore` impede `git add .kira/` (teste de texto do arquivo, padrão
  docs-only).

**Decisões abertas**:

- **D5** — cap do raw: proposta **512 KiB por arquivo, fixo em const** (sem
  env nova; se um dia precisar, justifica-se na PR).
- **D6** — raw redigido na escrita: proposta **sim** (conservador, custo ~zero).

---

## 8. PR38 — Dedup + agrupamento nos trechos do pack

**Objetivo**: cortar token repetido dos excerpts (barra do RTK: cada filtro só
entra se cortar ≥ 20%). Linhas idênticas **contíguas** colapsam com contador;
âncora `caminho:linha` da primeira ocorrência preservada.

**Escopo fechado**: dedup **intra-excerpt e contíguo** apenas — dedup global
entre arquivos quebra a semântica de âncora (proposta: recusado); sem mudar a
ordem/ranking do packer (determinismo M1); caps (`MAX_*`) intocados (PR40).

**Arquivos**:

- `src/context/repo.rs` — `grep_text()` (L129): colapso de runs contíguos de
  linhas repetidas em uma linha marcadora `<N linhas repetidas>`; `anchor()`
  (L88) continua apontando a primeira ocorrência.
- Fixtures sintéticas: `tests/fixtures/context/dedup/` (arquivo .rs com runs
  de linhas repetidas) — novo subdiretório de fixture.

**Testes de aceite**:

- Fixture mostra corte **≥ 20%** de chars no excerpt (const de barra
  `DEDUP_MIN_CUT_PCT: f64 = 20.0` — se um dia a estratégia não atingir, o
  teste reprova a adoção dela, não o código em produção).
- Âncora preservada: `path:linha` da primeira ocorrência no excerpt colapsado.
- Gate M1: mesmo estado ⇒ mesmo pack (colapso é função pura das linhas).
- Fixtures de prompt atualizadas na mesma PR (o pack muda bytes ⇒
  `triage_prompt_fixtures` + golden do `context_e2e` regenerados com revisão).

**Decisão aberta D7**: formato do colapso — proposta: linhas repetidas
omitidas + marcador literal `<N linhas repetidas>` (contagem exata,
determinístico, sem inventar conteúdo).

---

## 9. PR39 — Repo map determinístico no brief (~200 tokens)

**Objetivo**: o brief do pack ganha um **mapa** do repositório (lista de
caminhos, sem conteúdo) para o modelo saber o que existe sem gastar trecho —
até ~200 tokens (600 chars a 3 chars/token), mesma régua de `MAX_EXCERPT_CHARS`.

**Escopo fechado**: mapa é **apêndice do brief** (prioridade 1 vigente) — não
muda a ordem de prioridade do orçamento; sem árvore visual, sem contagem de
linhas por arquivo; caminhos só.

**Arquivos**:

- `src/context/mod.rs` — na montagem do brief (`pack_partial`, L331): bloco
  `MAPA:` com ≤ 600 chars, ≤ 20 caminhos, um por linha, truncado com marcador
  `... (+N arquivos)`. Ranking reusa a regra vigente: arquivos citados em
  `findings.json` primeiro, depois termos casados, depois ordem de caminhada —
  mesma ordenação dos excerpts.
- `tests/context_e2e.rs` — golden do mapa por estado.

**Testes de aceite**:

- Mesmo estado ⇒ mesmo mapa byte a byte (extensão do gate M1).
- Arquivo citado em `findings.json` aparece antes no mapa (regra vigente).
- Mapa vazio (repo vazio) ⇒ bloco omitido, pack byte-idêntico ao de hoje.

**Decisão aberta D8**: valores — proposta `≤20 arquivos / ≤600 chars`; formato
`- <caminho>` (um por linha). Alternativa: entrar na seção de trechos
(recusada — gastaria orçamento de evidência).

---

## 10. PR40 — `token_profile` em `config.toml`

**Objetivo**: o dono escolhe o ponto na curva custo/qualidade **uma vez** —
um perfil coordena teto e caps de pack (hoje os caps são consts soltas):
`budget` (econômico), `balanced` (default = hoje), `quality` (gasta mais,
cita mais).

**Escopo fechado**: perfil coordena **caps**, não o teto de janela
(`KIRA_NUM_CTX` continua mandando no `budget_chars()` L182); sem env nova
(flag `--profile` > chave `token_profile` em `config.toml` > default
`balanced`); inválido ⇒ `balanced` + `aviso:` (nunca erro).

**Arquivos**:

- `src/credentials.rs` — nova chave `token_profile` resolvida no padrão da
  casa (L418+: flag > env > config.toml — aqui sem env, documentado no PR
  **com justificativa**).
- `src/context/mod.rs` — consts `MAX_*` (L36–45) viram `Caps` por perfil:
  `budget {excerpts:4, pages:2, excerpt_chars:400, finding_lines:2}`,
  `balanced {8, 3, 600, 3}` (== hoje), `quality {12, 4, 800, 4}`. `Caps` é
  struct estático — determinismo por perfil.
- `tests/context_e2e.rs` — **3 packs dourados** (um por perfil), cada um
  byte-idêntico entre execuções.

**Testes de aceite**:

- Três estados ⇒ três packs distintos e reprodutíveis (gate M1 ×3).
- `token_profile: "esquisito"` ⇒ `balanced` + `aviso:` no stderr.
- Sem chave em `config.toml` ⇒ comportamento byte-idêntico ao de hoje
  (`balanced`).

**Decisão aberta D10**: valores exatos dos perfis (tabela acima) — o dono sela.

---

## 11. PR41 — `kira rea` wrapper + pergunta no setup

**Objetivo**: dar ao dono o wrapper do **rea** (revisor de código, 4º repo
estudado) no padrão da casa: binário resolvido no PATH antes do spawn, output
nunca cru no prompt (redact + teto), erro acionável sem spawn — e o `setup`
ensina que o comando existe (padrão ADR-0025).

**Escopo fechado**: sem daemon, sem integração com LLM, sem escrever nada no
board/repo; o wrapper é read-only (roda o rea, imprime redigido, capado).
A pergunta do setup **não grava config** — é aula + atalho (D11).

**Arquivos**:

- `src/rea/mod.rs` (**novo**) — justificado no PR: lógica específica do rea
  (args, output) fora do `binresolve` genérico. Resolve via `binresolve`:
  override `KIRA_REA_BIN` (mesma convenção de `KIRA_SCAN_<TOOL>_BIN` —
  justificativa no PR; valor vazio/inexistente ⇒ "instale o rea (…)" **sem
  spawn**), depois PATH × PATHEXT. Spawn com args passados, captura
  stdout/stderr, cada linha por `crate::redact`, cap de saída **8 KB**
  (coerente com `MAX_EXCERPT_CHARS` ×n), imprime. Falha do rea ⇒
  `a ferramenta rea falhou (exit N) (dica: …)` com primeira linha de stderr
  trimada ≤200 chars (`first_stderr_line`, L102) — sem body cru.
- `src/main.rs` — subcomando `Rea { args }` (passthrough).
- `src/setup.rs` — pergunta pós-ADR-0025: **aula primeiro** (o que é o rea,
  por que perguntamos, o que acontece se pular), menu `1) ensinar o comando
  agora` / `2) pular` (default `2`), não bloqueia, invalida no máx. 2x e
  assume default com aviso (mesmo contrato do passo PAT).

**Testes de aceite** (padrão `scan_e2e`):

- `KIRA_REA_BIN` aponta shim que imprime output com segredo ⇒ sai redigido e
  capado; shim exit 1 ⇒ erro compacto acionável; binário ausente (PATH vazio)
⇒ "instale o rea" sem spawn.
- Setup: pergunta ensina e não grava nada (e2e hermético, `KIRA_SECRET_STORE=off`).

**Decisão aberta D11**: pergunta do setup — proposta **teaching-only**
(nenhuma chave nova em `config.toml`; se o rea está no PATH, `kira rea`
funciona — o setup só ensina). Alternativa: persistir `rea_enabled`
(recusada: chave sem consumidor).

---

## 12. PR42 — Pressão de budget na spec W5 (doc-only)

**Objetivo**: a spec do loop (`docs/kira-w5-loop.md`) ganha watchdogs
**numerados** de pressão de budget — consistência com o mandato (50
chamadas/dia) antes do loop existir em código (`src/loop/` não existe na main).

**Escopo fechado**: nenhum código; nenhum gate novo de CI; a PR é docs-only e
roda os gates de sempre.

**Arquivos**: `docs/kira-w5-loop.md` — seção nova "Watchdogs de budget":

- **50%** (25/50 chamadas): nota no output do ciclo;
- **75%** (37/50): aviso + sugestão explícita de reduzir caps
  (`token_profile: budget`, PR40) antes do próximo ciclo;
- **90%** (45/50): parar e registrar pendência no store — **nunca** gasta a
  reserva; budget estourado **≠** aprovação (regra vigente).

**Testes de aceite**: doc consistente com o mandato (revisão do dono); links
internos resolvidos. Decisão aberta **D12**: os percentuais 50/75/90
(proposta na tabela acima).

---

## 13. PR43 — Painel de economia no dashboard (W6)

**Objetivo**: o dashboard (Ratatui) mostra a economia medida pelo PR34 — pack
vs teto, chamadas, hits de cache — sob o contrato vigente do AGENTS.md
(**read-only provado**: nada de disco/rede/LLM, store
`SQLITE_OPEN_READ_ONLY`). O painel é widget de exibição e **não adiciona
tecla de ação** — escrever segue proibido nele. Nota: se o contrato de ações
confirmadas (ADR-0026, em discussão na main) prevalecer, a prova read-only
desse painel roda via `kira dashboard --read-only` — o panel em si não muda.

**Escopo fechado**: **1 campo por PR** (regra do plano): esta PR entrega o
painel com o primeiro campo (`tokens_est vs budget_chars` da sessão mais
recente); campos seguintes (`llm_calls`, `hits cache-wiki`) viram PRs pequenos
posteriores. Nenhuma ação nova no painel. Timeline: **W6** (pós-gate M2) —
não entra na fila agora.

**Arquivos**:

- `src/tui/widgets/economy.rs` (**novo**) — padrão de `findings.rs`
  (`FindingsWidget` L22, `column_widths()` L68, `cell()` L81): dado ausente ⇒
  `—` + causa, nunca inventado.
- `src/tui/app.rs` — decisão de render continua na função pura `tui::app::decide`
  (nada de tecla nova escrevendo em disco).

**Testes de aceite** (padrão `dashboard_e2e`):

- Hash do `store.db` **byte-idêntico** antes/depois de rodar `kira dashboard`
  (com e sem `--snapshot`); se o contrato ADR-0026 prevalecer, a prova roda em
  `--read-only` (e2e deve cobrir os dois modos).
- Sem ledger na sessão ⇒ campo `—` com a causa (fail-open do store).

---

## 14. ADR-0028 — cache de raciocínio e economia medida

Draft já aceite na análise (PR #50, seção final). Vira arquivo
`docs/adr/ADR-0028-cache-raciocinio-e-economia-medida.md` na PR do plano de
implementação, com:

- **Decisão**: hit de wiki (similaridade ≥ 0.50) prediz a causa **sem LLM**;
  toda economia é medida (ledger PR34) — `chars/3` (PT-BR, conservador vs
  `bytes/4` do RTK), barra de filtro ≥ 20%, sinal de pack ocioso.
- **Alternativas recusadas**: embeddings/índice vetorial (contraria
  RETRIEVAL-FIRST, ADR-0015); tool-calling/LangGraph (idem); mudar estimador
  para `bytes/4` (PT-BR é mais denso — 3 chars/token fica).
- **Consequências**: PR35 promove autonomia **só com ≥70% no `--eval`** com
  os 20 rótulos do dono (GATE M2); cache errado é detectável por fonte
  (`cache-wiki`) e desligável (`--no-cache`).

---

## 15. Decisões abertas para o dono sela (resumo)

**SELO DO DONO (12/Nov/2026): D1 selada como "não gravar" e D2–D12 seladas
como propostas.** Os defaults abaixo são agora contrato; cada PR cita a
decisão selada que consumir.

| # | PR | Decisão | Proposta default (selada) |
|---|---|---|---|
| D1 | 35 | Hit grava episódio? | **Não** (página não é log); hit fica no output + ledger |
| D2 | 35 | Confiança no hit | Da página original, capada em `"média"` |
| D3 | 35 | Candidatos FTS5 | `K=3` |
| D4 | 36 | Formato dos 3 estados | Tag `(observado)`/`(inferido)` na linha do arquivo; 3 seções mantidas |
| D5 | 37 | Cap do raw | 512 KiB fixo por arquivo (const, sem env) |
| D6 | 37 | Raw redigido na escrita | Sim |
| D7 | 38 | Formato do colapso | Marcador literal `<N linhas repetidas>` |
| D8 | 39 | Valores do mapa | ≤20 arquivos / ≤600 chars |
| D9 | 34 | Limiar de pack ocioso | `< 25% do teto` ⇒ nota |
| D10 | 40 | Valores dos perfis | budget `{4,2,400,2}` · balanced `{8,3,600,3}` · quality `{12,4,800,4}` |
| D11 | 41 | Pergunta do setup | Teaching-only (não grava config) |
| D12 | 42 | Watchdogs | 50% nota · 75% aviso+sugestão · 90% para+pendência |

Seladas as propostas sem objeção, os PRs entram na fila com os defaults; o
dono pode trocar qualquer valor aqui antes da PR correspondente — o plano
fica honesto no git (cada PR cita a decisão selada).

## 16. Pendências do dono (inalteradas)

1. **Rotular os 20 bugs** em `tests/fixtures/triage_gold/` — bloqueia **medir**
   PR35/36 (gate ≥70%); não bloqueia escrever os PRs.
2. **Sellar as decisões D1–D12** (ou aceitar os defaults).
3. **Merge** é sempre humano — a onda entra PR a PR na ordem da seção 3.
