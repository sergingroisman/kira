import { tool } from "@langchain/core/tools";
import { exec } from "child_process";
import { promisify } from "util";
import { z } from "zod";
import type { ToolDefinition } from "./registry.js";

const execAsync = promisify(exec);

// Bash tool — unrestricted per D-09.
// Single-user system; owner controls the machine. No timeout, no blocklist.
// Returns stdout on success, error string on failure (does NOT throw).
// Error return means ToolNode emits a ToolMessage with that error string,
// which the reflect node then injects back to the LLM state.
const bashLcTool = tool(
  async ({ command }: { command: string }) => {
    try {
      const { stdout, stderr } = await execAsync(command, {
        shell: "/bin/zsh",
        maxBuffer: 10 * 1024 * 1024, // 10MB output cap
      });
      return stdout || stderr || "(no output)";
    } catch (err: unknown) {
      const e = err as { message: string; stderr?: string };
      return `Error: ${e.message}\n${e.stderr ?? ""}`.trim();
    }
  },
  {
    name: "Bash",
    description:
      "Execute a shell command in zsh and return its output. Use for: file listing, running scripts, git operations, system info, package management. Returns stdout on success or error details on failure.",
    schema: z.object({
      command: z.string().describe("The shell command to execute"),
    }),
  }
);

export const bashTool: ToolDefinition = {
  name: "Bash",
  lcTool: bashLcTool,
};
