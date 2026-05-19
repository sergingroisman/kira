import React from "react";
import { Box, Text } from "ink";

interface ToolCallPanelProps {
  toolName: string;
  args?: string;
  result?: string;
  status: "running" | "done" | "error";
}

const MAX_PREVIEW = 120;

function truncate(str: string): string {
  return str.length > MAX_PREVIEW ? str.slice(0, MAX_PREVIEW) + "…" : str;
}

export function ToolCallPanel({ toolName, args, result, status }: ToolCallPanelProps) {
  const statusColor = status === "running" ? "yellow" : status === "error" ? "red" : "green";
  const statusSymbol = status === "running" ? "⟳" : status === "error" ? "✗" : "✓";

  return (
    <Box
      flexDirection="column"
      borderStyle="single"
      borderColor={statusColor}
      paddingX={1}
      marginY={0}
    >
      <Box>
        <Text color={statusColor}>{statusSymbol} </Text>
        <Text bold color="cyan">{toolName}</Text>
        {args && <Text color="gray"> {truncate(args)}</Text>}
      </Box>
      {result && (
        <Text color={status === "error" ? "red" : "white"}>
          {truncate(result)}
        </Text>
      )}
    </Box>
  );
}
