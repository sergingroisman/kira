import { useState, useEffect, useRef, useCallback } from "react";
import {
  extractCompletionToken,
  getPreservedSelection,
  formatReplacementValue,
  findLongestCommonPrefix,
  type CompletionToken,
} from "./suggestion-engine.js";
import { getSuggestions, type FileSuggestion } from "./file-suggestions.js";
import { searchHistory } from "./repl-history.js";
import { getWordBoost, tokenize } from "./word-rank.js";

const DEBOUNCE_MS = 50;
const MAX_SUGGESTIONS = 15;

export interface FileSuggestionsState {
  suggestions: FileSuggestion[];
  suggestionIdx: number;
  showDropdown: boolean;
  atToken: CompletionToken | null;
  isLoading: boolean;
  ghostText: string | null;
  historyResults: string[];
  historyIdx: number;
}

export interface UseFileSuggestionsReturn extends FileSuggestionsState {
  setSuggestionIdx: (idx: number | ((prev: number) => number)) => void;
  selectNext: () => void;
  selectPrev: () => void;
  acceptCurrent: () => { text: string; cursorOffset: number } | null;
  acceptGhost: () => { text: string; cursorOffset: number } | null;
}

export function useFileSuggestions(
  text: string,
  cursorOffset: number
): UseFileSuggestionsReturn {
  const [suggestions, setSuggestions] = useState<FileSuggestion[]>([]);
  const [suggestionIdx, setSuggestionIdx] = useState(0);
  const [showDropdown, setShowDropdown] = useState(false);
  const [atToken, setAtToken] = useState<CompletionToken | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [ghostText, setGhostText] = useState<string | null>(null);
  const [historyResults, setHistoryResults] = useState<string[]>([]);
  const [historyIdx, setHistoryIdx] = useState(-1);

  const generationRef = useRef(0);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const suggestionsRef = useRef<FileSuggestion[]>([]);
  const prevTokenRef = useRef<string | null>(null);

  useEffect(() => {
    suggestionsRef.current = suggestions;
  }, [suggestions]);

  const doFetch = useCallback((token: CompletionToken) => {
    const gen = ++generationRef.current;
    setIsLoading(true);

    const pathPart = token.isQuoted
      ? token.value.slice(2, -1)
      : token.hasAtPrefix
        ? token.value.slice(1)
        : token.value;

    if (!pathPart) {
      setSuggestions([]);
      setShowDropdown(false);
      setGhostText(null);
      setIsLoading(false);
      return;
    }

    getSuggestions(pathPart).then((results) => {
      if (gen !== generationRef.current) return;

      const boost = getWordBoost(pathPart);
      let ranked = results;
      if (boost > 0) {
        const highFreqWords = tokenize(pathPart);
        ranked = [...results].sort((a, b) => {
          const aBoost = highFreqWords.some((w) => a.displayText.toLowerCase().includes(w)) ? boost : 0;
          const bBoost = highFreqWords.some((w) => b.displayText.toLowerCase().includes(w)) ? boost : 0;
          return bBoost - aBoost;
        });
      }

      const limited = ranked.slice(0, MAX_SUGGESTIONS);

      setSuggestions((prev) => {
        const newIdx = getPreservedSelection(prev, suggestionIdxRef.current, limited);
        suggestionIdxRef.current = newIdx;
        setSuggestionIdx(newIdx);
        return limited;
      });

      setShowDropdown(limited.length > 0);
      setIsLoading(false);

      if (limited.length > 0) {
        const displayTexts = limited.map((s) => s.displayText);
        const lcp = findLongestCommonPrefix(displayTexts);
        const currentPart = token.isQuoted
          ? token.value.slice(2)
          : token.hasAtPrefix
            ? token.value.slice(1)
            : token.value;
        const suffix = lcp.slice(currentPart.length);
        setGhostText(suffix || null);
      } else {
        setGhostText(null);
      }
    });
  }, []);

  const suggestionIdxRef = useRef(0);
  useEffect(() => { suggestionIdxRef.current = suggestionIdx; }, [suggestionIdx]);

  useEffect(() => {
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }

    const token = extractCompletionToken(text, cursorOffset, true);
    setAtToken(token);

    if (token && token.hasAtPrefix && token.value.length > 1) {
      const tokenStr = token.value;
      prevTokenRef.current = tokenStr;
      setHistoryResults([]);
      setHistoryIdx(-1);

      debounceTimerRef.current = setTimeout(() => {
        doFetch(token);
      }, DEBOUNCE_MS);
    } else if (text && !token) {
      // No @token — search history for plain text + ghost completion
      setSuggestions([]);
      setShowDropdown(false);
      setAtToken(null);

      debounceTimerRef.current = setTimeout(() => {
        const results = searchHistory(text, 5);
        setHistoryResults(results.map((r) => r.text));
        setHistoryIdx(-1);

        // Ghost text: longest common prefix suffix from top matches,
        // or the full suffix of the top result
        if (results.length > 0) {
          const topText = results[0]!.text;
          const suffix = topText.startsWith(text) ? topText.slice(text.length) : "";
          if (suffix && suffix.trim().length > 0) {
            setGhostText(suffix);
          } else {
            setGhostText(null);
          }
        } else {
          setGhostText(null);
        }
      }, DEBOUNCE_MS);
    } else {
      setSuggestions([]);
      setShowDropdown(false);
      setGhostText(null);
      setHistoryResults([]);
      setHistoryIdx(-1);
    }

    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, [text, cursorOffset, doFetch]);

  const selectNext = useCallback(() => {
    setSuggestionIdx((prev) => {
      const len = suggestionsRef.current.length;
      return prev >= len - 1 ? 0 : prev + 1;
    });
  }, []);

  const selectPrev = useCallback(() => {
    setSuggestionIdx((prev) => {
      const len = suggestionsRef.current.length;
      return prev <= 0 ? len - 1 : prev - 1;
    });
  }, []);

  const acceptCurrent = useCallback((): { text: string; cursorOffset: number } | null => {
    const idx = suggestionIdxRef.current;
    const items = suggestionsRef.current;
    const token = atToken;
    if (!token || idx < 0 || idx >= items.length) return null;

    const suggestion = items[idx]!;
    const replacement = formatReplacementValue(suggestion, token);

    const before = text.slice(0, token.startPos);
    const after = text.slice(token.startPos + token.value.length);
    const newText = before + replacement.text + after;
    const newOffset = before.length + replacement.text.length;

    return { text: newText, cursorOffset: newOffset };
  }, [atToken, text]);

  const acceptGhost = useCallback((): { text: string; cursorOffset: number } | null => {
    const token = atToken;
    const ghost = ghostText;
    if (!token || !ghost) return null;

    const before = text.slice(0, cursorOffset);
    const after = text.slice(cursorOffset);
    const newText = before + ghost + after;
    const newOffset = cursorOffset + ghost.length;

    return { text: newText, cursorOffset: newOffset };
  }, [atToken, ghostText, text, cursorOffset]);

  return {
    suggestions,
    suggestionIdx,
    showDropdown,
    atToken,
    isLoading,
    ghostText,
    historyResults,
    historyIdx,
    setSuggestionIdx,
    selectNext,
    selectPrev,
    acceptCurrent,
    acceptGhost,
  };
}
