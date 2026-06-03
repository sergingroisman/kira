const _origConsoleDebug = console.debug.bind(console);

export function logForDebugging(...args: unknown[]): void {
  if (process.env.KIRA_DEBUG) {
    _origConsoleDebug("[KIRA_INK]", ...args);
  }
}
