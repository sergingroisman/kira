import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";
import type { PersistedCompanionState } from "./types.js";
import { defaultTamagotchi } from "./tamagotchi.js";
import { defaultPomodoro } from "./pomodoro.js";

const DATA_DIR = join(homedir(), ".local", "share", "kira");
const STATE_PATH = join(DATA_DIR, "companion.json");

export function loadCompanionState(): PersistedCompanionState {
  if (existsSync(STATE_PATH)) {
    try {
      const raw = readFileSync(STATE_PATH, "utf-8");
      const parsed = JSON.parse(raw) as PersistedCompanionState;
      if (!parsed.tama) parsed.tama = defaultTamagotchi();
      if (!parsed.xp) parsed.xp = { total: 0, lastSeenTokensIn: 0, lastSeenTokensOut: 0 };
      if (parsed.hidden === undefined) parsed.hidden = false;
      if (!parsed.pomodoro) parsed.pomodoro = defaultPomodoro();
      parsed.tama.lastDecayAt = Date.now();
      // Fresh session: Tama always starts awake and alive.
      // Transient states (sleeping, death, sickness, timers) never persist.
      parsed.tama.sleeping = false;
      parsed.tama.isDead = false;
      parsed.tama.sick = false;
      parsed.tama.sickSince = null;
      parsed.tama.hungerZeroSince = null;
      parsed.tama.happinessZeroSince = null;
      if (parsed.tama.hunger < 10) parsed.tama.hunger = 10;
      if (parsed.tama.happiness < 10) parsed.tama.happiness = 10;
      if (parsed.tama.poopPiles > 4) parsed.tama.poopPiles = 4;
      if (parsed.pomodoro.phase !== "idle") {
        parsed.pomodoro.phase = "idle";
        parsed.pomodoro.startedAt = null;
      }
      return parsed;
    } catch {
      // corrupted; start fresh
    }
  }
  return {
    companion: null,
    xp: { total: 0, lastSeenTokensIn: 0, lastSeenTokensOut: 0 },
    tama: defaultTamagotchi(),
    hidden: false,
    pomodoro: defaultPomodoro(),
  };
}

export function saveCompanionState(state: PersistedCompanionState): void {
  if (!existsSync(DATA_DIR)) {
    mkdirSync(DATA_DIR, { recursive: true });
  }
  writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
}
