import { describe, it, expect } from "vitest";

// Integration test — requires live TAVILY_API_KEY.
// Skipped automatically in CI or when key is not set.
// Run manually: TAVILY_API_KEY=xxx npm test --workspace agent -- --run tests/tools/search.test.ts

const hasTavilyKey = !!process.env.TAVILY_API_KEY;

describe("searchTool (AGENT-04)", () => {
  it("tool name is 'WebSearch'", async () => {
    // Unit test — no API call, always runs
    const { searchTool } = await import("../../src/tools/search.js");
    expect(searchTool.name).toBe("WebSearch");
    expect(searchTool.lcTool).toBeDefined();
  });

  it.skipIf(!hasTavilyKey)(
    "returns search results for a query (live API)",
    async () => {
      const { searchTool } = await import("../../src/tools/search.js");
      // Tavily's _call returns the raw API response object with a results array
      const result: any = await searchTool.lcTool.invoke({ query: "LangGraph JS current version" });
      expect(result).toBeDefined();
      expect(Array.isArray(result.results)).toBe(true);
      if (result.results.length > 0) {
        expect(typeof result.results[0].title).toBe("string");
        expect(typeof result.results[0].content).toBe("string");
      }
    }
  );

  it.skipIf(!hasTavilyKey)(
    "result contains meaningful content for specific search",
    async () => {
      const { searchTool } = await import("../../src/tools/search.js");
      const result: any = await searchTool.lcTool.invoke({ query: "DeepSeek API model names 2025" });
      expect(result).toBeDefined();
      // Result should contain a results array, potentially with items mentioning deepseek or api
      if (result.results && result.results.length > 0) {
        const allContent = result.results.map((r: any) => (r.title + " " + r.content).toLowerCase()).join(" ");
        expect(allContent).toMatch(/deepseek|api|model/);
      }
    }
  );
});
