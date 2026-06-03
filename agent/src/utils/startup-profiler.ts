/**
 * Startup profiler — lightweight checkpoint tracking for Kira REPL.
 * Mirrors Claude Code's startupProfiler.ts pattern.
 *
 * Usage:
 *   profileCheckpoint('entry');
 *   // ... load heavy modules ...
 *   profileCheckpoint('ink_loaded');
 *   profileReport(); // prints timing report
 */

const MARKS: { name: string; ts: number }[] = [];

export function profileCheckpoint(name: string): void {
  MARKS.push({ name, ts: performance.now() });
}

export function profileReport(): void {
  if (MARKS.length === 0) return;
  const base = MARKS[0].ts;
  const lines: string[] = ["=== startup profile ==="];
  for (let i = 0; i < MARKS.length; i++) {
    const elapsed = (MARKS[i].ts - base).toFixed(1);
    const delta = i > 0 ? ` (+${(MARKS[i].ts - MARKS[i - 1].ts).toFixed(1)}ms)` : "";
    lines.push(`  ${MARKS[i].name.padEnd(24)} ${elapsed.padStart(8)}ms${delta}`);
  }
  lines.push("=======================");
  // eslint-disable-next-line no-console
  console.error(lines.join("\n"));
}

export function getMarks(): ReadonlyArray<{ name: string; ts: number }> {
  return MARKS;
}
