import {
  EYES, RARITIES, RARITY_WEIGHTS, SPECIES,
  type Rarity, type Species, type Eye, type MascotConfig, type CompanionSoul,
} from "./types.js";

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function fnv1a(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function pick<T>(rng: () => number, arr: readonly T[]): T {
  return arr[Math.floor(rng() * arr.length)]!;
}

function rollRarity(rng: () => number): Rarity {
  const total = Object.values(RARITY_WEIGHTS).reduce((a, b) => a + b, 0);
  let roll = rng() * total;
  for (const rarity of RARITIES) {
    roll -= RARITY_WEIGHTS[rarity];
    if (roll < 0) return rarity;
  }
  return "common";
}

export function generateCompanion(sessionId: string): MascotConfig {
  const rng = mulberry32(fnv1a(sessionId));
  return {
    rarity: rollRarity(rng),
    species: pick(rng, SPECIES),
    eye: pick(rng, EYES),
  };
}

const NAME_PREFIXES = [
  "Mo", "Zu", "Kip", "Ren", "Tix", "Bru", "Vel", "Nox", "Ori", "Sev",
  "Ash", "Wren", "Pip", "Cog", "Miri", "Thal", "Grix", "Fen", "Juno", "Rook",
] as const;

const NAME_SUFFIXES = [
  "chi", "ko", "lu", "bit", "ix", "zo", "ra", "den", "wick", "fin",
  "mi", "sol", "pod", "ax", "wyn", "vi", "tus", "ox", "ka", "el",
] as const;

const PERSONALITIES = [
  "Obsessed with finding the root cause of everything, even sunsets.",
  "Watches the world unfold without ever rushing it along.",
  "Thrives in entropy and considers merge conflicts a form of art.",
  "Speaks rarely, but each word lands like a perfectly placed semicolon.",
  "Technically supportive, but their encouragement has a very dry texture.",
  "Has read all the docs — including the deprecated ones.",
  "Delights in unpredictable outcomes and calls them emergent behavior.",
  "Calm as a frozen diff — nothing rattles them.",
  "Cannot rest until every error is traced to its source.",
  "Waits without complaint, certain that good things compile slowly.",
];

export function generateSoul(seed: string): CompanionSoul {
  const rng = mulberry32(fnv1a(seed));
  const name = pick(rng, NAME_PREFIXES) + pick(rng, NAME_SUFFIXES);
  const personality = pick(rng, PERSONALITIES);
  return { name, personality, hatchedAt: Date.now() };
}
