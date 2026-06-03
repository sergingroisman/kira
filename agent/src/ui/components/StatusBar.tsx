import React, { useEffect, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { usePomodoro } from "./PomodoroProvider.js";
import { POMODORO_COLORS, phaseLabel } from "../mascot/types.js";

const DOUBLE_TAP_WINDOW_MS = 800;

export interface StatusBarProps {
  tier: string;
  mode: "plan" | "yolo";
  agentCount: number;
  activeAgents: number;
  themeMode?: string;
  showExitHint?: boolean;
  onDismissExitHint?: () => void;
  newMessageCount?: number;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  tier,
  mode,
  agentCount,
  activeAgents,
  themeMode,
  showExitHint,
  onDismissExitHint,
  newMessageCount = 0,
}) => {
  const hintTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (showExitHint) {
      hintTimer.current = setTimeout(() => {
        onDismissExitHint?.();
        hintTimer.current = null;
      }, DOUBLE_TAP_WINDOW_MS + 200);
    }
    return () => {
      if (hintTimer.current) {
        clearTimeout(hintTimer.current);
        hintTimer.current = null;
      }
    };
  }, [showExitHint, onDismissExitHint]);

  const pomodoro = usePomodoro();
  const modeColor = mode === "yolo" ? "#DC143C" : "#555555";
  const tierColor =
    tier === "heavy" ? "#E879A8" : tier === "light" ? "#2E7D32" : "#87CEEB";

  return (
    <Box flexDirection="column" flexShrink={0}>
      <Box height={1} alignItems="center" paddingLeft={1} paddingRight={1}>
        <Text color={modeColor}>{mode.toUpperCase()}</Text>
        <Text color="#333333">{" \u00B7 "}</Text>
        <Text color={tierColor}>{tier}</Text>
        <Text color="#333333">{" \u00B7 "}</Text>
        <Text color="#555555">
          {agentCount}{activeAgents > 0 ? ` (${activeAgents})` : ""}
        </Text>
        {newMessageCount > 0 && (
          <>
            <Text color="#333333">{" \u00B7 "}</Text>
            <Text color="#87CEEB">{"\u2193"}{newMessageCount}</Text>
          </>
        )}
        {themeMode && (
          <>
            <Text color="#333333">{" \u00B7 "}</Text>
            <Text color="#555555">{themeMode}</Text>
          </>
        )}
        <Box flexGrow={1} />
        {pomodoro.state.phase !== "idle" && (
          <Text color={POMODORO_COLORS[pomodoro.state.phase]}>
            {"\u23F1 "}{pomodoro.timer}{" "}
            <Text color="#666666">{phaseLabel(pomodoro.state.phase)}</Text>
            <Text color="#444444">
              {" \u25CF".repeat(Math.min(pomodoro.state.completed, 5))}
              {pomodoro.state.completed > 5 ? `+${pomodoro.state.completed - 5}` : ""}
            </Text>
          </Text>
        )}
      </Box>
      {showExitHint && (
        <Box paddingLeft={1}>
          <Text color="#B8860B" dimColor>
            Press Ctrl+C again to exit
          </Text>
        </Box>
      )}
    </Box>
  );
};

export default StatusBar;
