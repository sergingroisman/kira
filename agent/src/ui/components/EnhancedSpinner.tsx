import React, { useEffect, useMemo, useState, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { interpolateColor } from "../art.js";
import { useTheme } from "./ThemeProvider.js";

const FRAMES = ["\u280B", "", "\u2839", "\u2838", "\u283C", "\u2834", "", "\u2827", "\u2807", "\u280F"];

const STALL_TIMEOUT_MS = 3000;
const ERROR_RED = "#DC143C";

export interface EnhancedSpinnerProps {
  verb?: string;
  color?: string;
  /** Track response output length to detect stalls. */
  responseLength?: number;
  showElapsed?: boolean;
  startedAt?: number;
}

export const EnhancedSpinner: React.FC<EnhancedSpinnerProps> = ({
  verb = "Thinking\u2026",
  color,
  responseLength,
  showElapsed = false,
  startedAt,
}) => {
  const { colors } = useTheme();
  const [frameIdx, setFrameIdx] = useState(0);
  const [stalledIntensity, setStalledIntensity] = useState(0);
  const [elapsedStr, setElapsedStr] = useState("");
  const lastLength = useRef(responseLength ?? 0);
  const stallTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const elapsedTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const baseColor = color ?? colors.kiraBlue;

  // Spinner animation
  useEffect(() => {
    const interval = setInterval(() => {
      setFrameIdx((i) => (i + 1) % FRAMES.length);
    }, 80);
    return () => clearInterval(interval);
  }, []);

  // Stalled detection
  useEffect(() => {
    if (responseLength === undefined) return;
    if (responseLength !== lastLength.current) {
      lastLength.current = responseLength;
      setStalledIntensity(0);
      if (stallTimer.current) {
        clearInterval(stallTimer.current);
        stallTimer.current = null;
      }
      return;
    }
    if (!stallTimer.current) {
      stallTimer.current = setInterval(() => {
        setStalledIntensity((prev) => Math.min(1, prev + 0.1));
      }, STALL_TIMEOUT_MS / 10);
    }
    return () => {
      if (stallTimer.current) {
        clearInterval(stallTimer.current);
        stallTimer.current = null;
      }
    };
  }, [responseLength]);

  // Elapsed time
  useEffect(() => {
    if (!showElapsed || !startedAt) return;
    const tick = () => {
      const elapsed = Date.now() - startedAt;
      const secs = Math.floor(elapsed / 1000);
      const mins = Math.floor(secs / 60);
      const hrs = Math.floor(mins / 60);
      if (hrs > 0) setElapsedStr(`${hrs}h ${mins % 60}m`);
      else if (mins > 0) setElapsedStr(`${mins}m ${secs % 60}s`);
      else setElapsedStr(`${secs}s`);
    };
    tick();
    elapsedTimer.current = setInterval(tick, 1000);
    return () => {
      if (elapsedTimer.current) clearInterval(elapsedTimer.current);
    };
  }, [showElapsed, startedAt]);

  const spinnerColor = useMemo(() => {
    if (stalledIntensity > 0) {
      return interpolateColor(baseColor, ERROR_RED, stalledIntensity);
    }
    return baseColor;
  }, [baseColor, stalledIntensity]);

  const verbDisplay = stalledIntensity > 0.5 ? `${verb} (stalled)` : verb;

  return (
    <Box>
      <Text color={spinnerColor}>{FRAMES[frameIdx]} </Text>
      <Text color={stalledIntensity > 0.5 ? colors.error : colors.textSecondary}>
        {verbDisplay}
      </Text>
      {elapsedStr && (
        <Text dimColor color={colors.textTertiary}>
          {" "}{elapsedStr}
        </Text>
      )}
    </Box>
  );
};

export default EnhancedSpinner;
