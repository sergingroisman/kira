# Phase 1: Agent Foundation - Research

**Researched:** 2026-05-09
**Domain:** LangGraph JS/TS + Ink (React CLI) + DeepSeek API + Tavily Search
**Confidence:** HIGH (core APIs verified against npm registry, official docs, and reference codebase)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Use **ink** (React-for-terminal) for the REPL UI — not readline/stdout.
- **D-02:** Target **close parity with Claude Code** UX: streaming token output, tool call panels, spinner during execution. Use `claude-mythos/src/ink/` and `claude-mythos/src/cli/` as the primary structural reference.
- **D-03:** **Stream tokens live** as they arrive from the DeepSeek API — do not buffer the full response before rendering.
- **D-04:** Build a **hand-crafted StateGraph** — do NOT use `createReactAgent`. Explicit nodes: `callLLM` → `routeNext` (conditional) → `executeTool` → `reflect` → back to `callLLM`.
- **D-05:** The **reflection node** injects the error message back into LLM state as a tool/system message and re-enters `callLLM`. The LLM decides whether to retry the tool or explain the failure to the user. No fixed retry counter for Phase 1.
- **D-06:** **Short-term memory** uses LangGraph's built-in `MemorySaver` checkpointer (in-memory). One thread ID per REPL session. Persists message history for the lifetime of the process only — cross-session persistence is Phase 2 scope.
- **D-07:** Use **Tavily** as the web search provider. It is LangGraph/LangChain-native (one import), has a free tier (1,000 searches/month), and returns structured results. `TAVILY_API_KEY` env var required.
- **D-08:** Implement a **plugin-loader pattern from the start**: a `ToolDefinition` interface + `ToolRegistry` class. Phase 1 tools (bash, file, search) register via the interface. Phase 2 custom tools (AGENT-09) load from a directory without refactoring the registry.
- **D-09:** The Bash tool runs **unrestricted** — no timeout, no command blocklist. Single-user system, owner controls the machine.
- **D-10:** Tool definitions live **inside `agent/`** for Phase 1. Extraction to `shared/` deferred until a concrete cross-package reuse need arises.

### Claude's Discretion

- File structure within `agent/` package
- Exact Ink component hierarchy
- TypeScript configuration
- Build tooling (tsx vs esbuild vs tsc)
- System prompt content and wording
- Error display format in UI

### Deferred Ideas (OUT OF SCOPE)

- Cross-session memory persistence (SQLite checkpointer) → Phase 2 scope
- Bash tool timeout/blocklist → add only if problems arise in practice
- Tool extraction to `shared/` → defer until Phase 5/7 cross-package reuse is real
- Sub-agent spawning → Phase 2
- Long-term summarized memory → Phase 2
- MCP integration → Phase 3
- Custom tool loading from disk → Phase 2 extends the registry
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AGENT-01 | Agent runs as a CLI REPL accepting user prompts and tool calls | Ink render() + useInput hook + StateGraph invoke loop |
| AGENT-02 | Agent can invoke Bash commands as a tool and return output to conversation | LangGraph tool() + ToolNode + child_process.exec |
| AGENT-03 | Agent can read, write, and edit files on the local filesystem | LangGraph tool() + fs/promises API (readFile, writeFile, appendFile) |
| AGENT-04 | Agent can perform web searches via an integrated search tool | @langchain/tavily TavilySearch tool bound to LLM via bindTools |
| AGENT-05 | Agent reflects on tool outputs and retries or corrects on error | reflect node injects ToolMessage(error) back into messages state; conditional edge routes to callLLM |
| AGENT-07 | Agent maintains short-term memory via conversation history | MemorySaver checkpointer + configurable.thread_id per session |
</phase_requirements>

---

## Summary

Phase 1 builds the core agentic loop: a terminal REPL that accepts user prompts, routes through a hand-crafted LangGraph `StateGraph`, calls tools (Bash, file I/O, web search), reflects on errors, and streams tokens live as they arrive from DeepSeek. The output is a `sergin` CLI binary in the `agent/` monorepo package.

The technical stack is well-established and production-ready. LangGraph JS 1.3.0 is the current stable version with a documented `MessagesAnnotation` + `ToolNode` pattern for hand-crafted graphs. Ink 7.0.2 is the terminal rendering standard (used by Claude Code itself). `@langchain/deepseek` 1.0.25 wraps the OpenAI-compatible DeepSeek API and supports streaming. `@langchain/tavily` 1.2.0 replaces the deprecated community Tavily tool and is the current recommended package.

