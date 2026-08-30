import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

type BlockNode = {
  flavour: string;
  children: BlockNode[];
  props: {
    type?: string;
    text?: { toString: () => string };
    title?: { toString: () => string };
  };
};

export function createM0Workspace(provider?: SyncProvider): {
  workspace: { docs: { size: number } };
  store: {
    root: BlockNode | null;
    spaceDoc: Doc;
    resetHistory: () => void;
    undo: () => void;
    canUndo: boolean;
  };
  docId: string;
  provider: SyncProvider;
};
