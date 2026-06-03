# Auditoria Kira × 7 Pilares do Harness Engineering

> **Data:** 2026-05-24  
> **Base:** PDF do workshop "Dev Assistido por IA — Harness Engineering na Prática"  
> **Referência:** Felipe Rodrigues, BHub.ai (Spektor)  
> **Auditor:** Código-fonte atual do Kira (`core/` + `agent/`)

---

## Resumo Executivo

O Kira está **surpreendentemente avançado** para v0.1. A implementação real vai muito além do que o `AGENTS.md` sugere. O core daemon Rust já tem **todos os 7 pilares** com algum nível de implementação — 4 deles em estado avançado (≥80%).

| Pilar | Status | Completude | Avaliação |
|-------|--------|------------|-----------|
| P1 · SDD | ✅ IMPLEMENTADO | 85% | 4 fases + auto-sizing. Judge entre fases pendente. |
| P2 · Pilot+Orch | ✅ IMPLEMENTADO | 70% | SDD-only. Pilot genérico planejado v0.5. |
| P3 · Judge | ⚠️ PARCIAL | 55% | Função server-side, não agente autônomo. |
| P4 · Gates | ✅ IMPLEMENTADO | 95% | 6 níveis! Excede a spec do Spektor. |
| P5 · Guardrails | ⚠️ PARCIAL | 65% | SC1 e SC2 têm gaps de segurança. |
| P6 · Resiliência | ✅ IMPLEMENTADO | 85% | Circuit breaker, stall detection, fallback. |
| P7 · Observabilidade | ✅ IMPLEMENTADO | 80% | Events + bundle hash + audit JSONL + redb. |

---

## P1 — SDD (Spec-Driven Development)

**Status:** ✅ IMPLEMENTADO (85%)

### O que existe

| Componente | Arquivo | Notas |
|------------|---------|-------|
| Pipeline 4 fases | `core/src/sdd/mod.rs:16-22` | SPECIFY → DESIGN → TASKS → EXECUTE |
| Auto-sizing | `core/src/sdd/mod.rs:48-60` | BugFix + ≤5 LOC + ≤1 arquivo → SkipSpecifyDesign |
| Phase prompts | `core/src/sdd/prompts.rs:6-8` | `include_str!` compilado no binário |
| API endpoints | `core/src/server/sdd_api.rs:14-36` | `POST /v1/sdd`, `GET /v1/sdd`, `GET /v1/sdd/:id` |
| Driver loop | `core/src/sdd/mod.rs:220-267` | Pilot → Orchestrator com persist por passo |
| Execution log | `core/src/orchestrator/mod.rs:426-445` | `tasks.json` → `execution.jsonl` |
| Durable state | `core/src/state/mod.rs:29` | `TBL_SDD` em redb |
| Per-phase artifacts | `core/src/orchestrator/mod.rs:136-248` | `spec.md`, `design.md`, `tasks.json` em `.kira/runs/` |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "A spec é o gabarito do design" | ✅ spec.md é artefato da fase SPECIFY, usado como input do DESIGN |
| "O design é o gabarito das tasks" | ✅ design.md alimenta a fase TASKS |
| "Tarefa trivial pula fases" | ✅ `auto_size()` retorna `SkipSpecifyDesign` |
| "Fase avança só se gate passa" | ⚠️ Judge entre fases retorna `judge_score: None` |

### Gaps

1. **Judge não chamado entre fases** — `judge_score: None` em todas as transições de fase (`orchestrator/mod.rs:136-248`). O judge só roda no cascade pós-agente, não entre fases SDD.
2. **Sem SingleShot para bugs triviais** — o `SkipSpecifyDesign` existe mas ainda passa por DESIGN+TASKS mínimo. O `SingleShot` (pula tudo, executa direto) está reservado para v0.5.
3. **Sem integração com workflows YAML** — o SDD atual é independente do sistema de workflows (`.kira/workflows/*.yaml`).

---

## P2 — Pilot + Orchestrator

**Status:** ✅ IMPLEMENTADO (70%)

### O que existe

