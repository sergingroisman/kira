export { SplashScreen, hasSeenSplash } from './SplashScreen.js';
export { ThemeProvider, useTheme } from './ThemeProvider.js';
export { Badge } from './Badge.js';
export { ProgressBar } from './ProgressBar.js';
export { BoxedText } from './BoxedText.js';
export { Header } from './Header.js';
export { StatusBar } from './StatusBar.js';
export { TurnRenderer, type Turn } from './TurnRenderer.js';
export { InputBar } from './InputBar.js';
export { EnhancedSpinner } from './EnhancedSpinner.js';
export { MessageBubble } from './MessageBubble.js';
export { HelpMenu } from './HelpMenu.js';
export { ShimmerText } from './ShimmerText.js';
export { useShimmerAnimation, type ShimmerDirection } from './ShimmerAnimation.js';
export { HeightRatchet } from './HeightRatchet.js';
export { AnimationClockProvider, useAnimationClock } from './AnimationClock.js';
export { NewMessagesPill } from './NewMessagesPill.js';
export { Typewriter } from './Typewriter.js';
export { AnimatedNumber } from './AnimatedNumber.js';
export { TextReveal } from './TextReveal.js';
export { ToastProvider, ToastContainer, useToast } from './Toast.js';
export type { ToastType, ToastItem } from './Toast.js';
export { PomodoroProvider, usePomodoro } from './PomodoroProvider.js';
export { BuddyChat } from './BuddyChat.js';
export { ApprovalPrompt, type ApprovalPromptProps } from './ApprovalPrompt.js';
export { PlanQuestions, type PlanQuestionsProps } from './PlanQuestions.js';
export { ThinkingDisplay, type SubAgentInfo, type ThinkingCategory } from './ThinkingDisplay.js';

// Attachment parser exports
export {
  parseAttachments,
  extractAtMentionedFiles,
  stripAtMentions,
  expandPath,
  parseLineRange,
  generateFakeToolPairs,
  formatAttachmentSummary,
  HAS_AT_SYMBOL_RE,
  type Attachment,
  type FileAttachment,
  type ImageAttachment,
  type ParseResult,
} from '../attachment-parser.js';

// File suggestions exports
export {
  getSuggestions,
  getAtTokenAtCursor,
  formatSuggestionInsertion,
  invalidateFileCache,
  type FileSuggestion,
} from '../file-suggestions.js';

// Suggestion engine exports
export {
  extractCompletionToken,
  formatReplacementValue,
  getPreservedSelection,
  findLongestCommonPrefix,
  debounce,
  firstGrapheme,
  type SuggestionItem,
  type CompletionToken,
  type Replacement,
} from '../suggestion-engine.js';

// REPL history exports
export {
  recordCommand,
  searchHistory,
  clearHistory,
  type HistoryResult,
} from '../repl-history.js';

// File suggestions hook
export {
  useFileSuggestions,
  type FileSuggestionsState,
  type UseFileSuggestionsReturn,
} from '../use-file-suggestions.js';

// Word rank exports
export {
  tokenize,
  recordWords,
  getTopWords,
  getWordBoost,
  clearWords,
  type WordRankResult,
} from '../word-rank.js';
