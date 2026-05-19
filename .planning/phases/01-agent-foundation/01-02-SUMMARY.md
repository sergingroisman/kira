---
phase: 01-agent-foundation
plan: 02
subsystem: agent
tags: langgraph, tools, bash, filesystem, typescript, ink, vitest

# Dependency graph
requires:
  - phase: 01-plan-01
    provides: Walking Skeleton graph (callLLM -> routeNext), Ink REPL, CLI entrypoint
provides:
  - ToolRegistry with register()/getLCTools() for plugin-loader tool management
  - Bash tool (unrestricted, child_process.exec with /bin/zsh)
  - FileRead tool (fs/promises.readFile)
  - FileWrite tool (fs/promises.writeFile with parent dir creation)
  - FileAppend tool (fs/promises.appendFile with parent dir creation)
  - executeTool node (ToolNode wrapper from @langchain/langgraph/prebuilt)
  - reflect node (enriches error ToolMessages with retry instructions)
  - Updated graph topology: callLLM -> routeNext -> (executeTool -> reflect -> callLLM) | END
  - ToolCallPanel Ink component for live tool call display
affects:
  - Plan 03: Search + Memory (uses registry for Tavily tool, builds on reflect loop)
  - Plan 04: UI Polish (builds on ToolCallPanel layout)
  - Phase 2: Plugin system (ToolRegistry.loadFromDir extension point)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - ToolDefinition interface + ToolRegistry singleton (plugin-loader pattern)
    - ToolNode from @langchain/langgraph/prebuilt (never hand-roll tool execution)
    - Error return (not throw) for tool failures — reflect node enriches ToolMessages
    - ToolCallPanel Ink component with border colors for status (yellow=running, red=error, green=done)

key-files:
  created:
    - agent/src/tools/registry.ts
    - agent/src/tools/bash.ts
    - agent/src/tools/fileRead.ts
    - agent/src/tools/fileWrite.ts
    - agent/src/graph/nodes/executeTool.ts
    - agent/src/graph/nodes/reflect.ts
    - agent/src/ui/ToolCallPanel.tsx
    - agent/tests/tools/bash.test.ts
    - agent/tests/tools/file.test.ts
    - agent/tests/graph/reflect.test.ts
  modified:
    - agent/src/graph/graph.ts
    - agent/src/ui/App.tsx

key-decisions:
  - "Error return pattern for tool failures: all three tools return error strings (not throw) so ToolNode wraps them as ToolMessages for the reflect node"
  - "ToolNode from @langchain/langgraph/prebuilt handles parallel calls, error wrapping, and ToolMessage ID assignment — no custom tool runner"

patterns-established:
  - "Pattern 5: ToolDefinition interface + registry.register() for all tool registration"
  - "Pattern 6: Tool errors returned as strings, not thrown — reflect node enriches ToolMessages with retry/explain instructions"
  - "Pattern 7: ToolCallPanel with status-driven border colors (yellow/running, red/error, green/done)"

requirements-completed: [AGENT-02, AGENT-03, AGENT-05]

# Metrics
duration: 8min
completed: 2026-05-09
---

# Phase 01 Agent Foundation — Tool Slice

**ToolRegistry with bash, file read/write/append tools, ToolNode-based executeTool, reflect node error enrichment, and full graph wiring with ToolCallPanel UI**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-09T22:52:00-03:00
- **Completed:** 2026-05-09T22:58:00-03:00
- **Tasks:** 2
- **Commits:** 4 (2 per task due to TDD split)
- **Files created:** 8
- **Files modified:** 2

## Accomplishments
- ToolRegistry singleton with ToolDefinition plugin contract — all tool registration goes through `registry.register()`
- Bash tool with unrestricted `/bin/zsh` shell per D-09, 10MB maxBuffer, error-string returns on failure
- FileRead/FileWrite/FileAppend tools using `fs/promises` with parent directory creation on write
- executeTool node wrapping `ToolNode` from `@langchain/langgraph/prebuilt` (not a custom runner)
- reflect node enriches error ToolMessages with retry/explain instructions per D-05
- Updated graph topology: callLLM → routeNext → (executeTool → reflect → callLLM) | END
- ToolCallPanel Ink component with status-driven border colors showing tool name, args, and result
- App.tsx enhanced with toolCalls state tracking via tool_call chunk detection in streaming loop
- TDD cycles completed: RED test commits (failing) -> GREEN implementation commits (passing)
- Four tool registrations: bashTool, fileReadTool, fileWriteTool, fileAppendTool
- bindLLMTools called with `tool_choice: "auto"` before graph.compile()

## Task Commits

Each task was committed atomically following TDD (RED → GREEN):

