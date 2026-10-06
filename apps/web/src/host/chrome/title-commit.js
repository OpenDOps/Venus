/** Idle delay before a title draft is written to the catalog. */
export const TITLE_COMMIT_MS = 300;

/**
 * Keystrokes stay local. `write` runs once per committed draft: after `delay`
 * with no further push, or on `flush` (blur / Enter). A flush clears the
 * idle timer, so blur and the timer do not both write.
 *
 * @param {(text: string) => unknown} write
 * @param {{
 *   later?: (fn: () => void, ms: number) => unknown,
 *   cancel?: (id: unknown) => void,
 *   delay?: number,
 * }} [clock]
 */
export function createTitleCommit(write, clock = {}) {
  const later = clock.later ?? ((fn, ms) => setTimeout(fn, ms));
  const cancelTimer = clock.cancel ?? ((id) => clearTimeout(/** @type {ReturnType<typeof setTimeout>} */ (id)));
  const delay = clock.delay ?? TITLE_COMMIT_MS;
  /** @type {unknown} */
  let timer = null;
  /** @type {string | null} */
  let pending = null;

  function clearTimer() {
    if (timer == null) return;
    cancelTimer(timer);
    timer = null;
  }

  function flush() {
    clearTimer();
    if (pending == null) return undefined;
    const text = pending;
    pending = null;
    return write(text);
  }

  return {
    /** @param {string} text */
    push(text) {
      pending = String(text);
      clearTimer();
      timer = later(() => {
        timer = null;
        flush();
      }, delay);
    },
    flush,
    cancel() {
      clearTimer();
      pending = null;
    },
  };
}
