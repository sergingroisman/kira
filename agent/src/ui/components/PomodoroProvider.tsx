import React, {
  useEffect,
  useMemo,
  useRef,
  useContext,
  createContext,
} from "react";
import { useAnimationClock } from "./AnimationClock.js";
import type { PomodoroState } from "../mascot/types.js";
import {
  defaultPomodoro,
  pomodoroTick,
  formatTimer,
} from "../mascot/pomodoro.js";

export interface PomodoroValue {
  state: PomodoroState;
  timer: string;
  running: boolean;
}

const PomodoroContext = createContext<PomodoroValue>({
  state: defaultPomodoro(),
  timer: "00:00",
  running: false,
});

export function usePomodoro(): PomodoroValue {
  return useContext(PomodoroContext);
}

export interface PomodoroProviderProps {
  state: PomodoroState;
  onTick: (state: PomodoroState) => void;
  children: React.ReactNode;
}

export const PomodoroProvider: React.FC<PomodoroProviderProps> = ({
  state,
  onTick,
  children,
}) => {
  const { paused } = useAnimationClock();
  const stateRef = useRef(state);
  const onTickRef = useRef(onTick);
  stateRef.current = state;
  onTickRef.current = onTick;

  // Tick every 1s when active and not paused
  useEffect(() => {
    if (state.phase === "idle" || paused) return;
    const id = setInterval(() => {
      const next = pomodoroTick(stateRef.current, Date.now());
      stateRef.current = next;
      onTickRef.current(next);
    }, 1000);
    return () => clearInterval(id);
  }, [state.phase, paused]);

  const timer = useMemo(() => {
    if (state.phase === "idle") return "00:00";
    const remaining =
      state.phase === "focus"
        ? state.focusDuration - state.elapsed
        : state.phase === "short_break"
          ? state.shortBreakDuration - state.elapsed
          : state.longBreakDuration - state.elapsed;
    return formatTimer(Math.max(0, remaining));
  }, [state]);

  const running = state.phase !== "idle";

  const value = useMemo<PomodoroValue>(
    () => ({ state, timer, running }),
    [state, timer, running],
  );

  return (
    <PomodoroContext.Provider value={value}>
      {children}
    </PomodoroContext.Provider>
  );
};

export default PomodoroProvider;
