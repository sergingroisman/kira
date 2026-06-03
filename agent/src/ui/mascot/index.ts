export { Mascot, type MascotProps } from "./Mascot.js";
export { CompanionPanel, type CompanionPanelProps } from "./CompanionPanel.js";
export { CompactCompanion, type CompactCompanionProps } from "./CompactCompanion.js";
export { generateCompanion, generateSoul } from "./generator.js";
export { EMOTION_EYES, EMOTION_COLORS } from "./expressions.js";
export { RARITY_COLORS, RARITY_STARS, xpToLevel, levelToRarity, progressBar, meterBar } from "./types.js";
export { loadCompanionState, saveCompanionState } from "./persistence.js";
export { defaultTamagotchi, applyDecay, dispatchAction, tamaEmotion } from "./tamagotchi.js";
export type {
  Species, Emotion, Eye, Rarity, MascotConfig,
  CompanionSoul, XPState, TamagotchiState, TamaAction, PersistedCompanionState,
} from "./types.js";
