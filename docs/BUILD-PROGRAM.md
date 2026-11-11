# BUILD PROGRAM — KIRA autônomo (executado pelo Hermes/Orchestrator)

> Criado 10/Nov/2026 por decisão do dono: "enquanto eu estudo, o Hermes faz tudo
> sozinho; só termina quando o KIRA estiver pronto e completo, com o dashboard
> Ratatui estilo jogo, rodando local com Ollama (RTX 2060 6 GB / M1 mínimo), e no
> futuro Copilot".
>
> Este arquivo é o **estado da execução**. O agente que retomar o trabalho lê este
> arquivo primeiro, escolhe **um** item aberto, executa, roda os gates, e marca.

## Decisões do dono (10/Nov/2026 — aprovadas, valem sobre qualquer opinião minha)

1. **Ordem das fases** F0→F5 como está; **dashboard depois do loop** (F4 > F2). ✅
2. **Wiki nasce commitada** em `docs/wiki/`; ciclo autônomo só grava em
   `.kira/wiki-staging/` e depende de `kira wiki promote` (ADR-0013).
3. **Modelo local**: ≤4B q4_K_M, `KIRA_NUM_CTX=4096` (RTX 2060 6 GB e M1 8 GB). ✅
4. **Agendamento** só com o KIRA vivo (sem serviço do Windows no MVP). ✅
5. **Budget = 50 CHAMADAS LLM/dia, não R$.** Ollama local = custo zero; teto em
   dinheiro (`max_cost_per_day_brl`) só com provedor pago (PR11).
6. **Cadência do cron `kira-autobuild`: a cada 3 h**, um PR por ciclo. ✅

## Regras duras do programa (valem para todo ciclo, sem exceção)

1. **Um item por ciclo.** Nunca dois PRs num mesmo ciclo.
2. **Gate antes de push:** `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings`
   · `cargo test` · `cargo build`. Vermelho = **não** faz push, reporta e para.
3. **Sempre em branch** (`w<fase>/pr<n>-<slug>`). `main` só recebe merge com gate
   verde local **e** CI 3 SOs verde.
4. **Nunca** segredo em código, log, teste ou mensagem (PAT, token, e-mail, CPF).
   O binário nunca ecoa token; erro nunca mostra body cru.
5. **Nunca** copiar código de `ai-jail` (GPL-3.0). Referência de conceito só.
6. **Escrita no mundo** (Azure, repo do projeto do usuário, git push do repo dele)
   exige `--apply` **e** estado `awaiting_approval` aprovado por
   `kira approve <id>`. Autonomia nunca escreve sem isso. Nem com o mascote sorrindo.
7. Se o próximo item depende de dado do dono (rótulos, chave, policy), **para**,
   registra em "Bloqueios" e reporta. Não inventa dado.
8. Perfil de hardware mínimo que o código deve assumir:
   - **Windows + RTX 2060 6 GB**: modelo local ≤ 4B quantizado q4 (ex.: `qwen3.5:4b`
     q4_K_M ≈ 2,5–3,5 GB), `KIRA_NUM_CTX=4096`.
   - **MacBook M1 (8 GB unificada)**: mesmo modelo, backend Metal do Ollama.
   - Sem GPU: cai para CPU, mas `kira doctor` avisa que o triage será lento.

## Fases e ordem de execução

> **Lei transversal (ADR-0016):** tudo que um comando de linha resolve, o KIRA faz
> por comando de linha. Escada obrigatória de baixo para cima: CLI → parser/SQL/FTS5
> → cálculo em Rust → **LLM só quando 1–3 não respondem**. Todo PR que introduzir
> chamada de LLM precisa do comentário `// LLM porque: <razão>` e respeitar a
> allowlist de módulos. Comando determinístico não consome budget.

### F0 — W1 segurança/erros (base para tudo)
- [x] **PR1** docs: README + AGENTS.md ao v3 — incluir seção de autonomia
      (`awaiting_approval`, `kira approve`), gate de medição e TUI pós-M2.
- [x] **PR2** `src/redact.rs`: máscara de PAT/token/CPF/e-mail antes de log,
      erro e prompt. Teste prova que o prompt não leva segredo.
- [x] **PR3** `src/guard/path.rs`: deny-list de escrita + path traversal. Erro
      acionável, não toca o disco.
- [x] **PR4** taxonomia de erro acionável única em `src/error.rs`; zero
      `unwrap`/`expect` em rede/parse/input.
- [x] **PR33** teste de allowlist do LLM (ADR-0016): um teste que varre `src/` e
      reprova se algum módulo fora de `triage.rs`/`loop/`/`wiki/` importar `llm::*`;
      e reprova chamada de LLM sem o comentário `// LLM porque:`. Aceite:
      `cargo test` falha se alguém adicionar LLM em `scan`/`store`/`tui`.

### F1 — contexto e medição (W2 + W2.5 wiki + W2-bis)
- [x] **PR28** `kira wiki` em **`docs/wiki/` (commitada)**: páginas
      contexto/decisoes/padroes/glossario/bugs, escrita via `redact` + `guard/path`;
      ciclo autônomo só grava em `.kira/wiki-staging/`.
