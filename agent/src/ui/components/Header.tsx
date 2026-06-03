import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";

export interface HeaderProps {
  coreUp: boolean | null;
  coreVersion: string | null;
  tier: string;
  mode: "plan" | "yolo";
  workingDir: string;
  rightSlot?: React.ReactNode;
}

export const Header: React.FC<HeaderProps> = ({
  coreUp,
  coreVersion,
  tier,
  mode,
  workingDir,
  rightSlot,
}) => {
  const statusColor = coreUp === null ? "gray" : coreUp ? "#4ADE80" : "#DC143C";
  const statusText =
    coreUp === null ? "connecting\u2026" : coreUp ? `v${coreVersion}` : "disconnected";

  const modeColor = mode === "yolo" ? "#DC143C" : "#555555";
  const tierColor =
    tier === "heavy" ? "#E879A8" : tier === "light" ? "#2E7D32" : "#87CEEB";

  return (
    <Box flexDirection="column" flexShrink={0}>
      <Box height={1} alignItems="center" paddingLeft={1} paddingRight={1}>
        <Text color={statusColor}>{statusText}</Text>

        <Box marginLeft={2}>
          <Text color={modeColor}>{mode.toUpperCase()}</Text>
        </Box>

        <Text color="#444444"> {"\u00B7"} </Text>

        <Text color={tierColor}>{tier}</Text>

        <Text color="#444444"> {"\u00B7"} </Text>

        <Text color="#777777">{shortPath(workingDir)}</Text>

        <Box flexGrow={1} />

        {rightSlot}
      </Box>

      <Text color="#222222">{"\u2500".repeat(2)}</Text>
    </Box>
  );
};

function shortPath(dir: string): string {
  const parts = dir.split("/").filter(Boolean);
  if (parts.length <= 2) return dir;
  return `~/${parts.slice(-2).join("/")}`;
}

export default Header;
