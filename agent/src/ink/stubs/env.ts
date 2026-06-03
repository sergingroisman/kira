export const env: Record<string, string | undefined> = {
  TERM: process.env.TERM ?? 'xterm-256color',
  COLORTERM: process.env.COLORTERM ?? '',
  FORCE_COLOR: process.env.FORCE_COLOR ?? '',
  KIRA_CORE_URL: process.env.KIRA_CORE_URL ?? '',
  NO_COLOR: process.env.NO_COLOR,
  CI: process.env.CI,
  terminal: process.env.TERM,
};
