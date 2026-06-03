import React from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";

export interface NewMessagesPillProps {
  count: number;
  onJump: () => void;
}

export const NewMessagesPill: React.FC<NewMessagesPillProps> = ({
  count,
  onJump,
}) => {
  const { colors } = useTheme();

  if (count <= 0) return null;

  return (
    <Box marginTop={1} justifyContent="center">
      <Box
        borderStyle="round"
        borderColor={colors.kiraBlue}
        paddingX={1}
        paddingY={0}
      >
        <Text color={colors.kiraBlue}>
          {"\u2193"} {count} new message{count > 1 ? "s" : ""}
        </Text>
      </Box>
    </Box>
  );
};

export default NewMessagesPill;
