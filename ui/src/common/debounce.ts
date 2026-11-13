export type Debounced<Args extends unknown[]> = ((...args: Args) => void) & { cancel: () => void };

/** Delays `fn` until `delayMs` passed without another call, then runs it with the last arguments. */
export function debounce<Args extends unknown[]>(fn: (...args: Args) => unknown, delayMs: number): Debounced<Args> {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const cancel = () => {
    clearTimeout(timer);
    timer = undefined;
  };

  const debounced = (...args: Args) => {
    cancel();
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, delayMs);
  };

  return Object.assign(debounced, { cancel });
}
