---
phase: 01-agent-foundation
plan: 01
subsystem: agent
tags: langgraph, deepseek, ink, cli, monorepo, typescript

# Dependency graph
requires: []
provides:
  - npm workspace monorepo with agent/ and shared/ packages
  - StateGraph with callLLM -> routeNext -> END topology
  - MemorySaver checkpointer with per-session thread_id
  - Ink REPL UI with streaming token output
  - CLI entrypoint (sergin bin) with env var validation
affects:
  - Plan 02: Tool Slice (builds on graph topology, adds tools)
  - Plan 03: Search + Memory (builds on MemorySaver, adds Tavily)
  - Plan 04: UI Polish (builds on Ink component structure)

# Tech tracking
tech-stack:
  added:
    - "@langchain/langgraph@1.3.0"
    - "@langchain/deepseek@1.0.25"
    - "ink@7.0.2"
    - "react@19.x"
    - "tsx@4.21.0"
    - "vitest@2.x"
  patterns:
    - Hand-crafted StateGraph (no createReactAgent)
    - Token streaming via graph.stream(streamMode: "messages")
    - Token batching via useRef accumulator flushed at 16ms
    - Navbar conditional edge routing (routeNext)
    - Ink useInput for raw mode (no direct setRawMode)
    - ESM-only (type: module, .js extension on local imports)
    - npm workspaces monorepo structure

key-files:
  created:
    - package.json (root workspace)
    - agent/package.json (sergin-agent package)
    - agent/src/graph/graph.ts (compiled StateGraph)
    - agent/src/graph/nodes/callLLM.ts (LLM invocation)
    - agent/src/graph/nodes/routeNext.ts (conditional edge)
    - agent/src/ui/App.tsx (root Ink component)
    - agent/src/ui/InputBar.tsx (useInput-based input)
    - agent/src/cli/index.ts (CLI entrypoint)
    - agent/tests/graph/skeleton.test.ts (smoke test)
  modified: []

key-decisions:
  - "Fixed react version to ^19.2.0 (ink 7.0.2 peerdep requires >=19.2.0; plan had ^18.3.1)"
  - "Using npm workspaces hoisted node_modules (packages resolve from workspace root)"

patterns-established:
  - "Pattern 1: Graph node signature — async (state: GraphState) => Partial<GraphState>"
  - "Pattern 2: MemorySaver with one thread_id per session, reused across turns"
  - "Pattern 3: Token accumulation via useRef with interval flush (16ms, ~60fps)"
  - "Pattern 4: Static for completed history, useState for streaming tokens"

requirements-completed:
  - AGENT-01
  - AGENT-07

# Metrics
duration: 9min
completed: 2026-05-09
---

# Phase 01 Agent Foundation — Walking Skeleton

**Monorepo scaffold with LangGraph StateGraph (callLLM -> END), Ink REPL UI with streaming token output, and MemorySaver conversation history**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-09T22:40:00-03:00
- **Completed:** 2026-05-09T22:49:00-03:00
- **Tasks:** 2
- **Commits:** 4 (2 per task due to TDD split)
- **Files created:** 20

## Accomplishments
- npm workspace monorepo with `agent/` and `shared/` packages, TypeScript ESM config
- Hand-crafted StateGraph: START -> callLLM -> routeNext -> END with MemorySaver checkpointer
- Ink REPL UI: InputBar (useInput), StreamingOutput (batched tokens), Spinner (animated)
- CLI entrypoint with dotenv loading and env var validation before render
- TDD cycle completed: RED test commit (3 failing tests) -> GREEN implementation (3 passing tests)
- RouteNext returns `__end__` when message has no tool_calls; ready for tool routing in Plan 02

## Task Commits

Each task was committed atomically:

1. **Task 1: Monorepo scaffold + agent package setup**
   - `2104411` feat(01-01): create monorepo scaffold and agent package
   - `87dc821` chore(01-01): add .gitignore and package-lock.json

2. **Task 2: Walking Skeleton graph + UI + CLI (TDD)**
   - `73d580b` test(01-01): add failing test for Walking Skeleton graph (RED)
   - `e52f98b` feat(01-01): implement Walking Skeleton graph, UI, and CLI entrypoint (GREEN)

**Plan metadata:** `pending` (docs: complete 01-01 plan)

## Files Created/Modified

