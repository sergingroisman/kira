import { ChatDeepSeek } from "@langchain/deepseek";
import type { GraphState } from "../state.js";

// LLM is initialized once at module load and reused across all calls.
// Tool binding happens in graph.ts after ToolRegistry is initialized.
// In the Walking Skeleton, no tools are bound — plain invoke.
let _llm: ReturnType<typeof buildLLM> | null = null;

function buildLLM() {
  return new ChatDeepSeek({
    apiKey: process.env.DEEPSEEK_API_KEY,
    model: "deepseek-chat",   // NOT deepseek-reasoner; reasoner has no tool calling
    temperature: 0,
    maxRetries: 2,
  });
}

// getLLM returns a memoized instance.
// graph.ts replaces this with a tool-bound LLM in Plan 02.
export function getLLM() {
  if (!_llm) _llm = buildLLM();
  return _llm;
}

// bindLLMTools replaces the LLM singleton with a tool-bound version.
// Called once from graph.ts after ToolRegistry is populated (Plan 02).
export function bindLLMTools(tools: Parameters<ReturnType<typeof buildLLM>["bindTools"]>[0]) {
  _llm = buildLLM().bindTools(tools, { tool_choice: "auto" });
}

export async function callLLM(state: GraphState): Promise<Partial<GraphState>> {
  const llm = getLLM();
  const response = await llm.invoke(state.messages);
  // MessagesAnnotation reducer appends — return array with single new message
  return { messages: [response] };
}
