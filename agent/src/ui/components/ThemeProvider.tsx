/**
 * ThemeProvider — React context provider for Kira theme system.
 *
 * Wraps the app to provide theme colors to all components.
 * Supports /theme command to switch between dark, light, and dark-ansi modes.
 */
import React, { createContext, useContext, useState, useCallback, useMemo } from "react";
import { useInput } from "../../ink/kira-ink.js";
import {
  type ThemeMode,
  type ThemeColors,
  THEMES,
  loadThemeMode,
  saveThemeMode,
} from "../theme.js";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------
interface ThemeContextValue {
  mode: ThemeMode;
  colors: ThemeColors;
  setMode: (mode: ThemeMode) => void;
  cycleMode: () => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const MODES: ThemeMode[] = ["dark", "light", "dark-ansi"];

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------
export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error("useTheme must be used within <ThemeProvider>");
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------
interface ThemeProviderProps {
  children: React.ReactNode;
  initialMode?: ThemeMode;
}

export const ThemeProvider: React.FC<ThemeProviderProps> = ({
  children,
  initialMode,
}) => {
  const [mode, setModeState] = useState<ThemeMode>(
    initialMode ?? loadThemeMode(),
  );

  const colors = useMemo(() => THEMES[mode], [mode]);

  const setMode = useCallback((newMode: ThemeMode) => {
    setModeState(newMode);
    saveThemeMode(newMode);
  }, []);

  const cycleMode = useCallback(() => {
    const idx = MODES.indexOf(mode);
    const next = MODES[(idx + 1) % MODES.length];
    setMode(next);
  }, [mode, setMode]);

  const value = useMemo(
    () => ({ mode, colors, setMode, cycleMode }),
    [mode, colors, setMode, cycleMode],
  );

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
};

export default ThemeProvider;
