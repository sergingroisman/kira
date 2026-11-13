# ROADMAP de PRs — KIRA (ledger das ondas)

> Fonte de verdade: `docs/kira-mandato.md` (v3) + `docs/kira-judge-veredicto.md` +
> plano v2 §8. Este arquivo existe porque o mandato nomeia **PR1–PR18 sem enumerá-los
> em nenhum arquivo** — aqui a numeração passa a ter conteúdo, aceite e status.
>
> Regra: PR sem definição fechada aparece como **a confirmar**. Não invente escopo —
> pergunte ao dono antes de codar.

## Regras de operação (não negociáveis)

1. **Um item = um PR.** Branch `w<onda>/pr<n>-<slug>`, PR em draft até o selo.
2. **Nunca merge da branch `dev`.** Histórias desconectadas (POC v1 Bun/Ink/TS). Só
   conceito, lido por `git show origin/dev:<path>`. Só sobrevive o que respeita o
   contrato da main: binário único, read-only, Ollama local, SQLite `bundled`.
3. **Delta escreve o código; Orchestrator sela** (`cargo fmt --check` ·
   `cargo clippy --all-targets -- -D warnings` · `cargo test` · `cargo build` ·
   CI 3 SOs · push). Nenhum PR entra em `main` sem o selo.
4. **Orquestrador não escreve nos arquivos do PR aberto da Delta.** Código = Delta;
   docs/ledger/gates = Orchestrator. Colisão de worktree é o único risco real aqui.

## Decisões de processo (dono, 11/Nov/2026)

- **PR por item, agrupado por onda** — um PR por item para isolar falha no CI; a onda é o
  agrupamento temático, não um PR único gigante.
- **Cobertura**: mínimo de **70%**, medido **localmente** no MVP (`cargo llvm-cov`); gate no
  CI só depois do MVP fechar.
- **Merge é sempre humano.** Agente/automação abre PR e só mergeia PR que ele mesmo criou,
  com CI verde, quando o dono autorizou.