**Primary recommendation:** Build the Walking Skeleton first — a Node process that boots Ink, renders a text input, sends one prompt through a minimal `StateGraph` (callLLM → END), and streams tokens to the terminal. Add tools, reflection, and memory in subsequent vertical slices.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI REPL input/output | CLI Process (Ink) | — | Terminal process owns stdin/stdout; no server layer needed |
| LLM invocation + streaming | LangGraph node (callLLM) | @langchain/deepseek | Graph node owns orchestration; deepseek client handles transport |
| Tool execution (Bash, file, search) | LangGraph node (executeTool) | OS / Tavily API | Tool runner decoupled from UI; results returned as ToolMessages |
| Error reflection | LangGraph node (reflect) | callLLM node | Reflect injects error context; LLM decides retry vs explain |
| Conversation memory | LangGraph MemorySaver | — | Checkpointer persists messages across graph invocations within session |
| Graph routing | Conditional edge (routeNext) | — | Edge function owns all routing logic; keeps nodes pure |
| Tool registry | ToolRegistry class (in-process) | — | Singleton loaded at startup; Phase 2 adds directory loader |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `@langchain/langgraph` | 1.3.0 | StateGraph, MemorySaver, ToolNode, MessagesAnnotation | Current stable; exports verified against npm registry |
| `@langchain/core` | 1.1.45 | BaseMessage, HumanMessage, AIMessage, ToolMessage, tool() | Required peer of langgraph; message types live here |
| `@langchain/deepseek` | 1.0.25 | ChatDeepSeek — OpenAI-compatible LLM client with streaming | Official LangChain deepseek integration; used in sergin-brain |
| `ink` | 7.0.2 | React-for-terminal: Box, Text, Static, useInput, render() | Used by Claude Code; claude-mythos reference codebase |
| `react` | (ink peer) | Ink component rendering | Required by ink |
| `@langchain/tavily` | 1.2.0 | TavilySearch tool — LLM-optimized web search | Replaces deprecated @langchain/community tavily tool |
| `zod` | 4.4.3 | Tool schema validation (required peer of langgraph 1.3.0) | Required by @langchain/langgraph peerDependencies |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tsx` | 4.21.0 | Run TypeScript directly without compile step | Dev entrypoint; shebang in CLI binary for local use |
| `typescript` | latest | Type safety across the monorepo | Always; project is TypeScript-first |
| `zod-to-json-schema` | 3.x | Converts Zod schemas to JSON Schema for tool definitions | Required peer of @langchain/langgraph 1.3.0 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `@langchain/tavily` | `@langchain/community/tools/tavily_search` | Community version is deprecated; `@langchain/tavily` is the official successor |
| `tsx` dev entry | `ts-node` | tsx is faster, ESM-native, and has no tsconfig setup requirement |
| `ink` | `blessed` / `terminal-kit` | Ink is React-based, matches claude-mythos architecture, easier to test |
| `MessagesAnnotation` (built-in) | Custom annotation | MessagesAnnotation provides the messages reducer out of the box; custom adds no value for this topology |

**Installation:**
```bash
npm install @langchain/langgraph @langchain/core @langchain/deepseek @langchain/tavily ink react zod zod-to-json-schema
npm install --save-dev tsx typescript @types/react @types/node
```

**Version verification:** All versions confirmed against npm registry on 2026-05-09. `@langchain/langgraph` 1.3.0 published 2026-05-05. [VERIFIED: npm registry]

---

## Architecture Patterns

### System Architecture Diagram

```
User (stdin)
     │
     ▼
┌─────────────────────────────────┐
│         Ink REPL Component      │  ← React component tree; renders
│  TextInput → onSubmit           │    StreamingOutput, ToolCallPanel,
│  StreamingOutput (useState)     │    Spinner
│  ToolCallPanel                  │
└──────────────┬──────────────────┘
               │ user message
               ▼
┌─────────────────────────────────┐
│      LangGraph StateGraph       │
│                                 │
│  ┌──────────┐                   │
│  │ callLLM  │◄──────────────────┼── MemorySaver (thread_id)
│  └────┬─────┘                   │
│       │ AIMessage               │
│       ▼                         │
│  ┌──────────────┐               │
│  │  routeNext   │ (conditional) │
│  └──┬───────────┘               │
│     │ has tool_calls?           │
│     ├─ YES ──►┌──────────────┐  │
│     │         │ executeTool  │  │
│     │         └──────┬───────┘  │
│     │                │ result   │
│     │                ▼          │
│     │         ┌──────────────┐  │
│     │         │   reflect    │  │
│     │         │ (on error)   │  │
│     │         └──────┬───────┘  │
│     │                │ back to callLLM
│     │                ▼          │
│     └─ NO ───► END              │
└─────────────────────────────────┘
               │ stream chunks
               ▼
     Ink StreamingOutput
     (setState per token)
```

### Recommended Project Structure

```
sergin-harness/
├── package.json                  ← Root workspace (workspaces: ["agent", "shared"])
├── agent/
│   ├── package.json              ← { "name": "sergin-agent", "bin": { "sergin": "./bin/sergin.js" } }
│   ├── tsconfig.json
│   ├── bin/
│   │   └── sergin.js             ← Thin JS shebang wrapper → imports src/cli/index.ts via tsx
│   └── src/
│       ├── cli/
│       │   └── index.ts          ← Entry: parse args, render(<App />)
│       ├── graph/
│       │   ├── state.ts          ← MessagesAnnotation extension + GraphState type
│       │   ├── graph.ts          ← StateGraph construction + compile()
│       │   └── nodes/
│       │       ├── callLLM.ts    ← Invokes ChatDeepSeek; returns AIMessage
│       │       ├── executeTool.ts← Runs ToolRegistry.execute(); returns ToolMessage[]
│       │       ├── reflect.ts    ← Injects error ToolMessage; sets reflect flag
│       │       └── routeNext.ts  ← Conditional edge: "tools" | "reflect" | "__end__"
│       ├── tools/
│       │   ├── registry.ts       ← ToolRegistry class + ToolDefinition interface
│       │   ├── bash.ts           ← Bash tool (child_process.exec, unrestricted)
│       │   ├── fileRead.ts       ← File read tool (fs/promises.readFile)
│       │   ├── fileWrite.ts      ← File write tool (fs/promises.writeFile)
│       │   └── search.ts         ← TavilySearch wrapper as ToolDefinition
│       └── ui/
│           ├── App.tsx           ← Root Ink component: layout + state
│           ├── TextInput.tsx     ← useInput hook → onSubmit callback
│           ├── StreamingOutput.tsx← useState(tokens) + progressive render
│           ├── ToolCallPanel.tsx ← Shows tool name + truncated args/result
│           └── Spinner.tsx       ← useEffect animation (frames array)
└── shared/                       ← Empty for Phase 1; populated Phase 5+
```

### Pattern 1: StateGraph Node Definition

**What:** Each graph node is an async function that receives the full state and returns a partial state update.
**When to use:** Every processing step in the agent loop.

```typescript
// Source: https://deepwiki.com/langchain-ai/langgraphjs/2.1-stategraph-and-graph-building
import { MessagesAnnotation } from "@langchain/langgraph";
import { AIMessage } from "@langchain/core/messages";
import { ChatDeepSeek } from "@langchain/deepseek";

