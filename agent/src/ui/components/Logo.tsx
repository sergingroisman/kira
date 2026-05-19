import React from 'react';
import { Box, Text } from 'ink';

interface LogoProps {
  size?: 'small' | 'medium' | 'large';
  color?: string;
  showText?: boolean;
}

export const Logo: React.FC<LogoProps> = ({ 
  size = 'medium', 
  color = '#8B5CF6',
  showText = true 
}) => {
  const sizes = {
    small: { width: 20, height: 10 },
    medium: { width: 40, height: 20 },
    large: { width: 60, height: 30 }
  };

  const currentSize = sizes[size];

  return (
    <Box flexDirection="column" alignItems="center">
      {/* Logo ASCII Art */}
      <Box flexDirection="column">
        <Text color={color}>
          {`
    ╭─────────────╮
   ╱               ╲
  │    ████████     │
  │   ██      ██    │
  │   ██  ██  ██    │
  │   ██  ██  ██    │
  │   ██      ██    │
  │    ████████     │
  │       ▄▄        │
  │   ▄▄▄    ▄▄    │
  │  ████    ████   │
  │   █████████     │
   ╲               ╱
    ╰─────────────╯
          `}
        </Text>
      </Box>
      
      {showText && (
        <Box marginTop={1}>
          <Text bold color={color}>
            KIRA
          </Text>
        </Box>
      )}
    </Box>
  );
};

export default Logo;
