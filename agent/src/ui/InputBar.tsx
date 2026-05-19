import React, { useState } from "react";
import { Box, Text, useInput } from "ink";

interface InputBarProps {
  onSubmit: (value: string) => void;
  disabled?: boolean;
}

export function InputBar({ onSubmit, disabled = false }: InputBarProps) {
  const [buffer, setBuffer] = useState("");

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
        return;
      }
      // Ignore control characters (Ctrl+C handled by exitOnCtrlC in render())
      if (!key.ctrl && !key.meta && input) {
        setBuffer(prev => prev + input);
      }
    },
    { isActive: !disabled }
  );

  return (
    <Box>
      <Text color={disabled ? "gray" : "green"} dimColor={disabled}>
        {disabled ? "·" : ">"}{" "}
      </Text>
      <Text dimColor={disabled}>{disabled ? "" : buffer}</Text>
      <Text color={disabled ? "gray" : "white"} dimColor>{disabled ? "" : "█"}</Text>
    </Box>
  );
}
