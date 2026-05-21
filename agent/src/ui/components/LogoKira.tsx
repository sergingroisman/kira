/**
 * LogoKira — REPL header logo, Death Note themed.
 *
 * Compact 5-line ASCII face inside a red frame. Two parallel animations:
 *  - Eyes blink every ~2s (frame 4/4 → closed)
 *  - Cursor next to KIRA pulses every 500ms
 *
 * Drop-in for the previous `Logo` (sized to fit in the REPL header).
 */

import React, { useEffect, useState } from "react";
import { Box, Text } from "ink";

const EYES = ["◉ ◉", "◉ ◉", "◉ ◉", "◉ ◉", "─ ─"] as const;
const CURSOR = [" ", "▌"] as const;

interface LogoKiraProps {
  text?: string;
  /** Override frame color (default red — Death Note). */
  frameColor?: string;
  /** Override text color (default magenta). */
  textColor?: string;
}

export const LogoKira: React.FC<LogoKiraProps> = ({
  text = "KIRA",
  frameColor = "red",
  textColor = "magenta",
}) => {
  const [eye, setEye] = useState(0);
  const [cur, setCur] = useState(0);

  useEffect(() => {
    const blink = setInterval(() => {
      setEye((p) => (p + 1) % EYES.length);
    }, 500);
    const pulse = setInterval(() => {
      setCur((p) => (p + 1) % CURSOR.length);
    }, 500);
    return () => {
      clearInterval(blink);
      clearInterval(pulse);
    };
  }, []);

  return (
    <Box flexDirection="column" alignItems="center">
      <Text color={frameColor}>╭─────────╮</Text>
      <Text color={frameColor}>│  {EYES[eye]}  │</Text>
      <Text color={frameColor}>│    ▽    │</Text>
      <Text color={frameColor}>╰─────────╯</Text>
      <Box>
        <Text bold color={textColor}>
          {text}
        </Text>
        <Text color={frameColor}>{CURSOR[cur]}</Text>
      </Box>
    </Box>
  );
};

export default LogoKira;
