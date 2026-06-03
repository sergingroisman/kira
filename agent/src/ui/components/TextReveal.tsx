import React, { useEffect, useRef, useState } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { useTheme } from "./ThemeProvider.js";

export interface TextRevealProps {
  text: string;
  color?: string;
  /** Duration of the transition in ms. */
  duration?: number;
}

/**
 * TextReveal — crossfade transition between old and new text.
 *
 * When `text` changes, the old text fades out while the new text fades in.
 * Uses Ink's `Text` color prop to simulate opacity.
 */
export const TextReveal: React.FC<TextRevealProps> = ({
  text,
  color,
  duration = 300,
}) => {
  const { colors } = useTheme();
  const [phase, setPhase] = useState<"stable" | "exiting" | "entering">("stable");
  const prevText = useRef(text);
  const [displayText, setDisplayText] = useState(text);
  const [progress, setProgress] = useState(1);

  useEffect(() => {
    if (text === prevText.current) return;
    const outgoing = prevText.current;
    prevText.current = text;
    const steps = Math.max(4, Math.floor(duration / 50));
    let step = 0;

    setPhase("exiting");
    setDisplayText(outgoing);

    const interval = setInterval(() => {
      step++;
      const t = step / steps;
      if (phase === "exiting") {
        setProgress(1 - t);
        if (t >= 1) {
          setPhase("entering");
          setDisplayText(text);
          setProgress(0);
          step = 0;
        }
      } else {
        setProgress(t);
        if (t >= 1) {
          setPhase("stable");
          setProgress(1);
          clearInterval(interval);
        }
      }
    }, 50);

    return () => clearInterval(interval);
  }, [text, duration, phase]);

  // For stable state, just render the text directly
  if (phase === "stable") {
    return <Text color={color ?? colors.text}>{text}</Text>;
  }

  // During transition, use a dimmer color to simulate opacity
  const dimColor = color ?? colors.text;
  const faded = phase === "exiting" ? colors.textTertiary : dimColor;

  return (
    <Box>
      <Text color={faded}>{displayText}</Text>
    </Box>
  );
};

export default TextReveal;
