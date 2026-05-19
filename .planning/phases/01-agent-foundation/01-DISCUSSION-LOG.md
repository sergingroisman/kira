# Phase 1: Agent Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-09
**Phase:** 1-Agent Foundation
**Areas discussed:** CLI rendering, LangGraph topology, Web search provider, Tool extensibility

---

## CLI Rendering

| Option | Description | Selected |
|--------|-------------|----------|
| ink (React-for-terminal) | Streaming output, colored components, spinner. Claude Code pattern; claude-mythos/ink/ reference available. | ✓ |
| readline + chalk | Node.js built-in REPL loop, chalk for color. Less code, no React model, sequential line output. | |
| You decide | Let Claude pick based on claude-mythos and LangGraph streaming patterns. | |

**User's choice:** ink

---

| Option | Description | Selected |
|--------|-------------|----------|
| Close parity with Claude Code | Streaming tokens, tool call panels, spinner. Use claude-mythos as primary component reference. | ✓ |
| Functional but minimal | Ink for REPL + streaming, but skip custom panels — plain colored text. | |
| Inspired but original | Use Ink but design UI independently. | |

**User's choice:** Close parity with Claude Code

---

| Option | Description | Selected |
|--------|-------------|----------|
| Stream tokens live | DeepSeek streaming API — print tokens as they arrive. | ✓ |
| Buffer full response | Wait for complete response before rendering. Simpler but sluggish. | |

**User's choice:** Stream tokens live

---

## LangGraph Topology

| Option | Description | Selected |
|--------|-------------|----------|
| createReactAgent helper | High-level prebuilt, ~20 lines. Fast to ship, less control. | |
| Hand-crafted StateGraph | Explicit nodes: callLLM → routeNext → executeTool → reflect → callLLM. Full control, Phase 2 slots in naturally. | ✓ |

**User's choice:** Hand-crafted StateGraph

---

| Option | Description | Selected |
|--------|-------------|----------|
| Retry with error context | Inject error into LLM state, re-enter callLLM. LLM decides to retry or explain. | ✓ |
| Fixed retry count, then explain | Retry N times (e.g., 3), then explain failure. | |
| Immediate LLM explanation | No retry — pass error to LLM immediately. | |

**User's choice:** Retry with error context

---

| Option | Description | Selected |
|--------|-------------|----------|
| LangGraph MemorySaver | Built-in in-memory checkpointer, thread-per-session. Zero setup. | ✓ |
| Custom in-memory store | Roll own message array on AgentState. More control, duplicates MemorySaver. | |
| SQLite checkpointer | Persist across restarts. Heavier than Phase 1 needs. | |

**User's choice:** LangGraph MemorySaver

---

## Web Search Provider

| Option | Description | Selected |
|--------|-------------|----------|
| Tavily | LangGraph-native, one import, free tier 1,000/mo, structured results. | ✓ |
| Brave Search API | Privacy-focused, 2,000/mo free, manual HTTP wrapper needed. | |
| SerpAPI | Google aggregator, LangChain tool exists, very limited free tier (100/mo). | |

**User's choice:** Tavily

---

## Tool Extensibility

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed tools for Phase 1, refactor in Phase 2 | Hardcode tools directly. Simpler now, refactor later. | |
| Plugin-loader from the start | ToolDefinition interface + ToolRegistry. Phase 2 custom tools drop in without refactor. | ✓ |

**User's choice:** Plugin-loader from the start

---

| Option | Description | Selected |
|--------|-------------|----------|
| Unrestricted Bash | No timeout, no blocklist. Single-user system, owner controls machine. | ✓ |
| Timeout only | Kill commands after N seconds. | |
| Timeout + blocklist | Kill + block destructive commands. | |

**User's choice:** Unrestricted

---

| Option | Description | Selected |
|--------|-------------|----------|
| agent/ only for Phase 1 | Keep tools co-located with agent. Extract to shared/ when real reuse need arises. | ✓ |
| shared/ from the start | Put ToolDefinition in shared/ for future cross-package reuse. | |

**User's choice:** agent/ only for Phase 1

---

## Claude's Discretion

None — all gray areas had explicit user decisions.

## Deferred Ideas

- Cross-session persistence (SQLite checkpointer) → Phase 2
- Bash tool timeout/blocklist → add only if operational problems arise
- Tool extraction to shared/ → defer until Phase 5/7 creates real cross-package reuse need
