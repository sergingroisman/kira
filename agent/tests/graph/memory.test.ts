import { describe, it, expect, vi } from "vitest";
import {
  StateGraph,
  MessagesAnnotation,
  MemorySaver,
  START,
  END,
} from "@langchain/langgraph";
import { HumanMessage, AIMessage } from "@langchain/core/messages";

// Build a minimal test graph that mirrors the production topology but with a fake LLM node.
// This validates MemorySaver behavior without any API calls.
function buildTestGraph() {
  const memory = new MemorySaver();

  // Fake LLM node: echoes the last HumanMessage content back as an AIMessage
  async function fakeCallLLM(state: typeof MessagesAnnotation.State) {
    const last = state.messages[state.messages.length - 1];
    const content =
      last instanceof HumanMessage
        ? `Echo: ${last.content}`
        : "No human message found";
    return { messages: [new AIMessage(content)] };
  }

  const graph = new StateGraph(MessagesAnnotation)
    .addNode("callLLM", fakeCallLLM)
    .addEdge(START, "callLLM")
    .addEdge("callLLM", END)
    .compile({ checkpointer: memory });

  return graph;
}

describe("MemorySaver — conversation history (AGENT-07)", () => {
  it("accumulates messages across turns with the same thread_id", async () => {
    const graph = buildTestGraph();
    const config = { configurable: { thread_id: crypto.randomUUID() } };

    // Turn 1
    await graph.invoke(
      { messages: [new HumanMessage("Hello, my name is Sergio")] },
      config
    );

    // Turn 2 — same thread_id
    await graph.invoke(
      { messages: [new HumanMessage("What is my name?")] },
      config
    );

    // MemorySaver should have accumulated all 4 messages
    const state = await graph.getState(config);
    expect(state.values.messages).toHaveLength(4);

    // Turn 1: HumanMessage + AIMessage
    expect(state.values.messages[0]).toBeInstanceOf(HumanMessage);
    expect(state.values.messages[1]).toBeInstanceOf(AIMessage);
    // Turn 2: HumanMessage + AIMessage
    expect(state.values.messages[2]).toBeInstanceOf(HumanMessage);
    expect(state.values.messages[3]).toBeInstanceOf(AIMessage);
  });

  it("isolates history by thread_id — different threads do not share messages", async () => {
    const graph = buildTestGraph();

    const configA = { configurable: { thread_id: crypto.randomUUID() } };
    const configB = { configurable: { thread_id: crypto.randomUUID() } };

    // Two turns on thread A
    await graph.invoke({ messages: [new HumanMessage("Thread A message 1")] }, configA);
    await graph.invoke({ messages: [new HumanMessage("Thread A message 2")] }, configA);

    // One turn on thread B
    await graph.invoke({ messages: [new HumanMessage("Thread B message 1")] }, configB);

    const stateA = await graph.getState(configA);
    const stateB = await graph.getState(configB);

    // Thread A has 4 messages (2 turns x 2 messages)
    expect(stateA.values.messages).toHaveLength(4);
    // Thread B has 2 messages (1 turn)
    expect(stateB.values.messages).toHaveLength(2);
  });

  it("makeSessionConfig generates a unique thread_id on every call", async () => {
    // Stub env before importing graph module (which calls ChatDeepSeek at module load)
    vi.stubEnv("DEEPSEEK_API_KEY", "test-key");
    const { makeSessionConfig } = await import("../../src/graph/graph.js");

    const config1 = makeSessionConfig();
    const config2 = makeSessionConfig();

    expect(config1.configurable.thread_id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
    );
    expect(config1.configurable.thread_id).not.toBe(config2.configurable.thread_id);
  });
});
