/**
 * File Suggestions — autocomplete for @path/file in the REPL input.
 *
 * Uses Bun's native Bun.glob for fast file scanning.
 * Results are cached with TTL to avoid repeated disk scans.
 *
 * Inspired by Claude Mythos's file index + suggestion system.
 */

// -- Types ---------------------------------------------------------------------

export interface FileSuggestion {
  id: string;
  path: string;
  type: "file" | "directory";
  displayText: string;
}

// -- Cache ---------------------------------------------------------------------

interface CacheEntry {
  suggestions: FileSuggestion[];
  timestamp: number;
}

const CACHE_TTL_MS = 5_000;
const MAX_RESULTS = 20;
const cache = new Map<string, CacheEntry>();

export function invalidateFileCache(): void {
  cache.clear();
}

// -- Core Logic ----------------------------------------------------------------

/**
 * Get file suggestions for a partial @path.
 *
 * Examples:
 *   getSuggestions("", "/workspace") -> top-level files/dirs
 *   getSuggestions("src/", "/workspace") -> files under src/
 *   getSuggestions("src/mai", "/workspace") -> files matching src/mai*
 */
export async function getSuggestions(
  partialPath: string,
  cwd = process.cwd()
): Promise<FileSuggestion[]> {
  let query = partialPath;
  if (query.startsWith("@")) query = query.slice(1);
  if (query.startsWith("~/")) query = query.slice(2);

  const cacheKey = `${cwd}::${query}`;
  const cached = cache.get(cacheKey);
  if (cached && Date.now() - cached.timestamp < CACHE_TTL_MS) {
    return filterSuggestions(cached.suggestions, query);
  }

  const pattern = buildGlobPattern(query);
  const results: FileSuggestion[] = [];

  try {
    const globs = [
      pattern,
      pattern.endsWith("/") ? pattern + "*" : pattern + "*/",
    ];

    for (const g of globs) {
      const glob = Bun.glob(g, { cwd, absolute: false });
      for await (const file of glob) {
        if (results.length >= MAX_RESULTS * 3) break;

        const isDir = file.endsWith("/");
        const displayPath = file.replace(/^\.\/?/, "");

        if (!query.startsWith(".") && displayPath.split("/").some(p => p.startsWith(".") && p !== ".")) {
          continue;
        }

        const id = isDir ? `dir-${displayPath}` : `file-${displayPath}`;
        results.push({
          id,
          path: file,
          type: isDir ? "directory" : "file",
          displayText: isDir ? displayPath + "/" : displayPath,
        });
      }
    }

    const seen = new Set<string>();
    const unique = results.filter((r) => {
      if (seen.has(r.id)) return false;
      seen.add(r.id);
      return true;
    });

    unique.sort((a, b) => {
      if (a.type !== b.type) return a.type === "directory" ? -1 : 1;
      return a.displayText.localeCompare(b.displayText);
    });

    const final = unique.slice(0, MAX_RESULTS);
    cache.set(cacheKey, { suggestions: final, timestamp: Date.now() });

    return filterSuggestions(final, query);
  } catch {
    return [];
  }
}

function filterSuggestions(
  suggestions: FileSuggestion[],
  query: string
): FileSuggestion[] {
  if (!query) return suggestions.slice(0, MAX_RESULTS);

  const normalized = query.toLowerCase();
  return suggestions
    .filter((s) => s.displayText.toLowerCase().startsWith(normalized))
    .slice(0, MAX_RESULTS);
}

function buildGlobPattern(query: string): string {
  if (!query || query === "/" || query === "./" || query === "~") {
    return "**/*";
  }

  if (query.startsWith("~/")) {
    query = query.slice(2);
  }
  if (query.startsWith("./")) {
    query = query.slice(2);
  }

  if (query.endsWith("/")) {
    return query + "**/*";
  }

  if (query.endsWith(".")) {
    return query + "*";
  }

  return query + "*";
}

/**
 * @deprecated Use formatReplacementValue from suggestion-engine.ts instead.
 */
export function formatSuggestionInsertion(
  suggestion: FileSuggestion,
  _originalPrefix: string
): string {
  return suggestion.type === "directory"
    ? suggestion.displayText
    : suggestion.displayText + " ";
}

/**
 * @deprecated Use extractCompletionToken from suggestion-engine.ts instead.
 */
export function getAtTokenAtCursor(
  text: string,
  cursorOffset: number
): string | null {
  const before = text.slice(0, cursorOffset);
  const match = before.match(/(^|\s)(@"[^"]*|@[^\s]*)$/);
  if (!match) return null;

  const token = match[2];
  if (token === "@" || token === '@"') return null;

  const pathPart = token.startsWith("@") ? token.slice(1) : token;
  if (!pathPart.includes("/") && !pathPart.startsWith(".") && !pathPart.startsWith("~")) {
    return null;
  }

  return token;
}
