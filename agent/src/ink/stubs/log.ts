const _origConsoleError = console.error.bind(console);

export function logError(...args: unknown[]): void {
  _origConsoleError("[KIRA_INK_ERR]", ...args);
}
