import type { TamagotchiState, TamaAction } from "./types.js";

const DEFAULT_FOCUS_MS = 25 * 60 * 1000;
let _decayFocusMs = DEFAULT_FOCUS_MS;

export function setDecayFocusDuration(focusMs: number): void {
  _decayFocusMs = Math.max(60_000, focusMs);
}

function decayPerMs(): number {
  return 30 / _decayFocusMs;
}

const POOP_MEAN_MS = 8 * 60 * 1000;
const DISEASE_PROB_PER_TICK = 0.05;
const DISEASE_DEATH_MS = 2 * 60 * 60 * 1000;
const HUNGER_DEATH_MS = 1 * 60 * 60 * 1000;
const HAPPINESS_DEATH_MS = 3 * 60 * 60 * 1000;

export function defaultTamagotchi(): TamagotchiState {
  return {
    hunger: 80,
    happiness: 80,
    poopPiles: 0,
    sleeping: false,
    sick: false,
    sickSince: null,
    lastDecayAt: Date.now(),
    isDead: false,
    hungerZeroSince: null,
    happinessZeroSince: null,
  };
}

export function applyDecay(tama: TamagotchiState, now: number): void {
  if (tama.isDead) return;
  if (tama.sleeping) {
    tama.lastDecayAt = now;
    return;
  }

  const deltaMs = Math.max(0, now - tama.lastDecayAt);
  const MAX_DECAY_WINDOW_MS = 5 * 60 * 1000;
  const clampedDelta = Math.min(deltaMs, MAX_DECAY_WINDOW_MS);
  if (clampedDelta < 50) return;

  const dpm = decayPerMs();
  tama.hunger = Math.max(0, tama.hunger - clampedDelta * dpm);
  tama.happiness = Math.max(0, tama.happiness - clampedDelta * dpm);

  if (tama.hunger <= 0) {
    if (tama.hungerZeroSince === null) tama.hungerZeroSince = now;
  } else {
    tama.hungerZeroSince = null;
  }

  if (tama.happiness <= 0) {
    if (tama.happinessZeroSince === null) tama.happinessZeroSince = now;
  } else {
    tama.happinessZeroSince = null;
  }

  const poopProb = 1 - Math.exp(-clampedDelta / POOP_MEAN_MS);
  if (Math.random() < poopProb) {
    tama.poopPiles = Math.min(tama.poopPiles + 1, 8);
  }

  if (tama.poopPiles >= 4 && !tama.sick && Math.random() < DISEASE_PROB_PER_TICK) {
    tama.sick = true;
    tama.sickSince = now;
  }

  if (tama.sick && tama.sickSince !== null && now - tama.sickSince >= DISEASE_DEATH_MS) {
    tama.isDead = true;
  }

  if (tama.hungerZeroSince !== null && now - tama.hungerZeroSince >= HUNGER_DEATH_MS) {
    tama.isDead = true;
  }

  if (tama.happinessZeroSince !== null && now - tama.happinessZeroSince >= HAPPINESS_DEATH_MS) {
    tama.isDead = true;
  }

  tama.lastDecayAt = now;
}

export function dispatchAction(tama: TamagotchiState, action: TamaAction): string | null {
  switch (action) {
    case "feed":
      if (tama.hunger < 100) {
        tama.hunger = Math.min(100, tama.hunger + 40);
        return "nom nom";
      }
      return "full!";
    case "play":
      if (tama.happiness < 100) {
        tama.happiness = Math.min(100, tama.happiness + 40);
        return "\u266B playing";
      }
      return "happy!";
    case "clean":
      if (tama.poopPiles > 0) {
        tama.poopPiles = 0;
        return "\u2728 cleaned";
      }
      return "sparkling!";
    case "medicate":
      if (tama.sick) {
        tama.sick = false;
        tama.sickSince = null;
        return "\uD83D\uDC8A medicated";
      }
      return "healthy!";
    case "sleep":
      if (!tama.isDead && !tama.sleeping) {
        tama.sleeping = true;
        return "zzz";
      }
      return null;
    case "wake":
      if (!tama.isDead && tama.sleeping) {
        tama.sleeping = false;
        tama.lastDecayAt = Date.now();
        return "\u2600 awake!";
      }
      return null;
    case "revive":
      tama.isDead = false;
      tama.hunger = 40;
      tama.happiness = 40;
      tama.poopPiles = 0;
      tama.sick = false;
      tama.sickSince = null;
      tama.hungerZeroSince = null;
      tama.happinessZeroSince = null;
      tama.lastDecayAt = Date.now();
      return "\u2728 revived!";
  }
}

export function pomodoroAutoCare(tama: TamagotchiState): void {
  if (tama.isDead) return;
  tama.hunger = Math.min(100, tama.hunger + 30);
  tama.happiness = Math.min(100, tama.happiness + 30);
  tama.poopPiles = Math.max(0, tama.poopPiles - 1);
}

export function tamaEmotion(tama: TamagotchiState): "happy" | "sad" | "surprised" | "angry" | "sleepy" | "sick" | "dead" | "neutral" {
  if (tama.isDead) return "dead";
  if (tama.sleeping) return "sleepy";
  if (tama.sick) return "sick";
  if (tama.happiness < 30) return "sad";
  if (tama.hunger < 30) return "angry";
  if (tama.happiness > 70) return "happy";
  return "neutral";
}
