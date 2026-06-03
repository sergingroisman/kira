const sentinel = Symbol.for("react.memo_cache_sentinel");

export function c<T = unknown>(size: number): (T | symbol)[] {
  const arr = new Array<T | symbol>(size);
  for (let i = 0; i < size; i++) {
    arr[i] = sentinel;
  }
  return arr;
}
