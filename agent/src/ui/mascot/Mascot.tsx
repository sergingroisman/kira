import React, { useEffect, useState, useMemo, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { useAnimationClock } from "../components/AnimationClock.js";
import { renderSprite, spriteFrameCount } from "./sprites.js";
import { emotionEyeChar, EMOTION_COLORS } from "./expressions.js";
import { RARITY_COLORS } from "./types.js";
import type { MascotConfig, Emotion } from "./types.js";

export interface MascotProps {
  config: MascotConfig;
  emotion?: Emotion;
  walking?: boolean;
}

const TICK_MS = 500;
const IDLE_SEQ = [0, 0, 0, 1, 0, 0, -1, 0, 0, 2, 0, 0, 1, 0, 0];

const SLEEP_FRAME_IDX: Record<string, number> = {
  duck: 7,
  cat: 7,
  rabbit: 7,
};

const WALK_FRAMES: Record<string, [number, number]> = {
  duck: [3, 4],
  cat: [3, 4],
  rabbit: [3, 4],
};

export const Mascot: React.FC<MascotProps> = ({ config, emotion = "neutral", walking = false }) => {
  const { paused } = useAnimationClock();
  const [tick, setTick] = useState(0);
  const [walkDir, setWalkDir] = useState(0);
  const frozen = emotion === "dead" || emotion === "sleepy";
  const lastDirChange = useRef(Date.now());

  useEffect(() => {
    if (paused || frozen) return;
    const timer = setInterval(() => {
      setTick((t) => t + 1);
      const now = Date.now();
      if (walking && now - lastDirChange.current > 2000) {
        lastDirChange.current = now;
        setWalkDir((d) => (d + 1) % 2);
      }
    }, TICK_MS);
    return () => clearInterval(timer);
  }, [paused, frozen, walking]);

  const frameCount = useMemo(() => spriteFrameCount(config.species), [config.species]);

  const frameIdx = useMemo(() => {
    if (frozen) {
      return SLEEP_FRAME_IDX[config.species] ?? Math.min(frameCount - 1, 3);
    }
    if (walking) {
      const [l, r] = WALK_FRAMES[config.species] ?? [0, 1];
      const seq = tick % 2 === 0 ? l : r;
      return seq % frameCount;
    }
    const raw = IDLE_SEQ[tick % IDLE_SEQ.length]!;
    if (raw < 0) return 0;
    return raw % frameCount;
  }, [tick, frameCount, frozen, walking, config.species]);

  const eye = useMemo(() => emotionEyeChar(emotion, config.eye), [emotion, config.eye]);

  const bobOffset = useMemo(() => {
    if (frozen) return 1;
    if (walking) return walkDir * 3;
    const p = (tick * 0.15) % (Math.PI * 2);
    return Math.round(Math.sin(p) * 2);
  }, [tick, frozen, walking, walkDir]);

  const isBlink = tick > 0 && IDLE_SEQ[tick % IDLE_SEQ.length] === -1;

  const lines = useMemo(() => {
    const raw = renderSprite(config.species, frameIdx, eye);
    return raw.filter((l) => l.trim().length > 0);
  }, [config.species, frameIdx, eye]);

  const color = emotion !== "neutral" && EMOTION_COLORS[emotion]
    ? EMOTION_COLORS[emotion]
    : RARITY_COLORS[config.rarity];

  return (
    <Box flexDirection="column" marginLeft={bobOffset + 1}>
      {lines.map((line, i) => (
        <Text key={i} color={color} dimColor={frozen}>
          {isBlink && !frozen ? line.replaceAll(eye, "-") : line}
        </Text>
      ))}
    </Box>
  );
};
