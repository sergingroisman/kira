import React from 'react';
import { Box, Text } from 'ink';
import gradient from 'gradient-string';

export const LogoGradient: React.FC = () => {
  const logoText = `
   ╭─────────────╮
  │  ● ● ● ● ●  │
  │  ●   ●   ●  │
  │  ●    ●    │
  │      ●      │
   ╰─────────────╯
  `;

  const gradientText = gradient('cyan', 'magenta', 'yellow')(logoText);

  return (
    <Box flexDirection="column" alignItems="center">
      <Text>{gradientText}</Text>
      <Text bold color="magenta">
        KIRA
      </Text>
    </Box>
  );
};

export default LogoGradient;
