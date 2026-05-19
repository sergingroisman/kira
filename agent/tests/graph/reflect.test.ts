import { describe, it, expect } from "vitest";
import { ToolMessage, AIMessage } from "@langchain/core/messages";

describe("reflect node (AGENT-05)", () => {
  it("enriches error ToolMessages with retry instructions", async () => {
    const { reflect } = await import("../../src/graph/nodes/reflect.js");

    const errorMsg = new ToolMessage({
      tool_call_id: "call_123",
      content: "Error: Command not found: nonexistent",
    });

    const state = { messages: [errorMsg] } as any;
    const result = await reflect(state);

    // Result should add an enriched message
    expect(result.messages).toHaveLength(1);
    expect((result.messages![0] as ToolMessage).content).toContain("[Reflect]");
    expect((result.messages![0] as ToolMessage).content).toContain("retry");
  });

  it("is a no-op for successful ToolMessages (happy path)", async () => {
    const { reflect } = await import("../../src/graph/nodes/reflect.js");

    const successMsg = new ToolMessage({
      tool_call_id: "call_456",
      content: "total 8\ndrwxr-xr-x  2 user group   64 Jan  1 00:00 .",
    });

    const state = { messages: [successMsg] } as any;
    const result = await reflect(state);

    // No-op: empty messages array means nothing appended
    expect(result.messages).toHaveLength(0);
  });

  it("is a no-op when last message is not a ToolMessage", async () => {
    const { reflect } = await import("../../src/graph/nodes/reflect.js");

    const state = { messages: [new AIMessage("Hello!")] } as any;
    const result = await reflect(state);

    expect(result.messages).toHaveLength(0);
  });
});
