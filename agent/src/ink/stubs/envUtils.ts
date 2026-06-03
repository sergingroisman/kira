export function isEnvTruthy(key: string): boolean {
  const val = process.env[key];
  if (val === undefined || val === '') return false;
  return !['0', 'false', 'no', 'off', 'disabled'].includes(val.toLowerCase());
}
