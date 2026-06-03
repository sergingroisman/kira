# KIRA — Harness Engineering na Prática

> **Roteiro de Apresentação / Slides**  
> Baseado no workshop de Felipe Rodrigues (Spektor, BHub.ai)  
> Adaptado para o KIRA — harness agentic solo-dev com core Rust + REPL Bun/Ink  
> **Data:** Maio 2026

---

## Slide 01 · Capa

**Título:** KIRA — Harness Engineering na Prática  
**Subtítulo:** Por que o harness ao redor do modelo importa mais do que o modelo.  
**Autor:** Sergin (solo-dev)  
**Inspiração arquitetural:** Spektor (Felipe Rodrigues, BHub.ai)  
**Stack:** Core Rust (`axum` + `tokio` em `127.0.0.1:7777`) + REPL Bun/Ink  
**Modelos:** **LLM-agnostic** — qualquer provider OpenAI-compatible pluga em `tokens.toml`. Tier ladder default prioriza barato primeiro: `local` (Ollama/Qwen) → `light/standard` (DeepSeek Flash) → `heavy` (DeepSeek Pro / GPT-4o). Operator troca tiers sem tocar no código.

> **Nota do apresentador:**  
> KIRA não é uma cópia do Spektor. É uma adaptação para solo-dev sem trancar em vendor — o harness fica indiferente ao modelo subjacente. O princípio é o mesmo do workshop: o modelo é commodity; o harness é a vantagem competitiva.

---

## Slide 02 · O Problema

**Título:** Agentes falham. E você não percebe.

Os quatro sintomas que separam demo de produção:

| # | Sintoma | Descrição no KIRA |
|---|---------|-------------------|
| **01** | **Vibe-codam** | Agente recebe task sem spec. Improvisa. Sem critério de pronto. Nada auditável. |
| **02** | **Falham silenciosamente** | Apaga teste, “completa” tarefa quebrada, ninguém percebe até produção. |
| **03** | **Queimam dinheiro** | Loops invisíveis, retries cegos, modelo caro para task trivial. Budget não para. |
| **04** | **Não dá pra explicar** | Quando dá errado, não dá pra rebobinar. Vira loteria, não engenharia. |

> **Nota do apresentador:**  
> No KIRA, esses sintomas são combatidos por: SDD (spec antes de código), gates (sistema prova pronto), budget hard-stop (`core/src/router/budget.rs`), e observabilidade completa (events SSE + audit JSONL + bundle hash).

---

## Slide 03 · Fundamentos

**Título:** Agente = Modelo + Harness

```
┌──────────────────────────────────────────────────────┐
│                    AGENTE                             │
│  ┌──────────────────┐   ┌──────────────────────────┐ │
│  │     MODELO        │   │        HARNESS           │ │
│  │  (O cérebro)      │   │  (O corpo, os reflexos,  │ │
│  │  Commodity —      │   │   a sala de cirurgia)     │ │
│  │  troca DeepSeek   │   │  Decide QUEM recebe COMO, │ │
│  │  pra Qwen — e o   │   │  valida o output.         │ │
│  │  problema acaba.  │   │                           │ │
│  └──────────────────┘   └──────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

- **Modelo** = o cérebro. É commodity — troca DeepSeek pra Qwen, pra Ollama, e o diferencial acaba.
- **Harness** = o corpo, os reflexos, a sala de cirurgia. Decide qual modelo recebe cada tarefa, com que prompt, e o que fazer com o output.
- **Consequência:** sem harness, o cérebro não vira trabalho útil.

> **Nota do apresentador:**  
> No KIRA, o harness é o `core/` Rust. O modelo é apenas um cliente na `core/src/llm/` (DashScope, DeepSeek, Ollama). Trocar de modelo é mudar uma config em `tokens.toml`.

---

## Slide 04 · A Régua

**Título:** Onde você está nesse espectro?

```
Outside ────→ In ────→ On ────→ Flywheel
 the loop     the loop  the loop  (Self-improving)

