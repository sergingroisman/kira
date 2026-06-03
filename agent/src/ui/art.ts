/**
 * art.ts — ASCII art generation via figlet + gradient utilities.
 *
 * Wraps figlet to generate KIRA logo art at runtime.
 * Produces plain text (no ANSI codes) compatible with Ink <Text> rendering.
 * Falls back to hardcoded art if figlet is unavailable.
 */
import figlet from "figlet";

// ---------------------------------------------------------------------------
// Supported fonts — choose ones with reasonable width for sidebar layout.
// ---------------------------------------------------------------------------
export type FigletFont = "ANSI Shadow" | "Doom" | "ANSI Regular" | "Big" | "Standard" | "Small";

export const SUPPORTED_FONTS: FigletFont[] = [
  "ANSI Shadow",
  "Doom",
  "ANSI Regular",
  "Big",
  "Standard",
  "Small",
];

export const DEFAULT_KIRA_FONT: FigletFont = "ANSI Shadow";

// ---------------------------------------------------------------------------
// Fallback — same blocky gothic art as before (Death Note style).
// ---------------------------------------------------------------------------
const FALLBACK_KIRA: string[] = [
  "█  █▀ ▄█ █▄▄▄▄ ██   ",
  "█▄█   ██ █  ▄▀ █ █  ",
  "█▀▄   ██ █▀▀▌  █▄▄█ ",
  "█  █  ▐█ █  █  █  █ ",
  "  █    ▐   █      █  ",
  " ▀        ▀      █   ",
];

// ---------------------------------------------------------------------------
// Cache — avoid repeated figlet calls on every render.
// ---------------------------------------------------------------------------
const cache = new Map<string, string[]>();

// ---------------------------------------------------------------------------
// Generate ASCII art for a given text + font.
// ---------------------------------------------------------------------------
export function figletArt(text: string, font: FigletFont = DEFAULT_KIRA_FONT): string[] {
  const key = `${text}:${font}`;
  const cached = cache.get(key);
  if (cached) return cached;

  try {
    const raw = figlet.textSync(text, { font });
    const lines = raw.split("\n");
    while (lines.length > 0 && lines[lines.length - 1].trim() === "") {
      lines.pop();
    }
    while (lines.length > 0 && lines[0].trim() === "") {
      lines.shift();
    }
    cache.set(key, lines);
    return lines;
  } catch {
    return FALLBACK_KIRA;
  }
}

/** Convenience: generate "KIRA" art. */
export function figletKira(font: FigletFont = DEFAULT_KIRA_FONT): string[] {
  return figletArt("KIRA", font);
}

// ---------------------------------------------------------------------------
// Gradient utilities
// ---------------------------------------------------------------------------

/** Check if a color string is an ANSI name (not hex). */
function isAnsiName(color: string): boolean {
  return !color.startsWith("#");
}

/**
 * Blend two hex colors by a factor `t` (0..1).
 * If either is ANSI-named, return the source.
 */
function blendHexColors(a: string, b: string, t: number): string {
  if (isAnsiName(a) || isAnsiName(b)) return a;
  const ar = Number.parseInt(a.slice(1, 3), 16);
  const ag = Number.parseInt(a.slice(3, 5), 16);
  const ab = Number.parseInt(a.slice(5, 7), 16);
  const br = Number.parseInt(b.slice(1, 3), 16);
  const bg = Number.parseInt(b.slice(3, 5), 16);
  const bb = Number.parseInt(b.slice(5, 7), 16);
  const r = Math.round(ar + (br - ar) * t);
  const g = Math.round(ag + (bg - ag) * t);
  const bl = Math.round(ab + (bb - ab) * t);
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${bl.toString(16).padStart(2, "0")}`;
}

/**
 * Interpolate a palette of `N` colors to produce exactly `count` colors.
 * Smooth transitions between hex colors; nearest-neighbor for ANSI names.
 */
/**
 * Interpolate between two hex colors by factor t (0..1).
 * Returns hex string `#rrggbb`. Handles ANSI names gracefully.
 */
export function interpolateColor(a: string, b: string, t: number): string {
  if (isAnsiName(a) || isAnsiName(b)) return t < 0.5 ? a : b;
  const ar = Number.parseInt(a.slice(1, 3), 16);
  const ag = Number.parseInt(a.slice(3, 5), 16);
  const ab = Number.parseInt(a.slice(5, 7), 16);
  const br = Number.parseInt(b.slice(1, 3), 16);
  const bg = Number.parseInt(b.slice(3, 5), 16);
  const bb = Number.parseInt(b.slice(5, 7), 16);
  const r = Math.round(ar + (br - ar) * t);
  const g = Math.round(ag + (bg - ag) * t);
  const bl = Math.round(ab + (bb - ab) * t);
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${bl.toString(16).padStart(2, "0")}`;
}

export function interpolatePalette(palette: string[], count: number): string[] {
  if (count === 0) return [];
  if (count === 1) return [palette[0]];
  if (palette.length === 0) return [];
  if (palette.every(isAnsiName)) {
    // ANSI names — nearest-neighbor
    return Array.from({ length: count }, (_, i) => {
      const idx = Math.round((i / Math.max(count - 1, 1)) * (palette.length - 1));
      return palette[idx];
    });
  }

  const result: string[] = [];
  for (let i = 0; i < count; i++) {
    const t = i / Math.max(count - 1, 1);
    const idx = t * (palette.length - 1);
    const low = Math.floor(idx);
    const high = Math.min(low + 1, palette.length - 1);
    const frac = idx - low;
    if (low === high) {
      result.push(palette[low]);
    } else {
      result.push(blendHexColors(palette[low], palette[high], frac));
    }
  }
  return result;
}
