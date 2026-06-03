export type ThemeMode = "dark" | "light" | "dark-ansi" | "dracula" | "catppuccin" | "nord" | "tokyonight" | "gruvbox" | "solarized";

export interface ThemeColors {
  kiraRed: string;
  kiraRedShimmer: string;
  kiraBlue: string;
  kiraBlueShimmer: string;
  kiraPink: string;
  kiraOrange: string;
  kiraOrangeShimmer: string;
  kiraGreen: string;
  kiraGreenShimmer: string;
  kiraShimmer: string;

  success: string;
  successShimmer: string;
  error: string;
  errorShimmer: string;
  warning: string;
  warningShimmer: string;
  info: string;
  infoShimmer: string;
  permission: string;

  text: string;
  textSecondary: string;
  textTertiary: string;
  border: string;
  bg: string;
  inputBg: string;
  userMessageBg: string;
  assistantMessageBg: string;
  selectionBg: string;

  agentRed: string;
  agentBlue: string;
  agentGreen: string;
  agentYellow: string;
  agentPurple: string;
  agentOrange: string;
  agentPink: string;
  agentCyan: string;

  logoGradientK: string[];
  logoGradientR: string[];
}

const DARK: ThemeColors = {
  kiraRed: "#FF1A1A",
  kiraRedShimmer: "#FF4444",
  kiraBlue: "#87CEEB",
  kiraBlueShimmer: "#B8E8FF",
  kiraPink: "#E879A8",
  kiraOrange: "#E8963E",
  kiraOrangeShimmer: "#F0B060",
  kiraGreen: "#10B981",
  kiraGreenShimmer: "#34D399",
  kiraShimmer: "#B8E8FF",

  success: "#2E7D32",
  successShimmer: "#4CAF50",
  error: "#FF1A1A",
  errorShimmer: "#FF4444",
  warning: "#B8860B",
  warningShimmer: "#D4A017",
  info: "#9E9E9E",
  infoShimmer: "#BDBDBD",
  permission: "#5C6BC0",

  text: "#C8C8C8",
  textSecondary: "#777777",
  textTertiary: "#444444",
  border: "#1A1A1A",
  bg: "#050505",
  inputBg: "#0A0A0A",
  userMessageBg: "#0D0D0D",
  assistantMessageBg: "#080808",
  selectionBg: "#1A1A2E",

  agentRed: "#DC143C",
  agentBlue: "#2090F5",
  agentGreen: "#30D158",
  agentYellow: "#B8860B",
  agentPurple: "#A753AE",
  agentOrange: "#E8963E",
  agentPink: "#E879A8",
  agentCyan: "#4ECDC4",

  logoGradientK: [
    "#F5EDE0", "#E0D5C5", "#C8BCA8", "#A89880", "#787060", "#484030", "#201810",
  ],
  logoGradientR: [
    "#FF4444", "#E83030", "#D02020", "#B81010", "#A00000", "#680000", "#300000",
  ],
};

const LIGHT: ThemeColors = {
  kiraRed: "#C62828",
  kiraRedShimmer: "#E53935",
  kiraBlue: "#4A90D9",
  kiraBlueShimmer: "#64B5F6",
  kiraPink: "#C2185B",
  kiraOrange: "#E65100",
  kiraOrangeShimmer: "#F57C00",
  kiraGreen: "#2E7D32",
  kiraGreenShimmer: "#43A047",
  kiraShimmer: "#87CEEB",

  success: "#2E7D32",
  successShimmer: "#43A047",
  error: "#C62828",
  errorShimmer: "#E53935",
  warning: "#F57F17",
  warningShimmer: "#F9A825",
  info: "#757575",
  infoShimmer: "#9E9E9E",
  permission: "#5C6BC0",

  text: "#212121",
  textSecondary: "#757575",
  textTertiary: "#9E9E9E",
  border: "#E0E0E0",
  bg: "#FAFAFA",
  inputBg: "#FFFFFF",
  userMessageBg: "#F5F5F5",
  assistantMessageBg: "#FFFFFF",
  selectionBg: "#BBDEFB",

  agentRed: "#C62828",
  agentBlue: "#4A90D9",
  agentGreen: "#2E7D32",
  agentYellow: "#F57F17",
  agentPurple: "#5C6BC0",
  agentOrange: "#E65100",
  agentPink: "#C2185B",
  agentCyan: "#00838F",

  logoGradientK: [
    "#4A90D9", "#4A90D9", "#66BB6A", "#C2185B", "#C2185B", "#C2185B", "#C2185B",
  ],
  logoGradientR: [
    "#C62828", "#B71C1C", "#A31414", "#C62828", "#B71C1C", "#C62828", "#B71C1C",
  ],
};

