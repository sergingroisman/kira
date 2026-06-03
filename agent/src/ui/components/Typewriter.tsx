import React, { useEffect, useRef, useState } from "react";
import { Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";

export interface TypewriterProps {
  text: string;
  color?: string;
  /** Min/max delay between characters in ms. */
  minDelay?: number;
  maxDelay?: number;
  /** Callback when typing completes. */
  onComplete?: () => void;
  /** If true, render instantly (skip animation). */
  instant?: boolean;
}

/**
 * Typewriter — renders text character by character with randomized delays.
 *
 * Use for welcome messages, splash screen text, or any "being written" effect.
 */
export const Typewriter: React.FC<TypewriterProps> = ({
  text,
  color,
  minDelay = 30,
  maxDelay = 120,
  onComplete,
  instant = false,
}) => {
  const { colors } = useTheme();
  const [visibleCount, setVisibleCount] = useState(instant ? text.length : 0);
  const indexRef = useRef(0);
  const completeCalled = useRef(false);

  useEffect(() => {
    if (instant) {
      setVisibleCount(text.length);
      onComplete?.();
      return;
    }

    indexRef.current = 0;
    completeCalled.current = false;
    setVisibleCount(0);

    const tick = () => {
      if (indexRef.current >= text.length) {
        if (!completeCalled.current) {
          completeCalled.current = true;
          onComplete?.();
        }
        return;
      }
      indexRef.current++;
      setVisibleCount(indexRef.current);
      const delay = minDelay + Math.random() * (maxDelay - minDelay);
      timer = setTimeout(tick, delay);
    };

    let timer = setTimeout(tick, 200);

    return () => clearTimeout(timer);
  }, [text, minDelay, maxDelay, instant, onComplete]);

  const visible = text.slice(0, visibleCount);

  return (
    <Text color={color ?? colors.text}>
      {visible}
      {visibleCount < text.length && <Text color={colors.kiraBlue}>{"\u258C"}</Text>}
    </Text>
  );
};

export default Typewriter;
