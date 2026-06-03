let scrollDraining = false;
let drainTimer: ReturnType<typeof setTimeout> | null = null;

export function markScrollActivity(): void {
  scrollDraining = true;
  if (drainTimer !== null) clearTimeout(drainTimer);
  drainTimer = setTimeout(() => {
    scrollDraining = false;
    drainTimer = null;
  }, 150);
}

export function updateLastInteractionTime(_immediate?: boolean): void {
  // no-op: Kira doesn't use idle detection
}

export function flushInteractionTime(): void {
  // no-op stub
}

export function getIsScrollDraining(): boolean {
  return scrollDraining;
}

export function waitForScrollIdle(): Promise<void> {
  if (!scrollDraining) return Promise.resolve();
  return new Promise((resolve) => {
    const check = () => {
      if (!scrollDraining) { resolve(); return; }
      setTimeout(check, 10);
    };
    check();
  });
}
