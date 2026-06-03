import React, { useCallback, useState } from "react";
import { Box, type BoxProps } from "../../ink/kira-ink.js";

export interface HeightRatchetProps extends BoxProps {
  /** When true, ratchet locks (always grows). When false, resets on content shrink. */
  engage?: boolean;
  children: React.ReactNode;
}

/**
 * HeightRatchet — remembers max height and never shrinks below it.
 *
 * Prevents layout jumps during streaming content.
 * When `engage` flips off, the ratchet resets on the next render.
 */
export const HeightRatchet: React.FC<HeightRatchetProps> = ({
  engage = true,
  children,
  ...boxProps
}) => {
  const [minHeight, setMinHeight] = useState(0);
  const prevEngage = React.useRef(engage);

  const measuredRef = useCallback(
    (node: BoxProps | null) => {
      if (!node) return;
      // Reset ratchet when engage transitions false->true
      if (!prevEngage.current && engage) {
        setMinHeight(0);
      }
      prevEngage.current = engage;

      if (engage) {
        // Store height from node props (Box has no direct measure API in Ink)
        // Instead, we approximate via children count or explicit height prop
        const h = (node as any).height ?? 0;
        if (h > minHeight) {
          setMinHeight(h);
        }
      }
    },
    [engage, minHeight],
  );

  return (
    <Box ref={measuredRef as any} minHeight={minHeight} {...boxProps}>
      {children}
    </Box>
  );
};

export default HeightRatchet;
