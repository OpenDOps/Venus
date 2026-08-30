import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

export const SYNC_TIMEOUT_MS: number;

type BlockNode = {
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
  workspace: { docs: { size: number } };
  store: {
    root: BlockNode | null;
    spaceDoc: Doc;
    resetHistory: () => void;
    undo: () => void;
    canUndo: boolean;
    blobSync: {
      main: { name: string };
      set: (value: Blob) => Promise<string>;
      get: (key: string) => Promise<Blob | null>;
    };
  };
  docId: string;
  provider: SyncProvider;
}>;
