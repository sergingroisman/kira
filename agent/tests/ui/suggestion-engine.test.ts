import { describe, it, expect } from "vitest";
import {
  extractCompletionToken,
  formatReplacementValue,
  getPreservedSelection,
  findLongestCommonPrefix,
  firstGrapheme,
} from "../../src/ui/suggestion-engine.js";

describe("extractCompletionToken", () => {
  it("returns null for empty input", () => {
    expect(extractCompletionToken("", 0, true)).toBeNull();
  });

  it("extracts simple @token at start", () => {
    const result = extractCompletionToken("@src/main", 9, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("@src/main");
    expect(result!.startPos).toBe(0);
    expect(result!.isQuoted).toBe(false);
    expect(result!.hasAtPrefix).toBe(true);
  });

  it("extracts @token in middle of text", () => {
    const result = extractCompletionToken("edit @src/main", 15, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("@src/main");
    expect(result!.startPos).toBe(5);
  });

  it("returns null for bare @ with no path chars", () => {
    expect(extractCompletionToken("@", 1, true)).toBeNull();
  });

  it("extracts quoted @ token", () => {
    const result = extractCompletionToken('@"my file', 10, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe('@"my file');
    expect(result!.isQuoted).toBe(true);
    expect(result!.hasAtPrefix).toBe(true);
  });

  it("extracts quoted @ token with closing quote", () => {
    const result = extractCompletionToken('@"my file.txt"', 15, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe('@"my file.txt"');
    expect(result!.isQuoted).toBe(true);
  });

  it("handles cursor in middle of token (extends forward)", () => {
    const result = extractCompletionToken("src/main.t", 10, false);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("src/main.t");
    // The PATH_CHAR_HEAD_RE will extend past cursor
    const extended = extractCompletionToken("src/main.ts", 10, false);
    expect(extended!.value.length).toBeGreaterThan("src/main.t".length);
  });

  it("handles unicode characters in path", () => {
    const result = extractCompletionToken("@áccent/directório", 20, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("@áccent/directório");
  });

  it("extracts token with dots", () => {
    const result = extractCompletionToken("@src/main.test.ts", 18, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("@src/main.test.ts");
  });

  it("extracts token with hyphens", () => {
    const result = extractCompletionToken("@my-component/index", 19, true);
    expect(result).not.toBeNull();
    expect(result!.value).toBe("@my-component/index");
  });
});

describe("formatReplacementValue", () => {
  const fileSuggestion = {
    id: "file-test.ts",
    path: "src/test.ts",
    displayText: "src/test.ts",
    type: "file" as const,
  };

  const dirSuggestion = {
    id: "dir-src/components",
    path: "src/components",
    displayText: "src/components/",
    type: "directory" as const,
  };

  const spacedFile = {
    id: "file-docs/my file.md",
    path: "docs/my file.md",
    displayText: "docs/my file.md",
    type: "file" as const,
  };

  it("adds space for file with @ prefix", () => {
    const result = formatReplacementValue(fileSuggestion, {
      isQuoted: false,
      hasAtPrefix: true,
    });
    expect(result.text).toBe("@src/test.ts ");
  });

  it("does not add space for directory with @ prefix", () => {
    const result = formatReplacementValue(dirSuggestion, {
      isQuoted: false,
      hasAtPrefix: true,
    });
    expect(result.text).toBe("@src/components/");
  });

  it("wraps spaced paths in quotes", () => {
    const result = formatReplacementValue(spacedFile, {
      isQuoted: false,
      hasAtPrefix: true,
    });
    expect(result.text).toBe('@"docs/my file.md" ');
  });

  it("preserves quotes when already quoted", () => {
    const result = formatReplacementValue(spacedFile, {
      isQuoted: true,
      hasAtPrefix: true,
    });
    expect(result.text).toBe('@"docs/my file.md" ');
  });

  it("handles non-@ prefix (bare path)", () => {
    const result = formatReplacementValue(fileSuggestion, {
      isQuoted: false,
      hasAtPrefix: false,
    });
    expect(result.text).toBe("src/test.ts ");
  });

  it("does not add space for directory without @", () => {
    const result = formatReplacementValue(dirSuggestion, {
      isQuoted: false,
      hasAtPrefix: false,
    });
    expect(result.text).toBe("src/components/");
  });
});

describe("getPreservedSelection", () => {
  const items = (ids: string[]) => ids.map((id) => ({ id }));

  it("returns -1 for empty newSuggestions", () => {
    expect(getPreservedSelection(items(["a", "b"]), 0, [])).toBe(-1);
  });

  it("returns 0 when no previous selection", () => {
    expect(getPreservedSelection(items(["a"]), -1, items(["b", "c"]))).toBe(0);
  });

  it("returns 0 when previous selection out of bounds", () => {
    expect(getPreservedSelection(items(["a"]), 5, items(["b", "c"]))).toBe(0);
  });

  it("preserves selection on same id", () => {
    const prev = items(["a", "b", "c"]);
    const next = items(["d", "a", "e"]);
    expect(getPreservedSelection(prev, 0, next)).toBe(1);
  });

  it("returns 0 when previously selected id is gone", () => {
    const prev = items(["a", "b"]);
    const next = items(["c", "d"]);
    expect(getPreservedSelection(prev, 0, next)).toBe(0);
  });
});

describe("findLongestCommonPrefix", () => {
  it("returns empty for empty list", () => {
    expect(findLongestCommonPrefix([])).toBe("");
  });

  it("returns the only string for single item", () => {
    expect(findLongestCommonPrefix(["hello"])).toBe("hello");
  });

  it("finds common prefix", () => {
    expect(findLongestCommonPrefix(["src/main.ts", "src/main.test.ts"])).toBe("src/main.t");
  });

  it("returns empty when no common prefix", () => {
    expect(findLongestCommonPrefix(["foo", "bar"])).toBe("");
  });

  it("handles all same strings", () => {
    expect(findLongestCommonPrefix(["abc", "abc", "abc"])).toBe("abc");
  });
});

describe("firstGrapheme", () => {
  it("returns first character", () => {
    expect(firstGrapheme("hello")).toBe("h");
  });

  it("returns empty for empty string", () => {
    expect(firstGrapheme("")).toBe("");
  });

  it("handles single character", () => {
    expect(firstGrapheme("a")).toBe("a");
  });
});
