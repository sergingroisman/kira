import { describe, it, expect, vi } from "vitest";

// Smoke test for graph module loading. Does not invoke the real LLM.
// Full integration tests are in memory.test.ts (Plan 03).

describe("Walking Skeleton — graph module", () => {
  it("exports graph and makeSessionConfig", async () => {
    // Mock environment before import
    vi.stubEnv("DEEPSEEK_API_KEY", "test-key");

    const { graph, makeSessionConfig } = await import("../../src/graph/graph.js");

    expect(graph).toBeDefined();
    expect(typeof makeSessionConfig).toBe("function");
  });

  it("makeSessionConfig returns a unique thread_id on each call", async () => {
    vi.stubEnv("DEEPSEEK_API_KEY", "test-key");
    const { makeSessionConfig } = await import("../../src/graph/graph.js");

    const config1 = makeSessionConfig();
    const config2 = makeSessionConfig();

    expect(config1.configurable.thread_id).toBeDefined();
    expect(config1.configurable.thread_id).not.toBe(config2.configurable.thread_id);
  });

  it("routeNext returns END when last message has no tool_calls", async () => {
    const { routeNext } = await import("../../src/graph/nodes/routeNext.js");
    const { AIMessage } = await import("@langchain/core/messages");

    const state = { messages: [new AIMessage("Hello, world!")] };
    const result = routeNext(state as any);

    // END is the string "__end__" in LangGraph
    expect(result).toBe("__end__");
  });
});