| Componente | Arquivo | Notas |
|------------|---------|-------|
| `Pilot` trait | `core/src/pilot/mod.rs:14-21` | `next_intent()` + `observe()` |
| `SddPilot` | `core/src/pilot/mod.rs:25-52` | Máquina de estados SDD: SPECIFY→DESIGN→TASKS→EXECUTE→Done |
| `Intent` enum | `core/src/proto/intent.rs:24-38` | 6 variantes: RunSddPhase, RunSddExecute, EscalateTier, RequestHumanReview, Done, Halt |
| `Orchestrator` | `core/src/orchestrator/mod.rs:37-445` | Executa Intents, spawn agents, gerencia transições de fase |
| Agent manager spawn | `core/src/agents/spawn.rs:172-273` | Loop completo com tool-calling, stall detection, lesson injection, gates |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "Pilot decide, Executor roda, Verifier confere" | ✅ Padrão puro: `pilot.next_intent()` → `orch.execute()` → `pilot.observe()` |
| "Máquina de estados decide próxima intenção" | ✅ `SddPilot` implementa o state machine pattern |
| "Nunca auto-aprova" | ✅ Pilot e Executor são componentes separados |
| "Maestro + instrumentos" | ⚠️ Pilot é SDD-only; orquestração geral é v0.5 |

### Gaps

1. **Pilot é SDD-only** — não existe Pilot para workflows genéricos, plans autônomos, ou PR review. O `Pilot` trait só tem implementação `SddPilot`.
2. **Sem hierarquia Pilot → Orchestrator** — AGENTS.md menciona "Pilot (M15) + Orchestrator (M16) hierarchy" para v0.5.
3. **`EscalateTier` e `RequestHumanReview` são no-ops** — os intents existem no enum mas retornam `Pending` sem ação real.
4. **Workflow PR review não usa Pilot** — tem seu próprio executor separado (`workflows/mod.rs`), sem integração com o padrão Pilot/Orchestrator.

---

## P3 — Judge (Verificação)

**Status:** ⚠️ PARCIAL (55%)

### O que existe

| Componente | Arquivo | Notas |
|------------|---------|-------|
| `AgentKind::Judge` | `core/src/proto/agent.rs:12` | Heavy tier, ReadOnly ownership |
| Judge system prompt | `core/src/agents/prompts/judge.md` | 100+ linhas, 5 exemplos few-shot |
| Judge no cascade L5 | `core/src/gates/mod.rs:633-715` | Heavy tier, prompt determinístico (temp=0.0, json_object) |
| Judge audit JSONL | `core/src/gates/mod.rs:1001-1055` | `~/.local/share/kira/audit/judge-decisions.jsonl` |
| 3-zone classification | `core/src/gates/mod.rs:501-568` | L4 faithfulness ≥ 0.80 → skip judge; < 0.55 → fail; entre → escalate |
| `ChatParams::judge_deterministic()` | `core/src/llm/types.rs:23-28` | temp=0.0, response_format=json_object |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "Modelo separado do executor" | ⚠️ Judge é o mesmo router `heavy` tier — pode coincidir com o modelo writer |
| "Olha o resultado com critérios de aceitação" | ✅ Judge prompt inclui SPEC vs OUTPUT comparison |
| "Jamais auto-aprova" | ❌ **Não há enforcement `judge_model ≠ writer_model`** |
| "Só escreve eval_runs" | ⚠️ Judge escreve em `judge-decisions.jsonl`, não em `eval_runs` (redb) |
| "Barato primeiro, caro só quando precisa" | ✅ L4 faithfulness zone salva $0.10 por gate run |

### Gaps (CRÍTICO)

1. **`judge_model ≠ writer_model` NÃO é enforced** — o judge usa `router.route_chat(tier=Heavy, ...)`, e o writer também pode usar Heavy tier. Se ambos caírem em `deepseek-v4-pro`, o agente está se auto-avaliando.
2. **Judge não é agente autônomo** — é uma função `gate_judge_llm()` server-side. Não tem worktree próprio, não tem ciclo de vida de agente, não tem mailbox. O AGENTS.md descreve o judge como um sub-agente completo.
3. **Sem tabela `eval_runs`** — o redb tem `TBL_SDD` mas não `TBL_EVAL`. Os julgamentos vão para JSONL flat, sem estrutura de banco.
4. **Prompt do judge inline no código** — o prompt principal está hardcoded em `gates/mod.rs:68-94` como `const JUDGE_PROMPT`, não carregado do arquivo `agents/prompts/judge.md` (que existe mas é usado apenas para spawn de agente Judge, não para o gate L5).

---

## P4 — Completion Gates

**Status:** ✅ IMPLEMENTADO (95%) — **Excede a spec do Spektor**

### O que existe

| Nível | Função | Custo | Arquivo |
|-------|--------|-------|---------|
| **L1** Structural Validation | `gate_structural()` + `gate_structural_lint()` | Grátis | `gates/mod.rs:204-360` |
| **L2** File Integrity | `gate_file_integrity()` | Grátis | `gates/mod.rs:366-439` |
| **L3** Change Sufficiency | `gate_change_sufficiency()` | Grátis | `gates/mod.rs:445-495` |
| **L4** Embedding Faithfulness | `gate_embedding_faithfulness()` | ~$0.00007 | `gates/mod.rs:501-568` |
| **L5** Judge LLM | `gate_judge_llm()` | ~$0.10 | `gates/mod.rs:633-715` |
| **L6** Meta-Review | `gate_meta_review()` | Grátis | `gates/mod.rs:727-826` |

