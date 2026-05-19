import { TavilySearch } from "@langchain/tavily";
import type { ToolDefinition } from "./registry.js";

// TavilySearch reads TAVILY_API_KEY from the environment.
// We pass tavilyApiKey with a fallback so the constructor doesn't throw
// when the env var is absent (e.g., in CI or local dev without the key).
// The fallback key is only used when the env var is missing; actual usage
// will fail at call time, but module import succeeds for structural availability.
// @langchain/tavily is the official package — NOT @langchain/community (deprecated).
const tavilyInstance = new TavilySearch({
  maxResults: 5,
  tavilyApiKey: process.env.TAVILY_API_KEY || "unset",
});

export const searchTool: ToolDefinition = {
  name: "WebSearch",
  lcTool: tavilyInstance as unknown as ReturnType<typeof import("@langchain/core/tools").tool>,
};
