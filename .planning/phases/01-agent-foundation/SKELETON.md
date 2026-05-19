# Walking Skeleton — Phase 1: Agent Foundation

> The thinnest possible end-to-end slice that proves the full stack works.
> After Plan 01 completes, this skeleton is alive. All later plans build on it.

---

## Proof Statement

A user runs `sergin` in their terminal, types a prompt, and watches DeepSeek tokens stream character-by-character in the Ink-rendered REPL. The process stays alive for the next prompt.

This proves: TypeScript compiles → Ink boots → LangGraph StateGraph executes → DeepSeek streams → tokens reach the terminal.

---

## Architectural Decisions (Locked for All Subsequent Phases)

### Runtime

| Decision | Value | Rationale |
|----------|-------|-----------|
| Language | TypeScript (ESM) | Type safety; LangGraph is TS-first |
| Node version | v24.14.0 (current on this machine) | Already installed; no overhead |
| Module system | `"type": "module"` in all packages | LangGraph 1.3.0 is ESM-first; avoids CJS interop bugs |
| Dev runner | `tsx` 4.21.0 | ESM-native, zero-config, replaces ts-node |
| Package manager | npm workspaces | Already installed; consistent with rest of toolchain |

### Framework

| Layer | Library | Version | Why |
|-------|---------|---------|-----|
| Agent orchestration | `@langchain/langgraph` | 1.3.0 | Provides StateGraph, MemorySaver, ToolNode, MessagesAnnotation |
| LangChain core | `@langchain/core` | 1.1.45 | Message types, tool() factory; required langgraph peer |
| LLM client | `@langchain/deepseek` | 1.0.25 | Official DeepSeek integration; OpenAI-compatible streaming |
| Terminal UI | `ink` | 7.0.2 | React-for-terminal; used by Claude Code itself |
| React | (ink peer) | react@18 | Required by ink |
| Tool schema | `zod` | 4.4.3 | Required peer of langgraph 1.3.0 |
| Schema bridge | `zod-to-json-schema` | 3.x | Required peer; LangChain uses internally |

### Search Tool

| Decision | Value |
|----------|-------|
| Provider | Tavily |
| Package | `@langchain/tavily` 1.2.0 (NOT `@langchain/community`) |
| Auth | `TAVILY_API_KEY` env var (read automatically) |

### LLM

| Decision | Value |
|----------|-------|
| Provider | DeepSeek |
| Model | `deepseek-chat` (tool-calling capable; NOT deepseek-reasoner) |
| Auth | `DEEPSEEK_API_KEY` env var |
| Streaming | `graph.stream()` with `streamMode: "messages"` — LangGraph intercepts at framework level |

### Graph Topology (hand-crafted per D-04)

```
START → callLLM → routeNext ─── (no tool_calls) ──→ END
                      │
                   (tool_calls)
                      ↓
                 executeTool (ToolNode)
                      ↓
                   reflect
                      ↓
                  callLLM  (loop)
```

- `callLLM`: `llm.invoke(state.messages)` — LangGraph streams tokens from this call
- `routeNext`: conditional edge — reads last AIMessage.tool_calls
- `executeTool`: `ToolNode` from `@langchain/langgraph/prebuilt` — handles parallel calls + error wrapping
- `reflect`: injects ToolMessage error context back into state; LLM decides retry vs explain (D-05)
- Checkpointer: `MemorySaver` with one `thread_id` per session (D-06)

### Tool Architecture (per D-08)

```typescript
interface ToolDefinition {
  name: string;
  lcTool: ReturnType<typeof tool>; // LangChain tool instance
}

class ToolRegistry {
  register(def: ToolDefinition): void
  getLCTools(): LangChainTool[]
  // Phase 2 adds: loadFromDir(dir: string): Promise<void>
}
```

Tools in Phase 1: `Bash` (D-09: unrestricted), `FileRead`, `FileWrite`, `WebSearch` (Tavily)

### Directory Layout

```
sergin-harness/                     ← Monorepo root (git repo)
├── package.json                    ← npm workspace: ["agent", "shared"]
├── tsconfig.base.json              ← Shared TS config (target: ES2022, module: NodeNext)
├── agent/
│   ├── package.json                ← name: "sergin-agent", type: "module"
│   ├── tsconfig.json               ← extends ../tsconfig.base.json
│   ├── vitest.config.ts            ← Test framework (Vitest)
│   ├── .env.example                ← DEEPSEEK_API_KEY, TAVILY_API_KEY
│   ├── bin/
│   │   └── sergin.js               ← Thin shebang → tsx src/cli/index.ts
│   └── src/
│       ├── cli/
│       │   └── index.ts            ← Entry: render(<App />), waitUntilExit
│       ├── graph/
│       │   ├── state.ts            ← MessagesAnnotation + GraphState type
│       │   ├── graph.ts            ← StateGraph wiring + compile()
│       │   └── nodes/
│       │       ├── callLLM.ts      ← llm.invoke(messages) → AIMessage
│       │       ├── executeTool.ts  ← ToolNode wrapper (prebuilt)
│       │       ├── reflect.ts      ← Error injection → ToolMessage
│       │       └── routeNext.ts    ← Conditional edge function
│       ├── tools/
│       │   ├── registry.ts         ← ToolDefinition interface + ToolRegistry class
│       │   ├── bash.ts             ← child_process.exec, unrestricted (D-09)
│       │   ├── fileRead.ts         ← fs/promises.readFile
│       │   ├── fileWrite.ts        ← fs/promises.writeFile + appendFile
│       │   └── search.ts           ← TavilySearch wrapped as ToolDefinition
│       └── ui/
│           ├── App.tsx             ← Root Ink component: layout, input state, stream state
│           ├── InputBar.tsx        ← useInput hook → onSubmit
│           ├── StreamingOutput.tsx ← Batched token render via useRef + interval
│           ├── ToolCallPanel.tsx   ← Tool name + args/result display
│           └── Spinner.tsx         ← Animated indicator during graph execution
└── shared/                         ← Empty for Phase 1; Phase 5+ adds shared types
    └── package.json                ← name: "sergin-shared", type: "module"
```

### Deployment / Auth

- No deployment in Phase 1 — runs locally on MacBook
- No auth system — single-user CLI
- API keys via `.env` loaded with `dotenv` (or `tsx --env-file .env`)

---

## Invariants (Must Hold in All Future Phases)

1. **No buffered LLM responses** — always `graph.stream()` with `streamMode: "messages"`, never `graph.invoke()` in the render path (D-03)
2. **No `createReactAgent`** — hand-crafted StateGraph with explicit reflect node (D-04)
3. **Messages live in LangGraph state** (MemorySaver), never in React component state
4. **`ToolRegistry.register()` is the only way to add tools** — no direct `bindTools()` calls outside graph.ts
5. **`deepseek-chat` model** — reasoner model does not support tool calls
6. **`@langchain/tavily`** — never `@langchain/community/tools/tavily_search`
7. **`"type": "module"`** in all package.json — LangGraph is ESM-first

---

## What Phase 2 Builds On

- `ToolRegistry.loadFromDir()` — loads custom tools from a directory (AGENT-09)
- `MemorySaver` → SQLite checkpointer for cross-session persistence (AGENT-08)
- Sub-agent spawning via nested graph invocations (AGENT-06)

The registry interface, graph topology, and Ink component structure are stable contracts after Phase 1.

---

*Walking Skeleton created: 2026-05-09*
*Phase: 01-agent-foundation*
*Valid until: Phase 2 architectural changes*
