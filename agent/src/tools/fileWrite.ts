import { tool } from "@langchain/core/tools";
import { writeFile, appendFile, mkdir } from "fs/promises";
import { dirname } from "path";
import { z } from "zod";
import type { ToolDefinition } from "./registry.js";

const fileWriteLcTool = tool(
  async ({ path, content }: { path: string; content: string }) => {
    try {
      // Create parent directories if they don't exist
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, content, "utf-8");
      return `Written ${Buffer.byteLength(content, "utf-8")} bytes to ${path}`;
    } catch (err: unknown) {
      const e = err as { message: string };
      return `Error: ${e.message}`;
    }
  },
  {
    name: "FileWrite",
    description:
      "Write content to a file, creating it if it does not exist and overwriting if it does. Creates parent directories automatically.",
    schema: z.object({
      path: z.string().describe("Absolute or relative path to the file to write"),
      content: z.string().describe("Content to write to the file"),
    }),
  }
);

const fileAppendLcTool = tool(
  async ({ path, content }: { path: string; content: string }) => {
    try {
      await mkdir(dirname(path), { recursive: true });
      await appendFile(path, content, "utf-8");
      return `Appended ${Buffer.byteLength(content, "utf-8")} bytes to ${path}`;
    } catch (err: unknown) {
      const e = err as { message: string };
      return `Error: ${e.message}`;
    }
  },
  {
    name: "FileAppend",
    description:
      "Append content to an existing file. Creates the file if it does not exist.",
    schema: z.object({
      path: z.string().describe("Absolute or relative path to the file"),
      content: z.string().describe("Content to append to the file"),
    }),
  }
);

export const fileWriteTool: ToolDefinition = {
  name: "FileWrite",
  lcTool: fileWriteLcTool,
};

export const fileAppendTool: ToolDefinition = {
  name: "FileAppend",
  lcTool: fileAppendLcTool,
};
