import React from "react";
import { Box, Text } from "ink";

interface StreamingOutputProps {
  tokens: string;
}

// Pure rendering component — no side effects.
// Parent (App.tsx) controls token batching via useRef + 16ms interval.
// Using <Text wrap="wrap"> ensures long responses wrap at terminal width.
export function StreamingOutput({ tokens }: StreamingOutputProps) {
  if (!tokens) return null;
  return (
    <Box flexDirection="column" marginBottom={1}>
      <Text color="gray" dimColor>Sergin</Text>
      <Text wrap="wrap">{tokens}</Text>
    </Box>
  );
}
