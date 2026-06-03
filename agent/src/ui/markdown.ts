/**
 * Minimal markdown-to-ANSI converter for Kira REPL.
 *
 * Converts common Markdown constructs to ANSI terminal codes
 * for rendering in Ink Text components.
 */

const ANSI_BOLD = "\x1b[1m";
const ANSI_DIM = "\x1b[2m";
const ANSI_ITALIC = "\x1b[3m";
const ANSI_UNDERLINE = "\x1b[4m";
const ANSI_RESET = "\x1b[0m";

const ANSI_GRAY = "\x1b[90m";
const ANSI_GREEN = "\x1b[32m";
const ANSI_YELLOW = "\x1b[33m";
const ANSI_CYAN = "\x1b[36m";

export function md2ansi(md: string): string {
  if (!md) return "";

  let result = md;

  // Code blocks (multiline) — strip, we don't render well in terminal
  result = result.replace(/```[\s\S]*?```/g, (match) => {
    const inner = match.replace(/```\w*\n?/g, "").replace(/```/g, "").trim();
    return ANSI_DIM + inner + ANSI_RESET;
  });

  // Inline code
  result = result.replace(/`([^`]+)`/g, ANSI_DIM + "$1" + ANSI_RESET);

  // Bold (**text** or __text__)
  result = result.replace(/\*\*(.+?)\*\*/g, ANSI_BOLD + "$1" + ANSI_RESET);
  result = result.replace(/__(.+?)__/g, ANSI_BOLD + "$1" + ANSI_RESET);

  // Italic (*text* or _text_) — but not inside words for underscores
  result = result.replace(/\*(.+?)\*/g, ANSI_ITALIC + "$1" + ANSI_RESET);
  result = result.replace(/(?<!\w)_(.+?)_(?!\w)/g, ANSI_ITALIC + "$1" + ANSI_RESET);

  // Headings
  result = result.replace(/^#### (.+)$/gm, ANSI_BOLD + "$1" + ANSI_RESET);
  result = result.replace(/^### (.+)$/gm, ANSI_BOLD + ANSI_UNDERLINE + "$1" + ANSI_RESET);
  result = result.replace(/^## (.+)$/gm, ANSI_BOLD + ANSI_CYAN + "$1" + ANSI_RESET);
  result = result.replace(/^# (.+)$/gm, ANSI_BOLD + ANSI_UNDERLINE + ANSI_CYAN + "$1" + ANSI_RESET);

  // Unordered lists
  result = result.replace(/^[*-] (.+)$/gm, "  " + ANSI_YELLOW + "•" + ANSI_RESET + " $1");

  // Ordered lists
  result = result.replace(/^\d+\. (.+)$/gm, "  " + ANSI_CYAN + "$&" + ANSI_RESET);

  // Blockquotes
  result = result.replace(/^> (.+)$/gm, ANSI_GRAY + "┃ " + "$1" + ANSI_RESET);

  // Horizontal rules
  result = result.replace(/^[*-_]{3,}$/gm, ANSI_GRAY + "─".repeat(40) + ANSI_RESET);

  // Links — keep text, dim URL
  result = result.replace(/\[([^\]]+)\]\(([^)]+)\)/g, ANSI_GREEN + "$1" + ANSI_RESET + ANSI_GRAY + " ($2)" + ANSI_RESET);

  return result;
}