**Destaques:**
- **L1 tem lint real** — se worktree path informado, roda `cargo check --offline` ou `bunx tsc --noEmit` (60s timeout)
- **L4 tem 3-zone classification** — sim < 0.55 → Fail; 0.55–0.80 → Escalate; ≥ 0.80 → Skip Judge (economia de $0.10)
- **L6 verifica `evidence_quote`** — checa se citações do judge existem literalmente no diff
- **YOLO mode** — `skip_embedding + skip_judge + skip_meta_review` para tarefas rápidas (L1+L2 apenas)
- **Embed cache** — redb-backed, max 5000 entries, FNV-1a keyed, eviction reativa de 500
- **API endpoint** — `POST /v1/gates/run`

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "Cascata do mais barato ao mais caro" | ✅ Short-circuit no primeiro Fail |
| "L1-L3 são grátis" | ✅ Structural + Integrity + Sufficiency |
| "Embedding faithfulness como filtro" | ✅ L4 com 3 zonas |
| "Judge LLM só quando precisa" | ✅ Só escala para L5 se L4 inconclusivo |
| "Gates são portas trancadas, não sugestões" | ✅ Fail bloqueia o agente |

### Gaps

1. **Sem gates pre-release** — AGENTS.md lista `cargo audit`, `bun audit`, `gitleaks detect` como gates manuais. Não estão integrados no cascade.
2. **Sem smoke test automatizado** — o hook que bloqueia `rm -rf` existe (`.kira/hooks.toml`) mas não como gate.
3. **Meta-review (L6) é bônus não documentado** — o workshop do Spektor descreve 5 níveis; Kira tem 6. O L6 não está mapeado no AGENTS.md.

---

## P5 — Guardrails

**Status:** ⚠️ PARCIAL (65%)

### O que existe

| Controle | Arquivo | Status |
|----------|---------|--------|
| Budget guard | `core/src/router/budget.rs:41-103` | ✅ Hard-stop mensal, warn pct, microcent precision |
| Ownership registry | `core/src/agents/ownership.rs:19-109` | ✅ Glob-based claims, Exclusive/Shared, WriteDecision::Denied |
| SC3 — Cmd injection | `core/src/safety/cmd_allowlist.rs` | ✅ Allowlist 20 executáveis, regex args, env_clear, sem shell expansion |
| SC4 — Path traversal | `core/src/safety/path_guard.rs` | ✅ Canonicalize + prefix check + '..' rejection |
| SSRF guard | `core/src/safety/ssrf_guard.rs` | ✅ Bloqueia IPs privados/loopback/metadata (hickory-resolver) |
| SC7 — MCP safety | `core/src/mcp/safety.rs` | ✅ Schema pinning SHA-256, param sanitization, response delimiting |
| File I/O caps | `core/src/safety/mod.rs:11-12` | ✅ 5 MiB hard cap em read/write |
| Hook system | `core/src/hooks/mod.rs` | ✅ PreToolUse (block-capable) + PostToolUse |
| `.kira/hooks.toml` | `.kira/hooks.toml` | ✅ Block `rm -rf`, log file_write |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "File Integrity: não pode reescrever spec e testes" | ✅ Ownership registry bloqueia `touched_paths ⊄ owned_paths` |
| "Change Sufficiency: diff proporcional à tarefa" | ✅ L3 gate verifica `actual_loc` vs `estimated_loc` |
| "Cost Cap: esgotou, para" | ✅ Budget hard-stop antes de cada chamada |

### Gaps

1. **SC1 — Prompt injection isolation (CRÍTICO)** — os delimitadores XML `<SYSTEM_INSTRUCTIONS>`, `<USER_DATA>`, `<TOOL_OUTPUT>`, `<RAG_CONTEXT untrusted>` **não são injetados nas mensagens**. Só `<TOOL_OUTPUT untrusted>` existe (ferramentas web). O restante é LLM instruction-only, sem runtime enforcement.
2. **SC2 — Tool risk classification para built-in tools** — bash, file_read, file_write, web_search, web_fetch **não têm nível de risco**. Só ferramentas MCP são classificadas. AGENTS.md exige HIGH+ approval e CRITICAL halt.
3. **SC5 — Secrets redaction** — regex existe no config (`secrets_redact_keys_regex`) mas **não há filtragem ativa** em logs ou audit JSONL.
4. **SC6 — Audit CR/LF sanitisation** — append-only JSONL existe mas sem sanitização explícita de CR/LF ou PII/API key nos campos de audit.
5. **`kira-core.toml` não existe** — AGENTS.md referencia este arquivo como runtime config central. Na prática, hooks, MCP, workflows cada um tem seu próprio arquivo em `.kira/`.

