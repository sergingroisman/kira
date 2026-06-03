import React, { useEffect, useRef, useState } from "react";
import { Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";

export interface AnimatedNumberProps {
  value: number;
  color?: string;
  format?: boolean;
  decimals?: number;
}

/**
 * AnimatedNumber — smoothly counts up/down to the target value.
 *
 * Uses spring-like interpolation: jumps 20% of the gap each tick (16ms).
 * Formats large numbers with compact notation (1.2k, 3.4M).
 */
export const AnimatedNumber: React.FC<AnimatedNumberProps> = ({
  value,
  color,
  format = true,
  decimals = 1,
}) => {
  const { colors } = useTheme();
  const [display, setDisplay] = useState(value);
  const animFrame = useRef<ReturnType<typeof setInterval> | null>(null);
  const lastTarget = useRef(value);

  useEffect(() => {
    lastTarget.current = value;
    if (animFrame.current) return;

    animFrame.current = setInterval(() => {
      setDisplay((prev) => {
        const target = lastTarget.current;
        if (Math.abs(prev - target) < 0.5) return target;
        return prev + (target - prev) * 0.25;
      });
    }, 16);

    return () => {
      if (animFrame.current) {
        clearInterval(animFrame.current);
        animFrame.current = null;
      }
    };
  }, [value]);

  useEffect(() => {
    setDisplay(value);
  }, [value]);

  const fmt = format ? compactNumber(Math.round(display)) : display.toFixed(decimals);

  return <Text color={color ?? colors.text}>{fmt}</Text>;
};

function compactNumber(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

export default AnimatedNumber;
