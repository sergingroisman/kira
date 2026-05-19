import React, { useEffect, useState } from 'react';
import { Box, Text } from 'ink';

interface LogoAnimatedProps {
  text?: string;
}

export const LogoAnimated: React.FC<LogoAnimatedProps> = ({ text = 'KIRA' }) => {
  const [frame, setFrame] = useState(0);
  
  const frames = [
    '◐', '◓', '◑', '◒'
  ];

  useEffect(() => {
    const interval = setInterval(() => {
      setFrame((prev) => (prev + 1) % frames.length);
    }, 200);

    return () => clearInterval(interval);
  }, []);

  return (
    <Box flexDirection="column" alignItems="center">
      <Text bold color="magenta">
        {frames[frame]} {text} {frames[frame]}
      </Text>
      <Box marginTop={1} flexDirection="column" alignItems="center">
        <Text color="cyan">
          ╭─────────────╮
        </Text>
        <Text color="white">
          │  ■ ■ ■ ■ ■  │
        </Text>
        <Text color="white">
          │  ■  ■ ■  ■  │
        </Text>
        <Text color="white">
          │  ■    ■    │
        </Text>
        <Text color="cyan">
          ╰─────────────╯
        </Text>
      </Box>
    </Box>
  );
};

export default LogoAnimated;
