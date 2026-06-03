import { useEffect, useState } from "react";

export type ShimmerDirection = "left-to-right" | "right-to-left";

export interface ShimmerState {
  glimmerIndex: number;
  direction: ShimmerDirection;
}

/**
 * Hook that computes a moving shimmer index over time.
 *
 * glimmerIndex sweeps across the text at intervalMs speed,
 * reversing direction based on mode. Pauses when `paused` is true.
 */
export function useShimmerAnimation(
  textLength: number,
  intervalMs: number = 50,
  direction: ShimmerDirection = "left-to-right",
  paused: boolean = false,
): ShimmerState {
  const [step, setStep] = useState(0);

  useEffect(() => {
    if (paused || textLength === 0) return;
    const interval = setInterval(() => {
      setStep((s) => s + 1);
    }, intervalMs);
    return () => clearInterval(interval);
  }, [textLength, intervalMs, paused]);

  const maxIndex = textLength > 0 ? textLength + 2 : 0;
  const raw = step % (maxIndex * 2);
  const glimmerIndex =
    raw < maxIndex
      ? raw
      : maxIndex * 2 - raw;

  return { glimmerIndex, direction };
}