const DARK_ANSI: ThemeColors = {
  kiraRed: "redBright",
  kiraRedShimmer: "redBright",
  kiraBlue: "cyanBright",
  kiraBlueShimmer: "cyanBright",
  kiraPink: "magentaBright",
  kiraOrange: "yellowBright",
  kiraOrangeShimmer: "yellowBright",
  kiraGreen: "greenBright",
  kiraGreenShimmer: "greenBright",
  kiraShimmer: "cyanBright",

  success: "greenBright",
  successShimmer: "greenBright",
  error: "redBright",
  errorShimmer: "redBright",
  warning: "yellowBright",
  warningShimmer: "yellowBright",
  info: "white",
  infoShimmer: "white",
  permission: "blueBright",

  text: "white",
  textSecondary: "gray",
  textTertiary: "gray",
  border: "gray",
  bg: "black",
  inputBg: "black",
  userMessageBg: "black",
  assistantMessageBg: "black",
  selectionBg: "cyan",

  agentRed: "redBright",
  agentBlue: "cyanBright",
  agentGreen: "greenBright",
  agentYellow: "yellowBright",
  agentPurple: "magentaBright",
  agentOrange: "yellowBright",
  agentPink: "magentaBright",
  agentCyan: "cyanBright",

  logoGradientK: [
    "white", "white", "gray", "gray", "gray", "gray", "gray",
  ],
  logoGradientR: [
    "redBright", "redBright", "red", "red", "red", "red", "red",
  ],
};

import { EXTRA_THEMES } from "./themes.js";

function mergeTheme(base: ThemeColors, partial: Record<string, any>): ThemeColors {
  const result = { ...base };
  for (const key of Object.keys(partial) as (keyof ThemeColors)[]) {
    const val = partial[key];
    if (Array.isArray(val)) {
      (result as any)[key] = [...val];
    } else if (typeof val === "string") {
      (result as any)[key] = val;
    }
  }
  return result;
}

const THEMES: Record<string, ThemeColors> = {
  dark: DARK,
  light: LIGHT,
  "dark-ansi": DARK_ANSI,
  dracula: mergeTheme(DARK, EXTRA_THEMES.dracula),
  catppuccin: mergeTheme(DARK, EXTRA_THEMES.catppuccin),
  nord: mergeTheme(DARK, EXTRA_THEMES.nord),
  tokyonight: mergeTheme(DARK, EXTRA_THEMES.tokyonight),
  gruvbox: mergeTheme(DARK, EXTRA_THEMES.gruvbox),
  solarized: mergeTheme(DARK, EXTRA_THEMES.solarized),
};

import * as fs from "fs";
import * as path from "path";
import * as os from "os";

const CONFIG_DIR = path.join(os.homedir(), ".kira");
const CONFIG_FILE = path.join(CONFIG_DIR, "config.json");

interface KiraConfig {
  themeMode?: ThemeMode;
  motionReduced?: boolean;
}

let cachedMotionReduced: boolean | undefined;

export function loadMotionReduced(): boolean {
  if (cachedMotionReduced !== undefined) return cachedMotionReduced;
  try {
    if (fs.existsSync(CONFIG_FILE)) {
      const config: KiraConfig = JSON.parse(fs.readFileSync(CONFIG_FILE, "utf-8"));
      cachedMotionReduced = config.motionReduced ?? false;
      return cachedMotionReduced;
    }
  } catch {
    // ignore
  }
  return false;
}

export function saveMotionReduced(reduced: boolean): void {
  cachedMotionReduced = reduced;
  try {
    if (!fs.existsSync(CONFIG_DIR)) {
      fs.mkdirSync(CONFIG_DIR, { recursive: true });
    }
    const config: KiraConfig = fs.existsSync(CONFIG_FILE)
      ? JSON.parse(fs.readFileSync(CONFIG_FILE, "utf-8"))
      : {};
    config.motionReduced = reduced;
    fs.writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2), "utf-8");
  } catch {
    // ignore
  }
}

export function loadThemeMode(): ThemeMode {
  try {
    if (fs.existsSync(CONFIG_FILE)) {
      const config: KiraConfig = JSON.parse(
        fs.readFileSync(CONFIG_FILE, "utf-8"),
      );
      if (config.themeMode && config.themeMode in THEMES) {
        return config.themeMode;
      }
    }
  } catch {
    // ignore
  }
  return "dark";
}

export function saveThemeMode(mode: ThemeMode): void {
  try {
    if (!fs.existsSync(CONFIG_DIR)) {
      fs.mkdirSync(CONFIG_DIR, { recursive: true });
    }
    const config: KiraConfig = fs.existsSync(CONFIG_FILE)
      ? JSON.parse(fs.readFileSync(CONFIG_FILE, "utf-8"))
      : {};
    config.themeMode = mode;
    fs.writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2), "utf-8");
  } catch {
    // ignore
  }
}

export function detectSystemTheme(): ThemeMode {
  const colorfgbg = process.env.COLORFGBG;
  if (colorfgbg) {
    const parts = colorfgbg.split(";");
    const bg = parts[1];
    if (bg === "15" || bg === "7") {
      return "light";
    }
  }
  return "dark";
}

export { THEMES, mergeTheme };
export type { ThemeColors };
export default DARK;
