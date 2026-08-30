import { afterEach, expect, test, vi } from 'vitest';
import { createMdPaneLoop } from './md-pane-loop.js';

afterEach(() => {
  vi.useRealTimers();
});

function stubExport(delayMs: number, calls: { n: number }) {
  return async () => {
    calls.n += 1;
    await new Promise((resolve) => {
      setTimeout(resolve, delayMs);
    });
    return {
      markdown: `run-${calls.n}`,
      sidecar: { docId: 'doc:home', clock: '', blocks: [] },
      mode: 'full' as const,
    };
  };
}

test('coalesce: 20 marks during a 50ms export yield at most two exportRuns', async () => {
  vi.useFakeTimers();
  const calls = { n: 0 };
  const paints: string[] = [];
  const loop = createMdPaneLoop({
    exportRun: stubExport(50, calls),
    onMarkdown: (md) => {
      paints.push(md);
    },
    reconcileEvery: 99,
  });

  loop.mark(['a']);
  await Promise.resolve();
  expect(calls.n).toBe(1);

  for (let i = 0; i < 20; i += 1) {
    loop.mark([`b${i}`]);
  }
  expect(calls.n).toBe(1);

  await vi.advanceTimersByTimeAsync(50);
  expect(calls.n).toBe(2);

  await vi.advanceTimersByTimeAsync(50);
  expect(calls.n).toBe(2);
  expect(paints).toEqual(['run-1', 'run-2']);

  loop.dispose();
});

test('dispose: in-flight export does not paint after unmount', async () => {
  vi.useFakeTimers();
  const paints: string[] = [];
  const loop = createMdPaneLoop({
    exportRun: stubExport(50, { n: 0 }),
    onMarkdown: (md) => {
      paints.push(md);
    },
  });

  loop.mark(['a']);
  await Promise.resolve();
  loop.dispose();
  await vi.advanceTimersByTimeAsync(50);
  expect(paints).toEqual([]);
});