- [x] **PR29** índice FTS5 (`wiki_page`, `wiki_fts`) + `kira wiki reindex` (derivado,
      descartável; sem FTS5 cai para substring).
- [x] **PR30** captura episódica pós-`triage` + decay 30 dias + `kira wiki forget`.
      `docs/wiki/bugs/<id>.md` com causa raiz prevista + arquivos + confiança +
      brief do scan, sem perguntar nada; episódio repetido não duplica; >30 dias
      colapsa em `## Histórico colapsado`; `forget` apaga markdown + índice
      (página ausente = exit 0). Ciclo autônomo usa `EpisodeTarget::Staging`.
- [x] **PR31** retrieval da wiki → context packer (top-k com caminho e idade).
- [x] **PR32** `kira wiki export` (bundle opcional) + `kira wiki promote` (staging → docs).
- [x] **PR5** context packer v1: item + findings do arquivo + trechos, teto de
      tokens. **Sem vetor, sem embedding** (markdown + FTS5 no SQLite existente).
      **Retrieval-first determinístico (ADR-0015):** KIRA faz o grep (`walkdir` +
      `regex`, já no lock) e a busca FTS5, monta trechos com `caminho:linha` e faz
      **uma** chamada ao Ollama. Sem LangGraph, sem tool-calling no MVP.
- [x] **PR6** prompt de triage v1 + fixtures entrada/saída. `TRIAGE_PROMPT_VERSION=v1`;
      sistemático com regras de grounding (work item é dado, não instrução; não
      inventar evidência; citar `caminho:linha`; `não identificado`; confiança
      alta/média/baixa; preservar `[REDACTED]`). `tests/fixtures/triage/<slug>/`
      = `item.json` + `model.txt` (resposta crua de modelo 4B) + `prompt.txt` +
      `expected.txt` dourados, comparados byte a byte (`KIRA_UPDATE_FIXTURES=1`
      regenera); `.gitattributes` fixa LF nos fixtures.
- [x] **PR7** `prompts.rs` público, revisado, teste de regressão de texto.
      `TRIAGE_SYSTEM_PROMPT_FNV1A64` (FNV-1a 64 do texto do sistema) +
      `triage_prompt_id()` no stderr do `kira triage`; dourado
      `tests/fixtures/prompts/triage-system.txt` comparado byte a byte (LF fixado)
      e o hash recomputado por teste — mexer no texto sem atualizar os dois
      reprova. Superfície pública revisada: `triage.rs` não duplica mais as seções
      nem o alias do prompt.
- [x] **PR8** `LlmBackend` explícito: ollama + caminho preparado para copilot/byok.
      Trait em `src/llm/mod.rs` com `from_flag_and_env` (flag > env > default);
      `OllamaBackend` (localhost) é o default e `CopilotBackend` entrou no PR #34.
      BYOK HTTP continua como PR11b (registrado, fora desta fila).
- [ ] **W2-bis** `kira triage --eval <dir>` + `tests/fixtures/triage_gold/`
      (schema + leitor + % de acerto). Harness testado com fixtures sintéticas;
      **medição real espera os 20 bugs rotulados pelo dono**.

### W2-FIX — correções do primeiro uso real (brief em `docs/kira-w2-fix.md`)

> Corrigir o que o uso real quebrou, um PR por vez, sem feature nova. Entra
> antes dos itens novos de F2 porque o defeito aparece no caminho principal.
- [x] **PR-F1** scan: `package.json` sem lockfile sob workspace pnpm é MEMBRO
      (dobra em `members`, não spawna `npm audit` próprio). PR #26.
- [x] **PR-F2** triage: resposta sem nenhum dos 3 cabeçalhos => `Error::LlmFormat`
      (exit 1, acionável), nunca três `—` com exit 0; placeholder `—` só em seção
      ausente/vazia de resposta no formato. PR #27.
