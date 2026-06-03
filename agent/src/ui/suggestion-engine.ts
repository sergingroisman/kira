export interface SuggestionItem {
  id: string;
  displayText: string;
  type: "file" | "directory";
  path: string;
}

export interface CompletionToken {
  value: string;
  startPos: number;
  isQuoted: boolean;
  hasAtPrefix: boolean;
}

export interface Replacement {
  text: string;
}

const AT_TOKEN_HEAD_RE = /^@[\p{L}\p{N}\p{M}_\-./\\()[\]~:]*/u;
const PATH_CHAR_HEAD_RE = /^[\p{L}\p{N}\p{M}_\-./\\()[\]~:]+/u;
const TOKEN_WITH_AT_RE = /(@[\p{L}\p{N}\p{M}_\-./\\()[\]~:]*|[\p{L}\p{N}\p{M}_\-./\\()[\]~:]+)$/u;
const TOKEN_WITHOUT_AT_RE = /[\p{L}\p{N}\p{M}_\-./\\()[\]~:]+$/u;

export function extractCompletionToken(
  text: string,
  cursorPos: number,
  includeAtSymbol = false
): CompletionToken | null {
  if (!text) return null;

  const textBeforeCursor = text.substring(0, cursorPos);

  if (includeAtSymbol) {
    const quotedAtRegex = /@"([^"]*)"?$/;
    const quotedMatch = textBeforeCursor.match(quotedAtRegex);
    if (quotedMatch && quotedMatch.index !== undefined) {
      const textAfterCursor = text.substring(cursorPos);
      const afterQuotedMatch = textAfterCursor.match(/^[^"]*"?/);
      const quotedSuffix = afterQuotedMatch ? afterQuotedMatch[0] : "";
      const isBare = quotedMatch[0] === '@"' && !quotedSuffix;
      if (!isBare) {
        return {
          value: quotedMatch[0] + quotedSuffix,
          startPos: quotedMatch.index,
          isQuoted: true,
          hasAtPrefix: true,
        };
      }
    }
  }

  if (includeAtSymbol) {
    const atIdx = textBeforeCursor.lastIndexOf("@");
    if (atIdx >= 0 && (atIdx === 0 || /\s/.test(textBeforeCursor[atIdx - 1]!))) {
      const fromAt = textBeforeCursor.substring(atIdx);
      const atHeadMatch = fromAt.match(AT_TOKEN_HEAD_RE);
      if (atHeadMatch && atHeadMatch[0].length === fromAt.length) {
        const textAfterCursor = text.substring(cursorPos);
        const afterMatch = textAfterCursor.match(PATH_CHAR_HEAD_RE);
        const tokenSuffix = afterMatch ? afterMatch[0] : "";
        const value = atHeadMatch[0] + tokenSuffix;
        if (value === "@") return null;
        return {
          value,
          startPos: atIdx,
          isQuoted: false,
          hasAtPrefix: true,
        };
      }
    }
  }

  const tokenRegex = includeAtSymbol ? TOKEN_WITH_AT_RE : TOKEN_WITHOUT_AT_RE;
  const match = textBeforeCursor.match(tokenRegex);
  if (!match || match.index === undefined) {
    return null;
  }

  const textAfterCursor = text.substring(cursorPos);
  const afterMatch = textAfterCursor.match(PATH_CHAR_HEAD_RE);
  const tokenSuffix = afterMatch ? afterMatch[0] : "";
  const value = match[0] + tokenSuffix;
  if (includeAtSymbol && value === "@") return null;
  return {
    value,
    startPos: match.index,
    isQuoted: false,
    hasAtPrefix: includeAtSymbol && match[0].startsWith("@"),
  };
}

export function formatReplacementValue(
  suggestion: { path: string; displayText: string; type: "file" | "directory" },
  token: { isQuoted: boolean; hasAtPrefix: boolean }
): Replacement {
  const isDir = suggestion.type === "directory";
  const needsQuotes = suggestion.path.includes(" ") && !token.isQuoted;

  if (needsQuotes) {
    return { text: `@"${suggestion.path}"${isDir ? "" : " "}` };
  }
  if (token.isQuoted) {
    return { text: `@"${suggestion.path}"${isDir ? "" : " "}` };
  }
  if (token.hasAtPrefix) {
    return { text: `@${isDir ? suggestion.displayText : suggestion.displayText + " "}` };
  }
  return { text: isDir ? suggestion.displayText : suggestion.displayText + " " };
}

export function getPreservedSelection(
  prevSuggestions: { id: string }[],
  prevSelection: number,
  newSuggestions: { id: string }[]
): number {
  if (newSuggestions.length === 0) return -1;
  if (prevSelection < 0 || prevSelection >= prevSuggestions.length) return 0;

  const prevId = prevSuggestions[prevSelection]?.id;
  if (!prevId) return 0;

  const newIndex = newSuggestions.findIndex((item) => item.id === prevId);
  return newIndex >= 0 ? newIndex : 0;
}

export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  ms: number
): (...args: Parameters<T>) => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return (...args: Parameters<T>) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      fn(...args);
    }, ms);
  };
}

export function findLongestCommonPrefix(strings: string[]): string {
  if (strings.length === 0) return "";
  if (strings.length === 1) return strings[0]!;

  let prefix = strings[0]!;
  for (let i = 1; i < strings.length; i++) {
    const s = strings[i]!;
    let j = 0;
    while (j < prefix.length && j < s.length && prefix[j] === s[j]) {
      j++;
    }
    prefix = prefix.substring(0, j);
    if (prefix === "") break;
  }
  return prefix;
}

export function firstGrapheme(s: string): string {
  for (const ch of s) {
    return ch;
  }
  return "";
}
