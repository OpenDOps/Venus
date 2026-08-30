import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

export const SYNC_TIMEOUT_MS: number;

type BlockNode = {
  id: string;
  flavour: string;
  children: BlockNode[];
  props: {
    type?: string;
    text?: { toString: () => string };
    title?: { toString: () => string };
  };
};

export function createM0Workspace(
  provider?: SyncProvider,
  options?: {
    signal?: AbortSignal;
    blobSources?: { main: { name: string }; shadows?: { name: string }[] };
  },
): Promise<{
  workspace: {
    id: string;
    docs: { size: number };
    meta: { docMetas: unknown[] };
  };
  store: {
    root: BlockNode | null;
    spaceDoc: Doc;
    doc: { id: string };
    resetHistory: () => void;
    undo: () => void;
    canUndo: boolean;
    addBlock: (
      flavour: string,
      props: Record<string, unknown>,
      parentId?: string,
    ) => string;
    deleteBlock: (model: string | { id: string }) => void;
    getTransformer: (middlewares?: unknown[]) => unknown;
    provider: unknown;
    blobSync: {
      main: { name: string };
      set: (value: Blob) => Promise<string>;
      get: (key: string) => Promise<Blob | null>;
    };
  };
  docId: string;
  provider: SyncProvider;
}>;
