# Design — Dynamic Research Planning (preflight + fan-out)

Status: **PROPOSAL — review before implementing**
Author: session 2026-06-03

## Problem

A YOLO "study X + write a doc" task burned all 12 Writer iters doing reactive
`web_search`/`web_fetch` (many 404s from guessed URLs) and **failed without
writing output** (`max_iters reached`, status Failed, $0.035, 0 deliverable).

Root cause: the agent **improvises** research inside the main loop — no plan of
*what* to search, no budget for *how many* sources, sequential. It runs out of
iterations before synthesis.

User's instinct (correct): Kira should *decide what to research and how many
search agents to spawn* — dynamically — **before** the heavy loop.

## Key insight: the machine already exists

The workflow EXECUTE phase already fans `tasks.json` into **parallel waves**
(`sdd/wave_executor.rs::execute` over a `TaskSpawner`). Research is the same
shape: N independent fetches → barrier → synthesize. We reuse it.

## Proposed flow

```
            ┌─ PREFLIGHT (1 cheap LLM call) ─┐
goal ──────▶│ emit research_plan JSON        │
            │  { queries:[...], max_sources, │
            │    parallel: true }            │
            └──────────────┬─────────────────┘
                           │ (dynamic N from task breadth)
                  ┌────────▼─────────┐
                  │ web_search each  │  → collect REAL result URLs
                  │ query (bounded)  │
                  └────────┬─────────┘
                           │ fan-out (reuse wave_executor)
        ┌──────────┬───────┴───────┬──────────┐
        ▼          ▼               ▼          ▼
     fetch u1   fetch u2   …    fetch uN   (parallel, read-only)
        └──────────┴───────┬───────┴──────────┘
                           ▼
                  research-notes.md  (aggregated)
                           ▼
                  Writer synthesizes the .md  (only from notes)
```

`research_plan.max_sources` scales with breadth:
- narrow ("what is X?") → 1–2
- "study X" → 4–6
- "comprehensive study + community usage" → 6–8 (hard cap ~8)

## Where it plugs in

Trigger: research-heavy task detection (extends the existing `looks_conceptual`
/ `is_research_doc` classifiers). When true:
1. Route to a **preflight** step instead of straight Writer.
2. Preflight → search → `wave_executor::execute` with one task per URL
   (`TaskSpawner` spawns a read-only fetch agent, `max_iters` ~2 each).
3. Aggregate tool outputs → write `research-notes.md` in the run dir.
4. Spawn the Writer with the notes as context (no web tools, just synthesize).

### Implementation options (pick at review)

| Option | What | Pros | Cons |
|--------|------|------|------|
| **A. New `Researcher` agent kind** | preflight+search+fan-out lives in a dedicated kind | clean separation, reusable from PLAN too | new kind + prompt + wiring |
| **B. Preflight node in chat_yolo** | yolo path does preflight→waves→Writer inline | smallest surface, YOLO-only | not reused by PLAN/SDD |
| **C. SDD `research` phase** | add a phase to a `research` workflow.toml | reuses workflow engine fully | heavier; gate/judge overhead |

**Recommendation: A (Researcher kind)** — reused by both YOLO and a future
`research` workflow; fan-out via existing `wave_executor`.

## Reused vs new

- **Reuse:** `wave_executor::execute` + `TaskSpawner` (fan-out), `web_search`/
  `web_fetch` tools, agent spawn infra, run-dir artifacts.
- **New:** `Researcher` kind + prompt (`agents/prompts/researcher.md`), a
  preflight that emits `research_plan` JSON, a fetch-task spawner adapter
  (bounded `max_iters`, read-only), notes aggregation.

## Guardrails (cost safety)

- Hard cap `max_sources` (≤8) regardless of plan.
- Each fetch sub-agent `max_iters` ≤ 2 (fetch + done).
- Dedup URLs; skip already-fetched (tool cache already helps).
- Anti-fabrication URL rule (shipped in c3be89f) keeps fetches on real links.
- Total research budget cap (USD) before synthesis.

## Out of scope (v1)

- Iterative deepening (re-plan after first pass).
- Quality scoring of sources.
- Cross-run research memory.

## Open questions for review

1. Option A / B / C?
2. Preflight model — cheap (`qwen` local) or `standard` (deepseek)? Local 3B
   may emit bad JSON (we've seen lesson-gen JSON fail on qwen).
3. Should PLAN's SPECIFY/DESIGN phases also route research through this, or
   keep it YOLO-only for v1?
4. `max_sources` hard cap — 8 ok?
