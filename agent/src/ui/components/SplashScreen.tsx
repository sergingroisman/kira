import React, { useEffect, useMemo, useState } from "react";
import { Box, Text, useInput, useStdout } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";
import { figletKira, interpolatePalette, type FigletFont } from "../art.js";
import { Typewriter } from "./Typewriter.js";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

const PRESS_CURSOR = ["\u2591", " ", "\u2591", " "];

const CONFIG_DIR = path.join(os.homedir(), ".kira");
const CONFIG_FILE = path.join(CONFIG_DIR, "config.json");

interface KiraConfig {
  splashSeen?: boolean;
}

function loadConfig(): KiraConfig {
  try {
    if (fs.existsSync(CONFIG_FILE)) {
      return JSON.parse(fs.readFileSync(CONFIG_FILE, "utf-8"));
    }
  } catch {
    /* ignore */
  }
  return {};
}

function saveConfig(config: KiraConfig): void {
  try {
    if (!fs.existsSync(CONFIG_DIR)) {
      fs.mkdirSync(CONFIG_DIR, { recursive: true });
    }
    fs.writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2), "utf-8");
  } catch {
    /* ignore */
  }
}

export function hasSeenSplash(): boolean {
  return loadConfig().splashSeen === true;
}

export function markSplashSeen(): void {
  const config = loadConfig();
  config.splashSeen = true;
  saveConfig(config);
}

export interface SplashScreenProps {
  coreUp: boolean | null;
  coreVersion: string | null;
  tier: string;
  mode: "plan" | "yolo";
  workingDir: string;
  onDismiss: () => void;
  figletFont?: FigletFont;
}

export const SplashScreen: React.FC<SplashScreenProps> = ({
  coreUp,
  coreVersion,
  tier,
  mode,
  workingDir,
  onDismiss,
  figletFont,
}) => {
  const { stdout } = useStdout();
  const { colors } = useTheme();
  const termWidth = stdout?.columns ?? 80;
  const [cursorIdx, setCursorIdx] = useState(0);

  // Generate art and row colors (warm tone for splash)
  const splashArt = useMemo(() => figletKira(figletFont), [figletFont]);
  const rowColors = useMemo(
    () => interpolatePalette(colors.logoGradientR, splashArt.length),
    [colors.logoGradientR, splashArt.length],
  );

  const statusColor = coreUp === null ? "gray" : coreUp ? "green" : "#DC143C";
  const statusText =
    coreUp === null ? "connecting\u2026" : coreUp ? `v${coreVersion}` : "disconnected";
  const modeColor = mode === "yolo" ? "#DC143C" : "#555555";
  const tierColor =
    tier === "heavy" ? "#E879A8" : tier === "light" ? "#2E7D32" : "#87CEEB";

  const dividerWidth = Math.max(20, termWidth - 2);

  useEffect(() => {
    const interval = setInterval(() => {
      setCursorIdx((i) => (i + 1) % PRESS_CURSOR.length);
    }, 500);
    return () => clearInterval(interval);
  }, []);

  useInput((ch, key) => {
    if (key.return) {
      markSplashSeen();
      onDismiss();
    }
  });

  return (
    <Box flexDirection="column">
      <Box flexDirection="column" alignItems="center" marginTop={1}>
        {splashArt.map((line, i) => (
          <Text key={i}>
            {line.split("").map((ch, c) => {
              if (ch === " ") {
                return <Text key={c}> </Text>;
              }
              return (
                <Text key={c} color={rowColors[i]}>
                  {ch}
                </Text>
              );
            })}
          </Text>
        ))}
      </Box>

      <Box marginTop={2}>
        <Text color="#1A1A1A">{"\u2501".repeat(dividerWidth)}</Text>
      </Box>

      <Box flexDirection="column" marginTop={1} marginLeft={2}>
        <Box>
          <Text color={statusColor}>{"\u25CF"} Kira Core</Text>
          <Text color="#555555"> {statusText}</Text>
        </Box>
        <Box>
          <Text color="#444444">mode=</Text>
          <Text bold color={modeColor}>
            {mode.toUpperCase()}
          </Text>
          <Text color="#333333">{"  \u00B7  "}</Text>
          <Text color="#444444">tier=</Text>
          <Text color={tierColor}>{tier}</Text>
        </Box>
        <Box>
          <Text color="#444444">dir=</Text>
          <Text color="#555555">{shortPath(workingDir)}</Text>
        </Box>
      </Box>

      <Box marginTop={1} marginLeft={2}>
        <Typewriter
          text="Your silent agentic harness awaits."
          color="#777777"
          minDelay={40}
          maxDelay={100}
        />
      </Box>

      <Box marginTop={2} marginLeft={2} flexDirection="column">
        <Box>
          <Text color="#666666">/agent writer</Text>
          <Text color="#444444">{"  "}create content in owned files</Text>
        </Box>
        <Box>
          <Text color="#666666">/plan draft</Text>
          <Text color="#444444">{"  "}generate a step-by-step plan</Text>
        </Box>
        <Box>
          <Text color="#666666">/tier {"<h|s|l>"}</Text>
          <Text color="#444444">{"  "}switch model tier</Text>
        </Box>
        <Box>
          <Text color="#666666">? </Text>
          <Text color="#444444">{"  "}show all shortcuts</Text>
        </Box>
      </Box>

      <Box marginTop={2} marginLeft={2}>
        <Text color="#444444">Press </Text>
        <Text color="#777777">Enter</Text>
        <Text color="#444444"> to begin  </Text>
        <Text color="#555555">{PRESS_CURSOR[cursorIdx]}</Text>
      </Box>
    </Box>
  );
};

function shortPath(dir: string): string {
  const parts = dir.split("/").filter(Boolean);
  if (parts.length <= 2) return dir;
  return `~/${parts.slice(-2).join("/")}`;
}

export default SplashScreen;