---

## P6 — Resiliência

**Status:** ✅ IMPLEMENTADO (85%)

### O que existe

| Mecanismo | Arquivo | Notas |
|-----------|---------|-------|
| Circuit breaker | `core/src/router/circuit.rs:33-128` | Lock-free atomic: Closed→Open→HalfOpen→Closed; 5 falhas/60s, 30s open |
| Fallback chain | `core/src/router/mod.rs:147-304` | Itera candidates, pula quebrados, tier-config-driven |
| DeepSeek tool demotion | `core/src/router/mod.rs:211-218` | DeepSeek movido pro fim quando tools presentes (thinking mode) |
| Failure classification | `core/src/router/failure.rs:39-62` | 6 classes: AgentMistake, InfraTransient, InfraPermanent, BudgetExceeded, Stall, Cancelled |
| Retry policy | `core/src/router/failure.rs:74-129` | AgentMistake: 3 retries/2s-8s; InfraTransient: 4 retries/500ms-8s; Stall: 0 retries |
| Stall detection | `core/src/agents/spawn.rs:43-54` | 3 sinais: SameToolLoop (3x), OutputUnchanged (3x), NoProgress (90s) |
| Watchdog: agent heartbeat | `core/src/watchdogs.rs:53-81` | 60s scan; 5min sem progresso → Stale |
| Watchdog: worktree cleanup | `core/src/watchdogs.rs:87-143` | 60s scan; worktrees >10min órfãos → destroy |
| Agent cancellation | `core/src/agents/spawn.rs:143-150` | `cancel()` via oneshot channel |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "Stall detection: agente parado → sinaliza" | ✅ 3 sinais independentes + watchdog |
| "Fallback chain: primário → fallback 1 → fallback 2" | ✅ Config-driven via `tokens.toml [tiers.*]` |
| "Escalation: cadeia esgota → parar ou pular" | ⚠️ Circuit breaker abre, mas sem escalation para tier superior |
| "Failure classification: tratar diferente cada tipo" | ✅ 6 classes + retry policies diferenciadas |

### Gaps

1. **Circuit breaker state volátil** — estado `AtomicU8` em memória. Reiniciar o daemon reseta todos os breakers para Closed.
2. **Sem escalation cross-tier** — quando todos os candidatos de um tier esgotam, o router retorna erro. Não há promoção automática para tier superior (ex: Light→Standard).
3. **Sem GracefulShutdown** — watchdogs rodam indefinidamente até o tokio runtime dropar.

---

## P7 — Observabilidade + Logs

**Status:** ✅ IMPLEMENTADO (80%)

### O que existe

| Componente | Arquivo | Notas |
|------------|---------|-------|
| Harness bundle (P7) | `core/src/observability/bundle.rs` | SHA-256 de tokens.toml + workflows + prompts + lessons |
| Bundle hash componentes | `core/src/observability/bundle.rs:43-51` | 4 componentes hasheados independentemente |
| Pillar events | `core/src/observability/pillar.rs:23-57` | `emit()` + `emit_full()` com broadcast channel (1024) + audit JSONL |
| Pillar event types | `core/src/proto/pillar.rs:12-27` | P1Sdd, P2Coordination, P3Verification, P4Gates, P5Guardrails, P6Resilience, P7Cognition |
| SSE events | `core/src/server/events_api.rs:27-56` | `GET /v1/events/stream` com filtro `run_id` |
| Audit JSONL | `core/src/observability/pillar.rs:69-96` | Append-only `~/.local/share/kira/audit/pillars.jsonl` |
| Judge audit | `core/src/gates/mod.rs:1004-1055` | `~/.local/share/kira/audit/judge-decisions.jsonl` |
| Dataset export CLI | `core/src/dataset.rs` | `kira-cored dataset export --judge` → LLaMA-Factory JSONL |
| Durable state (redb) | `core/src/state/mod.rs:34-260` | 6 tabelas: worktrees, agents, plans, lessons, sdd_runs, embed_cache |
| Stats endpoint | `core/src/server/stats_api.rs` | `GET /v1/stats` — uso de tokens por modelo |
| Tracing | Todos os módulos | `#[instrument]` + `tracing-subscriber` (JSON e pretty modes) |
| Health endpoint | `core/src/server/health.rs` | `GET /v1/health` com uptime |

