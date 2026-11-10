# BUILD PROGRAM — KIRA autônomo (executado pelo Hermes/Orchestrator)

> Criado 10/Nov/2026 por decisão do dono: "enquanto eu estudo, o Hermes faz tudo
> sozinho; só termina quando o KIRA estiver pronto e completo, com o dashboard
> Ratatui estilo jogo, rodando local com Ollama (RTX 2060 6 GB / M1 mínimo), e no
> futuro Copilot".
>
> Este arquivo é o **estado da execução**. O agente que retomar o trabalho lê este
> arquivo primeiro, escolhe **um** item aberto, executa, roda os gates, e marca.

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

### F0 — W1 segurança/erros (base para tudo)
- [ ] **PR1** docs: README + AGENTS.md ao v3 — incluir seção de autonomia
      (`awaiting_approval`, `kira approve`), gate de medição e TUI pós-M2.
- [ ] **PR2** `src/redact.rs`: máscara de PAT/token/CPF/e-mail antes de log,
      erro e prompt. Teste prova que o prompt não leva segredo.
- [ ] **PR3** `src/guard/path.rs`: deny-list de escrita + path traversal. Erro
      acionável, não toca o disco.
- [ ] **PR4** taxonomia de erro acionável única em `src/error.rs`; zero
      `unwrap`/`expect` em rede/parse/input.

### F1 — contexto e medição (W2 + W2-bis)
- [ ] **PR5** context packer v1: item + findings do arquivo + trechos, teto de
      tokens. **Sem vetor, sem embedding** (markdown + FTS5 no SQLite existente).
- [ ] **PR6** prompt de triage v1 + fixtures entrada/saída.
- [ ] **PR7** `prompts.rs` público, revisado, teste de regressão de texto.
- [ ] **PR8** `LlmBackend` explícito: ollama + caminho preparado para copilot/byok.
- [ ] **W2-bis** `kira triage --eval <dir>` + `tests/fixtures/triage_gold/`
      (schema + leitor + % de acerto). Harness testado com fixtures sintéticas;
      **medição real espera os 20 bugs rotulados pelo dono**.

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
- [ ] **PR11** Copilot SDK atrás de flag/env (pin preview, módulo isolado,
      fallback spawn de CLI). Bloqueado por policy Avanade → ollama-only é válido.

### F4 — dashboard Ratatui (estilo jogo)
- [ ] **PR21** shell do TUI: loop de render 30 fps, layout de "sala de controle",
      teclas, tema colorido; modo texto continua funcionando (não substitui).
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

1. `git fetch` e checar se há PR aberto de ciclo anterior; se sim, selar/mergear
   antes de começar outro.
2. Ler este arquivo, pegar o **primeiro item aberto** de F0 e ir subindo.
3. Implementar em branch nova, com teste.
4. Rodar os 4 gates. Vermelho → corrigir até 2 tentativas; persistindo, reverter,
   marcar o item como bloqueado com a saída do erro e **parar**.
5. Verde → commit, push, abrir PR, esperar CI, mergear.
6. Marcar `[x]` neste arquivo, commitar o estado, reportar em 5 linhas.
