import React, { useEffect, useState, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import { Mascot } from "../mascot/Mascot.js";
import type { MascotConfig, CompanionSoul, TamagotchiState, Emotion } from "../mascot/types.js";
import { levelToRarity, RARITY_COLORS, POMODORO_COLORS, phaseLabel, meterBar } from "../mascot/types.js";
import { usePomodoro } from "./PomodoroProvider.js";
import { useAnimationClock } from "./AnimationClock.js";

const GLOW_DURATION_MS = 3000;
const SPARK_CHARS = ["\u2726", "\u2727", "\u2726", "\u2727"];

export interface BuddyChatProps {
  mascotConfig: MascotConfig;
  companionSoul: CompanionSoul;
  message: string | null;
  tamaState: TamagotchiState;
  level: number;
  emotion: Emotion;
  onMessageExpired: () => void;
  duration?: number;
}

export const BuddyChat: React.FC<BuddyChatProps> = ({
  mascotConfig,
  companionSoul,
  message,
  tamaState,
  level,
  emotion,
  onMessageExpired,
  duration = 6000,
}) => {
  const rarity = levelToRarity(level);
  const color = RARITY_COLORS[rarity];
  const [activeMessage, setActiveMessage] = useState<string | null>(null);
  const [fading, setFading] = useState(false);
  const messageTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const prevLevelRef = useRef(level);
  const [glowColor, setGlowColor] = useState<string | null>(null);
  const glowTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { paused } = useAnimationClock();

  useEffect(() => {
    if (level > prevLevelRef.current) {
      prevLevelRef.current = level;
      const isHigh = level >= 7;
      const nextGlow = isHigh ? "#F97316" : "#FBBF24";
      setGlowColor(nextGlow);
      if (glowTimer.current) clearTimeout(glowTimer.current);
      glowTimer.current = setTimeout(() => setGlowColor(null), GLOW_DURATION_MS);
    } else if (level < prevLevelRef.current) {
      prevLevelRef.current = level;
    }
    return () => {
      if (glowTimer.current) clearTimeout(glowTimer.current);
    };
  }, [level]);

  useEffect(() => {
    if (message) {
      setActiveMessage(message);
      setFading(false);
      if (messageTimer.current) clearTimeout(messageTimer.current);
      if (fadeTimer.current) clearTimeout(fadeTimer.current);

      const fadeDelay = Math.max(0, duration - 1200);
      fadeTimer.current = setTimeout(() => setFading(true), fadeDelay);
      messageTimer.current = setTimeout(() => {
        setActiveMessage(null);
        setFading(false);
        onMessageExpired();
      }, duration);
    }
    return () => {
      if (fadeTimer.current) clearTimeout(fadeTimer.current);
      if (messageTimer.current) clearTimeout(messageTimer.current);
    };
  }, [message, duration, onMessageExpired]);

  const [sparkTick, setSparkTick] = useState(0);
  useEffect(() => {
    if (paused || !glowColor) {
      setSparkTick(0);
      return;
    }
    const timer = setInterval(() => setSparkTick((t) => (t + 1) % SPARK_CHARS.length), 350);
    return () => clearInterval(timer);
  }, [paused, glowColor]);

  const pomodoro = usePomodoro();
  const pomodoroLabel = pomodoro.state.phase !== "idle"
    ? ` \u23F1 ${pomodoro.timer} [${phaseLabel(pomodoro.state.phase)}]`
    : "";

  const hearts = meterBar(tamaState.hunger, "\u2665", "\u2661", 5);
  const poopTag = tamaState.poopPiles > 0 ? ` \uD83D\uDCA9${tamaState.poopPiles}` : "";
  const sickTag = tamaState.sick ? " [sick]" : "";
  const sleepTag = tamaState.sleeping ? " [zzz]" : "";
  const deadTag = tamaState.isDead ? " [dead]" : "";

  const leftSpark = glowColor ? SPARK_CHARS[sparkTick]! : null;
  const rightSpark = glowColor ? SPARK_CHARS[(sparkTick + 2) % SPARK_CHARS.length]! : null;
  const bubbleBorderColor = glowColor ?? color;

  return (
    <Box flexDirection="column" alignItems="center">
      <Text>
        <Text color={color}>{companionSoul.name}</Text>
        <Text> </Text>
        <Text color="#888888">{mascotConfig.eye}</Text>
        <Text> </Text>
        <Text color={glowColor ?? "#666666"}>Lv.{level}</Text>
        <Text> </Text>
        <Text>{hearts}</Text>
        {poopTag ? <Text color="#A0522D">{poopTag}</Text> : null}
        {sickTag ? <Text color="#F97316">{sickTag}</Text> : null}
        {sleepTag ? <Text color="#888888">{sleepTag}</Text> : null}
        {deadTag ? <Text color="#666666">{deadTag}</Text> : null}
        {pomodoroLabel ? <Text color={POMODORO_COLORS[pomodoro.state.phase]}>{pomodoroLabel}</Text> : null}
      </Text>

      {activeMessage && (
        <Box>
          {glowColor && <Text color={glowColor}>{leftSpark} </Text>}
          <Box
            borderStyle="round"
            borderColor={bubbleBorderColor}
            paddingX={1}
            marginTop={0}
            marginBottom={0}
          >
            <Text dimColor={fading} color="#D4D4D4">
              <Text bold color={glowColor ?? color}>{companionSoul.name}:</Text>
              {" "}{"\u201C"}{activeMessage}{"\u201D"}
            </Text>
          </Box>
          {glowColor && <Text color={glowColor}> {rightSpark}</Text>}
        </Box>
      )}

      <Mascot config={mascotConfig} emotion={emotion} />
    </Box>
  );
};

export default BuddyChat;
