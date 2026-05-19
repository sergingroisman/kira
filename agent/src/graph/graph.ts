import { StateGraph, MessagesAnnotation, MemorySaver, START } from "@langchain/langgraph";
import { callLLM, bindLLMTools } from "./nodes/callLLM.js";
import { routeNext } from "./nodes/routeNext.js";
import { makeToolNode } from "./nodes/executeTool.js";
import { reflect } from "./nodes/reflect.js";
import { registry } from "../tools/registry.js";
import { bashTool } from "../tools/bash.js";
import { fileReadTool, } from "../tools/fileRead.js";
import { fileWriteTool, fileAppendTool } from "../tools/fileWrite.js";
import { searchTool } from "../tools/search.js";

// ── Tool Registration ─────────────────────────────────────────────────────────
// Phase 1 tools: Bash, FileRead, FileWrite, FileAppend, WebSearch (Tavily)
// Phase 2 will add: registry.loadFromDir('./skills/')
// Phase 3 will add: registry.register(mcpTool) for each MCP-sourced tool
registry.register(bashTool);
registry.register(fileReadTool);
registry.register(fileWriteTool);
registry.register(fileAppendTool);
registry.register(searchTool);   // Tavily web search (AGENT-04)

// ── LLM Binding ──────────────────────────────────────────────────────────────
// bindLLMTools replaces the LLM singleton in callLLM.ts with a tool-bound version.
// tool_choice: "auto" ensures DeepSeek considers tool calls (Pitfall 1 guard).
bindLLMTools(registry.getLCTools());

// ── Graph Construction ────────────────────────────────────────────────────────
const memory = new MemorySaver();
const toolNode = makeToolNode(registry);

// Full Phase 1 topology (D-04):
//   START → callLLM → routeNext ──(no tools)──→ END
//                         │
//                    (has tool_calls)
//                         ↓
//                   executeTool → reflect → callLLM (loop)
export const graph = new StateGraph(MessagesAnnotation)
  .addNode("callLLM", callLLM)
  .addNode("executeTool", toolNode)
  .addNode("reflect", reflect)
  .addEdge(START, "callLLM")
  .addConditionalEdges("callLLM", routeNext)
  .addEdge("executeTool", "reflect")
  .addEdge("reflect", "callLLM")
  .compile({ checkpointer: memory });

// Session config factory — call once per CLI session, reuse on every turn (Pitfall 3 guard)
export function makeSessionConfig() {
  return { configurable: { thread_id: crypto.randomUUID() } };
}
