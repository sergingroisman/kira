import { describe, it, expect, beforeEach } from "vitest";
import { tokenize, getWordBoost, clearWords, recordWords } from "../../src/ui/word-rank.js";

describe("tokenize", () => {
  it("splits text into lowercase words", () => {
    const result = tokenize("Refatorar o módulo de autenticação");
    expect(result).toContain("refatorar");
    expect(result).toContain("módulo");
    expect(result).toContain("autenticação");
  });

  it("removes stop words (pt/en)", () => {
    const result = tokenize("o the a de em para and with");
    expect(result).toEqual([]);
  });

  it("ignores words shorter than 3 characters", () => {
    const result = tokenize("ox it is to no");
    expect(result).toEqual([]);
  });

  it("removes punctuation", () => {
    const result = tokenize("refatorar, módulo! autenticação...");
    expect(result).toEqual(["refatorar", "módulo", "autenticação"]);
  });

  it("handles empty input", () => {
    expect(tokenize("")).toEqual([]);
  });

  it("handles mixed case", () => {
    const result = tokenize("REFATORAR Módulo");
    expect(result).toContain("refatorar");
    expect(result).toContain("módulo");
  });
});

describe("getWordBoost", () => {
  beforeEach(() => {
    clearWords();
  });

  it("returns 0 for empty word map", () => {
    expect(getWordBoost("refatorar")).toBe(0);
  });

  it("returns boost when query contains known words", () => {
    recordWords("refatorar módulo autenticação");
    recordWords("refatorar testes");
    recordWords("refatorar api");

    const boost = getWordBoost("refatorar módulo");
    expect(boost).toBeGreaterThan(0);
    expect(boost).toBeLessThanOrEqual(1);
  });

  it("returns 0 for query with no known words", () => {
    recordWords("refatorar módulo");
    expect(getWordBoost("xyznonexistent")).toBe(0);
  });

  it("higher boost for more frequent words", () => {
    recordWords("frequent"); // recorded once
    for (let i = 0; i < 10; i++) {
      recordWords("veryfrequent"); // recorded 10x
    }

    const frequentBoost = getWordBoost("frequent");
    const veryBoost = getWordBoost("veryfrequent");
    expect(veryBoost).toBeGreaterThanOrEqual(frequentBoost);
  });
});