const llm = new ChatDeepSeek({
  model: "deepseek-chat",   // tool-calling capable model
  temperature: 0,
}).bindTools(registeredTools); // bindTools called once at startup

async function callLLM(state: typeof MessagesAnnotation.State) {
  const response = await llm.invoke(state.messages);
  return { messages: [response] }; // MessagesAnnotation reducer appends
}
```

### Pattern 2: Conditional Edge (routeNext)

**What:** A function that reads the last message and returns the name of the next node.
**When to use:** After callLLM to route to tools, reflect, or end.

```typescript
// Source: https://medium.com/@barsegyan96armen/langgraph-101-understanding-the-core-concepts-of-state-nodes-and-edges-in-javascript-f91068683d7d
import { END } from "@langchain/langgraph";
import { AIMessage } from "@langchain/core/messages";

function routeNext(state: typeof MessagesAnnotation.State): string {
  const last = state.messages[state.messages.length - 1] as AIMessage;
  if (last?.tool_calls?.length) return "executeTool";
  return END;
}
```

### Pattern 3: Reflect Node — Error Injection

**What:** On tool error, the reflect node appends a ToolMessage with the error text so the LLM sees it on the next callLLM iteration.
**When to use:** When executeTool catches an exception.

```typescript
// Source: [ASSUMED] — pattern derived from LangGraph state update semantics
import { ToolMessage } from "@langchain/core/messages";

async function reflect(state: typeof MessagesAnnotation.State) {
  // executeTool stored the error in a custom field; we surface it as a ToolMessage
  const errorDetails = (state as any)._lastToolError;
  const errorMsg = new ToolMessage({
    tool_call_id: errorDetails.toolCallId,
    content: `Tool error: ${errorDetails.message}. Please retry with corrected parameters or explain why the task cannot be completed.`,
  });
  return { messages: [errorMsg] };
}
```

### Pattern 4: Graph Compilation with MemorySaver

**What:** The complete graph wiring with in-memory checkpointer.
**When to use:** Once at startup.

```typescript
// Source: https://deepwiki.com/langchain-ai/langgraphjs/2.1-stategraph-and-graph-building
import { StateGraph, MessagesAnnotation, MemorySaver, END, START } from "@langchain/langgraph";
import { ToolNode } from "@langchain/langgraph/prebuilt";

const memory = new MemorySaver();

const graph = new StateGraph(MessagesAnnotation)
  .addNode("callLLM", callLLM)
  .addNode("executeTool", new ToolNode(tools))
  .addNode("reflect", reflect)
  .addEdge(START, "callLLM")
  .addConditionalEdges("callLLM", routeNext)
  .addEdge("executeTool", "reflect")
  .addEdge("reflect", "callLLM")
  .compile({ checkpointer: memory });

// Per-session invocation
const config = { configurable: { thread_id: `session-${Date.now()}` } };
```

### Pattern 5: LangGraph Token Streaming

**What:** Streaming mode `"messages"` yields `[messageChunk, metadata]` tuples — one per token.
**When to use:** In the REPL loop to feed tokens to Ink's setState.

```typescript
// Source: https://docs.langchain.com/oss/javascript/langgraph/streaming
for await (const [chunk, metadata] of await graph.stream(
  { messages: [new HumanMessage(userInput)] },
  { ...config, streamMode: "messages" }
)) {
  if (chunk.content && typeof chunk.content === "string") {
    onToken(chunk.content); // → setState in Ink component
  }
}
```

### Pattern 6: Custom Tool Definition (ToolRegistry)

**What:** A `ToolDefinition` wraps a LangChain `tool()` instance. `ToolRegistry` holds all tools and exposes the array for `bindTools`.
**When to use:** At agent startup; tool instances are stable references.

```typescript
// Source: https://www.js-craft.io/blog/tool-calling-langchain-js-toolmessage-schemas/
import { tool } from "@langchain/core/tools";
import { z } from "zod";

export interface ToolDefinition {
  name: string;
  lcTool: ReturnType<typeof tool>; // LangChain tool instance
}

