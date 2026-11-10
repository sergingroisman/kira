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
| PR6 | Prompt de triage v1 + fixtures de entrada/saída | **a confirmar** |
| PR7 | Prompt review: `prompts.rs` público, revisado, com teste de regressão de texto | **a confirmar** |
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

## Permanece REJEITADO

Daemon Axum/SSE · DB além do SQLite `bundled` · embeddings/semantic search (pós-M2) ·
framework de orquestração/LangChain · fan-out de agentes · MCP como dependência ·
escrita autônoma antes do gate M2 + `--apply` explícito.