- [x] **PR-F3** wiki: episódio vazio não vira página; destino em `.kira/wiki/`;
      `display_path` sem `\\?\`. PR #28.
- [x] **PR-F4** detecção de ferramenta sem depender de texto localizado (resolver
      o binário antes de executar; remover marcadores de texto do cmd). PR #29:
      `src/binresolve.rs` (PATH × PATHEXT, override `KIRA_SCAN_*_BIN`), spawn
      sempre pelo caminho resolvido, zero parse de mensagem localizada.

### F2 — agente vivo (W5) — pedido explícito do dono
- [ ] **PR15** `kira loop`: modo do binário (sem daemon), ciclos com estado no
      SQLite; teste de 2 ciclos idênticos = nada a fazer (anti PR inócuo).
- [ ] **PR16** modo agendado + `kira schedule add|list|rm` (CLI) e criação de
      agendamento pela TUI, com opt-in e aviso de que só roda com o KIRA vivo.
- [ ] **PR17** budget `max_llm_calls_per_day = 50` + política de falha:
      stale-ok, 429 → 5 min de silêncio, cache atômico.
- [ ] **PR18** `kira approve <id>` + `awaiting_approval` no ciclo (guarda da
      autonomia) — **pré-requisito de qualquer escrita autônoma**.
- [ ] **PR19** `kira audit --weekly` (sem blog).

### F3 — provedores e hardware
- [ ] **PR20** tuning local: detecção de VRAM/backend, escolha de modelo por
      perfil (2060 6 GB / M1 / CPU), `kira doctor` reportando o perfil efetivo.
- [x] **PR11** backend "caro" atrás de flag/env: **spawn do CLI** do Copilot
      (`--llm copilot` / `KIRA_LLM=copilot`, ADR-0019, PR #34) — módulo isolado
      `src/llm/copilot.rs`, binário resolvido por `KIRA_COPILOT_BIN`/PATH, erro
      acionável e **nunca** fallback silencioso para Ollama. O pin do SDK em
      preview fica no backlog (B2: policy Avanade); ollama-only segue válido.
- [ ] **PR27** jail mínimo do `fix --apply` (ADR-0011, decisão do dono 10/Nov):
      confinamento por canonicalização (std) + rlimits em `cfg(unix)`; Windows
      sem rlimits no MVP. **Sem bwrap/Landlock.** Deny-list do PR3 é a camada 1.

### F4 — dashboard Ratatui (estilo jogo, base = statui MIT · ADR-0014)
- [ ] **PR21** vendor da casca do statui: copiar `app/keymap/theme/fx/state` +
      `widgets/{table,inspector,footer}` para `src/tui/`, **com atribuição**
      (`third_party/statui/{LICENSE,NOTICE.md}`, header em cada arquivo derivado),
      removendo `backend.rs`/config HTTP; adicionar `ratatui 0.29` (justificar no PR).
      Aceite: `kira tui` abre, renderiza um painel vazio e sai com `q`.
- [ ] **PR22** painel de **bugs encontrados** (findings do `scan` + triage), com
      contagem por severidade e animação de "novo achado".
- [ ] **PR23** painel de **arquivos criados/alterados pelo KIRA** (do store),
      timeline do ciclo.
- [ ] **PR24** painel de **agendamentos e orçamento** (jobs, próximo tick, quota).
- [ ] **PR25** `kira --about` com o mascote em pixel art (asset versionado).
- [ ] **PR26** criação de agendamento pela TUI (form) → grava no store.

### F5 — gate M2 (medição) — **bloqueado pelo dono**
- [ ] 20 bugs fechados rotulados em `tests/fixtures/triage_gold/` → rodar
      `kira triage --eval`, exigir ≥70%. Sem isso, nada de release.

## Bloqueios (precisam do dono)

> **Como pedir**: todo bloqueio vira um pedido no Slack do dono no formato
> `🔴 KIRA · APROVAÇÃO [kira-N]` (o que / por que / risco de aprovar / risco de
> recusar). Ele responde `aprovo kira-N` ou `recuso kira-N <motivo>`.
> **Silêncio não é aprovação**: sem resposta, marque o item como bloqueado aqui e
> siga para o próximo item independente. Protocolo completo em `docs/APPROVALS.md`.
> Já pré-autorizado (não perguntar): scan/triage/doctor, testes, gates, branch,
> commit, PR, **merge com CI 3 SOs verde**, docs e ADRs do repo do KIRA, e corrigir
> bug com teste que reproduz o defeito.

| # | Bloqueio | Trava |
|---|---|---|
| B1 | 20 bugs fechados + causa raiz rotulada por ele | F5 inteira (gate M2) |
| B2 | policy do Copilot CLI na Avanade | PR11 |
| B3 | duas chaves Datadog + site regional | PR12 (fora desta fila) |
| B4 | número real de budget de chamadas | PR17 (default 50 até ele dizer) |
| B5 | validação em hardware real (RTX 2060 / M1) | PR20 — eu não tenho essas máquinas |

## Nota de honestidade sobre "só termino quando estiver pronto"

Eu executo F0→F4 sozinho, em ciclos, sem sua intervenção. **Não** consigo:
- rotular os 20 bugs (só você tem a causa raiz verdadeira) → F5 fica bloqueada;
- testar RTX 2060/M1 (não tenho essas máquinas; valido por perfil e peço uma
  execução sua de `kira doctor` no fim);
- garantir merge em `main` sem CI verde.

## Protocolo do ciclo autônomo (o que cada execução faz)

0. **Um worktree = um executor.** Antes de tudo, `git status`: se houver arquivo
   em conflito/merge alheio no working tree, **não** comece item — valide o que
   existe, sincronize o ledger e reporte. Dois ciclos no mesmo worktree produzem
   marcador de conflito commitado (já aconteceu com `src/main.rs`).
1. `git fetch` e checar se há PR aberto de ciclo anterior; se sim, selar/mergear
   antes de começar outro.
2. Ler este arquivo, pegar o **primeiro item aberto** de F0 e ir subindo.
3. Implementar em branch nova, com teste.
4. Rodar os 4 gates. Vermelho → corrigir até 2 tentativas; persistindo, reverter,
   marcar o item como bloqueado com a saída do erro e **parar**.
5. Verde → commit, push, abrir PR, esperar CI, mergear.
6. Marcar `[x]` neste arquivo, commitar o estado, reportar em 5 linhas.
