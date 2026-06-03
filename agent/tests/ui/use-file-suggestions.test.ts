import { describe, it, expect } from "vitest";
import {
  extractCompletionToken,
  formatReplacementValue,
  getPreservedSelection,
  findLongestCommonPrefix,
} from "../../src/ui/suggestion-engine.js";
import type { FileSuggestion } from "../../src/ui/file-suggestions.js";

describe("use-file-suggestions integration logic", () => {
  describe("token detection > suggestion fetch > replacement flow", () => {
    const suggestions: FileSuggestion[] = [
      { id: "file-src/main.ts", path: "src/main.ts", displayText: "src/main.ts", type: "file" },
      { id: "file-src/main.test.ts", path: "src/main.test.ts", displayText: "src/main.test.ts", type: "file" },
      { id: "dir-src/components", path: "src/components", displayText: "src/components/", type: "directory" },
    ];

    it("extractCompletionToken detects @mention at end", () => {
      const token = extractCompletionToken("edit @src/mai", 13, true);
      expect(token).not.toBeNull();
      expect(token!.value).toBe("@src/mai");
      expect(token!.startPos).toBe(5);
      expect(token!.hasAtPrefix).toBe(true);
    });

    it("formatReplacementValue produces correct replacement for file", () => {
      const token = extractCompletionToken("@src/mai", 8, true);
      const suggestion = suggestions[0]!;
      const replacement = formatReplacementValue(suggestion, {
        isQuoted: token!.isQuoted,
        hasAtPrefix: token!.hasAtPrefix,
      });
      expect(replacement.text).toBe("@src/main.ts ");
    });

    it("formatReplacementValue produces correct replacement for directory", () => {
      const token = extractCompletionToken("@src/comp", 9, true);
      const suggestion = suggestions[2]!;
      const replacement = formatReplacementValue(suggestion, {
        isQuoted: token!.isQuoted,
        hasAtPrefix: token!.hasAtPrefix,
      });
      expect(replacement.text).toBe("@src/components/");
    });

    it("getPreservedSelection keeps index when item still present", () => {
      const prev = suggestions.slice(0, 2);
      const next = [suggestions[2]!, suggestions[0]!];
      expect(getPreservedSelection(prev, 0, next)).toBe(1);
    });

    it("getPreservedSelection returns 0 when previously selected item is gone", () => {
      const prev = suggestions.slice(0, 2);
      const next = [suggestions[2]!];
      expect(getPreservedSelection(prev, 0, next)).toBe(0);
    });

    it("ghost text is longest common prefix minus current token", () => {
      // Token typed: @src/mai
      // Suggestions displayText: ["src/main.ts", "src/main.test.ts"]
      // LCP of displayTexts: "src/main"
      // Current part (without @): "src/mai"
      // Ghost suffix: "n" (since "src/main" - "src/mai" = "n")
      const displayTexts = ["src/main.ts", "src/main.test.ts"];
      const lcp = findLongestCommonPrefix(displayTexts);
      expect(lcp).toBe("src/main.t");
    });

    it("acceptCurrent computes correct new text and cursor position", () => {
      const text = "edit @src/mai";
      const cursorOffset = 13;
      const token = extractCompletionToken(text, cursorOffset, true);
      expect(token).not.toBeNull();

      const suggestion = suggestions[0]!;
      const replacement = formatReplacementValue(suggestion, {
        isQuoted: token!.isQuoted,
        hasAtPrefix: token!.hasAtPrefix,
      });

      const before = text.slice(0, token!.startPos);
      const after = text.slice(token!.startPos + token!.value.length);
      const newText = before + replacement.text + after;
      const newOffset = before.length + replacement.text.length;

      expect(newText).toBe("edit @src/main.ts ");
      expect(newOffset).toBe(18);
    });

    it("acceptGhost appends ghost text at cursor when cursor is at end", () => {
      const text = "@src/mai";
      const cursorOffset = 8;
      const ghostSuffix = "n.ts ";

      const newText = text + ghostSuffix;
      const newOffset = cursorOffset + ghostSuffix.length;

      expect(newText).toBe("@src/main.ts ");
      expect(newOffset).toBe(13);
    });
  });
});
