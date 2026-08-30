import type { Doc } from 'yjs';

export type SidecarBlockRange = {
  id: string;
  start: number;
  end: number;
};

export type Sidecar = {
  docId: string;
  clock: string;
  blocks: SidecarBlockRange[];
};

export function encodeSidecarClock(ydoc: Doc): string;

export function fromDoc(
  store: {
    root: {
      id: string;
      flavour: string;
      props?: { title?: { toString: () => string } };
      children?: unknown[];
    } | null;
    spaceDoc: Doc;
    doc?: { id: string };
    id?: string;
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
  },
  workspace: { id: string; meta: { docMetas: unknown[] } },
): Promise<{ markdown: string; sidecar: Sidecar }>;

export function roundTripFromDoc(
  store: Parameters<typeof fromDoc>[0],
  workspace: Parameters<typeof fromDoc>[1],
): Promise<{
  first: { markdown: string; sidecar: Sidecar };
  imported: {
    root: {
      children: {
        flavour: string;
        children: { flavour: string; props: { type?: string } }[];
      }[];
    } | null;
  };
  second: { markdown: string; sidecar: Sidecar };
}>;
