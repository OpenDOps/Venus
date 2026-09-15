import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

export const SYNC_TIMEOUT_MS: number;

export function waitUntilSynced(
  provider: SyncProvider,
  signal?: AbortSignal,
): Promise<void>;

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

type M0Workspace = {
  id: string;
  docs: { size: number };
  meta: {
    docMetas: unknown[];
    getDocMeta?: (id: string) => { title?: string } | undefined;
    setDocMeta?: (id: string, props: Record<string, unknown>) => void;
  };
  createDoc: (id: string) => { getStore: () => M0Store };
  getDoc?: (id: string) => { spaceDoc: Doc } | null;
};

type M0Store = {
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
    parentIndex?: number,
  ) => string;
  updateBlock: (id: string, props: Record<string, unknown>) => void;
  getBlock: (id: string) => { model?: { text?: { toDelta?: () => unknown } } };
  deleteBlock: (model: string | { id: string }) => void;
  getTransformer: (middlewares?: unknown[]) => unknown;
  provider: unknown;
  blobSync: {
    main: { name: string };
    set: (value: Blob) => Promise<string>;
    get: (key: string) => Promise<Blob | null>;
  };
};

type BlobSourcesOption = {
  blobSources?: { main: { name: string }; shadows?: { name: string }[] };
};

/** Offline Store from Yjs update v1. No seed, no SyncProvider. */
export function hydrateM0FromUpdate(
  bytes: Uint8Array,
  options?: BlobSourcesOption,
): {
  workspace: M0Workspace;
  store: M0Store;
  docId: string;
};

export function createM0Workspace(
  provider?: SyncProvider,
  options?: BlobSourcesOption & {
    signal?: AbortSignal;
    /** Extra docs to connect before the single whenReady wait (catalog). */
    connectDocs?: Array<{ docId: string; ydoc: Doc }>;
  },
): Promise<{
  workspace: M0Workspace;
  store: M0Store;
  docId: string;
  provider: SyncProvider;
}>;

/**
 * Second page on an existing collection. `uuid` is the minted SQL uuid.
 * Empty `affine:page` seed (not home). Do not call from App boot.
 */
export function openWorkspaceDoc(
  workspace: M0Workspace,
  provider: SyncProvider,
  uuid: string,
  options?: { signal?: AbortSignal },
): Promise<{
  workspace: M0Workspace;
  store: M0Store;
  docId: string;
  provider: SyncProvider;
}>;
