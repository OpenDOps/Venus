import type { Sidecar } from './from-doc.js';
import type { fromDoc } from './from-doc.js';

export type IncrementalExport = {
  markdown: string;
  sidecar: Sidecar;
  mode: 'splice' | 'full';
};

export function incrementalFromDoc(
  store: Parameters<typeof fromDoc>[0],
  workspace: Parameters<typeof fromDoc>[1],
  previous: { markdown: string; sidecar: Sidecar },
  dirtyIds: string[],
  options?: { forceFull?: boolean },
): Promise<IncrementalExport>;
