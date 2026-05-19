import { END } from "@langchain/langgraph";
import { AIMessage } from "@langchain/core/messages";
import type { GraphState } from "../state.js";

// Conditional edge: routes to executeTool if LLM produced tool_calls, else END.
// In the Walking Skeleton, always returns END (no tools registered yet).
export function routeNext(state: GraphState): string {
  const last = state.messages[state.messages.length - 1];
  if (last instanceof AIMessage && last.tool_calls && last.tool_calls.length > 0) {
    return "executeTool";
  }
  return END;
}
