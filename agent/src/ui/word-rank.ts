import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

const STOP_WORDS = new Set([
  "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
  "of", "with", "by", "from", "as", "is", "was", "be", "been", "are",
  "were", "has", "have", "had", "do", "does", "did", "will", "would",
  "could", "should", "may", "might", "can", "shall", "not", "no", "nor",
  "so", "if", "then", "than", "that", "this", "these", "those", "it",
  "its", "he", "she", "they", "them", "we", "you", "me", "him", "her",
  "my", "your", "his", "their", "our", "all", "each", "every", "some",
  "any", "both", "few", "more", "most", "other",
  "o", "a", "de", "em", "para", "com", "por", "sem", "sob", "sobre",
  "após", "antes", "durante", "entre", "até", "após",
  "e", "ou", "mas", "nem", "também", "além", "conforme",
  "um", "uma", "uns", "umas", "o", "os", "as",
  "este", "esta", "estes", "estas", "esse", "essa", "esses", "essas",
  "aquele", "aquela", "aqueles", "aquelas",
  "eu", "tu", "ele", "ela", "nós", "vós", "eles", "elas",
  "meu", "minha", "teu", "tua", "seu", "sua", "nosso", "nossa",
  "que", "como", "quando", "onde", "porque", "pois",
  "já", "mais", "menos", "muito", "pouco", "bem", "mal",
  "sim", "não", "talvez",
]);

const WORDS_FILE = join(homedir(), ".local", "share", "kira", "word-rank.json");

interface WordEntry {
  count: number;
}

function loadWords(): Map<string, number> {
  try {
    if (!existsSync(WORDS_FILE)) return new Map();
    const raw = readFileSync(WORDS_FILE, "utf-8");
    const parsed = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return new Map();
    const map = new Map<string, number>();
    for (const [key, val] of Object.entries(parsed)) {
      const entry = val as WordEntry;
      if (typeof entry.count === "number") {
        map.set(key, entry.count);
      }
    }
    return map;
  } catch {
    return new Map();
  }
}

function saveWords(map: Map<string, number>): void {
  try {
    if (!existsSync(join(homedir(), ".local", "share", "kira"))) {
      mkdirSync(join(homedir(), ".local", "share", "kira"), { recursive: true });
    }
    const obj: Record<string, WordEntry> = {};
    for (const [key, count] of map) {
      obj[key] = { count };
    }
    writeFileSync(WORDS_FILE, JSON.stringify(obj, null, 2), "utf-8");
  } catch {
    // Silent fail
  }
}

let _words: Map<string, number> | null = null;
let _dirty = false;

function getWords(): Map<string, number> {
  if (!_words) {
    _words = loadWords();
  }
  return _words;
}

function persist(): void {
  if (_words && _dirty) {
    saveWords(_words);
    _dirty = false;
  }
}

export function tokenize(text: string): string[] {
  const words = text.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  return words.filter((w) => w.length >= 3 && !STOP_WORDS.has(w));
}

export function recordWords(text: string): void {
  const words = tokenize(text);
  if (words.length === 0) return;

  const map = getWords();
  for (const word of words) {
    map.set(word, (map.get(word) || 0) + 1);
  }
  _dirty = true;
  persist();
}

export interface WordRankResult {
  word: string;
  count: number;
}

export function getTopWords(limit = 10): WordRankResult[] {
  const map = getWords();
  const sorted = Array.from(map.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, limit);
  return sorted.map(([word, count]) => ({ word, count }));
}

export function getWordBoost(query: string): number {
  if (!query.trim()) return 0;

  const words = tokenize(query);
  const map = getWords();
  if (map.size === 0) return 0;

  let totalBoost = 0;
  let matched = 0;

  for (const word of words) {
    const count = map.get(word);
    if (count) {
      totalBoost += count;
      matched++;
    }
  }

  if (matched === 0) return 0;

  const maxCount = Math.max(1, ...Array.from(map.values()));
  const normalizedScore = totalBoost / (matched * maxCount);
  return Math.min(1, normalizedScore);
}

export function clearWords(): void {
  _words = new Map();
  _dirty = true;
  persist();
}
