import type { Emotion } from "./types.js";

export const EMOTION_EYES: Record<Emotion, string> = {
  happy: "^",
  sad: "\u25E0",
  surprised: "O",
  angry: ">",
  sleepy: "-",
  sick: "\u00D7",
  dead: "\u2715",
  neutral: "\u00B7",
};

export const EMOTION_COLORS: Record<Emotion, string> = {
  happy: "#4ADE80",
  sad: "#60A5FA",
  surprised: "#FBBF24",
  angry: "#EF4444",
  sleepy: "#888888",
  sick: "#F97316",
  dead: "#666666",
  neutral: "#AAAAAA",
};

export function emotionEyeChar(emotion: Emotion, defaultEye: string): string {
  return emotion === "neutral" ? defaultEye : EMOTION_EYES[emotion];
}
