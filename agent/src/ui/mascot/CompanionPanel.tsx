import React, { useEffect, useState, useRef } from "react";
import { Box, Text, useStdout } from "../../ink/kira-ink.js";
import { Mascot } from "./Mascot.js";
import type { MascotConfig, CompanionSoul, XPState, TamagotchiState, Emotion } from "./types.js";
import { xpToLevel, levelToRarity, RARITY_STARS, RARITY_COLORS, progressBar, meterBar } from "./types.js";
import { tamaEmotion } from "./tamagotchi.js";

const OVERLAY_DURATION_MS = 3000;

export interface CompanionPanelProps {
  mascotConfig: MascotConfig;
  companionSoul: CompanionSoul | null;
  xpState: XPState;
  tamaState: TamagotchiState;
  sessionStart: number;
  overlayMessage: string | null;
  activeAgents: number;
  onOverlayClear: () => void;
}

function formatClock(ms: number): string {
  const totalSec = Math.floor(Math.max(0, ms) / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export const CompanionPanel: React.FC<CompanionPanelProps> = ({
  mascotConfig,
  companionSoul,
  xpState,
  tamaState,
  sessionStart,
  overlayMessage,
  activeAgents,
  onOverlayClear,
}) => {
  const { stdout } = useStdout();
  const termWidth = stdout?.columns ?? 80;
  const [elapsed, setElapsed] = useState(Date.now() - sessionStart);
  const overlayTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [visibleOverlay, setVisibleOverlay] = useState<string | null>(null);

  useEffect(() => {
    const timer = setInterval(() => {
      setElapsed(Date.now() - sessionStart);
    }, 1000);
    return () => clearInterval(timer);
  }, [sessionStart]);

  useEffect(() => {
    if (overlayMessage) {
      setVisibleOverlay(overlayMessage);
      if (overlayTimer.current) clearTimeout(overlayTimer.current);
      overlayTimer.current = setTimeout(() => {
        setVisibleOverlay(null);
        onOverlayClear();
      }, OVERLAY_DURATION_MS);
    }
    return () => {
      if (overlayTimer.current) clearTimeout(overlayTimer.current);
    };
  }, [overlayMessage, onOverlayClear]);

  const narrow = termWidth < 70;

  const { level, xpInLevel, xpNeeded } = xpToLevel(xpState.total);
  const dynamicRarity = levelToRarity(level);
  const dynamicConfig: MascotConfig = { ...mascotConfig, rarity: dynamicRarity };

  const emotion: Emotion = tamaEmotion(tamaState);
  const walking = activeAgents > 0 && !tamaState.sleeping && !tamaState.isDead && !tamaState.sick;

  if (narrow) return <Box />;

  const color = RARITY_COLORS[dynamicRarity];
  const stars = RARITY_STARS[dynamicRarity];

  const heart = "\u2665";
  const emptyHeart = "\u2661";
  const smile = "\u263A";
  const emptySmile = "\u2639";

  const hearts = meterBar(tamaState.hunger, heart, emptyHeart, 5);
  const smiles = meterBar(tamaState.happiness, smile, emptySmile, 5);

  return (
    <Box flexDirection="column" marginLeft={1} flexShrink={0}>
      {/* Sprite */}
      <Mascot config={dynamicConfig} emotion={emotion} walking={walking} />

      {/* Overlay message */}
      {visibleOverlay && (
        <Text color="#FBBF24">
          {visibleOverlay}
        </Text>
      )}

      {/* Name + Rarity + Level */}
      {companionSoul && (
        <Box marginTop={0}>
          <Text bold color={color}>
            {companionSoul.name}
          </Text>
        </Box>
      )}
      <Box>
        <Text color={color}>
          {stars} {dynamicRarity} {mascotConfig.species}
        </Text>
        <Text color="#666666">
          {" Lv."}{level}
        </Text>
      </Box>

      {/* XP bar */}
      <Box>
        <Text color="#888888">{progressBar(xpInLevel, xpNeeded, 10)}</Text>
        <Text color="#555555">
          {" "}{(xpState.total >= 1000 ? `${(xpState.total / 1000).toFixed(1)}K` : xpState.total)}
        </Text>
      </Box>

      {/* Tama bars */}
      <Box>
        <Text color="#EF4444">{hearts}</Text>
        <Text> </Text>
        <Text color="#4ADE80">{smiles}</Text>
      </Box>

      {/* Status tags */}
      <Box>
        {tamaState.sick && <Text color="#F97316"> [sick]</Text>}
        {tamaState.sleeping && <Text color="#888888"> [zzz]</Text>}
        {tamaState.isDead && <Text color="#666666"> [dead]</Text>}
        {tamaState.poopPiles > 0 && <Text color="#A0522D"> {"\uD83D\uDCA9"}{tamaState.poopPiles}</Text>}
      </Box>

      {/* Personality */}
      {companionSoul && (
        <Text color="#555555" dimColor>
          {"\u201C"}{companionSoul.personality.length > 40
            ? companionSoul.personality.slice(0, 40) + "\u2026"
            : companionSoul.personality}
        </Text>
      )}

      {/* Session clock */}
      <Box>
        <Text color="#444444">
          {"\u23F1 "}{formatClock(elapsed)}
        </Text>
      </Box>
    </Box>
  );
};

export default CompanionPanel;