## Gates (todo PR)

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
GitHub Actions CI (ubuntu/macos/windows)
```

---

## W1 — segurança/erros (PR1–4)

Porta conceitual da `dev`: `core/src/safety/{secrets_filter,path_guard,ssrf_guard}.rs`,
`core/src/*/error.rs`. Nada de código TS/Bun; reimplementação no contrato da main.

| PR | Objetivo | Arquivos | Aceite | Status |
|---|---|---|---|---|
| **PR1** | Contrato escrito ao v3: README + AGENTS.md com `awaiting_approval`/`kira approve`, gate W2-bis, TUI só pós-M2, sem daemon, sem blog autônomo | `README.md`, `AGENTS.md` | Docs sem contradição com o mandato v3; `cargo test` verde | **proposto** — pronto para a Delta |
| **PR2** | Filtro de segredos: máscara de PAT/token/CPF/e-mail em qualquer log, erro ou prompt | `src/redact.rs` (novo), chamadas em `format.rs`/`triage.rs`/`llm/prompts.rs` | Fixtures de segredo (PAT, token em URL, CPF, e-mail) saem mascaradas; teste prova que o prompt não contém segredo | **proposto** |
| **PR3** | Guarda de caminho: deny-list de escrita (ex.: `.git/`, `target/`, `.kira/` fora do root, `~/.config/kira`) | `src/guard/path.rs` (novo) | Escrita em deny-list retorna erro acionável e **não toca o disco**; teste com path traversal | **proposto** |
| **PR4** | Taxonomia de erro acionável única em `src/error.rs`: toda falha diz o que fazer; nunca body/stderr cru | `src/error.rs`, `src/ado/`, `src/scan/` | Teste por categoria de erro asserting a mensagem acionável; zero `unwrap`/`expect` em rede/parse/input | **proposto** |
| **PR27** | Jail mínimo do `fix --apply`: confinamento por canonicalização + rlimits (`cfg(unix)`); Windows sem rlimits no MVP | `src/guard/jail.rs` (novo) | Escrita fora do root canonicalizado é recusada mesmo com `..`, symlink e junction; ADR-0011 | **aceito** (dono, 10/Nov) — entra no M3 |
| **PR33** | Enforcement do CLI-first/LLM-last (ADR-0016): teste de allowlist de módulos + comentário `// LLM porque:` obrigatório | `tests/llm_allowlist.rs` (novo) | `cargo test` reprova `llm::*` fora de `triage.rs`/`loop/`/`wiki/`, e reprova call site sem justificativa | **aceito** (dono, 10/Nov) |

`ssrf_guard` (bloquear URL de terceiro) fica para quando existir fetch arbitrário
(pós-PR11); **não** entra na W1 — hoje o único destino é Azure/Ollama.

## W2 — prompts/eval (PR5–8)

Onda de contexto e prompt. Âncoras conhecidas: prompts versionados em
`src/llm/prompts.rs` (constantes revisadas em PR), fixtures por prompt, e o
**context packer** que o M2 exige (o `kira triage` ao vivo de 10/Nov provou a
ausência: "Arquivos prováveis" saiu descritivo).

| PR | Objetivo | Status |
|---|---|---|
| PR5 | Context packer v1: item + findings do arquivo + trechos, com teto de tokens (`KIRA_NUM_CTX=4096`) | escopo detalhado **a confirmar** |
| PR6 | Prompt de triage v1 + fixtures de entrada/saída | **feito** — `TRIAGE_PROMPT_VERSION=v1` com regras de grounding; `tests/fixtures/triage/<slug>/` (`item.json` + `model.txt` + `prompt.txt` + `expected.txt`) comparados byte a byte em `tests/triage_prompt_fixtures.rs`; `.gitattributes` fixa LF |
| PR7 | Prompt review: `prompts.rs` público, revisado, com teste de regressão de texto | **feito** — `TRIAGE_SYSTEM_PROMPT_FNV1A64` + `triage_prompt_id()` no stderr; dourado `tests/fixtures/prompts/triage-system.txt` byte a byte com LF fixado; hash recomputado por teste (texto e `TRIAGE_SECTIONS` numa origem só) |
| PR8 | Backend LLM por trait (`LlmBackend`): ollama + caminho explícito para copilot/byok | **a confirmar** |

## W2.5 — Wiki do KIRA (PR28–32) — inspirada no ai-memory

Markdown é a fonte de verdade; FTS5 no `store.db` é índice **derivado e
descartável**. Sem servidor, sem MCP, sem embeddings até o M2. Ver `ADR-0013`.

| PR | Objetivo | Aceite |
|---|---|---|
| **PR28** | `kira wiki` + páginas `contexto/decisoes/padroes/glossario/bugs/*` em **`docs/wiki/` (commitada)**, toda escrita passando por `redact` e pelo `guard/path` | Nenhum segredo no disco; página criada/atualizada por comando; fail-open se o store quebrar; ciclo autônomo só grava em `.kira/wiki-staging/` |
| **PR29** | Índice FTS5 (`wiki_page`, `wiki_fts`) + `kira wiki reindex` reconstruindo do zero | Apagar o índice e reindexar devolve o mesmo resultado de busca; sem FTS5 cai para substring sem erro fatal |
| **PR30** | Captura episódica pós-`triage` + decay de 30 dias + `kira wiki forget` | Episódio gravado sem interação; página antiga colapsada em resumo; forget apaga página e índice |
| **PR31** | Retrieval para o context packer (liga no PR5) | Prompt recebe top-k páginas com caminho e idade; respeita `KIRA_NUM_CTX` |
| **PR32** | `kira wiki export` (bundle `.md` para o usuário comitar onde quiser) | Export não escreve no repo do usuário sem caminho explícito |

## W2-bis — gate de medição do M2 (antes do PR15)

`kira triage --eval <dir>` + `tests/fixtures/triage_gold/` com **20 bugs fechados**
(input JSON + causa raiz esperada, **rotulados pelo dono**). Saída = % de acerto.

**Sem `(d)` do dono não existe medição, e sem medição não existe W5.** Este PR é o
que transforma o gate M2 em número em vez de opinião.

## W3 — flywheel (PR9–10)

A confirmar detalhamento. Âncora: `kira note`/`history` alimentam o ciclo
(estado → nota → próxima sugestão) sem virar dashboard.

## W4 — "Devin-lite" (PR11–14, com os patches da juíza)

| PR | Objetivo | Patch obrigatório |
|---|---|---|
| PR11 | Backend Copilot SDK (pin preview, módulo isolado, fallback spawn de CLI) | Se a policy da Avanade bloquear: **ollama-only registrado no ROADMAP**, BYOK vira **PR11b** |
| PR12 | Fonte de log Datadog (`kira setup` pergunta "Fonte de log? datadog / nenhum") | **PII é pré-requisito deste PR** (logs carregam e-mail/CPF/token em URL); teto de linhas/tokens na janela |
| PR13 | `commands.toml` por OS (win/linux/mac) + golden test por OS | `toml` entra como **dep. declarada e justificada** (crates.io 0.9) |
| PR14 | `install.ps1` (Windows obrigatório) — valida e orienta o cliente | `Set-ExecutionPolicy -Scope Process`; **agendamento NÃO entra aqui** |
| PR11b | BYOK HTTP | registrado, não bloqueia PR11 |

`awaiting_approval` + `kira approve <id>` atravessa W4: **toda** escrita no mundo
(Azure, repo do projeto) para no estado até aprovação humana — budget excedido ≠
aprovação, `--apply` ≠ aprovação.

## GATE M2 — ≥70% no `triage --eval`

Não bateu? **Pare e conserte o context packer.** Sem isso não se pinta a nave.

## W5 — loop (PR15–18)

| PR | Objetivo | Patch |
|---|---|---|
| PR15 | `kira loop` (modo do binário, sem daemon) + teste de **2 ciclos seguidos com saída-estado idêntica** | critério anti "PR inócuo em loop" |
| PR16 | Modo agendado + registro opt-in (Task Scheduler) | só depois do modo existir — senão falha em silêncio |
| PR17 | Flywheel de autonomia (budget `max_llm_calls_per_day = 50`) | budget checado **antes** de cada transição |
| PR18 | `kira audit --weekly` | **blog saiu**; vira ROADMAP "a confirmar com o dono" |

## W5 — spec pronta, execução bloqueada pelo gate M2

O desenho do `kira loop` (estados, cadências, budget, watchdogs, kill switch e o modo
sugestão sem PR) está em [`kira-w5-loop.md`](kira-w5-loop.md). Nada disso vira código
antes do gate M2: 20 bugs rotulados e ≥70% medido por `kira triage --eval`.

## W6 — TUI + mascote (só pós-M2)

- **Base: shell do `statui` (MIT) adaptado** — `src/tui/{app,keymap,theme,fx,state}.rs`
  e `widgets/{table,inspector,footer}`. Atribuição obrigatória em
  `third_party/statui/{LICENSE,NOTICE.md}`. Ver `ADR-0014`. Não entra o
  `backend.rs` (probes HTTP) nem o modelo de config deles.
- TUI = **Ratatui, modo interativo do mesmo binário**. Ratatui é o shell do statui;
  o código Bun/Ink da `dev` é **referência de UX apenas**.
- 3 leis: (a) não substitui o modo texto (loop/agendado continua consumível por log;
  erro em texto copiável); (b) mesmo contrato, read-only; (c) 1 PR por campo do painel.
- Mascote pixel art = **arte**, não PR de código: sprites gerados/desenhados e
  versionados como asset; o binário só consome (`kira --about`).

## Onda TE — economia de token (PR34–43, plano de 12/Nov)

Plano de implementação em [`te-implementacao.md`](te-implementacao.md), fonte
na análise dos 4 repos escolhidos pelo dono (PR #50), decisões **D1–D12
propostas em 12/Nov/2026 (selo pendente)** e
[ADR-0028](adr/ADR-0028-cache-raciocinio-e-economia-medida.md) **proposto**.
Ordem: PR34 → PR35 → PR36/37 → PR38/39/40 → PR41 → PR42 (43 é W6).

| PR | Objetivo | Selo/estado |
|---|---|---|
| PR34 | Ledger de economia (pack vs teto, chamadas, pack ocioso; schema v4) | fila |
| PR35 | Cache de raciocínio — wiki-hit ≥0.50 = causa sem LLM, `cache-wiki`, `--no-cache`; hit **não grava episódio** (D1) | fila; mede só com os 20 rótulos |
| PR36 | Evidência em 3 estados (tag por arquivo, prompt v3) | fila |
| PR37 | Digest bruto `.kira/raw/` (512 KiB, redigido) + `.kira/` fora do git + BOM-sniff | fila |
| PR38 | Dedup contíguo nos trechos (barra ≥20% por estratégia) | fila |
| PR39 | Repo map determinístico no brief (≤20 arquivos/≤600 chars) | fila |
| PR40 | `token_profile` em `config.toml` (budget/balanced/quality, 3 packs dourados) | fila |
| PR41 | `kira rea` wrapper + pergunta teaching-only no setup | fila |
| PR42 | Watchdogs de budget 50/75/90% na spec W5 (doc-only) | fila |
| PR43 | Painel de economia no dashboard (1 campo por PR; lê, nunca escreve — prova `dashboard_e2e`) | **W6** (pós-M2) |

## Onda M — ingestão → digestão → emissão (ADR-0038, 12–13/Nov)

Ledger vivo em [`BUILD-PROGRAM.md`](BUILD-PROGRAM.md) (M1–M8); aqui o fechamento por PR.
Os PRs de fechamento formam **stack** — merge na ordem, cada um com a base declarada no corpo.

| PR | Objetivo | Item | Status |
|---|---|---|---|
| #84 `feat/m1-ingest-v2` | `kira ingest` — denylist, detector de stack, inventário + digest determinístico | M1 | **mergeado** |
| #85 `feat/m2-posts` | tabela `posts` + view `outbox` + claim/lease + `kira trust` (schema v6) | M2 | **mergeado** |
| #86 `feat/m3-digest` | `kira digest` — Courier, Ollama primeiro, cache com `lang`, briefing | M3 | **mergeado** |
| #87 `feat/m5-emit` | `kira emit` — publicação com gate por hash + linhas do doctor | M5 | **mergeado** |
| `fix/m5-emit-gate` | fecha o aceite do M5: arquivo sem `published_hash` recusado sem `--force` (com caminho na mensagem), escrita pelo guard, testes do doctor, 5º e2e | M5 (lacuna) | branch empilhada — aguarda merge |
| `fix/m7-bom-scan` | P1: `src/scan/decode.rs` (BOM UTF-8/UTF-16 LE/BE) + fixture real UTF-16 LE | M7-P1 | branch empilhada — aguarda merge |
| `feat/m6-wizard-ingest-lang` | M6: etapa Ingestão no wizard + ingest no motor (`run_scan=false`) + `--lang`/`output_lang` (D4) + resumo com próximo passo dinâmico | M6 | branch empilhada — aguarda merge |
| `feat/m-pack-digest-context` | item 4: `## Perfil`/`## Árvore` do digest no context pack (tetos 1 500 chars / 40 linhas; goldens intactos) | — | branch empilhada — aguarda merge |
| `feat/m7-gitignore` | P5: subset gitwildmatch em `src/context/gitignore.rs`, uma política nos três caminhos; **fecha M7** | M7-P5 | branch empilhada — aguarda merge |
| `docs/m8-jornada` | este: jornada do setup real de 5 etapas, JM na doc de jornadas, roadmap, README, ledger | M8 | branch empilhada — aguarda merge |
| M4 caixa de correio | `Screen::Posts` no dashboard (preview nativo, `read_at`, snapshot) | M4 | aberto — próxima PR da fila |

Achado registrado (issue própria, não corrigido na stack): `ollama list` spawna sem timeout
e um daemon ocupado pode travar `kira setup --yes` — os e2es de binário rodam herméticos
com `PATH=""` até a correção.

## Permanece REJEITADO

Daemon Axum/SSE · DB além do SQLite `bundled` · embeddings/semantic search (pós-M2) ·
framework de orquestração/LangChain · fan-out de agentes · MCP como dependência ·
escrita autônoma antes do gate M2 + `--apply` explícito.

## Backlog pós-W6 (pedidos do dono, 11/Nov — escopo a confirmar)

| Item | O que é | Pré-condição |
|---|---|---|
| `kira git` / release tree | **Mini gestão de git dentro do Kira**: desenhar a árvore de release (branches, tags, merges, o que está em cada release), status/branch por projeto, agrupado por stack do `kira scan`. Só leitura + desenho (Ratatui/nome de árvore em texto) na v1 | pós-M2; precisa ADR própria (o que é `release` neste repo? tag? `CHANGELOG`?) |
| Blog (devolvido do PR18) | `kira audit --weekly` gerando DRAFT no repo do blog — nota: "PR18" aqui é a numeração antiga do plano; no ledger atual (BUILD-PROGRAM F2) PR18 é `kira approve` | pós-W5, com aprovação humana explícita |
| Logs multi-provedor | depois do Datadog: `setup` já pergunta a fonte; novos provedores entram como backend do mesmo trait | pós-PR12 |
| Embeddings/busca em Rust | retrieval local (usearch/candle) para contexto do triage | pós-M2 (está na lista de rejeitados-por-agora) |
