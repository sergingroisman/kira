import type { PomodoroPhase, PomodoroState, TamagotchiState } from "./types.js";

export const DEFAULT_FOCUS_MS = 25 * 60 * 1000;
export const DEFAULT_SHORT_BREAK_MS = 5 * 60 * 1000;
export const DEFAULT_LONG_BREAK_MS = 15 * 60 * 1000;
export const DEFAULT_LONG_BREAK_INTERVAL = 4;

export function defaultPomodoro(): PomodoroState {
  return {
    phase: "idle",
    startedAt: null,
    elapsed: 0,
    completed: 0,
    focusDuration: DEFAULT_FOCUS_MS,
    shortBreakDuration: DEFAULT_SHORT_BREAK_MS,
    longBreakDuration: DEFAULT_LONG_BREAK_MS,
    longBreakInterval: DEFAULT_LONG_BREAK_INTERVAL,
  };
}

export function formatTimer(ms: number): string {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export function pomodoroTick(state: PomodoroState, now: number): PomodoroState {
  if (state.phase === "idle") return state;
  if (!state.startedAt) return state;

  const delta = Math.max(0, now - state.startedAt);
  const elapsed = state.elapsed + delta;
  const next = { ...state, elapsed, startedAt: now };

  if (next.phase === "focus" && elapsed >= state.focusDuration) {
    const completed = state.completed + 1;
    const isLong = completed % state.longBreakInterval === 0;
    return {
      ...next,
      phase: isLong ? "long_break" : "short_break",
      elapsed: 0,
      completed,
      startedAt: now,
    };
  }

  if (next.phase === "short_break" && elapsed >= state.shortBreakDuration) {
    return { ...next, phase: "idle", elapsed: 0, startedAt: null };
  }

  if (next.phase === "long_break" && elapsed >= state.longBreakDuration) {
    return { ...next, phase: "idle", elapsed: 0, startedAt: null };
  }

  return next;
}

export function startPomodoro(state: PomodoroState): PomodoroState {
  const now = Date.now();
  return {
    ...state,
    phase: "focus",
    startedAt: now,
    elapsed: 0,
  };
}

export function stopPomodoro(state: PomodoroState): PomodoroState {
  return {
    ...state,
    phase: "idle",
    startedAt: null,
    elapsed: 0,
  };
}

export function pausePomodoro(state: PomodoroState): PomodoroState {
  if (state.phase === "idle") return state;
  return {
    ...state,
    startedAt: null,
  };
}

export function resumePomodoro(state: PomodoroState): PomodoroState {
  if (state.phase === "idle") return state;
  if (state.startedAt) return state;
  return {
    ...state,
    startedAt: Date.now(),
  };
}

export function configurePomodoro(
  state: PomodoroState,
  focusMin: number,
  breakMin: number,
): PomodoroState {
  return {
    ...state,
    focusDuration: focusMin * 60 * 1000,
    shortBreakDuration: breakMin * 60 * 1000,
    longBreakDuration: Math.min(breakMin * 3, 30) * 60 * 1000,
  };
}

export function pomodoroAutoCare(tama: TamagotchiState): void {
  if (tama.isDead) return;
  tama.hunger = Math.min(100, tama.hunger + 30);
  tama.happiness = Math.min(100, tama.happiness + 30);
  tama.poopPiles = Math.max(0, tama.poopPiles - 1);
}
