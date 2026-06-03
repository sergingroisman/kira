import React, { useMemo } from "react";
import { Text } from "../../ink/kira-ink.js";
import { useShimmerAnimation, type ShimmerDirection } from "./ShimmerAnimation.js";

export interface ShimmerTextProps {
  text: string;
  baseColor?: string;
  shimmerColor?: string;
  intervalMs?: number;
  direction?: ShimmerDirection;
  paused?: boolean;
}

/**
 * Renders text with a shimmer sweep effect.
 *
 * A glimmerIndex sweeps across the text, coloring each character:
 * - characters at glimmerIndex ±1 get shimmerColor
 * - all others get baseColor
 * - spaces are rendered transparently
 */
export const ShimmerText: React.FC<ShimmerTextProps> = ({
  text,
  baseColor = "#777777",
  shimmerColor = "#C8C8C8",
  intervalMs = 50,
  direction = "left-to-right",
  paused = false,
}) => {
  const chars = useMemo(() => [...text], [text]);
  const { glimmerIndex } = useShimmerAnimation(
    chars.length,
    intervalMs,
    direction,
    paused,
  );

  return (
    <Text>
      {chars.map((ch, i) => {
        if (ch === " ") return <Text key={i}> </Text>;
        const dist = Math.abs(i - glimmerIndex);
        const isShimmer = dist <= 1;
        return (
          <Text key={i} color={isShimmer ? shimmerColor : baseColor}>
            {ch}
          </Text>
        );
      })}
    </Text>
  );
};

export default ShimmerText;