"Resolve aí"  Micromanage  Harness     O harness
— e torce     — vira o     Engineering  aprende
pro melhor    próprio      — edita o    com cada
              gargalo      harness,     erro
              (RISCO)      não o output
```

> *"The difference between 'in the loop' and 'on the loop' shows up when you don't like the result. People on the loop change the harness that produced the artifact."* — Kief Morris

- **Outside:** vibe coding.
- **In:** revisa cada linha. Você vira o gargalo.
- **On:** constrói e mantém o harness. Não conserta o código — muda o harness que produziu o código.
- **Flywheel:** o harness aprende sozinho. Cada erro vira regra que entra na próxima rodada.

> **Nota do apresentador:**  
> Quando o KIRA erra, não editamos o output. Editamos: `core/agents/prompts/*.md`, `.kira/workflows/*.yaml`, `tokens.toml`, hooks, thresholds, lessons.

---

## Slide 05 · Como Funciona

**Título:** O harness em volta dos modelos.

**SPEKTOR (referência):** harness em volta de CLIs (Cursor, Claude Code, Codex) via subprocesso.

**KIRA (implementação):** harness híbrido com core Rust daemon + REPL Bun/Ink.

```
┌────────────────────────────────────────────────────┐
│                   HARNESS                           │
│                                                     │
│   SDD  ──  Gates  ──  Judge  ──  Observabilidade   │
│                                                     │
│            CHAMADA via HTTP/SSE                   │
│         ┌──────┐  ┌──────────┐  ┌───────┐          │
│         │DeepSeek│  │Qwen    │  │Ollama │  ...     │
│         └──────┘  └──────────┘  └───────┘          │
└────────────────────────────────────────────────────┘
```

**Por que daemon HTTP em vez de subprocesso CLI?**

- **Agnóstico de modelo:** hoje DeepSeek, amanhã Qwen, depois Ollama. O harness não muda.
- **Isolamento de execução:** cada agente vive em seu próprio git worktree — pode abortar, retomar, limitar.
- **O harness vê tudo:** stdout/stderr, tool calls, gate outcomes, costs — tudo passa pelo core.
- **Trocar é configuração:** mudar de modelo é mudar uma flag em `tokens.toml`.

> **Nota do apresentador:**  
> Diferença arquitetural intencional: Spektor envolve CLIs externos; KIRA é um daemon que orquestra agentes internos (`tokio::spawn`) com worktrees isolados. O princípio é o mesmo: o harness é independente do modelo.

---

## Slide 06 · O Contexto

**Título:** Três coisas mudaram no último ano.

| # | Mudança | Impacto no KIRA |
|---|---------|-----------------|
| **01** | Mesmo modelo, harness diferente → resultado diferente | O router (`core/src/router/`) não é o produto; o produto é o harness ao redor. |
| **02** | Fronteira ainda falha sem disciplina | Opus 4.5 perde tasks longas sem `feature_list`, progresso, commits. KIRA obriga isso via SDD. |
| **03** | OWASP elevou a risco de segurança | Top 10 for Agents 2026: sandbox insuficiente, gates ausentes, fallback mal disparado. KIRA trata isso como first-class. |

> **Tese:** quando você compara dois agentes, você não está comparando modelos. Está comparando harnesses.

---

## Slide 07 · Os 7 Pilares

**Parte 02 / 03**

```
01  Spec-Driven Execution
02  Coordenação Pilot
03  Verification & Judge
04  Completion Gates
05  Guardrails
06  Resiliência
07  Observabilidade + Cognition Lessons

Bônus: Sandbox
```

> **Nota do apresentador:**  
> No KIRA, os 7 pilares estão implementados com nível de maturidade variando de 55% a 95%. Ver `docs/kira-audit-7-pilares.md` para auditoria completa.

---

## Slide 08 · Pilar 01 — Spec-Driven Execution

**Título:** Escreve antes. Executa depois.

**O problema:**
- Agente recebe “implementa SSO”. Sem spec, decide na hora se é SAML, OIDC, cookie, JWT.
- O juiz que vai avaliar depois **não tem critério de aceitação** — porque o critério era a spec que nunca foi escrita.

**O fluxo no KIRA:**

```
┌──────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
│ SPECIFY  │ → │ DESIGN  │ → │ TASKS   │ → │ EXECUTE │
│ spec.md  │   │ design  │   │ tasks   │   │ per-task│
│          │   │ .md     │   │ .json   │   │ agents  │
└──────────┘   └─────────┘   └─────────┘   └─────────┘
      ↑              ↑             ↑
      └── A spec é o gabarito do design ──┘
      └── O design é o gabarito das tasks ──┘
      └── As tasks, com critérios de aceite, são o gabarito da execução ──┘
```

**Como o KIRA faz:**
- Pipeline 4 fases: `SPECIFY → DESIGN → TASKS → EXECUTE` (`core/src/sdd/mod.rs:16-22`)
- Auto-sizing: bug fix ≤5 LOC + ≤1 arquivo → `SkipSpecifyDesign` (`core/src/sdd/mod.rs:48-60`)
- Artefatos: `spec.md`, `design.md`, `tasks.json` em `.kira/runs/<id>/`
- Fase avança só se gate passa (judge entre fases pendente — roadmap v0.5)

> **Nota do apresentador:**  
> SDD não vira burocracia. Tarefa trivial passa por auto-sizing e pula fases. A engine decide sozinha.

---

## Slide 09 · Pilar 02 — Coordenação Pilot

**Título:** Um maestro, vários instrumentos.

**O problema:**
- Agente recebe: “migra autenticação pra OIDC, atualiza dashboard, faz documentação”.
- Sem coordenação, empilha tudo numa chamada, perde contexto, esquece metade, entrega a parte fácil e some.

**O padrão no KIRA:**

```
┌───────┐     ┌──────────┐     ┌──────────┐
│ PILOT │ ──→ │ORCHESTRATOR│ ──→ │  GATES   │
└───────┘     └──────────┘     └──────────┘
  decide         roda a           confere,
 qual task       task             passa pelas
   vem           com o            gates,
                 modelo           reporta
```

- **Pilot** (`core/src/pilot/mod.rs`): decide a próxima intenção — rodar fase SDD, chamar revisão, esperar input humano, abortar.
- **Orchestrator** (`core/src/orchestrator/mod.rs`): pega cada tarefa, prepara contexto, chama o modelo, passa pelas gates, reporta de volta.
- **Gates** (`core/src/gates/mod.rs`): verificam se ficou pronta.

**Como o KIRA faz:**
- `Pilot` trait com `next_intent()` + `observe()`
- `SddPilot`: máquina de estados SPECIFY→DESIGN→TASKS→EXECUTE→Done
- Intents: `RunSddPhase`, `RunSddExecute`, `EscalateTier`, `RequestHumanReview`, `Done`, `Halt`
- Failure policy: `FallbackChain` por `FailureClass` (retry/escalate/halt)

> **Nota do apresentador:**  
> Gap atual: Pilot é SDD-only. Workflows genéricos e PR review ainda não usam o mesmo padrão Pilot/Orchestrator. General Pilot é roadmap v0.5.

---

## Slide 10 · Pilar 03 — Verification & Judge

**Título:** Quem confere o trabalho do agente?

**O problema:**
- Agente recebe “adicionar logout no SSO”.
- Faz refatoração que deleta login antigo, não adiciona logout, testes continuam passando porque ninguém testava o caminho deletado.
- Sistema declara “sucesso” e segue.

> “Testes verdes” não é prova de “tarefa pronta”. É prova de “nada do que estava testado quebrou”.

**O conceito no KIRA:**

**LLM-as-Judge**
- Um segundo modelo, separado do executor, olha o resultado com base em critérios de aceitação e decide se passa.
- **No planejamento:** o plano cobre os requisitos?
- **Na execução:** o entregue corresponde ao pedido?

**Anti-padrão:** mesmo modelo executando e julgando. Ele se autoaprova.

> **judge_model ≠ writer_model** é obrigatório.

**Como o KIRA faz:**
- `AgentKind::Judge` existe no schema (`core/src/proto/agent.rs:12`)
- Judge system prompt: `core/src/agents/prompts/judge.md` (100+ linhas, 5 exemplos few-shot)
- Gate L5: `gate_judge_llm()` (`core/src/gates/mod.rs:633-715`)
- Determinístico: temp=0.0, `response_format=json_object`
- 3-zone classification: L4 faithfulness ≥ 0.80 → skip judge; < 0.55 → fail; entre → escalate

**Gaps críticos:**
- `judge_model ≠ writer_model` **não é enforced** — ambos podem cair em `deepseek-v4-pro` (`heavy` tier)
- Judge é função server-side, não agente autônomo com worktree próprio
- Sem tabela `eval_runs` em redb (julgamentos vão para JSONL flat)

---

## Slide 11 · Pilar 04 — Completion Gates

**Título:** “Pronto” não é o agente dizer. É o sistema provar.

**A cascata no KIRA (6 níveis — excede SPEKTOR):**

```
L1 — Structural Validation      (grátis)  lint · testes · typecheck
  ↓
L2 — File Integrity             (grátis)  não destruiu crítico?
  ↓
L3 — Change Sufficiency         (grátis)  mudou o que foi pedido?
  ↓
L4 — Embedding Faithfulness     (~$0.00007)  “parece” a spec?
  ↓
L5 — Judge LLM                    (~$0.10)  cumpre a spec?
  ↓
L6 — Meta-Review                  (grátis)  evidence_quote existe no diff?
  ↓
TAREFA ACEITA (ou rejeitada)
```

**Short-circuit no primeiro Fail.**

**Como o KIRA faz:**
- L1 tem lint real: `cargo check --offline` ou `bunx tsc --noEmit` (60s timeout)
- L4 tem 3-zone classification: sim < 0.55 → Fail; 0.55–0.80 → Escalate; ≥ 0.80 → Skip Judge (economia de $0.10)
- L6 verifica se citações do judge existem literalmente no diff
- YOLO mode: `skip_embedding + skip_judge + skip_meta_review` para tarefas rápidas (L1+L2 apenas)
- Embed cache: redb-backed, 5000 entries, FNV-1a keyed
- API endpoint: `POST /v1/gates/run`

> **Nota do apresentador:**  
> KIRA excede o Spektor: 6 níveis vs 5 do original. L6 (MetaReview) é bônus não documentado no workshop.

---

## Slide 12 · Pilar 05 — Guardrails

**Título:** Os limites que o agente não pode cruzar.

**Não são sugestões. São portas trancadas.**

| Guardrail | O que faz no KIRA | Arquivo |
|-----------|-------------------|---------|
| **File Integrity** | Ownership registry bloqueia `touched_paths ⊄ owned_paths`. Agente não pode apagar teste que valida a spec. | `core/src/agents/ownership.rs` |
| **Change Sufficiency** | L3 gate verifica `actual_loc` vs `estimated_loc`. Tarefa grande com diff de 2 linhas é sinal vermelho. | `core/src/gates/mod.rs:445-495` |
| **Cost Cap** | Budget hard-stop antes de cada chamada. Esgotou: para. | `core/src/router/budget.rs:41-103` |

**Outros controles implementados:**
- SC3 — Cmd injection: allowlist 20 executáveis, regex args, env_clear, sem shell expansion
- SC4 — Path traversal: `canonicalize()` + prefix check + `'..'` rejection
- SSRF guard: bloqueia IPs privados/loopback/metadata
- SC7 — MCP safety: schema pinning SHA-256, param sanitization
- File I/O caps: 5 MiB hard cap
- Hook system: `PreToolUse` (block-capable) + `PostToolUse`

**Gaps críticos:**
- SC1 — Prompt injection isolation: delimitadores XML não injetados em runtime
- SC2 — Tool risk para built-in tools: bash, file_write não têm nível de risco classificado
- SC5 — Secrets redaction: regex existe mas não filtra logs/audit ativamente
- `kira-core.toml` não existe — config está dispersa em `.kira/hooks.toml`, `.kira/mcp.toml`, etc.

---

## Slide 13 · Pilar 06 — Resiliência

**Título:** Modelos quebram. Provedores quebram. Tarefas travam.

**Os 4 mecanismos no KIRA:**

| Mecanismo | Implementação | Arquivo |
|-----------|---------------|---------|
| **Stall detection** | 3 sinais: SameToolLoop (3x), OutputUnchanged (3x), NoProgress (90s wallclock) | `core/src/agents/spawn.rs:43-54` |
| **Fallback chain** | Tier-config-driven: Heavy → Standard → Light → LongCtx | `core/src/router/mod.rs:147-304` |
| **Escalation policy** | `EscalateTier` intent existe mas é no-op em v0.1 | `core/src/pilot/mod.rs:77-89` |
| **Failure classification** | 6 classes: AgentMistake, InfraTransient, InfraPermanent, BudgetExceeded, Stall, Cancelled | `core/src/router/failure.rs:39-62` |

**Caso real — o que o KIRA evita:**
- Agente popular tinha fallback chain que só disparava em `rate-limit`.
- Stream travava sem `rate-limit` → 15 minutos parado, centenas de reais queimados.
- **Lição:** stall e timeout precisam disparar fallback, não só rate-limit.

**Como o KIRA faz:**
- Circuit breaker: lock-free atomic, Closed→Open→HalfOpen→Closed, 5 falhas/60s, 30s open
- Retry policy diferenciada por classe: AgentMistake (3 retries/2s-8s), InfraTransient (4 retries/500ms-8s), Stall (0 retries)
- Watchdog: agent heartbeat (60s scan, 5min sem progresso → Stale), worktree cleanup (10min órfãos → destroy)
- Agent cancellation: `cancel()` via oneshot channel

**Gaps:**
- Circuit breaker state volátil (reiniciar daemon reseta tudo)
- Sem escalation cross-tier (Light→Standard automático)
- Sem graceful shutdown

---

## Slide 14 · Pilar 07 — Observabilidade

**Título:** Caixa-preta não escala em produção.

**O problema:**
- Se deu certo, o agente disse que deu certo. → *Não dá pra provar.*
- Se deu errado, o agente disse que deu errado. → *Não dá pra refutar.*
- Custo? → *Não dá pra atribuir.*
- Decisão? → *Não dá pra rastrear.*

> Você não tem produto. Você tem fé.

**Duas camadas no KIRA:**

| Camada | Implementação | Arquivo |
|--------|---------------|---------|
| **1. Events** | Pillar events com broadcast SSE + persist JSONL. Exportável (OTel roadmap). | `core/src/observability/pillar.rs:23-57` |
| **2. Harness Bundles** | SHA-256 de `tokens.toml` + workflows + prompts + lessons | `core/src/observability/bundle.rs` |

> “Ficou mais lento desde terça?” — com bundle hash, você compara duas versões do harness e sabe exatamente o que mudou.

**Outros componentes:**
- Audit JSONL: `~/.local/share/kira/audit/pillars.jsonl`
- Judge audit: `~/.local/share/kira/audit/judge-decisions.jsonl`
- Stats endpoint: `GET /v1/stats` — uso de tokens por modelo
- Dataset export CLI: `kira-cored dataset export --judge` → LLaMA-Factory JSONL
- Durable state: redb com 6 tabelas (worktrees, agents, plans, lessons, sdd_runs, embed_cache)
- Tracing: `#[instrument]` + `tracing-subscriber` (JSON e pretty modes)

**Gaps:**
- Sem export OTel nativo (SSE existe, mas sem integração com OpenTelemetry Collector)
- Sem shadow runs / promotion pipeline
- Bundle hash sem endpoint HTTP

---

## Slide 15 · Pilar 07 — Cognition Lessons

**Título:** Erro vira regra. Regra vira playbook.

**O problema:**
- Log diz o que falhou. Playbook diz o que repetir ou evitar antes do próximo tiro.
- A maioria dos agentes para no log. Você acumula 10.000 linhas de trace por semana e continua cometendo o mesmo erro.

**Cognition Lesson no KIRA:**
- Regra compacta gerada quando algo dá errado.
- Não é o trace inteiro.

Estrutura:
- **Gatilho** — qual sinal disparou
- **Anti-padrão** — o que o agente fez de errado
- **Padrão preferido** — o que fazer no lugar
- **Prioridade e reincidência**

**Duas trilhas:**
- **Planning** — antes do executor
- **Execution** — quando gate falha

**Como o KIRA faz:**
- `LessonRegistry` + `LessonGenerator`
- `render_inject_snippet()` injeta lições no system prompt do agente
- BM25 reranker (M20 Sprint 3) filtra lições por relevância à task, não só recência
- Lições persistentes em redb (`TBL_LESSONS`)

**Gap:**
- Módulo `text/` (M17-M20: tokenizer, chunker, compressor, reranker) ainda não implementado

---

## Slide 16 · Bônus — Sandbox

**Título:** Onde o código gerado pelo agente roda?

**Espectro de isolamento:**

```
NENHUM ── DOCKER ── SECCOMP ── gVisor ── Firecracker ── microVMs
(host nu)                                                          (estado da arte)
  ← pior                                          estado da arte →
```

**Por que importa:**
- Código gerado por IA precisa rodar em sandbox real.
- Senão: Docker comprometido → código que apaga arquivos do host, exfiltra dados ou escala privilégio.

**Posição honesta do KIRA:**
- Hoje: isola por git worktree + path guard + command allowlist.
- Melhor que rodar no host nu, mas ainda não é isolamento de nível microVM.
- Caminho natural: Docker → SECCOMP → gVisor → Firecracker.
- Sandbox module existe (`core/src/sandbox/mod.rs`) mas precisa de evolução.

---

## Slide 17 · Comparação — KIRA vs Mercado

**Título:** Não estamos todos no mesmo jogo.

| Ferramenta | Foco primário | Modelo | Disponibilidade |
|---|---|---|---|
| **Spektor** | SDD para software dev | Agent-agnostic, local-first | Em construção |
| **Devin** | Engenheiro autônomo end-to-end | Sandbox cloud proprietário | Closed · US$ 500/mês |
| **KIRA** | Harness agentic solo-dev | Local-first, **LLM-agnostic** (qualquer provider OpenAI-compatible) | Open source (em breve) |
| **OpenClaw** | Assistente pessoal multi-canal | Local + multi-LLM | Open source MIT |
| **Hermes** | Agente pessoal auto-melhorável | Local + multi-LLM | Open source MIT |

> **KIRA** é Spektor-like, **LLM-agnostic** e local-first. Não é assistente pessoal (JARVIS) nem engenheiro cloud fechado. É harness construído em volta de SDD para dev de software; o operador escolhe o(s) provider(s) sem trancar em vendor.

---

## Slide 18 · Comparação — A Tabela Completa

**Título:** A tabela completa — no cenário de desenvolvimento de software.

| Pilar | KIRA | Spektor | Devin | OpenClaw | Hermes |
|---|---|---|---|---|---|
| **1 · Spec-Driven Execution** | ✅ Core, 4 fases | ✅ Core, 4 fases | ❌ Sem método | ❌ | ❌ |
| **2 · Coordenação Pilot** | ✅ Pilot + Orchestrator (SDD-only) | ✅ Pilot + Orchestrator | ✅ Planner/Exec/Verifier | ~ Multi-agent | ~ Subagents |
| **3 · Verification & Judge** | ⚠️ Gate L5 (não agente autônomo) | ✅ Calibrável | ~ Interno | ❌ | ~ Feedback |
| **4 · Completion Gates** | ✅ Cascata 6 níveis | ✅ Cascata 5 níveis | ~ Testes | ❌ | ❌ |
| **5 · Guardrails** | ⚠️ File + Change + Cost (SC1/SC2 gaps) | ✅ File + Change + Cost | ✅ Sandbox cloud | ~ Permissões | ~ Constraint |
| **6 · Resiliência** | ✅ Circuit breaker + fallback + stall | ✅ Multi-gatilho + class. | ✅ Recovery | ~ Retry básico | ~ Retry básico |
| **7 · Observabilidade** | ✅ Events + Bundles + redb | ✅ Event store + Bundles | ~ Dashboard | ~ Dashboard local | ~ Trajectories |
| **Sandbox** | ⚠️ Worktree + path guard | ✅ Configurável até microVM | ✅ | ❌ | ❌ |

> **Só no KIRA:** LLM-agnostic com ladder de tiers config-driven (operator não toca código pra trocar provider), lessons com BM25 reranker, eval framework (OpenRouter opt-in), Rust core + Bun REPL híbrido, fast-path chat conceitual → tier `local` Ollama (~0 tokens pagos).

---

## Slide 19 · Fechamento

**Título:** O modelo é commodity. O harness é a vantagem competitiva.

**O que levar deste workshop:**

1. **Não edite o output. Edite o harness.** Quando o agente erra, mude os prompts, as gates, os guardrails — não o código gerado.
2. **SDD é o gabarito.** Sem spec escrita antes, não há o que julgar depois.
3. **Gates são portas, não sugestões.** L1-L3 são grátis. Use-as sempre.
4. **Judge não pode ser o mesmo modelo que escreveu.** `judge_model ≠ writer_model`.
5. **Observabilidade não é luxo.** Se você não sabe por que deu errado, não sabe o que consertar.
6. **Soberania é estratégia.** DeepSeek e Qwen no hot path. OpenRouter só para eval, opt-in.
7. **O harness aprende.** Cada erro vira lesson. Cada lesson vira regra injetada no próximo agente.

> *"The difference between 'in the loop' and 'on the loop' shows up when you don't like the result. People on the loop change the harness that produced the artifact."* — Kief Morris

---

## Slide 20 · Próximos Passos

**Título:** O que falta para v0.5?

| Prioridade | Item | Esforço |
|---|---|---|
| 🔴 Critical | SC1 — Prompt injection isolation | Médio |
| 🔴 Critical | SC2 — Tool risk para built-in tools | Baixo |
| 🔴 Critical | `judge_model ≠ writer_model` enforcement | Baixo |
| 🟡 Important | Judge autônomo com worktree próprio | Alto |
| 🟡 Important | `kira-core.toml` — config centralizada | Baixo |
| 🟢 Nice-to-have | Módulo `text/` (tokenizer, chunker, compressor, reranker) | Alto |
| 🟢 Nice-to-have | Circuit breaker persistente | Médio |
| 🟢 Nice-to-have | Cross-tier escalation (Light→Standard) | Médio |
| 🟢 Nice-to-have | OTel export nativo | Médio |

**Referências:**
- Workshop original: Felipe Rodrigues, BHub.ai
- Auditoria KIRA × 7 Pilares: `docs/kira-audit-7-pilares.md`
- AGENTS.md: `AGENTS.md`
- Código-fonte: `core/src/`

---

*Fim do roteiro.*
