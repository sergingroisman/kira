import { MessagesAnnotation } from "@langchain/langgraph";

// GraphState is the state type used in all node signatures.
// MessagesAnnotation provides the messages reducer (append semantics).
// Additional fields added in Phase 2 (e.g., _lastToolError for reflect).
export type GraphState = typeof MessagesAnnotation.State;