### Task 1 — Scaffold (8 files)
- `sergin-harness/package.json` — Root workspace with `agent` and `shared` workspaces
- `sergin-harness/tsconfig.base.json` — Shared TypeScript config (ES2022, NodeNext)
- `sergin-harness/.gitignore` — Excludes node_modules, .env, .claude/, dist/
- `sergin-harness/shared/package.json` — Empty shared package for future cross-package types
- `sergin-harness/agent/package.json` — sergin-agent with Ink, LangGraph, DeepSeek, tsx, Vitest
- `sergin-harness/agent/tsconfig.json` — Extends base, includes src/tests/bin
- `sergin-harness/agent/vitest.config.ts` — Vitest config (node env, 30s timeout)
- `sergin-harness/agent/.env.example` — Documents DEEPSEEK_API_KEY and TAVILY_API_KEY
- `sergin-harness/agent/bin/sergin.js` — Thin shebang wrapper delegating to tsx

### Task 2 — Walking Skeleton (10 files)
- `sergin-harness/agent/src/cli/index.ts` — CLI entrypoint: dotenv, env var validation, render(App)
- `sergin-harness/agent/src/graph/state.ts` — GraphState type alias for MessagesAnnotation.State
- `sergin-harness/agent/src/graph/graph.ts` — StateGraph: callLLM -> routeNext -> END, MemorySaver
- `sergin-harness/agent/src/graph/nodes/callLLM.ts` — ChatDeepSeek.invoke(state.messages)
- `sergin-harness/agent/src/graph/nodes/routeNext.ts` — Conditional edge: tool_calls -> executeTool, else END
- `sergin-harness/agent/src/ui/App.tsx` — Root Ink component: Static history, StreamingOutput, InputBar
- `sergin-harness/agent/src/ui/InputBar.tsx` — useInput hook with buffer management
- `sergin-harness/agent/src/ui/StreamingOutput.tsx` — Renders accumulated token string
- `sergin-harness/agent/src/ui/Spinner.tsx` — Animated spinner (10-frame braille sequence)
- `sergin-harness/agent/tests/graph/skeleton.test.ts` — 3 smoke tests (exports, thread_id, routeNext)

## Decisions Made

- **React 19 instead of 18:** Ink 7.0.2 requires react >=19.2.0 as a peer dependency. The plan specified react@^18.3.1 which would not resolve. Updated to react@^19.2.0 and @types/react@^19.2.0.
- **Token batching pattern:** Tokens accumulate in a useRef and flush to state on a 16ms interval (~60fps). This prevents the Ink render cycle from being overwhelmed by fast token arrival.
- **Eager env var validation:** cli/index.ts checks DEEPSEEK_API_KEY before calling render(), so the user gets a clear error message rather than a cryptic import/build failure.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Fixed react peer dependency version**
- **Found during:** Task 1 (npm install)
- **Issue:** Plan specified react@^18.3.1 but ink@7.0.2 requires react >=19.2.0. npm install failed with ERESOLVE conflict.
- **Fix:** Updated react to ^19.2.0 and @types/react to ^19.2.0 in agent/package.json
- **Files modified:** sergin-harness/agent/package.json
- **Verification:** npm install succeeds, packages resolve cleanly
- **Committed in:** 2104411 (Task 1 commit, included in initial scaffold)

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Required for npm install to succeed. No scope creep — react 19 is the ink 7.x peer requirement.

## TDD Gate Compliance

- RED gate: `73d580b test(01-01): add failing test for Walking Skeleton graph` -- present
- GREEN gate: `e52f98b feat(01-01): implement Walking Skeleton graph, UI, and CLI entrypoint` -- present
- REFACTOR gate: Not needed (code clean on first pass)

## Issues Encountered
- None — execution was clean after the react version fix.

## Threat Surface Scan

No new threat surfaces introduced beyond those documented in the plan's threat model:
- T-01-01: Mitigated — CLI validates DEEPSEEK_API_KEY at startup, exits with error message
- T-01-02: Mitigated — .env excluded via .gitignore, only .env.example committed
- T-01-04: Mitigated — API key read from process.env only, never logged

## User Setup Required

**Before running `node agent/bin/sergin.js`:**

1. Copy `.env.example` to `.env`:
   ```bash
   cp agent/.env.example agent/.env
   ```
2. Edit `agent/.env` and set your `DEEPSEEK_API_KEY`:
   ```
   DEEPSEEK_API_KEY=sk-your-actual-key-here
   ```
3. Run the CLI:
   ```bash
   DEEPSEEK_API_KEY=sk-xxx node agent/bin/sergin.js
   ```

## Known Stubs

None — all code is production-ready for the Walking Skeleton scope.

## Next Phase Readiness

- Graph topology is stable and ready for Plan 02 (Tool Slice): add executeTool, reflect, and tool registration
- routeNext's `"executeTool"` return path is the insertion point for Plan 02 edges
- callLLM.ts exports `bindLLMTools()` for when tools are registered in Plan 02
- Ink component structure (App, InputBar, StreamingOutput, Spinner) ready for ToolCallPanel in Plan 04

---
*Phase: 01-agent-foundation*
*Completed: 2026-05-09*
