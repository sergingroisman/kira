<div align="center">

# 🪐 Kira

**A methodology-agnostic agentic harness. Full Rust. LLM-agnostic.**

*The harness is the product; the output is a consequence.*

</div>

---

## What is Kira?

Kira is a personal agentic harness — the engineering layer that sits between you and any
LLM and turns "resolve aí e torce" into a controlled, auditable, self-improving pipeline.

Two principles define it:

- **Full Rust.** A single Rust monorepo: a daemon (`core/`) and a TUI/CLI (`kira-cli/`).
  No Node in the hot path. `#![forbid(unsafe_code)]` in both crates.
- **LLM-agnostic.** No model is privileged in code. Any OpenAI-compatible provider
  (DeepSeek, Qwen, Ollama, OpenRouter, …) plugs in via `tokens.toml`. CI rejects hardcoded
  provider URLs. Swap the brain without touching the harness.

> The difference between *"in the loop"* and *"on the loop"* shows up when you don't like
> the result. When the output is bad, **edit the harness, not the output.**

---

## The core idea: everything compiles to Waves

Kira treats a development methodology like a **compiler front-end**. SDD, GSD2, Spec Kit,
Superpowers, BMAD — they all differ only in the *front half* (how a goal becomes a task
graph). The *back half* — executing that graph in parallel, gating it, shipping it — is
universal.

```
   front-end (methodology)        narrow waist             back-end (universal)
 ┌───────────────────────┐       ┌──────────────┐        ┌──────────────────┐
 │ SDD    specify→design │──┐    │              │        │ Waves            │
 │ GSD2   discuss→plan   │──┼──▶ │  tasks.json  │ ──────▶│ parallel agents  │
 │ SpecKit constit→…     │──┤    │  (Task DAG)  │        │ → gates → ship   │
 │ Superpowers brain→…   │──┘    │              │        │                  │
 └───────────────────────┘       └──────────────┘        └──────────────────┘
```

- A **Workflow** = a methodology, declared as a `workflow.toml` (a DAG of **Phases**).
- The **narrow waist** = `tasks.json`, a task dependency graph.
- A **Wave** = a group of independent tasks executed in parallel, with a barrier between
  waves. *Everything ends up here.*

Adding a new methodology is authoring a new TOML — not writing Rust. Methodologies that
need real loops (Ralph, TDD red-green) drop down to a custom Rust `Pilot`.

See the diagrams: [`docs/deck/kira-compiler.excalidraw`](docs/deck/kira-compiler.excalidraw)
(flow) and [`docs/deck/kira-arch.excalidraw`](docs/deck/kira-arch.excalidraw) (layers).

---

## The cast

Kira's agents carry a light Death Note theme. Codename *Kira* = the silent operator.

| Agent | Role |
|---|---|
| **Pilot** | Decides the next `Intent` from run state (pure state machine, no I/O). |
| **Watari** | The orchestrator — executes the intents the Pilot emits. |
| **Judge** | LLM-as-Judge evaluator. Gates phases. `judge_model ≠ writer_model`, enforced at the router. |
| **Detective** | Universal multi-dimension code reviewer (security · requirements · tests · architecture · regression · performance). Language- and framework-agnostic. Runs as a workflow gate or standalone. |
| **Ryuk** | Observability — watches everything: pillar events (SSE + JSONL), audit log, bundle hash. |

---

## Architecture

| Crate | Binary | Role |
|---|---|---|
| `core/` | `kira-cored` | Daemon — Axum HTTP+SSE on `127.0.0.1:7777`. Cognition, guardrails, memory, queue, agents, workflows. |
| `kira-cli/` | `kira` | Ratatui TUI + non-interactive CLI. Operator interface. |

Communication is local HTTP `/v1/*` + SSE.

**Built-in guardrails** (3 levels): Input (injection/PII/forbidden) → Tool (risk
classification, HITL on HIGH/CRITICAL) → Output (LLM-as-Judge). XML delimiters isolate
trusted instructions from untrusted data. Worktree isolation per agent run.

For the full agent-facing reference, read [`AGENTS.md`](AGENTS.md).

---

## Quickstart

Prereqs: `rustc 1.85+` stable, Mac aarch64 or Linux x86_64. Configure providers in
`tokens.toml` and secrets in `.env` (chmod 600).

```bash
# Terminal 1 — daemon
cargo run -p kira-core            # http://127.0.0.1:7777

# Terminal 2 — TUI
cargo run -p kira-cli

# non-interactive
cargo run -p kira-cli -- chat "hello"

# start a workflow run
kira workflow --goal "add JWT auth to /v1/user" --workflow sdd
kira workflow --goal "fix off-by-one in parser" --workflow gsd2
kira workflows                           # list: sdd, gsd2, …

# Detective — 6-lens code review
kira detective                           # reviews git diff HEAD
kira detective --format json             # machine-readable output
kira detective --spec .kira/runs/X/spec.md --tasks .kira/runs/X/tasks.json

# TUI slash commands
# /workflow <goal>  /detective  /workflows  /waves  /memory search <q>
```

### Quality gates

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

---

## Provider config (`tokens.toml`)

```toml
[providers.deepseek]
url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
enabled = true

[providers.ollama]
url = "http://127.0.0.1:11434/v1"
enabled = true

[tiers.judge]
candidates = ["deepseek:deepseek-chat"]   # MUST differ from writer
```

No provider is privileged. The operator chooses freely.

---

## Status

Kira is under active evolution. The methodology-agnostic Workflow engine (front-ends →
Waves) and the Detective reviewer are the current frontier — see the plan and
[`docs/`](docs/) for the roadmap and the 7-pillar audit.

---

<div align="center">
<sub>On the loop, not in the loop.</sub>
</div>
