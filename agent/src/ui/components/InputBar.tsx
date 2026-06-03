import React, { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { Box, Text, useInput, useApp } from "../../ink/kira-ink.js";
import { useFileSuggestions } from "../use-file-suggestions.js";
import { recordCommand } from "../repl-history.js";
import { recordWords } from "../word-rank.js";

export interface InputBarProps {
  mode: "plan" | "yolo";
  disabled?: boolean;
  onSubmit: (text: string) => void;
  onFirstCtrlC?: () => void;
  onDoubleCtrlC?: () => void;
}

const MAX_HISTORY = 50;
const CURSOR_CHAR = "█";
const DOUBLE_TAP_WINDOW_MS = 800;

class InputCursor {
  readonly text: string;
  readonly offset: number;

  constructor(text: string, offset: number) {
    this.text = text;
    this.offset = Math.max(0, Math.min(offset, text.length));
  }

  left(): InputCursor {
    return new InputCursor(this.text, this.offset - 1);
  }

  right(): InputCursor {
    return new InputCursor(this.text, this.offset + 1);
  }

  get lines(): string[] { return this.text.split('\n'); }

  get lineIndex(): number {
    let idx = 0;
    for (let i = 0; i < this.text.length && i < this.offset; i++) {
      if (this.text[i] === '\n') idx++;
    }
    return idx;
  }

  get colIndex(): number {
    const lastNewline = this.text.lastIndexOf('\n', this.offset - 1);
    return lastNewline === -1 ? this.offset : this.offset - lastNewline - 1;
  }

  start(): InputCursor {
    const lines = this.lines;
    const li = this.lineIndex;
    let off = 0;
    for (let i = 0; i < li; i++) off += lines[i].length + 1;
    return new InputCursor(this.text, off);
  }

  end(): InputCursor {
    const lines = this.lines;
    const li = this.lineIndex;
    let off = 0;
    for (let i = 0; i <= li; i++) off += lines[i].length + 1;
    return new InputCursor(this.text, off - 1);
  }

  moveUp(): InputCursor {
    const lines = this.lines;
    const li = this.lineIndex;
    if (li <= 0) return this;
    const col = this.colIndex;
    const prevLen = lines[li - 1].length;
    const newCol = Math.min(col, prevLen);
    let off = 0;
    for (let i = 0; i < li - 1; i++) off += lines[i].length + 1;
    return new InputCursor(this.text, off + newCol);
  }

  moveDown(): InputCursor {
    const lines = this.lines;
    const li = this.lineIndex;
    if (li >= lines.length - 1) return this;
    const col = this.colIndex;
    const nextLen = lines[li + 1].length;
    const newCol = Math.min(col, nextLen);
    let off = 0;
    for (let i = 0; i <= li; i++) off += lines[i].length + 1;
    return new InputCursor(this.text, off + newCol);
  }

  prevWord(): InputCursor {
    const before = this.text.slice(0, this.offset);
    const match = before.match(/(\s+)\S*$/);
    if (match) {
      const newPos = this.offset - match[0].length;
      return new InputCursor(this.text, Math.max(0, newPos));
    }
    return this.start();
  }

  nextWord(): InputCursor {
    const after = this.text.slice(this.offset);
    const match = after.match(/^\s*\S*/);
    if (match) {
      return new InputCursor(this.text, this.offset + match[0].length);
    }
    return this.end();
  }

  insert(str: string): InputCursor {
    const newText = this.text.slice(0, this.offset) + str + this.text.slice(this.offset);
    return new InputCursor(newText, this.offset + str.length);
  }

  backspace(): InputCursor {
    if (this.offset === 0) return this;
    const newText = this.text.slice(0, this.offset - 1) + this.text.slice(this.offset);
    return new InputCursor(newText, this.offset - 1);
  }

  del(): InputCursor {
    if (this.offset >= this.text.length) return this;
    const newText = this.text.slice(0, this.offset) + this.text.slice(this.offset + 1);
    return new InputCursor(newText, this.offset);
  }

  deleteToStart(): InputCursor {
    return new InputCursor(this.text.slice(this.offset), 0);
  }

  deleteToEnd(): InputCursor {
    return new InputCursor(this.text.slice(0, this.offset), this.offset);
  }

  deleteWordBefore(): InputCursor {
    const before = this.prevWord();
    return new InputCursor(
      before.text.slice(0, before.offset) + this.text.slice(this.offset),
      before.offset
    );
  }

  deleteWordAfter(): InputCursor {
    const after = this.nextWord();
    return new InputCursor(
      this.text.slice(0, this.offset) + after.text.slice(after.offset),
      this.offset
    );
  }

  equals(other: InputCursor): boolean {
    return this.text === other.text && this.offset === other.offset;
  }
}

function getAtMentionHighlights(text: string): Array<{ start: number; end: number }> {
  const highlights: Array<{ start: number; end: number }> = [];
  const regex = /(^|\s)(@"[^"]+"|@[^\s]+)/g;
  let match;
  while ((match = regex.exec(text)) !== null) {
    const start = match.index + match[1].length;
    const end = start + match[2].length;
    highlights.push({ start, end });
  }
  return highlights;
}

function renderHighlightedInput(
  text: string,
  cursorOffset: number,
  highlights: Array<{ start: number; end: number }>,
  cursorChar: string,
  ghostText?: string | null
): React.ReactNode {
  if (highlights.length === 0) {
    const before = text.slice(0, cursorOffset);
    const atCursor = cursorOffset < text.length ? text[cursorOffset] : " ";
    const after = text.slice(cursorOffset + 1);

    const isAtEnd = cursorOffset >= text.length;
    if (isAtEnd && ghostText) {
      const firstGhost = ghostText[0] || "";
      const ghostRest = ghostText.slice(1);
      return (
        <>
          <Text>{before}</Text>
          <Text inverse>{firstGhost || cursorChar}</Text>
          <Text dimColor>{ghostRest}</Text>
          <Text>{after}</Text>
        </>
      );
    }

    return (
      <>
        <Text>{before}</Text>
        <Text inverse>{atCursor === " " ? cursorChar : atCursor}</Text>
        <Text>{after}</Text>
      </>
    );
  }

  const sorted = [...highlights].sort((a, b) => a.start - b.start);
  const segments: Array<{ text: string; highlighted: boolean }> = [];
  let pos = 0;

  for (const h of sorted) {
    if (h.start > pos) {
      segments.push({ text: text.slice(pos, h.start), highlighted: false });
    }
    segments.push({ text: text.slice(h.start, h.end), highlighted: true });
    pos = h.end;
  }
  if (pos < text.length) {
    segments.push({ text: text.slice(pos), highlighted: false });
  }

  let cursorSegmentIdx = -1;
  let cursorPosInSegment = -1;
  let segPos = 0;
  for (let i = 0; i < segments.length; i++) {
    const segEnd = segPos + segments[i].text.length;
    if (cursorOffset >= segPos && cursorOffset < segEnd) {
      cursorSegmentIdx = i;
      cursorPosInSegment = cursorOffset - segPos;
      break;
    }
    segPos = segEnd;
  }
  if (cursorOffset === text.length && segments.length > 0) {
    cursorSegmentIdx = segments.length - 1;
    cursorPosInSegment = segments[segments.length - 1].text.length;
  }

  const isAtEnd = cursorOffset >= text.length;

  return (
    <>
      {segments.map((seg, idx) => {
        if (idx !== cursorSegmentIdx) {
          return (
            <Text key={idx} color={seg.highlighted ? "#87CEEB" : undefined}>
              {seg.text}
            </Text>
          );
        }

        const before = seg.text.slice(0, cursorPosInSegment);
        const atCursor = cursorPosInSegment < seg.text.length ? seg.text[cursorPosInSegment] : " ";
        const after = seg.text.slice(cursorPosInSegment + 1);

        if (isAtEnd && idx === segments.length - 1 && ghostText) {
          const firstGhost = ghostText[0] || "";
          const ghostRest = ghostText.slice(1);
          return (
            <Text key={idx} color={seg.highlighted ? "#87CEEB" : undefined}>
              {before}
              <Text inverse color={seg.highlighted ? "#87CEEB" : undefined}>
                {firstGhost}
              </Text>
              <Text dimColor>{ghostRest}</Text>
              {after}
            </Text>
          );
        }

        return (
          <Text key={idx} color={seg.highlighted ? "#87CEEB" : undefined}>
            {before}
            <Text inverse color={seg.highlighted ? "#87CEEB" : undefined}>
              {atCursor === " " ? cursorChar : atCursor}
            </Text>
            {after}
          </Text>
        );
      })}
      {text.length === 0 && (
        ghostText ? (
          <Text inverse>{ghostText[0] || CURSOR_CHAR}</Text>
        ) : (
          <Text inverse>{cursorChar}</Text>
        )
      )}
      {text.length === 0 && ghostText && ghostText.length > 1 && (
        <Text dimColor>{ghostText.slice(1)}</Text>
      )}
    </>
  );
}

export const InputBar: React.FC<InputBarProps> = ({ mode, disabled, onSubmit, onFirstCtrlC, onDoubleCtrlC }) => {
  const { exit } = useApp();

  const [cursor, setCursor] = useState<InputCursor>(new InputCursor("", 0));
  const [history, setHistory] = useState<string[]>([]);
  const [historyIdx, setHistoryIdx] = useState(-1);
  const [cursorVisible, setCursorVisible] = useState(true);

  const {
    suggestions,
    suggestionIdx,
    showDropdown,
    ghostText,
    selectNext,
    selectPrev,
    acceptCurrent,
    acceptGhost,
    setSuggestionIdx,
  } = useFileSuggestions(cursor.text, cursor.offset);

  const cursorRef = useRef(cursor);
  const lastCtrlCAt = useRef<number>(0);
  const historyIdxRef = useRef(historyIdx);

  useEffect(() => { cursorRef.current = cursor; }, [cursor]);
  useEffect(() => { historyIdxRef.current = historyIdx; }, [historyIdx]);

  useEffect(() => {
    const interval = setInterval(() => {
      setCursorVisible((v) => !v);
    }, 530);
    return () => clearInterval(interval);
  }, []);

  const highlights = useMemo(() => getAtMentionHighlights(cursor.text), [cursor.text]);

  const handleSubmit = useCallback((text: string) => {
    const trimmed = text.trim();
    if (trimmed) {
      recordCommand(trimmed);
      recordWords(trimmed);
      setHistory((h) => {
        const next = [...h, trimmed];
        return next.slice(-MAX_HISTORY);
      });
      onSubmit(trimmed);
    }
    setCursor(new InputCursor("", 0));
    setHistoryIdx(-1);
  }, [onSubmit]);

  useInput(
    useCallback(
      (ch, key) => {
        if (disabled) return;

        const current = cursorRef.current;

        // -- Enter / Shift+Enter -------------------------------------------------
        if (key.return) {
          if (key.shift) {
            setCursor((c) => c.insert("\n"));
            setHistoryIdx(-1);
          } else {
            handleSubmit(current.text);
          }
          return;
        }

        // -- Backspace -----------------------------------------------------------
        if (key.backspace) {
          if (key.ctrl || key.meta) {
            setCursor((c) => c.deleteWordBefore());
          } else {
            setCursor((c) => c.backspace());
          }
          setHistoryIdx(-1);
          return;
        }

        // -- Delete --------------------------------------------------------------
        if (key.delete) {
          if (key.ctrl || key.meta) {
            setCursor((c) => c.deleteWordAfter());
          } else {
            setCursor((c) => c.del());
          }
          return;
        }

        // -- Ctrl+C: double-tap to exit ------------------------------------------
        if (key.ctrl && ch === "c") {
          const now = Date.now();
          const elapsed = now - lastCtrlCAt.current;
          lastCtrlCAt.current = now;

          if (elapsed < DOUBLE_TAP_WINDOW_MS) {
            onDoubleCtrlC?.();
          } else {
            setCursor(new InputCursor("", 0));
            setHistoryIdx(-1);
            onFirstCtrlC?.();
          }
          return;
        }

        // -- Escape: clear input -------------------------------------------------
        if (key.escape) {
          setCursor(new InputCursor("", 0));
          setHistoryIdx(-1);
          return;
        }

        // -- Tab: accept autocomplete suggestion ---------------------------------
        if (key.tab && showDropdown && suggestions.length > 0) {
          const result = acceptCurrent();
          if (result) {
            setCursor(new InputCursor(result.text, result.cursorOffset));
          }
          return;
        }

        // -- Right arrow ---------------------------------------------------------
        if (key.rightArrow) {
          if (!key.ctrl && !key.meta && ghostText) {
            const result = acceptGhost();
            if (result) {
              setCursor(new InputCursor(result.text, result.cursorOffset));
              return;
            }
          }
          if (key.ctrl || key.meta) {
            setCursor((c) => c.nextWord());
          } else {
            setCursor((c) => c.right());
          }
          return;
        }

        // -- Up arrow: navigate suggestions, history, or lines -------------------
        if (key.upArrow) {
          if (showDropdown && suggestions.length > 0) {
            selectPrev();
            return;
          }

          if (current.text.includes("\n") && current.lineIndex > 0) {
            setCursor((c) => c.moveUp());
            return;
          }

          setHistory((h) => {
            if (h.length === 0) return h;
            setHistoryIdx((idx) => {
              const next = idx === -1 ? h.length - 1 : Math.max(0, idx - 1);
              setCursor(new InputCursor(h[next]!, h[next]!.length));
              return next;
            });
            return h;
          });
          return;
        }

        // -- Down arrow: navigate suggestions, history, or lines -----------------
        if (key.downArrow) {
          if (showDropdown && suggestions.length > 0) {
            selectNext();
            return;
          }

          if (current.text.includes("\n") && current.lineIndex < current.lines.length - 1) {
            setCursor((c) => c.moveDown());
            return;
          }

          const hIdx = historyIdxRef.current;
          if (hIdx === -1) return;

          setHistoryIdx((idx) => {
            if (idx >= history.length - 1) {
              setCursor(new InputCursor("", 0));
              return -1;
            }
            const next = idx + 1;
            setCursor(new InputCursor(history[next]!, history[next]!.length));
            return next;
          });
          return;
        }

        // -- Left arrow ----------------------------------------------------------
        if (key.leftArrow) {
          if (key.ctrl || key.meta) {
            setCursor((c) => c.prevWord());
          } else {
            setCursor((c) => c.left());
          }
          return;
        }

        // -- Home / End ----------------------------------------------------------
        if (key.home || (key.ctrl && ch === "a")) {
          setCursor((c) => c.start());
          return;
        }

        if (key.end || (key.ctrl && ch === "e")) {
          setCursor((c) => c.end());
          return;
        }

        // -- Ctrl+K: delete to end -----------------------------------------------
        if (key.ctrl && ch === "k") {
          setCursor((c) => c.deleteToEnd());
          return;
        }

        // -- Ctrl+U: delete to start ---------------------------------------------
        if (key.ctrl && ch === "u") {
          setCursor((c) => c.deleteToStart());
          return;
        }

        // -- Regular character input ---------------------------------------------
        if (ch && !key.meta && !key.ctrl) {
          setCursor((c) => c.insert(ch));
          setHistoryIdx(-1);
          return;
        }
      },
      [disabled, onSubmit, history, exit, onDoubleCtrlC, onFirstCtrlC, history.length, handleSubmit, showDropdown, suggestions.length, acceptCurrent, acceptGhost, ghostText, selectNext, selectPrev],
    ),
    { isActive: !disabled },
  );

  const modeColor = mode === "yolo" ? "#FF3333" : "#555555";
  const modeLabel = mode === "yolo" ? "YOLO" : "PLAN";

  const cursorChar = cursorVisible ? CURSOR_CHAR : " ";

  const lines = cursor.text.split("\n");
  const cursorLine = cursor.lineIndex;
  const cursorCol = cursor.colIndex;
  const MAX_VISIBLE_LINES = 8;

  const inputLines = lines.slice(
    Math.max(0, cursorLine - MAX_VISIBLE_LINES + 1),
    cursorLine + 1,
  );
  const scrollOffset = Math.max(0, cursorLine - MAX_VISIBLE_LINES + 1);

  return (
    <Box flexDirection="column">
      {/* Input area */}
      <Box flexDirection="column">
        {inputLines.map((lineText, lineIdx) => {
          const globalLineIdx = scrollOffset + lineIdx;
          const isCursorLine = globalLineIdx === cursorLine;
          const lineOffset = (() => {
            let off = 0;
            for (let i = 0; i < globalLineIdx; i++) off += lines[i].length + 1;
            return off;
          })();

          return (
            <Box key={globalLineIdx}>
              {globalLineIdx === 0 && (
                <>
                  <Text color={modeColor}>{"┃ "}</Text>
                  <Text color={modeColor}>
                    [{modeLabel}] ▸{" "}
                  </Text>
                </>
              )}
              {globalLineIdx > 0 && (
                <>
                  <Text color={modeColor}>{"┃ "}</Text>
                  <Text color="#444444">{"  "}</Text>
                </>
              )}
              {isCursorLine ? (
                <Text>
                  {renderHighlightedInput(
                    lineText,
                    cursorCol,
                    getAtMentionHighlights(lineText),
                    cursorChar,
                    globalLineIdx === lines.length - 1 ? ghostText : undefined,
                  )}
                </Text>
              ) : (
                <Text>{lineText}</Text>
              )}
            </Box>
          );
        })}
      </Box>

      {/* Autocomplete dropdown */}
      {showDropdown && suggestions.length > 0 && (
        <Box marginLeft={10} marginTop={0}>
          <Box flexDirection="column">
            {suggestions.map((s, idx) => {
              const isSelected = idx === suggestionIdx;
              const icon = s.type === "directory" ? "📁" : "📄";
              return (
                <Text key={s.id}>
                  {isSelected ? (
                    <Text inverse> {icon} {s.displayText}</Text>
                  ) : (
                    <>
                      <Text color="#666666"> {icon} </Text>
                      <Text color="#AAAAAA">{s.displayText}</Text>
                    </>
                  )}
                </Text>
              );
            })}
          </Box>
        </Box>
      )}

      {/* Hint text */}
      {showDropdown && (
        <Box marginLeft={10}>
          <Text color="#555555" dimColor>
            ↑↓ navigate · Tab accept
          </Text>
        </Box>
      )}
    </Box>
  );
};

export default InputBar;
