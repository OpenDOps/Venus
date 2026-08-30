/** Full `fromDoc` after this many splices so RAM ranges cannot drift. */
export const RECONCILE_EVERY = 8;

/**
 * Single-flight + dirtyIds. Collapses Store events during a slow export
 * into one follow-up. `exportRun` is injected so Vitest can fake timers.
 */
export function createMdPaneLoop({
  exportRun,
  onMarkdown,
  onError,
  reconcileEvery = RECONCILE_EVERY,
}) {
  let dirty = false;
  let running = false;
  let cancelled = false;
  /** @type {Set<string>} */
  let dirtyIds = new Set();
  /** @type {{ markdown: string, sidecar: object } | null} */
  let previous = null;
  let splicesSinceReconcile = 0;

  function mark(ids) {
    if (cancelled) return;
    if (Array.isArray(ids)) {
      for (const id of ids) {
        if (typeof id === 'string' && id.length > 0) dirtyIds.add(id);
      }
    }
    dirty = true;
    void loop();
  }

  async function loop() {
    if (running || cancelled) return;
    running = true;
    try {
      while (dirty && !cancelled) {
        const ids = [...dirtyIds];
        dirtyIds = new Set();
        dirty = false;
        const forceFull =
          previous == null || splicesSinceReconcile >= reconcileEvery;
        let result;
        try {
          result = await exportRun(previous, ids, { forceFull });
        } catch (err) {
          if (!cancelled && onError) onError(err);
          continue;
        }
        if (cancelled) return;
        previous = { markdown: result.markdown, sidecar: result.sidecar };
        if (result.mode === 'splice') splicesSinceReconcile += 1;
        else splicesSinceReconcile = 0;
        onMarkdown(result.markdown, result);
      }
    } finally {
      running = false;
      if (dirty && !cancelled) void loop();
    }
  }

  return {
    mark,
    start() {
      mark([]);
    },
    dispose() {
      cancelled = true;
      dirty = false;
      dirtyIds = new Set();
    },
  };
}
