import React, { useState, useEffect, useRef } from "react";
import { Box, Text, useInput, useApp } from "ink";

interface InputBarProps {
  onSubmit: (value: string) => void;
  disabled?: boolean;
}

const DOUBLE_TAP_WINDOW_MS = 800;

export function InputBar({ onSubmit, disabled = false }: InputBarProps) {
  const { exit } = useApp();
  const [buffer, setBuffer] = useState("");
  const [showExitHint, setShowExitHint] = useState(false);
  const lastCtrlCAt = useRef<number>(0);
  const hintTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear exit hint after timeout
  useEffect(() => {
    if (showExitHint) {
      hintTimer.current = setTimeout(() => {
        setShowExitHint(false);
        hintTimer.current = null;
      }, DOUBLE_TAP_WINDOW_MS + 200);
    }
    return () => {
      if (hintTimer.current) {
        clearTimeout(hintTimer.current);
        hintTimer.current = null;
      }
    };
  }, [showExitHint]);

  // useInput from ink uses useLayoutEffect internally — raw mode is set
  // synchronously during commit phase. Do NOT call process.stdin.setRawMode() directly.
  useInput(
    (input, key) => {
      if (key.return) {
        const trimmed = buffer.trim();
        if (trimmed) {
          onSubmit(trimmed);
          setBuffer("");
        }
        return;
      }
      if (key.backspace || key.delete) {
        setBuffer(prev => prev.slice(0, -1));
        return;
      }
      if (key.escape) {
        setBuffer("");
        setShowExitHint(false);
        return;
      }
      // Ctrl+C: double-tap to exit, single tap clears input
      if (key.ctrl && input === "c") {
        const now = Date.now();
        const elapsed = now - lastCtrlCAt.current;
        lastCtrlCAt.current = now;

        if (elapsed < DOUBLE_TAP_WINDOW_MS) {
          exit();
        } else {
          setBuffer("");
          setShowExitHint(true);
        }
        return;
      }
      // Ignore other control characters
      if (!key.ctrl && !key.meta && input) {
        setBuffer(prev => prev + input);
      }
    },
    { isActive: !disabled }
  );

  return (
    <Box flexDirection="column">
      <Box>
        <Text color={disabled ? "gray" : "green"} dimColor={disabled}>
          {disabled ? "·" : ">"}{" "}
        </Text>
        <Text dimColor={disabled}>{disabled ? "" : buffer}</Text>
        <Text color={disabled ? "gray" : "white"} dimColor>{disabled ? "" : "█"}</Text>
      </Box>
      {showExitHint && !disabled && (
        <Box marginLeft={2}>
          <Text color="yellow" dimColor>
            Press Ctrl+C again to exit
          </Text>
        </Box>
      )}
    </Box>
  );
}
