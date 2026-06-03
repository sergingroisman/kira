import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";

/** Unicode eighths blocks for sub-character precision. */
const EIGHTHS = [" ", "\u258F", "\u258E", "\u258D", "\u258C", "\u258B", "\u258A", "\u2589", "\u2588"];

export interface ProgressBarProps {
  value: number;
  max?: number;
  width?: number;
  color?: string;
  trackColor?: string;
  showLabel?: boolean;
}

export const ProgressBar: React.FC<ProgressBarProps> = ({
  value,
  max = 1,
  width = 20,
  color,
  trackColor,
  showLabel = true,
}) => {
  const { colors } = useTheme();
  const fillColor = color ?? colors.success;
  const emptyColor = trackColor ?? colors.textTertiary;

  const ratio = Math.min(1, Math.max(0, value / max));
  const totalSubCells = width * 8;
  const filledSub = Math.round(ratio * totalSubCells);
  const fullCells = Math.floor(filledSub / 8);
  const remainder = filledSub % 8;

  const full = EIGHTHS[8].repeat(fullCells);
  const partial = remainder > 0 ? EIGHTHS[remainder] : "";
  const empty = EIGHTHS[0].repeat(width - fullCells - (remainder > 0 ? 1 : 0));

  return (
    <Box>
      <Text color={fillColor}>{full}</Text>
      {partial && <Text color={fillColor}>{partial}</Text>}
      <Text color={emptyColor}>{empty}</Text>
      {showLabel && (
        <Text dimColor color={colors.textSecondary}> {Math.round(ratio * 100)}%</Text>
      )}
    </Box>
  );
};

export default ProgressBar;