export class ToolRegistry {
  private tools: ToolDefinition[] = [];

  register(def: ToolDefinition): void {
    this.tools.push(def);
  }

  getLCTools() {
    return this.tools.map(d => d.lcTool);
  }
}

// Usage:
const registry = new ToolRegistry();
registry.register(bashTool);
registry.register(fileReadTool);
registry.register(searchTool);
// Phase 2: registry.loadFromDir('./skills/')

const llmWithTools = llm.bindTools(registry.getLCTools());
```

### Pattern 7: Bash Tool (Unrestricted)

**What:** LangChain tool wrapping `child_process.exec`. Returns stdout or stderr on error.
**When to use:** AGENT-02 requirement.

```typescript
// Source: [ASSUMED] — standard Node.js child_process pattern
import { tool } from "@langchain/core/tools";
import { exec } from "child_process";
import { promisify } from "util";
import { z } from "zod";

const execAsync = promisify(exec);

export const bashTool = tool(
  async ({ command }) => {
    try {
      const { stdout, stderr } = await execAsync(command, { shell: "/bin/zsh" });
      return stdout || stderr || "(no output)";
    } catch (err: any) {
      return `Error: ${err.message}\n${err.stderr ?? ""}`;
    }
  },
  {
    name: "Bash",
    description: "Execute a shell command and return its output. Use for system operations, file listing, running scripts, etc.",
    schema: z.object({
      command: z.string().describe("The shell command to execute"),
    }),
  }
);
```

### Pattern 8: Ink Streaming Output Component

**What:** An Ink component that renders streamed tokens progressively using `useState`.
**When to use:** Display LLM response as it arrives (D-03).

```typescript
// Source: https://github.com/vadimdemedes/ink + https://dev.to/skirianov/building-reactive-clis-with-ink-react-cli-library-4jpa
import React, { useState, useEffect } from "react";
import { Box, Text } from "ink";

interface StreamingOutputProps {
  tokenStream: AsyncIterable<string>;
  onDone: (full: string) => void;
}

export function StreamingOutput({ tokenStream, onDone }: StreamingOutputProps) {
  const [text, setText] = useState("");

  useEffect(() => {
    let full = "";
    async function consume() {
      for await (const token of tokenStream) {
        full += token;
        setText(full);
      }
      onDone(full);
    }
    consume();
  }, [tokenStream]);

  return (
    <Box>
      <Text>{text}</Text>
    </Box>
  );
}
```

### Pattern 9: Ink useInput for REPL

**What:** `useInput` hook captures keypresses in raw mode. Paired with a controlled text buffer for prompt input.
**When to use:** Main input loop in the REPL.

```typescript
// Source: /Users/avanade/Workspace/claude-mythos/src/ink/hooks/use-input.ts (verified)
import { useInput } from "ink"; // ink 7.0.2 re-exports useInput