### Alinhamento com o workshop

| Princípio do workshop | Kira |
|-----------------------|------|
| "Events: cada execução emite eventos" | ✅ Pillar events com broadcast SSE + persist JSONL |
| "Hard Links: hash imutável do harness" | ✅ SHA-256 custom de config + prompts + workflows + lessons |
| "Exportável pra Grafana/OTel" | ⚠️ SSE endpoint existe, mas sem export OTel nativo |
| "Ficou mais lento desde terça?" | ✅ Bundle hash permite comparação A/B de versões do harness |

### Gaps

1. **State DB path hardcoded** — `default_state_path()` usa `~/.local/share/kira/state.redb` fixo, ignorando `tokens.toml → paths.state_db_mac`.
2. **Sem shadow runs** — AGENTS.md menciona "parallel bundle_B vs current to A/B harness changes" para v1.
3. **Sem promotion pipeline** — "only promote bundle_B → main when metrics improve" não implementado.
4. **Sem export OTel nativo** — SSE endpoint existe mas sem integração com OpenTelemetry Collector.
5. **Bundle hash sem API endpoint** — a função de hash existe mas não está exposta como endpoint HTTP.

---

## Recomendações Prioritárias

Ordenadas por impacto × esforço:

### 🔴 Critical (segurança)

| # | Gap | Ação | Esforço |
|---|-----|------|---------|
| 1 | **SC1 — Prompt injection isolation** | Implementar delimitadores `<SYSTEM_INSTRUCTIONS>` / `<USER_DATA>` / `<TOOL_OUTPUT>` / `<RAG_CONTEXT untrusted>` em `agents/spawn.rs` na montagem das mensagens | Médio |
| 2 | **SC2 — Tool risk para built-in tools** | Adicionar `risk_level` em `agents/tools.rs` para bash, file_write, etc. Implementar approval gate para HIGH+ | Baixo |
| 3 | **`judge_model ≠ writer_model`** | Adicionar tier/rota separada para judge no cascade L5 (`gates/mod.rs:633`). Criar `tiers.judge` em `tokens.toml` | Baixo |

### 🟡 Important (arquitetura)

| # | Gap | Ação | Esforço |
|---|-----|------|---------|
| 4 | **Judge autônomo** | Transformar `gate_judge_llm()` em spawn de agente Judge com worktree próprio. Criar tabela `TBL_EVAL` | Alto |
| 5 | **`kira-core.toml`** | Criar arquivo central de runtime config, migrar hooks + MCP + workflow config para ele, ou atualizar AGENTS.md | Baixo |
| 6 | **State DB path config-drive** | Usar `tokens.toml → paths.state_db_mac` em `state/mod.rs` | Baixo |

### 🟢 Nice-to-have (expansão)

| # | Gap | Ação | Esforço |
|---|-----|------|---------|
| 7 | **Módulo `text/` (M17-M20)** | Implementar tokenizer, chunker, compressor, reranker | Alto |
| 8 | **Circuit breaker persistente** | Salvar estado do breaker em redb ou arquivo para sobreviver restart | Médio |
| 9 | **Cross-tier escalation** | Fallback chain promover para tier superior quando todos candidatos esgotam | Médio |
| 10 | **OTel export** | Adicionar `opentelemetry` crate + exporter OTLP para métricas de gate/agente | Médio |

---

## Notas Finais

1. **O Kira não é uma cópia do Spektor.** Ele adapta os mesmos princípios para um contexto solo-dev. KIRA é **LLM-agnostic**: a ladder de tiers default prioriza barato primeiro (Ollama local → DeepSeek Flash → DeepSeek Pro), mas o operador troca provider em `tokens.toml [providers.*]` + `[tiers.*]` sem mexer no harness. Workshop é literal: "o modelo é commodity, o harness é a vantagem competitiva".

2. **AGENTS.md está desatualizado.** O documento de design ficou para trás em relação à implementação real. Ele sugere que features como SDD, gates, e resiliência são "v0.5+", mas já estão implementadas com qualidade de produção.

3. **A fundação é sólida.** Os 4 pilares core (SDD, Gates, Resiliência, Observabilidade) estão em estado avançado. Os gaps estão concentrados em segurança (SC1, SC2) e no judge autônomo — áreas de menor superfície de código.

4. **O maior risco não é técnico.** É a desconexão entre a documentação (AGENTS.md) e a realidade do código. Isso pode levar a decisões erradas de priorização e investimento em features que já existem.
