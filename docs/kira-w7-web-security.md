# W7 — Web security: guardrails de prompt/redação + teste de segurança do próprio KIRA

- **Data**: 12/Nov/2026 · **Status**: proposto — aguardando aval do dono para
  entrar na fila (regra do roadmap: escopo só codifica quando fechado).
- **Fontes**: [`awesome-web-security-vale-trazer.md`](awesome-web-security-vale-trazer.md)
  · [ADR-0024](adr/ADR-0024-guardrails-invisiveis-prompt-v2.md) ·
  [ADR-0026](adr/ADR-0026-probe-suite-seguranca.md)
- **Pré-condição**: nenhuma — segurança do próprio KIRA não depende do gate
  M2 (não é autonomia; é blindagem do que já roda).

## Regras herdadas (não negociáveis)

Um item = um PR · branch `w7/pr<num>-<slug>` (numeração global na hora de
abrir) · gates completos (`fmt --check`, `clippy -D warnings`, `test`,
`build`, CI 3 SOs) · **merge é sempre humano** · decisão relevante = ADR (já
feitos: 0024 e 0025).

## Itens

| Item | Objeto | Branch | Aceite | Status |
|---|---|---|---|---|
| **W7.1** | `redact` v2: remoção de invisíveis (zero-width/bidi) antes do masking (ADR-0024 R1) + fixtures de payload | `w7/pr-<num>-redact-invisible` | Reexecução do probe de 12/Nov: as famílias vazantes passam a sair mascaradas; `contains_secret` cobre; idempotência mantida; gates verdes | **implementado** (#55) |
| **W7.2** | Prompt v2: bloco delimitado de conteúdo não confiável + regra anti-instrução embutida (ADR-0024 R2) | `w7/pr72-prompt-v2` | `TRIAGE_PROMPT_VERSION=v2`, FNV-1a recalculado, dourado + `tests/fixtures/triage/*/prompt.txt` reescritos no mesmo commit; `--eval` intacto; gates verdes | **implementado** (#56) |
| **W7.3** | Probe suite `tests/security_probe.rs` (ADR-0026) | `w7/pr73-security-probes` | Suíte verde nos 3 SOs; mapeamento OWASP LLM no cabeçalho; fixtures em `tests/fixtures/security/`; sem comando novo/dep nova/rede | **implementado** (#58) |
| **W7.4** (opcional) | `gitleaks` como ferramenta do `kira scan` (G4 do relatório) | `w7/pr-<num>-scan-gitleaks` | **A confirmar com o dono** — se confirmar: ADR próprio (nº definido na hora; ADR-0027 já tomado) + PR separado; KIRA orquestra (resolve no PATH, parse, fingerprint), não analisa | a confirmar |

## Ordem e dependências

1. **W7.1 primeiro** — corrige o vazamento real; W7.3 reutiliza as fixtures
   de payload que nascerem aqui.
2. **W7.2 depois** — a regra v2 do system prompt menciona `[REDACTED]`
   (preservado) e o bloco delimitado; fixtures de prompt são estáveis quanto
   ao W7.1 (texto de fixture sem invisível), mas a sequência evita rebase.
3. **W7.3 fecha a onda** — agrega as categorias LLM01/LLM02/LLM06 + escrita +
   e2e, e reexecuta o probe do relatório como evidência antes/depois.
4. **W7.4** só depois de decisão do dono (não bloqueia a onda).

## O que já foi medido hoje (baseline 12/Nov/2026)

- `cargo test --no-fail-fast` (suíte completa, estado do commit atual):
  **482 testes passando, 1 falha pré-existente e ambiental, 1 ignorado**.
  A falha é `dashboard_e2e::snapshot_without_pat_and_store…` — o keyring real
  desta máquina tem PAT salvo pelo `kira setup` e o teste espera ausência
  (`pat: —`); o e2e não isola o keyring do OS, então só falha em máquina de
  dev provisionada (no CI, sem keyring, passa). Item preexistente, fora do
  escopo da W7 — registrado como candidato a PR de isolamento.
- Mapa: lib 375 · main 3 · ado_integration 1 ignorado (alvo vivo) ·
  ado_live_errors 4 · context_e2e 6 · dashboard_e2e 13 ok + 1 ambiental ·
  eval_e2e 6 · llm_allowlist 3 · no_runtime_unwrap 1 · prdraft_e2e 6 ·
  scan_e2e 7 · setup_e2e 20 · store_e2e 4 · triage_e2e 14 ·
  triage_prompt_fixtures 6 · wiki_e2e 14 · doc-tests 0.
- Probe de bypass do `redact`: **5 vazamentos completos + 1 parcial em 10
  famílias** — ver seção "Achado empírico" do relatório.
- Suíte já existente cobre: `guard::path` (traversal, case, symlink, chaves),
  allowlist de `llm::*` (PR33), `no_runtime_unwrap`, determinismo M1,
  dashboard read-only byte-idêntico (`tests/dashboard_e2e.rs`).

## Definição de pronto da onda

- Bypass fechado (W7.1) + prompt v2 selado (W7.2) + probe suite verde nos
  gates (W7.3).
- Relatório do reaproveitamento atualizado com a reexecução do probe
  (antes/depois) e o link dos PRs.
- Nenhuma dependência nova no `Cargo.toml`; nenhum `unwrap`/`expect` novo em
  runtime; comentários em PT explicando o porquê.

## Riscos

- **Falso senso de segurança**: probe suite unitária não substitui e2e — por
  isso W7.3 tem e2e do binário onde couber (padrão dos e2e existentes).
- **Conflito de fixtures**: W7.2 reescreve dourado de prompt — PRs separados
  evitam colisão de worktree com a fila Delta em andamento (regra do roadmap).
- **Payload esquecido**: payload novo só entra com fixture versionada — sem
  teste inline descartável.