useInput((input, key) => {
  if (key.return) {
    onSubmit(inputBuffer);
    setInputBuffer("");
    return;
  }
  if (key.backspace || key.delete) {
    setInputBuffer(prev => prev.slice(0, -1));
    return;
  }
  setInputBuffer(prev => prev + input);
});
```

### Pattern 10: Monorepo bin Entry Point

**What:** A thin JS wrapper that invokes the TypeScript entrypoint via tsx. The `bin` field in `package.json` points to this wrapper.
**When to use:** Required for `npm link` / `npm install -g` to make `sergin` available on PATH.

```javascript
// agent/bin/sergin.js
#!/usr/bin/env node
// This file exists so npm can symlink it. It imports the TypeScript source via tsx.
import('../node_modules/.bin/tsx').then(m => {
  // tsx re-executes this with TypeScript support
}).catch(() => {
  require('child_process').spawnSync(
    process.execPath,
    ['--import', 'tsx/esm', new URL('../src/cli/index.ts', import.meta.url).pathname],
    { stdio: 'inherit' }
  );
});
```

Simpler alternative for local dev: add to `agent/package.json`:
```json
{
  "scripts": { "start": "tsx src/cli/index.ts" },
  "bin": { "sergin": "./bin/sergin.js" }
}
```

### Anti-Patterns to Avoid

- **Using `createReactAgent`:** Locked out by D-04. Hand-craft the StateGraph to get the explicit `reflect` node topology.
- **Buffering LLM response before render:** Violates D-03. Always use `graph.stream()` with `streamMode: "messages"`, never `graph.invoke()` in the render path.
- **Rebuilding `ToolNode` from scratch:** LangGraph's prebuilt `ToolNode` handles parallel tool calls, error wrapping, and ToolMessage formatting. Use it for the `executeTool` node.
- **Storing conversation history in component state:** Messages must live in the LangGraph state (via MemorySaver), not React state. Component state is display-only.
- **Using `@langchain/community/tools/tavily_search`:** Deprecated. Use `@langchain/tavily` / `TavilySearch`.
- **Using `deepseek-reasoner` model for tool calling:** Tool calling requires `deepseek-chat`. Reasoner model does not support tool calls as of 2025-01-27. [VERIFIED: LangChain docs]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tool execution + ToolMessage formatting | Custom tool runner | `ToolNode` from `@langchain/langgraph/prebuilt` | Handles parallel calls, error wrapping, ToolMessage IDs automatically |
| Message history reducer | Custom array merge | `MessagesAnnotation` built-in reducer | Correctly appends/replaces messages; avoids history corruption bugs |
| Token streaming from LangGraph | Manual LLM stream + state update | `graph.stream()` with `streamMode: "messages"` | Graph-aware streaming; works through MemorySaver checkpoints |
| Zod schema → tool JSON schema | Custom serializer | `zod-to-json-schema` (required peer, already installed) | LangChain uses this internally; duplicating causes format mismatches |
| Terminal layout | ANSI escape sequences | Ink `Box` + `Text` with flexbox props | Ink handles terminal resize, cursor, and diff rendering |
| Ink raw mode setup | Manual `process.stdin.setRawMode` | `useInput` hook from `ink` | useLayoutEffect timing ensures raw mode is active before first render |

**Key insight:** LangGraph's prebuilt primitives (`ToolNode`, `MessagesAnnotation`, `MemorySaver`) eliminate entire categories of bugs around message history management and tool call lifecycle. The custom code budget should go to business logic (tools, UI, routing), not infrastructure.

---

## Common Pitfalls

### Pitfall 1: deepseek-chat Tool Calls Dropped
**What goes wrong:** `bindTools()` works, but the model response has `tool_calls: []` even when a tool should be called.
**Why it happens:** DeepSeek API requires `tool_choice: "auto"` to be explicit; some versions default to "none" unless prompted. Also, the system prompt must make tool use feel natural.
**How to avoid:** Set `tool_choice: "auto"` explicitly in `bindTools` options: `llm.bindTools(tools, { tool_choice: "auto" })`. Write a system prompt that mentions the available tools.
**Warning signs:** LLM always responds with text even for tasks that clearly need a tool.

### Pitfall 2: Streaming Blocked Inside StateGraph Nodes
**What goes wrong:** `graph.stream(streamMode: "messages")` returns no token chunks — only the final message after the full response.
**Why it happens:** If the node calls `llm.invoke()` (not streaming), LangGraph has no tokens to bubble up — only the final AIMessage surfaces through the messages stream mode.
**How to avoid:** The `callLLM` node must use the standard `llm.invoke()` pattern — LangGraph's `streamMode: "messages"` intercepts the LLM call at the framework level and captures streaming chunks automatically. Do NOT try to manually stream inside the node with `llm.stream()`.
**Warning signs:** Chunks appear only once at end of generation.

### Pitfall 3: MemorySaver Thread ID Not Set
**What goes wrong:** Each REPL turn starts with empty history — conversation context is lost between prompts.
**Why it happens:** `configurable: { thread_id }` must be passed on every `graph.stream()` call. Forgetting it means no checkpoint is stored or restored.
**How to avoid:** Generate `thread_id` once at session start (`crypto.randomUUID()`) and store in module scope. Pass in every invocation config.
**Warning signs:** LLM cannot reference earlier turns; each response is context-free.

### Pitfall 4: Ink Re-Render Performance on Token Stream
**What goes wrong:** Terminal flickers or stutters under fast token arrival.
**Why it happens:** Ink re-renders on every `setState`. Very fast tokens (< 5ms apart) can overwhelm the render cycle.
**How to avoid:** Batch token accumulation: append to a string ref (`useRef`) and flush to state on a `requestAnimationFrame`-style interval (16ms) or every N tokens. See claude-mythos `src/ink/components/App.tsx` for how it handles high-frequency updates.
**Warning signs:** Terminal output appears choppy or Ink renders partial frames.

### Pitfall 5: ESM / CommonJS Module Conflict
**What goes wrong:** `Cannot use import statement in a module` or `require is not defined` when mixing packages.
**Why it happens:** `@langchain/langgraph` 1.3.0 is ESM-first. `tsx` handles this transparently, but if you compile with `tsc` to CJS you hit interop issues.
**How to avoid:** Set `"type": "module"` in `agent/package.json` and use `tsx` for dev execution. For production builds, use `tsup` with `format: ["esm"]`. Never mix `require()` and `import` in the same file.
**Warning signs:** Runtime errors mentioning `__dirname` (CJS-only) or `import.meta.url` (ESM-only) in the wrong context.

### Pitfall 6: Ink `useInput` + Raw Mode Ordering
**What goes wrong:** Keystrokes echo to terminal or cursor is visible during input.
**Why it happens:** `useInput` uses `useLayoutEffect` (not `useEffect`) to set raw mode synchronously. If you try to set raw mode yourself outside the hook, timing conflicts occur.
**How to avoid:** Use only `useInput` from `ink`. Do not call `process.stdin.setRawMode()` directly. Verified in `claude-mythos/src/ink/hooks/use-input.ts`.
**Warning signs:** Characters appear doubled or cursor blinks in wrong position.

### Pitfall 7: Zod Version Peer Conflict
**What goes wrong:** `@langchain/langgraph` 1.3.0 requires `zod: "^3.25.32 || ^4.2.0"`. Using an older Zod causes schema validation errors at runtime.
**How to avoid:** Install `zod@4.4.3` (current). Check `npm ls zod` to detect version conflicts. Do not pin to `zod@3` unless other dependencies require it.
**Warning signs:** `z.object` schema errors at tool registration time, not call time.

---

## Code Examples

### Complete Graph Topology (Verified Pattern)

```typescript
// Source: https://deepwiki.com/langchain-ai/langgraphjs/2.1-stategraph-and-graph-building
// Source: https://medium.com/@barsegyan96armen/langgraph-101 (tool calling pattern)
import { StateGraph, MessagesAnnotation, MemorySaver, END, START } from "@langchain/langgraph";
import { ToolNode } from "@langchain/langgraph/prebuilt";
import { ChatDeepSeek } from "@langchain/deepseek";
import { AIMessage } from "@langchain/core/messages";

