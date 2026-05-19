import { describe, it, expect } from "vitest";
import { bashTool } from "../../src/tools/bash.js";

describe("bashTool (AGENT-02)", () => {
  it("executes echo and returns stdout", async () => {
    const result = await bashTool.lcTool.invoke({ command: "echo hello_world" });
    expect(result).toContain("hello_world");
  });

  it("returns error string for nonexistent command", async () => {
    const result = await bashTool.lcTool.invoke({ command: "nonexistent_command_xyz_123" });
    expect(result).toMatch(/error/i);
  });

  it("captures stderr for failing commands", async () => {
    const result = await bashTool.lcTool.invoke({ command: "ls /nonexistent_path_xyz_123" });
    expect(result).toMatch(/error|no such file/i);
  });

  it("does not throw on command failure — returns error string", async () => {
    // Reflect node depends on error return (not throw) to inject error into state
    await expect(bashTool.lcTool.invoke({ command: "exit 1" })).resolves.toBeDefined();
  });

  it("tool name is 'Bash'", () => {
    expect(bashTool.name).toBe("Bash");
    expect(bashTool.lcTool.name).toBe("Bash");
  });
});
