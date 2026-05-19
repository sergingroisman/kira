import { tool } from "@langchain/core/tools";
import { readFile } from "fs/promises";
import { z } from "zod";
import type { ToolDefinition } from "./registry.js";

const fileReadLcTool = tool(
  async ({ path }: { path: string }) => {
    try {
      const content = await readFile(path, "utf-8");
      return content;
    } catch (err: unknown) {
      const e = err as { code?: string; message: string };
      if (e.code === "ENOENT") return `Error: File not found: ${path}`;
      return `Error: ${e.message}`;
    }
  },
  {
    name: "FileRead",
    description:
      "Read the contents of a file at the given absolute or relative path. Returns the full file content as a string.",
    schema: z.object({
      path: z.string().describe("Absolute or relative path to the file to read"),
    }),
  }
);

export const fileReadTool: ToolDefinition = {
  name: "FileRead",
  lcTool: fileReadLcTool,
};