// ── Bootstrap ──────────────────────────────────────────────────────────────
const registry = new ToolRegistry();
registry.register(bashTool);
registry.register(fileReadTool);
registry.register(fileWriteTool);
registry.register(tavilySearchTool);

const llm = new ChatDeepSeek({
  model: "deepseek-chat",
  temperature: 0,
  apiKey: process.env.DEEPSEEK_API_KEY,
}).bindTools(registry.getLCTools(), { tool_choice: "auto" });

// ── Nodes ──────────────────────────────────────────────────────────────────
async function callLLM(state: typeof MessagesAnnotation.State) {
  const response = await llm.invoke(state.messages);
  return { messages: [response] };
}

function routeNext(state: typeof MessagesAnnotation.State): string {
  const last = state.messages[state.messages.length - 1] as AIMessage;
  return last?.tool_calls?.length ? "executeTool" : END;
}

// ── Graph ──────────────────────────────────────────────────────────────────
const memory = new MemorySaver();

export const graph = new StateGraph(MessagesAnnotation)
  .addNode("callLLM", callLLM)
  .addNode("executeTool", new ToolNode(registry.getLCTools()))
  .addEdge(START, "callLLM")
  .addConditionalEdges("callLLM", routeNext)
  .addEdge("executeTool", "callLLM")  // loop back; reflect handles errors via ToolMessage
  .compile({ checkpointer: memory });
```

### DeepSeek Streaming (Verified from sergin-brain)

```typescript
// Source: /Users/avanade/Workspace/sergin-brain/src/llm/llm.service.ts (verified file read)
// Streaming is via @langchain/deepseek ChatDeepSeek.stream() — same API used inside LangGraph
import { ChatDeepSeek } from "@langchain/deepseek";

const llm = new ChatDeepSeek({
  apiKey: process.env.DEEPSEEK_API_KEY,
  model: "deepseek-chat",
  temperature: 0,
  maxRetries: 2,
});

// Direct streaming (reference only — in the graph, use graph.stream() instead):
async function* streamDirect(messages: [string, string][], signal?: AbortSignal) {
  const stream = await llm.stream(messages, { signal });
  for await (const chunk of stream) {
    const text = typeof chunk.content === "string" ? chunk.content : "";
    if (text) yield text;
  }
}
```

### Tavily Search Tool

```typescript
// Source: https://www.npmjs.com/package/@langchain/tavily
import { TavilySearch } from "@langchain/tavily";

const tavilyTool = new TavilySearch({
  maxResults: 5,
  // searchDepth: "basic",  // "basic" | "advanced"
});
// TAVILY_API_KEY read from environment automatically

// Wrap in ToolDefinition for registry:
export const searchTool: ToolDefinition = {
  name: "WebSearch",
  lcTool: tavilyTool,
};
```

### Ink Minimal App Bootstrap

```typescript
// Source: https://github.com/vadimdemedes/ink
import React from "react";
import { render } from "ink";
import { App } from "./ui/App.js";

