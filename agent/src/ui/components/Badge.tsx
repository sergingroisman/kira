/**
 * Badge — small colored label for mode, tier, status indicators.
 */
import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";

export interface BadgeProps {
  label: string;
  color?: string;
  dim?: boolean;
}

export const Badge: React.FC<BadgeProps> = ({
  label,
  color = "cyan",
  dim = false,
}) => {
  return (
    <Box>
      <Text color={color} dimColor={dim}>
        {" "}
        {label}{" "}
      </Text>
    </Box>
  );
};

export default Badge;
