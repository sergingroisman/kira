import { ToolNode } from "@langchain/langgraph/prebuilt";
import type { ToolRegistry } from "../../tools/registry.js";

// makeToolNode creates a ToolNode bound to the registry's tools.
// Called once from graph.ts after all tools are registered.
// ToolNode from @langchain/langgraph/prebuilt handles:
//   - Parallel tool calls
//   - Error wrapping into ToolMessages
//   - ToolMessage ID assignment
// Do NOT build a custom tool runner — ToolNode is the standard (see RESEARCH.md "Don't Hand-Roll")
export function makeToolNode(reg: ToolRegistry): ToolNode {
  return new ToolNode(reg.getLCTools());
}