const { waitUntilExit } = render(
  <App graph={graph} sessionId={crypto.randomUUID()} />,
  { exitOnCtrlC: true }
);
await waitUntilExit();
```

---

## Reference Codebase: claude-mythos Key Findings

Verified by reading source files in `/Users/avanade/Workspace/claude-mythos/`.

### Ink Component Observations

- **`src/ink/hooks/use-input.ts`:** Uses `useLayoutEffect` (not `useEffect`) to call `setRawMode(true)` synchronously during commit phase. This is critical — using `useEffect` leaves terminal in cooked mode for one tick, causing key echo. [VERIFIED: file read]
- **`src/ink/components/App.tsx`:** Root class component handles stdin events, mouse tracking, and keyboard dispatch. For Sergin, we need only a fraction of this — `useInput` + component state is sufficient for Phase 1.
- **Ink version in claude-mythos:** The claude-mythos codebase is a heavily customized fork of Ink internals (the `src/ink/` directory contains vendored Ink source, not just imports). Sergin should import `ink` from npm directly — do not copy claude-mythos ink internals.

### Tool Structure Observations

- **`src/tools/BashTool/BashTool.tsx`:** Claude Code's BashTool is extremely complex (156KB) with security validation, sandbox, file history, and background task support. For Sergin Phase 1, a minimal bash tool (< 50 lines) is correct — no security layer needed per D-09.
- **Tool file per tool pattern:** Each tool lives in its own directory (`BashTool/`, `FileReadTool/`, etc.) with `toolName.ts` exporting a constant. Clean for Phase 2 extensibility.
- **`buildTool` / `ToolDef` pattern in claude-mythos:** Claude Code uses a proprietary `buildTool` factory. For Sergin, use `tool()` from `@langchain/core/tools` directly — same concept, standard LangGraph API.

### State Management Observations

- **`src/state/AppState.tsx`:** Claude Code uses a Zustand-like external store with `useSyncExternalStore`. For Phase 1 Sergin, React component `useState` is sufficient. Scale to external store only if multiple components need the same graph state.
- **Key insight:** Keep display state (what the Ink UI shows) separate from agent state (what LangGraph's MemorySaver stores). They are different concerns with different lifetimes.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `TavilySearchResults` from `@langchain/community` | `TavilySearch` from `@langchain/tavily` | 2024 Q4 | Import path changes; community version deprecated but still works |
| `createReactAgent` for simple agents | Hand-crafted `StateGraph` for custom topology | LangGraph 0.2+ | More control; required for custom reflect node (D-04) |
| `channels` object for state schema | `Annotation.Root` / `MessagesAnnotation` | LangGraph 0.2 | Cleaner type inference; `channels` still works but new patterns use Annotation |
| Ink v3/v4 | Ink 7.0.2 | 2024 | API stable; `useInput`, `Box`, `Text`, `Static` unchanged; `render()` return type now includes `waitUntilExit` |

**Deprecated/outdated:**
- `@langchain/community/tools/tavily_search` (TavilySearchResults): Deprecated. Use `@langchain/tavily`. [VERIFIED: LangChain docs]
- `deepseek-reasoner` for tool calling: Tool calling not supported. Use `deepseek-chat`. [VERIFIED: LangChain docs]
- `new StateGraph({ channels: {...} })`: Still works but `Annotation.Root` is the current pattern. [ASSUMED]

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | reflect node injects ToolMessage and routes back to callLLM via a simple edge (`executeTool → reflect → callLLM`) | Architecture Patterns, Pattern 3 | LangGraph may need the reflect-to-callLLM edge to pass through a separate conditional; testable during Walking Skeleton |
| A2 | `deepseek-chat` model name is still the correct tool-calling model ID (not a new name like `deepseek-v4-flash`) | Standard Stack | API calls fail with 404 or bad request; check DeepSeek platform dashboard before first run |
| A3 | `TavilySearch` from `@langchain/tavily` works as a drop-in `ToolDefinition` without extra adapter code | Pattern 6 | May need `lcTool.asTool()` or similar wrapping; verify on first integration |
| A4 | Ink 7.0.2 `useInput` API is unchanged from v4 (same hook signature, same raw mode behavior) | Pattern 9 | Minor API difference; fixable with docs check |
| A5 | `MessagesAnnotation` reducer correctly handles `ToolMessage` appending (not just HumanMessage/AIMessage) | Pattern 1 | Message history corruption; verify by logging `state.messages` after a tool call |

---

## Open Questions (RESOLVED)

1. **DeepSeek model ID for tool calling**
   - What we know: `deepseek-chat` was the tool-calling model as of 2025-01; sergin-brain uses `deepseek-chat` successfully.
   - What's unclear: DeepSeek has updated API naming; `deepseek-v4-flash` was mentioned in one search result as a current alias.
   - Recommendation: Start with `deepseek-chat` (confirmed in sergin-brain codebase). If tool calls fail, check the DeepSeek dashboard for current model aliases.
   - **RESOLVED:** Use `deepseek-chat` — confirmed working in sergin-brain codebase. Plans use this model ID consistently.

2. **Reflect node routing topology**
   - What we know: The locked decision (D-04/D-05) requires `executeTool → reflect → callLLM`. LangGraph's prebuilt `ToolNode` is designed for `executeTool → callLLM` (no reflect step).
   - What's unclear: Whether to use prebuilt `ToolNode` for executeTool and insert reflect as a pass-through node, or build a custom executeTool that handles errors internally.
   - Recommendation: Use `ToolNode` for execution. On error, `ToolNode` already wraps errors in ToolMessages. The reflect node can be a simple pass-through that adds extra context. If ToolNode error format is sufficient, the reflect node may be a no-op in the happy path.
   - **RESOLVED:** Use `ToolNode` for execution + a custom `reflect` pass-through node per D-04/D-05. The reflect node re-emits the error ToolMessage with a `[Reflect]` prefix so the LLM sees it as an explicit retry signal. Wired as `executeTool → reflect → callLLM`.

3. **Ink input component approach**
   - What we know: claude-mythos uses a heavily customized Ink fork. The standard `useInput` hook handles all keypresses.
   - What's unclear: Does the Phase 1 REPL need multi-line input? History navigation (up/down arrows)?
   - Recommendation: Start with single-line input via `useInput`. Add history navigation (in-memory array) if the user requests it during testing.
   - **RESOLVED:** Single-line `useInput` for Phase 1. History navigation deferred — add only if explicitly requested during testing.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Runtime | Yes | v24.14.0 | — |
| npm | Package manager | Yes | 11.9.0 | — |
| tsx | Dev CLI entry | No | — | `npx tsx` OR install globally: `npm i -g tsx` |
| DEEPSEEK_API_KEY | LLM inference | Unknown | — | Set in `.env` before first run |
| TAVILY_API_KEY | Web search | Unknown | — | Set in `.env`; 1,000 free searches/month |

**Missing dependencies with no fallback:**
- `DEEPSEEK_API_KEY` and `TAVILY_API_KEY` — env vars must be set before the agent can run. Wave 0 should include a `.env.example` file.

**Missing dependencies with fallback:**
- `tsx` not installed globally — install as devDependency in `agent/package.json` and reference via `./node_modules/.bin/tsx` in the bin wrapper.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Vitest (recommended) or Jest |
| Config file | `agent/vitest.config.ts` — Wave 0 gap |
| Quick run command | `npm test --workspace agent -- --run` |
| Full suite command | `npm test --workspace agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AGENT-01 | CLI boots and renders Ink output | smoke | `tsx src/cli/index.ts --help 2>&1` exits 0 | Wave 0 |
| AGENT-02 | Bash tool executes `echo hello` and returns stdout | unit | `vitest run tests/tools/bash.test.ts` | Wave 0 |
| AGENT-03 | File read returns file contents; write creates file | unit | `vitest run tests/tools/file.test.ts` | Wave 0 |
| AGENT-04 | Search tool returns non-empty results for a query | integration (needs key) | `vitest run tests/tools/search.test.ts` | Wave 0 |
| AGENT-05 | Graph routes to reflect on tool error | unit | `vitest run tests/graph/reflect.test.ts` | Wave 0 |
| AGENT-07 | Second turn references first turn's content | integration | `vitest run tests/graph/memory.test.ts` | Wave 0 |

