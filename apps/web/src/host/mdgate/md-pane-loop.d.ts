import type { Sidecar } from './from-doc.js';
import type { IncrementalExport } from './splice.js';

export const RECONCILE_EVERY: number;

export type MdPaneLoop = {
  mark: (ids?: string[]) => void;
  start: () => void;
  dispose: () => void;
};

export function createMdPaneLoop(options: {
  exportRun: (
    previous: { markdown: string; sidecar: Sidecar } | null,
    dirtyIds: string[],
    options: { forceFull: boolean },
  ) => Promise<IncrementalExport>;
  onMarkdown: (markdown: string, result: IncrementalExport) => void;
  onError?: (err: unknown) => void;
  reconcileEvery?: number;
}): MdPaneLoop;
