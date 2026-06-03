import React, { type ReactNode, useRef } from "react";
import { Box, ScrollBox } from "../../ink/kira-ink.js";
import type { ScrollBoxHandle } from "../../ink/components/ScrollBox.js";

interface FullscreenLayoutProps {
  header: ReactNode;
  scrollable: ReactNode;
  bottom: ReactNode;
  terminalRows: number;
}

export function FullscreenLayout({
  header,
  scrollable,
  bottom,
  terminalRows,
}: FullscreenLayoutProps) {
  const scrollRef = useRef<ScrollBoxHandle>(null);

  return (
    <Box flexDirection="column" height={terminalRows} width="100%" overflow="hidden">
      {header}
      <ScrollBox ref={scrollRef} flexGrow={1} flexDirection="column" stickyScroll={true}>
        {scrollable}
      </ScrollBox>
      <Box flexDirection="column" flexShrink={0} width="100%">
        {bottom}
      </Box>
    </Box>
  );
}
