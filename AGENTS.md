# AGENTS.md — Kira

> Agent-first documentation following the [agents.md spec](https://agents.md/). Read this before proposing changes. Keep short, actionable, current.

## Overview

**Kira** is Sergin's personal agentic harness. Codename from Death Note (silent agent). Core thesis: **LLM-agnostic** — any OpenAI-compatible provider plugs in via `tokens.toml`. The harness is the product; output is a consequence.

**Architecture**: Pure Rust monorepo with two crates:

| Crate | Binary | Role |
|---|---|---|
| `core/` | `kira-cored` | Daemon (Axum HTTP+SSE on `127.0.0.1:7777`). Cognition, guardrails, memory, queue, agents, tools. |
| `kira-cli/` | `kira` | TUI (Ratatui) + non-interactive CLI. Operator interface. |

Communication: local HTTP `/v1/*` + SSE for event streaming.

## Operating Principle

> *"The difference between 'in the loop' and 'on the loop' shows up when you don't like the result."*

### Maturity Scale

| Level | Name | Behavior |
|---|---|---|
| 0 | Outside the loop (Vibe Coding) | "Resolve aí" e torce. **RISCO.** |
| 1 | In the loop (Micromanage) | Revisa cada linha gerada. **GARGALO.** |
| 2 | On the loop (Harness Engineering) | Constrói e mantém o harness. **QUALIDADE.** |
| 3 | Flywheel (Self-Improving) | Erro vira regra automática. **DIFERENCIAL.** |

When agent output is bad, **edit the harness, not the output**:
1. **Workflows** in `.kira/workflows/*.yaml`
2. **Prompts** in `core/agents/prompts/*.md`
3. **Hooks** in `.kira/hooks.toml`
4. **Tokens / thresholds** in `tokens.toml`

## Build & Run

### Core daemon

```bash
cargo run -p kira-core                    # dev (port 7777)
cargo build --release -p kira-core        # binary: target/release/kira-cored
```

### CLI / TUI

```bash
cargo run -p kira-cli                     # TUI mode
cargo run -p kira-cli -- chat "hello"     # non-interactive
cargo build --release -p kira-cli         # binary: target/release/kira
```

### Full dev workflow

- Terminal 1: `cargo run -p kira-core`
- Terminal 2: `cargo run -p kira-cli`

### Quality gates

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

Prereqs: `rustc 1.85+` stable. Mac aarch64 or Linux x86_64.

## Test

| Layer | Command |
|---|---|
| Unit + integration | `cargo test --workspace` |
| Property-based | `cargo test --workspace -- --include-ignored` (proptest) |
| TUI snapshot | `cargo test -p kira-cli -- tui` |
| E2E | run both binaries, issue commands, inspect audit/*.jsonl |

## LLM-Agnostic Provider Stack

**Principle**: KIRA is a harness, not a model brand. Any OpenAI-compatible provider plugs in.

**Configuration** (`tokens.toml`):

```toml
[providers.deepseek]
url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
enabled = true

[providers.ollama]
url = "http://127.0.0.1:11434/v1"
enabled = true

[providers.openrouter]
url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
enabled = false  # opt-in

[tiers.heavy]
candidates = ["deepseek:deepseek-chat"]
[tiers.standard]
candidates = ["deepseek:deepseek-chat", "ollama:qwen2.5:7b"]
[tiers.light]
candidates = ["ollama:qwen2.5:1.5b"]
[tiers.judge]
candidates = ["deepseek:deepseek-chat"]  # MUST differ from writer
```

**Strict rule**: the operator chooses providers freely. No provider is privileged in code. CI enforces no hardcoded provider URLs:

```bash
grep -ri 'api\.anthropic\.com\|api\.openai\.com' core/ kira-cli/src/ && exit 1
```

## 7 Pillars (v0.5 target)

| # | Pillar | Status | Notes |
|---|---|---|---|
| P1 | **Workflows** | 🔄 Generalizing | SDD (SPECIFY→DESIGN→TASKS→EXECUTE) is the first Workflow; engine becoming methodology-agnostic (any front-end → Waves). Auto-Sizing skips trivial phases. |
| P2 | **Pilot + Watari** | 🔄 Generalizing | Generic `WorkflowPilot` walks any Workflow DAG; **Watari** (orchestrator) executes intents. `SddPilot` retired after parity. |
| P3 | **Judge** | ✅ | LLM-as-Judge, `judge_model ≠ writer_model` enforced at router |
| P4 | **Detective** | ✅ | Universal 6-lens code review (security, requirements, tests, architecture, regression, performance). Gate `"detective"` pluggable into any workflow. |
| P5 | **Guardrails** | ✅ | 3-level: Input (injection/PII/forbidden), Tool (risk classification/HITL), Output (LLM-as-Judge) |
| P6 | **Resilience** | ✅ | Circuit breaker, fallback chain, stall detection, failure classification |
| P7 | **Ryuk / Observability** | ✅ | Pillar events (SSE+JSONL), bundle hash (SHA-256), Lessons (error→rule), GuardrailTriggered, MemoryInjected |

## Guardrails Architecture (3 Levels)

```
Level 1 — Input (core/src/safety/input.rs)
├── Prompt injection detection (keyword + Levenshtein + XML delimiter neutralization)
├── PII redaction (email, CPF, phone → [REDACTED])
└── Forbidden words blocklist

Level 2 — Tool (core/src/safety/tool.rs)
├── XML delimiter neutralization on tool output
├── Truncation (> tool_output_max_bytes)
├── Risk classification: file_read→LOW, bash→HIGH, bash+rm→CRITICAL
└── HITL approval for HIGH/CRITICAL

Level 3 — Output (core/src/safety/output.rs)
├── LLM-as-Judge evaluation (judge_model ≠ writer_model)
├── Verdicts: pass | warn | block
└── Emits GuardrailTriggered event
```

**XML Delimiter Enforcement** (`core/src/safety/delimiters.rs`):
- `<SYSTEM_INSTRUCTIONS>` — only trusted region (commands)
- `<USER_DATA>` — user messages (data, never commands)
- `<TOOL_OUTPUT untrusted>` — tool returns
- `<RAG_CONTEXT untrusted>` — memory/RAG fragments

## Resilience Architecture

**Stall Detection** (3 signals):
1. Same tool called 3x consecutively with same args
2. File hash unchanged after iteration
3. 90 seconds without progress (no new tool call or output)

**Failure Classification**:
- `AgentMistake` — logic error, retry with lesson injection
- `InfraTransient` — provider timeout/5xx, retry with fallback
- `Stall` — detected via 3 signals above, abort + notify

**Fallback Chain**: Heavy → Standard → Light (transparent escalation on provider failure)

**Circuit Breaker**: Closed → Open → HalfOpen (lock-free atomic state)

## Sandbox Roadmap

| Phase | Isolation | Status |
|---|---|---|
| v0 | Git Worktree + Path Guard | ✅ Active |
| v0.5 | Docker container per agent | Planned |
| v1.0 | MicroVM (Firecracker) per task | Future — gold standard |

> Code generated by AI executing bash is attack vector #1. The roadmap paves the path to MicroVMs.

## Memory Architecture (Cross-Thread Semantic + Maturation)

```
core/src/memory/
├── store.rs              — Persist fragments (sqlite-vec default | postgres+pgvector)
├── retrieval.rs          — Pre-LLM cosine search + decay scoring, inject in <RAG_CONTEXT>
├── extractor.rs          — Post-run async extraction (tier light LLM)
├── tier.rs               — MemoryTier enum (working → episodic → semantic → procedural)
├── decay.rs              — Exponential decay scoring at retrieval time (read-only)
├── consolidator.rs       — Tiered consolidation engine + periodic timer
├── handoff.rs            — Session handoff generation, persistence (redb), injection
├── maturation_events.rs  — SSE event emitters for all maturation operations
├── sqlite_backend.rs     — SQLite-vec backend with maturation schema migration
└── reranker.rs           — BM25 reranker to filter lessons/fragments by relevance
```

### Memory Maturation (P7 Cognition)

Three complementary subsystems that compact knowledge and enable session continuity:

1. **Consolidation** — fragments progress through tiers (working → episodic → semantic → procedural), reducing retrieval token cost while preserving knowledge.
2. **Exponential Decay** — memories not accessed lose retrieval priority via passive scoring at retrieval time. Procedural-tier fragments are exempt.
3. **Session Handoff** — structured summary generated at session end, injected at next session start for continuity.

**Tier Lifecycle**:
```
[New Fragment] → Working → (session end, LLM) → Episodic
                                                    ↓
                              (clustering, LLM) → Semantic
                                                    ↓
                         (access threshold, no LLM) → Procedural (exempt from decay)
```

**Configuration** (`tokens.toml`):
```toml
[memory.maturation]
enabled = true
decay_factor = 0.95              # per-day decay (0.5–0.999)
decay_floor = 0.01               # minimum effective score
min_fragments_for_consolidation = 3
semantic_similarity_threshold = 0.75
min_episodic_for_semantic = 5
semantic_check_interval_hours = 6
procedural_access_threshold = 10
procedural_min_age_days = 7
max_handoff_chars = 2000
max_promotions_per_cycle = 50
```

**Hot-reload**: `kill -HUP <pid>` reloads `[memory.maturation]` without restart. In-progress operations finish with old config.

**Graceful degradation**: LLM unavailable → consolidation/handoff skip, decay still applies, no request path blocked.

**API**:
- `GET /v1/memory/{run_id}/context` — fragments with tier field
- `GET /v1/memory/search?q=` — decay-adjusted ranking
- `DELETE /v1/memory/{id}`
- `GET /v1/memory/maturation/status` — consolidation state, fragment counts per tier
- `POST /v1/memory/maturation/consolidate` — trigger immediate consolidation (202/409)
- `GET /v1/memory/handoff` — current session handoff (200/404)
- `DELETE /v1/memory/handoff` — clear pending handoff (204)
- `GET /v1/memory/{fragment_id}/metadata` — tier, access_count, effective_score

**Prompt files** (editable by operator, loaded from disk at each invocation):
- `core/agents/prompts/memory_consolidator.md` — instructs LLM to merge fragments
- `core/agents/prompts/session_handoff.md` — instructs LLM to generate handoff summary

## Queue Architecture (Resilience Buffer)

```
core/src/queue/
├── mod.rs         — PostgreSQL message_queue table ops
└── worker.rs      — Tokio long-running task, semaphore concurrency, FIFO, retry+backoff
```

**API**: `GET /v1/queue`, `DELETE /v1/queue/{id}`
**Flow**: POST /v1/chat → 202 Accepted → Worker consumes → Agent executes

## Rust Conventions

- **Errors**: `anyhow::Result<T>` everywhere. No `unwrap()` or `expect()` in production.
- **Async**: `#[tokio::main]`. Native `impl Trait in trait` (Rust 1.75+).
- **Concurrency**: `tokio::sync::mpsc` channels preferred over `Arc<Mutex<T>>` where possible.
- **Lifetimes**: prefer owned types (`String`, `Vec<T>`) in structs.
- **Logs**: `tracing` + `#[instrument]` on handlers. No `println!` in production.
- **No literals**: zero hardcoded URLs or paths in source. Everything via `tokens.toml`.
- **No `unsafe`**: `#![forbid(unsafe_code)]` in both crates.
- **Config**: loaded once at startup, shared via `Arc<Tokens>`.

## Directory Structure

```
kira/
├── AGENTS.md                  ← this file
├── Cargo.toml                 ← workspace root (members: core, kira-cli)
├── tokens.toml                ← single source of truth (providers, tiers, safety, memory, queue)
├── .env                       ← secrets, gitignored, chmod 600
├── core/                      ← Rust daemon
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── config/            ← TOML loaders
│       ├── server/            ← axum routes /v1/*
│       ├── router/            ← LLM tier routing + circuit breaker
│       ├── llm/               ← Provider clients (OpenAI-compatible)
│       ├── agents/            ← Spawn, pilot, orchestrator
│       ├── safety/            ← Guardrails (input, tool, output, delimiters)
│       ├── memory/            ← Cross-thread semantic memory
│       ├── queue/             ← PostgreSQL resilience buffer
│       ├── gates/             ← Completion gates cascade
│       ├── hooks/             ← Hook runner
│       ├── sdd/               ← Spec-Driven Execution
│       ├── worktree/          ← Git worktree manager
│       ├── observability/     ← Pillar events + audit
│       └── state/             ← redb wrappers
├── kira-cli/                  ← Rust TUI + CLI
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── config.rs
│       ├── app.rs
│       ├── tui.rs
│       ├── ui.rs
│       ├── core_client.rs
│       ├── sse.rs
│       ├── polling.rs
│       ├── commands/
│       └── views/
├── .kira/                     ← runtime state, gitignored
│   ├── workflows/*.yaml
│   ├── hooks.toml
│   └── runs/
└── docs/
```

## Sub-agents

| Kind | Tier | Owns (FS) | Role |
|---|---|---|---|
| `writer` | Standard | `src/`, `public/` | Executes workflow phases — writes code, specs, tasks |
| `deployer` | Light | `scripts/`, `dist/` | Runs deploy scripts |
| `judge` | Judge (≠ writer) | read-only | LLM-as-Judge gate evaluator |
| `detective` | Judge (≠ writer) | read-only | 6-lens code reviewer — security/requirements/tests/arch/regression/perf |
| `reviewer` | Heavy | read-only | Generic review agent (single-dim; use detective for multi-dim) |
| `explorer` | Standard | read-only | Codebase exploration, research |

**Detective** runs as a star topology: one agent per lens in parallel → consolidator → verdict `pass|warn|block`. Add as a workflow gate: `gate = "detective"` in any phase.
| `detective` | Judge (≠ writer) | read-only | universal code reviewer; runs as gate or standalone |

Coordination: Star topology (core orchestrates). Agents communicate via FS mailbox at `.kira/mailbox/<agent_id>/`.

### Detective — universal code review (embedded)

`detective` is a language/framework-agnostic multi-dimension reviewer (generalized from the
`pr-review` skill, with no `gh`/Jira coupling). It fans out **6 lenses** in parallel (star
topology), each a judge-tier sub-agent, then a consolidator merges findings into a verdict
`pass | warn | block` (Gate semantics).

| Lens | Looks for |
|---|---|
| Security | injection, secrets, missing authz, PII in logs, unsafe patterns |
| Requirements/DoD | diff vs. the Workflow's own `spec.md` + `tasks.json` (no Jira needed) |
| Tests | new public fns/handlers/endpoints without coverage |
| Architecture | repo conventions (`AGENTS.md`, `.kira/review/*.md`) |
| Regression/Hallucination | unrelated deletions, phantom imports, dup logic, weakened assertions |
| Performance | N+1, unbounded queries, parallelizable sequential `await` |

- **Source:** worktree `git diff` (primary); optional VCS adapter for a GitHub PR.
- **Embedding:** agent kind `detective` · registry gate `gate = "detective"` · CLI `kira detective [--diff | --pr N]`.
- **Config** (`tokens.toml`): `[detective] dimensions = [...]`, `min_confidence = 0.8`, `tier = "judge"`.
- **Rules:** confidence ≥ 0.8, one positive highlight, never auto-approves, dedup `{path,line}±3`.

## Workflows (Methodology-Agnostic)

A **Workflow** is a development methodology declared as data, not code. SDD is the first one;
GSD2, Spec Kit, Superpowers, BMAD plug in the same way. They differ only in the *front half*
(goal → task graph); the *back half* (Waves → gates → ship) is universal.

```
workflows/*.toml ─▶ WorkflowDef ─▶ WorkflowPilot ─▶ Intent ─▶ Watari ─▶ Waves
   (front-end)       (parse)        (DAG walk)       (IR)    (+registry)  (back-end)
```

- **Workflow** = a `workflow.toml` (a DAG of Phases). **Phase** = one step (specify, design…).
- **Narrow waist** = `tasks.json` (Task DAG). Every Workflow ends by emitting this → Waves.
- **Phases** route via conditional edges (`EdgeCond`: gate passed/failed, sizing skip). Loops
  that can't be expressed declaratively drop to a custom Rust `Pilot`.
- **Shared services** are resolved by name from a registry (DI): `gate = "judge"`,
  `gate = "detective"`, `sizing = "loc_heuristic"`. Adding a service = Rust; using it = config.

```toml
# core/workflows/sdd.toml (illustrative)
sizing = "loc_heuristic"

[[phase]]
id = "specify"
prompt = "sdd/specify.md"
gate = "judge"
clarify = true
produces = "spec.md"
edges = [{ to = "design" }]

[[phase]]
id = "tasks"
prompt = "sdd/tasks.md"
produces = "tasks.json"          # the narrow waist → Waves
edges = [{ to = "execute" }]
```

## Waves (Parallel Execution Model)

Waves are the universal execution back-end every Workflow compiles to. They represent groups of tasks that can execute in parallel, with synchronization barriers between groups.

**Planning** (SDD TASKS phase):
1. SDD generates tasks with explicit dependency declarations
2. The Pilot analyzes the dependency graph and groups tasks into waves
3. Tasks with no unresolved dependencies are placed in the same wave
4. A wave N+1 only contains tasks whose dependencies are all in waves ≤ N

**Execution** (Orchestrator):
1. Orchestrator processes waves sequentially (wave 1 → wave 2 → ...)
2. Within a wave, Orchestrator spawns one agent per task in parallel (star topology)
3. All agents in a wave must reach terminal status before the next wave begins
4. If any agent in a wave fails, the Orchestrator applies the configured policy:
   - `continue`: skip failed task, proceed to next wave (default for non-blocking tasks)
   - `abort`: halt execution, mark remaining tasks as `skipped`
   - `retry`: re-spawn failed agent up to `max_attempts` before deciding

**Configuration** (`tokens.toml`):
```toml
[waves]
max_parallel_agents = 3          # max concurrent agents per wave
failure_policy = "continue"      # continue | abort | retry
retry_max_attempts = 2
```

**Visualization** (CLI):
- `/waves` shows the current wave plan with status per task
- Thread view groups agents by wave, showing wave progress (e.g., "Wave 2: 3/5 done")
- QueueDepthChanged events reflect wave-level parallelism

## Security Rules

| # | Rule | Mechanism |
|---|---|---|
| 1 | Prompt injection isolation | XML delimiters in `safety/delimiters.rs` |
| 2 | Tool risk classification | `safety/tool.rs` — LOW/HIGH/CRITICAL |
| 3 | OS command injection | `bash` tool: allowlist + arg regex + `Command::args(&[...])` |
| 4 | Path traversal | `canonicalize()` + prefix check |
| 5 | Secrets | `.env` chmod 600, log redaction regex |
| 6 | Audit integrity | JSONL append-only, ISO timestamps, no PII |
| 7 | Judge separation | `judge_model ≠ writer_model` enforced at router level |

## Hooks Contract

Configured in `.kira/hooks.toml`:

```toml
[[hooks]]
event = "PreToolUse"
matcher = { tool = "bash", regex = "rm -rf" }
command = ["sh", "-c", "echo 'BLOCKED' >&2 && exit 1"]
timeout_secs = 5

[[hooks]]
event = "PostToolUse"
matcher = { tool = "file_write", path_glob = "**/*.rs" }
command = ["cargo", "fmt", "--", "{{file}}"]
timeout_secs = 30
```

Events: `SessionStart`, `PreToolUse` (block-capable), `PostToolUse`, `WorktreeSpawn`, `WorktreeDestroy`, `BudgetWarn`, `BudgetStop`.

## Operating Philosophy

- **LLM-agnostic**: any OpenAI-compatible provider plugs in via config. The harness doesn't care which model runs.
- **On the loop**: edit the harness, not the output.
- **Worktrees are disposable**: durable state lives in `redb` and `audit/`.
- **Tokens are expensive, agents are cheap**: prefer two Light-tier sub-agents over one Heavy.
- **Fail closed**: when risk classification, budget guard, or audit logging fails — halt.
- **Memory compounds**: every successful run potentially generates memory fragments for future runs.
- **Bundle hash**: every execution carries a SHA-256 fingerprint of config + prompts + workflows. If performance drops, compare exact bundles.

## Glossary

| Term | Meaning |
|---|---|
| **Kira** | this harness |
| **Core** | Rust daemon on `127.0.0.1:7777` |
| **CLI** | Ratatui TUI + Clap non-interactive client |
| **Worktree** | git worktree isolated per agent run |
| **Pilot** | state machine deciding the next intent; generic `WorkflowPilot` walks any Workflow DAG |
| **Watari** | the orchestrator — executes intents emitted by the Pilot |
| **Judge** | LLM-as-Judge evaluator/gate; `judge_model ≠ writer_model` |
| **Detective** | universal multi-dimension code reviewer (6 lenses), agent kind + registry gate |
| **Ryuk** | observability — pillar events, audit, bundle hash |
| **Workflow** | a methodology as data (`workflow.toml`), a DAG of Phases |
| **Phase** | one step of a Workflow (specify, design, tasks, execute…) |
| **Narrow waist** | `tasks.json` (Task DAG) — every Workflow compiles to this → Waves |
| **Gate** | deterministic check in the 6-level cascade |
| **SDD** | Spec-Driven Execution — the first Workflow (specify→design→tasks→execute) |
| **Lesson** | compact rule generated from a failure |
| **Memory Fragment** | semantic memory unit with embedding, persisted cross-thread |
| **Memory Tier** | maturation level — working (raw), episodic (session summary), semantic (cross-session), procedural (rules) |
| **Consolidator** | background subsystem that merges lower-tier fragments into higher-tier knowledge via LLM |
| **Handoff** | structured session summary (context, open questions, next steps, decisions) for continuity |
| **Decay** | exponential scoring adjustment at retrieval time — stale fragments lose priority |
| **Queue** | PostgreSQL message buffer for resilience |
| **Tier** | LLM routing level (heavy, standard, light, judge) |
| **HITL** | Human-in-the-loop approval for HIGH/CRITICAL risk |

## When in doubt, read

1. **Specs**: `.kiro/specs/kira-cli/` (unified requirements + design)
2. **Workshop Harness Engineering**: `docs/workshop-harness-engineering.md`
3. **Auditoria 7 Pilares**: `docs/kira-audit-7-pilares.md`
4. [agents.md spec](https://agents.md/) · [OWASP Cheatsheets](https://cheatsheetseries.owasp.org/)
