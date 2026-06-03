export const RARITIES = ["common", "uncommon", "rare", "epic", "legendary"] as const;
export type Rarity = (typeof RARITIES)[number];

const c = String.fromCharCode;

export const duck = c(0x64,0x75,0x63,0x6b) as "duck";
export const goose = c(0x67,0x6f,0x6f,0x73,0x65) as "goose";
export const blob = c(0x62,0x6c,0x6f,0x62) as "blob";
export const cat = c(0x63,0x61,0x74) as "cat";
export const dragon = c(0x64,0x72,0x61,0x67,0x6f,0x6e) as "dragon";
export const octopus = c(0x6f,0x63,0x74,0x6f,0x70,0x75,0x73) as "octopus";
export const owl = c(0x6f,0x77,0x6c) as "owl";
export const penguin = c(0x70,0x65,0x6e,0x67,0x75,0x69,0x6e) as "penguin";
export const turtle = c(0x74,0x75,0x72,0x74,0x6c,0x65) as "turtle";
export const snail = c(0x73,0x6e,0x61,0x69,0x6c) as "snail";
export const ghost = c(0x67,0x68,0x6f,0x73,0x74) as "ghost";
export const axolotl = c(0x61,0x78,0x6f,0x6c,0x6f,0x74,0x6c) as "axolotl";
export const capybara = c(0x63,0x61,0x70,0x79,0x62,0x61,0x72,0x61) as "capybara";
export const cactus = c(0x63,0x61,0x63,0x74,0x75,0x73) as "cactus";
export const robot = c(0x72,0x6f,0x62,0x6f,0x74) as "robot";
export const rabbit = c(0x72,0x61,0x62,0x62,0x69,0x74) as "rabbit";
export const mushroom = c(0x6d,0x75,0x73,0x68,0x72,0x6f,0x6f,0x6d) as "mushroom";
export const chonk = c(0x63,0x68,0x6f,0x6e,0x6b) as "chonk";

export const SPECIES = [
  duck, goose, blob, cat, dragon, octopus, owl, penguin,
  turtle, snail, ghost, axolotl, capybara, cactus, robot,
  rabbit, mushroom, chonk,
] as const;
export type Species = (typeof SPECIES)[number];

export const EYES = ["\u00B7", "\u2726", "\u00D7", "\u25C9", "@", "\u00B0"] as const;
export type Eye = (typeof EYES)[number];

export const RARITY_WEIGHTS = {
  common: 60,
  uncommon: 25,
  rare: 10,
  epic: 4,
  legendary: 1,
} as const satisfies Record<Rarity, number>;

export const RARITY_COLORS = {
  common: "#888888",
  uncommon: "#4ADE80",
  rare: "#60A5FA",
  epic: "#A78BFA",
  legendary: "#FBBF24",
} as const satisfies Record<Rarity, string>;

export type Emotion = "happy" | "sad" | "surprised" | "angry" | "sleepy" | "sick" | "dead" | "neutral";

export type MascotConfig = {
  species: Species;
  eye: Eye;
  rarity: Rarity;
};

// ─── Companion Soul ───────────────────────────────────────────────────

export type CompanionSoul = {
  name: string;
  personality: string;
  hatchedAt: number;
};

// ─── XP / Level ───────────────────────────────────────────────────────

export type XPState = {
  total: number;
  lastSeenTokensIn: number;
  lastSeenTokensOut: number;
};

export const LEVEL_THRESHOLDS = [0, 500, 2000, 5500, 12500, 27500, 57500, 117500, 237500, 477500];

export const RARITY_STARS: Record<Rarity, string> = {
  common: "\u2605",
  uncommon: "\u2605\u2605",
  rare: "\u2605\u2605\u2605",
  epic: "\u2605\u2605\u2605\u2605",
  legendary: "\u2605\u2605\u2605\u2605\u2605",
};

export function levelToRarity(level: number): Rarity {
  if (level >= 9) return "legendary";
  if (level >= 7) return "epic";
  if (level >= 5) return "rare";
  if (level >= 3) return "uncommon";
  return "common";
}

export function xpToLevel(totalXP: number): { level: number; xpInLevel: number; xpNeeded: number } {
  let level = 1;
  for (let i = 1; i < LEVEL_THRESHOLDS.length; i++) {
    if (totalXP >= LEVEL_THRESHOLDS[i]!) level = i + 1;
    else break;
  }
  const prev = LEVEL_THRESHOLDS[level - 1] ?? 0;
  const next = LEVEL_THRESHOLDS[Math.min(level, LEVEL_THRESHOLDS.length - 1)]!;
  return { level, xpInLevel: totalXP - prev, xpNeeded: Math.max(1, next - prev) };
}

export function progressBar(current: number, total: number, width = 10): string {
  const pct = Math.min(1, Math.max(0, current / total));
  const filled = Math.round(pct * width);
  return "\u2588".repeat(filled) + "\u2591".repeat(width - filled);
}

export function meterBar(value: number, filled: string, empty: string, count: number): string {
  const n = Math.max(0, Math.min(count, Math.round((value / 100) * count)));
  return filled.repeat(n) + empty.repeat(count - n);
}

// ─── Tamagotchi State ─────────────────────────────────────────────────

export type TamagotchiState = {
  hunger: number;
  happiness: number;
  poopPiles: number;
  sleeping: boolean;
  sick: boolean;
  sickSince: number | null;
  lastDecayAt: number;
  isDead: boolean;
  hungerZeroSince: number | null;
  happinessZeroSince: number | null;
};

export type TamaAction = "feed" | "play" | "clean" | "medicate" | "sleep" | "wake" | "revive";

// ─── Pomodoro State ──────────────────────────────────────────────────

export type PomodoroPhase = "idle" | "focus" | "short_break" | "long_break";

export const POMODORO_COLORS: Record<PomodoroPhase, string> = {
  idle: "#666666",
  focus: "#4ADE80",
  short_break: "#60A5FA",
  long_break: "#60A5FA",
};

export function phaseLabel(phase: PomodoroPhase): string {
  switch (phase) {
    case "focus": return "FOCUS";
    case "short_break": return "BREAK";
    case "long_break": return "LONG BREAK";
    default: return "";
  }
}

export type PomodoroState = {
  phase: PomodoroPhase;
  startedAt: number | null;
  elapsed: number;
  completed: number;
  focusDuration: number;
  shortBreakDuration: number;
  longBreakDuration: number;
  longBreakInterval: number;
};

export type PersistedCompanionState = {
  companion: CompanionSoul | null;
  xp: XPState;
  tama: TamagotchiState;
  hidden: boolean;
  pomodoro: PomodoroState;
};
