import { ToolMessage } from "@langchain/core/messages";
import type { GraphState } from "../state.js";

// Reflect node (D-05): examines the last ToolMessage in state.
// If it contains an error, it enriches the message so the LLM has a clear signal.
// The LLM then decides: retry with corrected parameters, or explain the failure to the user.
// No fixed retry counter for Phase 1 — LLM decides when to stop.
//
// Edge: executeTool → reflect → callLLM (always loops back — the LLM decides to stop, not the graph)
export async function reflect(state: GraphState): Promise<Partial<GraphState>> {
  const messages = state.messages;
  const last = messages[messages.length - 1];

  // Only enrich ToolMessages that look like errors
  if (
    last instanceof ToolMessage &&
    typeof last.content === "string" &&
    last.content.startsWith("Error:")
  ) {
    const enriched = new ToolMessage({
      tool_call_id: last.tool_call_id,
      content:
        `${last.content}\n\n` +
        `[Reflect] The tool returned an error. ` +
        `Please either: (a) retry the tool call with corrected parameters, ` +
        `or (b) explain to the user why this cannot be completed.`,
    });
    return { messages: [enriched] };
  }

  // Happy path: no error — pass through without adding messages
  return { messages: [] };
}
