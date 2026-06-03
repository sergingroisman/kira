import React, { useEffect, useState, useRef } from "react";
import { Box, Text } from "../../ink/kira-ink.js";
import type { MascotConfig, CompanionSoul, TamagotchiState } from "./types.js";
import { meterBar, levelToRarity, RARITY_COLORS, POMODORO_COLORS, phaseLabel } from "./types.js";
import { usePomodoro } from "../components/PomodoroProvider.js";

export interface CompactCompanionProps {
  mascotConfig: MascotConfig;
  companionSoul: CompanionSoul;
  message: string | null;
  tamaState: TamagotchiState;
  level: number;
  onMessageExpired: () => void;
  duration?: number;
}

export const CompactCompanion: React.FC<CompactCompanionProps> = ({
  mascotConfig,
  companionSoul,
  message,
  tamaState,
  level,
  onMessageExpired,
  duration = 4000,
}) => {
  const rarity = levelToRarity(level);
  const color = RARITY_COLORS[rarity];
  const [activeMessage, setActiveMessage] = useState<string | null>(null);
  const [fading, setFading] = useState(false);
  const messageTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fadeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  const hearts = meterBar(tamaState.hunger, "\u2665", "\u2661", 5);
  const poopTag = tamaState.poopPiles > 0 ? ` \uD83D\uDCA9${tamaState.poopPiles}` : "";
  const sickTag = tamaState.sick ? " [sick]" : "";
  const sleepTag = tamaState.sleeping ? " [zzz]" : "";
  const deadTag = tamaState.isDead ? " [dead]" : "";
  const pomodoro = usePomodoro();
  const pomodoroLabel = pomodoro.state.phase !== "idle"
    ? ` \u23F1 ${pomodoro.timer} [${phaseLabel(pomodoro.state.phase)}]`
    : "";

  return (
    <Box flexDirection="column">
      <Text>
        <Text color={color}>{companionSoul.name}</Text>
        <Text> </Text>
        <Text color="#888888">{mascotConfig.eye}</Text>
        <Text> </Text>
        <Text color="#666666">Lv.{level}</Text>
        <Text> </Text>
        <Text>{hearts}</Text>
        {poopTag ? <Text color="#A0522D">{poopTag}</Text> : null}
        {sickTag ? <Text color="#F97316">{sickTag}</Text> : null}
        {sleepTag ? <Text color="#888888">{sleepTag}</Text> : null}
        {deadTag ? <Text color="#666666">{deadTag}</Text> : null}
        {pomodoroLabel ? <Text color={POMODORO_COLORS[pomodoro.state.phase]}>{pomodoroLabel}</Text> : null}
      </Text>
      {activeMessage && (
        <Text dimColor={fading} color="#FBBF24">
          {"\u201C"}{activeMessage}
        </Text>
      )}
    </Box>
  );
};

export default CompactCompanion;
