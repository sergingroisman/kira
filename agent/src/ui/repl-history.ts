import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

interface HistoryEntry {
  count: number;
  timestamp: number;
}

const HISTORY_DIR = join(homedir(), ".local", "share", "kira");
const HISTORY_FILE = join(HISTORY_DIR, "repl-history.json");
const MAX_HISTORY = 100;
const RECENCY_HALF_LIFE_MS = 7 * 24 * 60 * 60 * 1000; // 7 days

function loadHistory(): Map<string, HistoryEntry> {
  try {
    if (!existsSync(HISTORY_FILE)) return new Map();
    const raw = readFileSync(HISTORY_FILE, "utf-8");
    const parsed = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return new Map();
    const map = new Map<string, HistoryEntry>();
    for (const [key, val] of Object.entries(parsed)) {
      const entry = val as HistoryEntry;
      if (typeof entry.count === "number" && typeof entry.timestamp === "number") {
        map.set(key, entry);
      }
    }
    return map;
  } catch {
    return new Map();
  }
}

function saveHistory(map: Map<string, HistoryEntry>): void {
  try {
    if (!existsSync(HISTORY_DIR)) {
      mkdirSync(HISTORY_DIR, { recursive: true });
    }
    const obj: Record<string, HistoryEntry> = {};
    for (const [key, val] of map) {
      obj[key] = val;
    }
    writeFileSync(HISTORY_FILE, JSON.stringify(obj, null, 2), "utf-8");
  } catch {
    // Silently fail on persistence errors
  }
}

let _history: Map<string, HistoryEntry> | null = null;
let _dirty = false;

function getHistory(): Map<string, HistoryEntry> {
  if (!_history) {
    _history = loadHistory();
  }
  return _history;
}

function persist(): void {
  if (_history && _dirty) {
    saveHistory(_history);
    _dirty = false;
  }
}

export function recordCommand(text: string): void {
  const history = getHistory();
  const existing = history.get(text);
  if (existing) {
    existing.count += 1;
    existing.timestamp = Date.now();
  } else {
    if (history.size >= MAX_HISTORY) {
      // Remove oldest entry by timestamp
      let oldestKey: string | null = null;
      let oldestTs = Infinity;
      for (const [key, val] of history) {
        if (val.timestamp < oldestTs) {
          oldestTs = val.timestamp;
          oldestKey = key;
        }
      }
      if (oldestKey) history.delete(oldestKey);
    }
    history.set(text, { count: 1, timestamp: Date.now() });
  }
  _dirty = true;
  persist();
}

export interface HistoryResult {
  text: string;
  score: number;
}

function recencyBoost(timestamp: number): number {
  const age = Date.now() - timestamp;
  return Math.max(0, Math.exp(-age / RECENCY_HALF_LIFE_MS));
}

function matchQuality(text: string, query: string): number {
  const lowerText = text.toLowerCase();
  const lowerQuery = query.toLowerCase();
  const idx = lowerText.indexOf(lowerQuery);
  if (idx === -1) return 1; // worst
  return idx / Math.max(text.length, 1);
}

export function searchHistory(query: string, limit = 10): HistoryResult[] {
  if (!query.trim()) return [];

  const history = getHistory();
  if (history.size === 0) return [];

  const maxCount = Math.max(1, ...Array.from(history.values(), (e) => e.count));

  const results: HistoryResult[] = [];

  for (const [text, entry] of history) {
    const lowerText = text.toLowerCase();
    const lowerQuery = query.toLowerCase();
    if (!lowerText.includes(lowerQuery)) continue;

    const recency = recencyBoost(entry.timestamp);
    const frequency = entry.count / maxCount;
    const match = matchQuality(text, query);

    const score = recency * 0.4 + frequency * 0.3 + (1 - match) * 0.3;

    results.push({ text, score });
  }

  results.sort((a, b) => b.score - a.score);
  return results.slice(0, limit);
}

export function clearHistory(): void {
  _history = new Map();
  _dirty = true;
  persist();
}
