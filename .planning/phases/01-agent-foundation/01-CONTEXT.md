# Phase 1: Agent Foundation - Context

**Gathered:** 2026-05-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a CLI REPL agent using LangGraph + DeepSeek that can invoke tools (Bash, file read/write/edit, web search), reflect on tool errors, and maintain conversation history within a session. Output is a runnable `sergin` CLI binary in the `agent/` package.

**In scope:** CLI REPL loop, Ink-based UI, LangGraph StateGraph, tool registry with bash/file/search tools, error reflection node, MemorySaver short-term memory.
**Out of scope:** Sub-agent spawning (Phase 2), long-term summarized memory (Phase 2), MCP integration (Phase 3), custom tool loading from disk (Phase 2 extends the registry).

</domain>

<decisions>
## Implementation Decisions

### CLI Rendering
- **D-01:** Use **ink** (React-for-terminal) for the REPL UI — not readline/stdout.
- **D-02:** Target **close parity with Claude Code** UX: streaming token output, tool call panels, spinner during execution. Use `claude-mythos/src/ink/` and `claude-mythos/src/cli/` as the primary structural reference.
- **D-03:** **Stream tokens live** as they arrive from the DeepSeek API — do not buffer the full response before rendering.

### LangGraph Graph Topology
- **D-04:** Build a **hand-crafted StateGraph** — do NOT use `createReactAgent`. Explicit nodes: `callLLM` → `routeNext` (conditional) → `executeTool` → `reflect` → back to `callLLM`.
- **D-05:** The **reflection node** injects the error message back into LLM state as a tool/system message and re-enters `callLLM`. The LLM decides whether to retry the tool or explain the failure to the user. No fixed retry counter for Phase 1.
- **D-06:** **Short-term memory** uses LangGraph's built-in `MemorySaver` checkpointer (in-memory). One thread ID per REPL session. Persists message history for the lifetime of the process only — cross-session persistence is Phase 2 scope.

### Web Search Tool
- **D-07:** Use **Tavily** as the web search provider. It is LangGraph/LangChain-native (one import), has a free tier (1,000 searches/month), and returns structured results. `TAVILY_API_KEY` env var required.

### Tool Architecture
- **D-08:** Implement a **plugin-loader pattern from the start**: a `ToolDefinition` interface + `ToolRegistry` class. Phase 1 tools (bash, file, search) register via the interface. Phase 2 custom tools (AGENT-09) load from a directory without refactoring the registry.
- **D-09:** The Bash tool runs **unrestricted** — no timeout, no command blocklist. Single-user system, owner controls the machine.
- **D-10:** Tool definitions live **inside `agent/`** for Phase 1. Extraction to `shared/` deferred until a concrete cross-package reuse need arises.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture Reference
- `/Users/avanade/Workspace/claude-mythos/src/ink/` — Ink component structure reference for CLI UI
- `/Users/avanade/Workspace/claude-mythos/src/cli/` — CLI entrypoint and REPL loop patterns
- `/Users/avanade/Workspace/claude-mythos/src/tools/` — Tool definition patterns to study
- `/Users/avanade/Workspace/claude-mythos/src/coordinator/` — Orchestration/routing patterns
- `/Users/avanade/Workspace/claude-mythos/src/state/` — State management patterns

### Existing Ecosystem
- `/Users/avanade/Workspace/sergin-brain/src/llm/` — DeepSeek API client patterns (TypeScript, existing)
- `/Users/avanade/Workspace/sergin-brain/src/memory/` — Memory/session patterns (reference only)

### Planning
- `.planning/REQUIREMENTS.md` §AGENT-01 through AGENT-07 — Phase 1 requirements
- `.planning/ROADMAP.md` §Phase 1 — Success criteria

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `claude-mythos/src/ink/` — Ink component library: study for streaming output, spinner, tool call panels. Do not copy wholesale — extract the structural patterns.
- `sergin-brain/src/llm/` — DeepSeek API integration in TypeScript. Reference for model client setup and streaming call pattern.

### Established Patterns
- claude-mythos uses Ink + React component tree for terminal UI — adopt this as the rendering foundation.
- sergin-brain is NestJS/TypeScript — the `agent/` package should be plain TypeScript (no NestJS), matching the CLI-first goal.

### Integration Points
- Phase 2 will extend `ToolRegistry` with directory-loading — the registry interface must be exported cleanly from `agent/`.
- Phase 3 (MCP) will register MCP-sourced tools via the same `ToolRegistry` — keep `ToolDefinition` interface generic enough.

</code_context>

<specifics>
## Specific Ideas

- CLI feel should match Claude Code: the user specifically chose "close parity" when shown the comparison.
- Token streaming is non-negotiable — buffered responses feel sluggish and were explicitly rejected.
- The `ToolRegistry` preview snippet shows the desired API shape:
  ```ts
  registry.register(bashTool);
  // Phase 2: registry.loadFromDir('./skills/')
  ```

</specifics>

<deferred>
## Deferred Ideas

- Cross-session memory persistence (SQLite checkpointer) → Phase 2 scope
- Bash tool timeout/blocklist → add only if problems arise in practice
- Tool extraction to `shared/` → defer until Phase 5/7 cross-package reuse is real

</deferred>

---

*Phase: 1-Agent Foundation*
*Context gathered: 2026-05-09*