1. **Task 1: ToolRegistry + Bash + File tools (TDD)**
   - `12961fe` test(01-02): add failing tests for ToolRegistry, Bash, File tools (RED)
   - `199ad80` feat(01-02): implement ToolRegistry, Bash, File tools (GREEN)

2. **Task 2: executeTool + reflect + graph wiring + ToolCallPanel (TDD)**
   - `cdb8252` test(01-02): add failing tests for reflect node (RED)
   - `1807824` feat(01-02): implement executeTool, reflect nodes, wire full graph, add ToolCallPanel UI (GREEN)

## Files Created/Modified

### Task 1 — ToolRegistry + Tools (6 files)
- `agent/src/tools/registry.ts` — ToolDefinition interface, ToolRegistry class, registry singleton
- `agent/src/tools/bash.ts` — Bash tool via child_process.exec with /bin/zsh, 10MB buffer
- `agent/src/tools/fileRead.ts` — File read via fs/promises.readFile with ENOENT handling
- `agent/src/tools/fileWrite.ts` — File write/append via fs/promises with parent dir creation
- `agent/tests/tools/bash.test.ts` — 5 tests for bashTool behavior
- `agent/tests/tools/file.test.ts` — 6 tests for file read/write/append

### Task 2 — Graph nodes + UI (6 files)
- `agent/src/graph/nodes/executeTool.ts` — makeToolNode wrapping ToolNode
- `agent/src/graph/nodes/reflect.ts` — Error enrichment for ToolMessages
- `agent/src/graph/graph.ts` — Full topology: tool registration, bindLLMTools, all 3 nodes
- `agent/src/ui/ToolCallPanel.tsx` — Ink component for tool call display
- `agent/src/ui/App.tsx` — Enhanced with toolCalls state tracking
- `agent/tests/graph/reflect.test.ts` — 3 tests for reflect node behavior

## Decisions Made

- **Error return pattern for tool failures:** All three tools return error strings (not throw) on failure. This ensures ToolNode wraps them as ToolMessages automatically, which the reflect node then enriches with retry/explain instructions for the LLM. This is the standard LangGraph pattern.
- **ToolNode from prebuilt:** Using `@langchain/langgraph/prebuilt`'s `ToolNode` instead of a custom runner. It handles parallel tool calls, error wrapping, and ToolMessage ID assignment.
- **bindLLMTools before compile:** Tool binding happens at module load time (top-level in graph.ts), not lazily. This ensures `tool_choice: "auto"` is set before any LLM invocation.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None — execution was clean. No dependency issues or build errors.

## TDD Gate Compliance

- Task 1: RED gate `12961fe` (test commit) — present; GREEN gate `199ad80` (feat commit) — present; REFACTOR gate: not needed
- Task 2: RED gate `cdb8252` (test commit) — present; GREEN gate `1807824` (feat commit) — present; REFACTOR gate: not needed

## Threat Surface Scan

No new threat surfaces introduced beyond those documented in the plan's threat model:
- T-02-01: Accepted — unrestricted bash per D-09
- T-02-02: Accepted — single-user filesystem writes
- T-02-03: Accepted — single-user filesystem reads
- T-02-04: Mitigated — Zod schema validates tool inputs at LangGraph boundary
- T-02-05: Accepted — 10MB maxBuffer cap on bash output

## Known Stubs

None — all tool implementations are production-ready for the Phase 1 scope.

## Next Phase Readiness

- Graph topology is stable with the full tool loop: callLLM -> routeNext -> executeTool -> reflect -> callLLM
- ToolRegistry is populated and ready for additional tool registrations (Tavily search in Plan 03)
- reflect node is wired and ready to handle errors from any tool
- ToolCallPanel is integrated into App.tsx and will render tool call status during streaming

## Self-Check: PASSED

All 15 files verified as created. All 4 commit hashes verified in git history.

## Final Verification

```bash
# Full test suite: 4 test files, 17 tests, all passing
npm test --workspace agent -- --run 2>&1 | tail -5
# Output:
# Test Files  4 passed (4)
#      Tests  17 passed (17)

# Graph topology
grep -E "addEdge.*executeTool|addEdge.*reflect" agent/src/graph/graph.ts
# Output:
# .addEdge("executeTool", "reflect")
# .addEdge("reflect", "callLLM")

# Tool registrations
grep "registry.register" agent/src/graph/graph.ts
# Output:
# registry.register(bashTool);
# registry.register(fileReadTool);
# registry.register(fileWriteTool);
# registry.register(fileAppendTool);

# bindLLMTools + tool_choice
grep "bindLLMTools\|tool_choice" agent/src/graph/graph.ts agent/src/graph/nodes/callLLM.ts
# Output:
# bindLLMTools(registry.getLCTools());
# tool_choice: "auto"
```

---
*Phase: 01-agent-foundation*
*Completed: 2026-05-09*
