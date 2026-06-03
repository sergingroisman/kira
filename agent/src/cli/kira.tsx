#!/usr/bin/env bun
/**
 * Kira REPL entrypoint (Bun runtime).
 *
 * Talks to kira-cored at http://127.0.0.1:7777 by default.
 * Override via env KIRA_CORE_URL.
 *
 * Usage:
 *   bun run src/cli/kira.tsx
 *   KIRA_CORE_URL=http://other:7777 bun run src/cli/kira.tsx
 */

import { profileCheckpoint, profileReport } from "../utils/startup-profiler.js";

profileCheckpoint("entry");

const VERSION = "0.1.0";

const args = process.argv.slice(2);

// Fast-path: --version / -v with zero heavy imports
if (args.length === 1 && (args[0] === "--version" || args[0] === "-v" || args[0] === "-V")) {
  console.log(VERSION);
  process.exit(0);
}

// Fast-path: --help with zero heavy imports
if (args.length === 1 && (args[0] === "--help" || args[0] === "-h")) {
  console.log(`Kira v${VERSION} — REPL for kira-cored`);
  console.log("");
  console.log("Usage: kira [options]");
  console.log("");
  console.log("Options:");
  console.log("  -v, --version    Print version and exit");
  console.log("  -h, --help       Print this help and exit");
  console.log("  --profile        Print startup timing profile");
  console.log("");
  console.log("Environment:");
  console.log("  KIRA_CORE_URL    Override core URL (default: http://127.0.0.1:7777)");
  process.exit(0);
}

const shouldProfile = args.includes("--profile");

// Bun does not expose isTTY (returns undefined); Ink 7 checks it internally
// and skips stdin attachment when falsy → REPL exits immediately.
// Also, Bun's stdin lacks ref()/unref()/setRawMode() which Ink needs when
// running in TTY mode. Polyfill everything before any Ink imports.
const polyfill = (obj: any, prop: string, val: any) => {
  if (typeof obj[prop] === "undefined") obj[prop] = val;
};
polyfill(process.stdout, "isTTY", true);
polyfill(process.stdin, "isTTY", true);
polyfill(process.stdin, "ref", () => {});
polyfill(process.stdin, "unref", () => {});
polyfill(process.stdin, "setRawMode", () => {});

// Bun bug observed 2026-05-26: `process.stdout.columns` / `rows` return 0
// even when `isTTY === true`. Ink's `columns || 80` fallback then locks
// layout at 80×24 while the actual terminal is wider, so the writer
// positions content in a tiny window and the rest of the screen looks
// empty (only the rightmost mascot + clock leak through because they
// space-between to the frame edge). Detect the bug and patch stdout
// before Ink reads it — `tput` is the canonical source on macOS.
function detectTerminalSize(): { cols: number; rows: number } {
  const c = (process.stdout as { columns?: number }).columns ?? 0;
  const r = (process.stdout as { rows?: number }).rows ?? 0;
  if (c > 0 && r > 0) return { cols: c, rows: r };
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const { spawnSync } = require("node:child_process") as typeof import("node:child_process");
    const tCols = spawnSync("tput", ["cols"], { stdio: ["inherit", "pipe", "ignore"] });
    const tRows = spawnSync("tput", ["lines"], { stdio: ["inherit", "pipe", "ignore"] });
    const cols = Number.parseInt(String(tCols.stdout ?? "").trim(), 10);
    const rows = Number.parseInt(String(tRows.stdout ?? "").trim(), 10);
    if (Number.isFinite(cols) && cols > 0 && Number.isFinite(rows) && rows > 0) {
      return { cols, rows };
    }
  } catch { /* fall through to defaults */ }
  return { cols: 120, rows: 32 };
}

{
  const cur = (process.stdout as { columns?: number }).columns ?? 0;
  if (cur === 0) {
    const { cols, rows } = detectTerminalSize();
    Object.defineProperty(process.stdout, "columns", { value: cols, configurable: true, writable: true });
    Object.defineProperty(process.stdout, "rows", { value: rows, configurable: true, writable: true });
    if (process.env.KIRA_DEBUG) {
      console.error(`[kira] polyfilled terminal size: ${cols}×${rows}`);
    }
  }
}

// Lazy load all heavy modules — React, Ink, and the full app tree.
// This keeps --version/--help sub-10ms and reduces bundle DCE surface.
async function main(): Promise<void> {
  profileCheckpoint("lazy_import_start");

  const [{ default: React }, { render }, { KiraApp }, { ThemeProvider }, { CoreClient }] =
    await Promise.all([
      import("react"),
      import("../ink/kira-ink.js"),
      import("../ui/KiraApp.js"),
      import("../ui/components/ThemeProvider.js"),
      import("../core-client/client.js"),
    ]);

  profileCheckpoint("heavy_modules_loaded");

  const client = new CoreClient();

  const app = await render(
    <ThemeProvider>
      <KiraApp client={client} />
    </ThemeProvider>,
    {
      exitOnCtrlC: false,
    },
  );

  profileCheckpoint("render_done");

  if (shouldProfile) {
    profileReport();
  }

  await app.waitUntilExit();
  app.unmount();
  process.exit(0);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