### Sampling Rate

- **Per task commit:** `npm test --workspace agent -- --run tests/tools/bash.test.ts`
- **Per wave merge:** `npm test --workspace agent -- --run`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `agent/vitest.config.ts` — test framework config
- [ ] `agent/tests/tools/bash.test.ts` — covers AGENT-02
- [ ] `agent/tests/tools/file.test.ts` — covers AGENT-03
- [ ] `agent/tests/tools/search.test.ts` — covers AGENT-04 (requires TAVILY_API_KEY in env)
- [ ] `agent/tests/graph/reflect.test.ts` — covers AGENT-05
- [ ] `agent/tests/graph/memory.test.ts` — covers AGENT-07
- [ ] `agent/.env.example` — documents required env vars

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Single-user local CLI; no auth layer |
| V3 Session Management | No | MemorySaver is in-process; no session tokens |
| V4 Access Control | No | Single-user; D-09 explicitly removes restrictions |
| V5 Input Validation | Partial | Tool inputs validated via Zod schemas at LangGraph boundary |
| V6 Cryptography | No | No secrets stored; API keys are env vars only |

### Known Threat Patterns for {stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Prompt injection via tool output | Tampering | Out of scope for Phase 1 (single-user, trusted inputs) |
| API key leakage in logs | Information Disclosure | Never log `process.env.*` values; ensure tool output is not logged at DEBUG level |
| Unrestricted Bash execution | Elevation of Privilege | Accepted risk (D-09, owner-controlled machine); document in README |

---

## Sources

### Primary (HIGH confidence)
- `npm view @langchain/langgraph` — version 1.3.0, peerDependencies, exports
- `npm view ink` — version 7.0.2
- `npm view @langchain/deepseek` — version 1.0.25
- `npm view @langchain/tavily` — version 1.2.0
- `/Users/avanade/Workspace/sergin-brain/src/llm/llm.service.ts` — ChatDeepSeek streaming pattern (verified file read)
- `/Users/avanade/Workspace/claude-mythos/src/ink/hooks/use-input.ts` — useInput + useLayoutEffect pattern (verified file read)
- `/Users/avanade/Workspace/claude-mythos/src/ink/components/App.tsx` — Ink root component structure (verified file read)

### Secondary (MEDIUM confidence)
- [deepwiki.com — LangGraph JS StateGraph](https://deepwiki.com/langchain-ai/langgraphjs/2.1-stategraph-and-graph-building) — StateGraph API, Annotation, compile() options
- [LangChain streaming docs](https://docs.langchain.com/oss/javascript/langgraph/streaming) — streamMode "messages" pattern
- [LangChain deepseek integration](https://docs.langchain.com/oss/javascript/integrations/chat/deepseek) — model names, tool calling limitation on reasoner
- [npmjs.com/@langchain/tavily deprecation note](https://docs.langchain.com/oss/javascript/integrations/tools/tavily_search_community) — TavilySearchResults deprecation confirmed

### Tertiary (LOW confidence)
- [Medium — LangGraph 101 JS](https://medium.com/@barsegyan96armen/langgraph-101-understanding-the-core-concepts-of-state-nodes-and-edges-in-javascript-f91068683d7d) — StateGraph builder pattern
- [langgraphjs.guide/streaming](https://langgraphjs.guide/streaming/) — streamEvents pattern
- [js-craft.io — Tool Calling LangChain.js Part 2](https://www.js-craft.io/blog/tool-calling-langchain-js-toolmessage-schemas/) — tool() + ToolMessage pattern

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all versions npm-verified; packages exist and are active
- LangGraph StateGraph API: HIGH — confirmed by multiple sources + peerDep inspection
- Ink streaming pattern: HIGH — verified in claude-mythos source + official ink README
- Reflect node pattern: MEDIUM — derived from LangGraph state semantics; no exact reference found for the exact D-04 topology
- Tavily deprecation: HIGH — confirmed by official LangChain docs page title and redirect
- Bash tool pattern: MEDIUM — standard Node.js child_process, but exact error surface not verified

**Research date:** 2026-05-09
**Valid until:** 2026-06-09 (30 days — LangGraph releases frequently; recheck on major version bump)
