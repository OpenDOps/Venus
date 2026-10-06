import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

export const SYNC_TIMEOUT_MS: number;
export const OPEN_PAGE_ATTEMPTS: number;
export const OPEN_PAGE_RETRY_MS: number;

export function isOpenPageTransportError(err: unknown): boolean;

export function shouldSeedEmptyPage(
  provider: { kind?: string },
  options?: { seedIfEmpty?: boolean },
): boolean;

export function waitUntilSynced(
  provider: SyncProvider,
  signal?: AbortSignal,
  docId?: string,
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
  docs: { size: number; has: (id: string) => boolean };
  meta: {
    docMetas: unknown[];
    getDocMeta?: (id: string) => { title?: string } | undefined;
    setDocMeta?: (id: string, props: Record<string, unknown>) => void;
  };
  createDoc: (id: string) => { getStore: () => M0Store };
  getDoc?: (id: string) => { getStore: () => M0Store; spaceDoc: Doc } | null;
};

type HistorySignal = {
  peek: () => boolean;
  subscribe: (fn: (value: boolean) => void) => () => void;
};

type M0Store = {
  root: BlockNode | null;
  spaceDoc: Doc;
  doc: { id: string };
  resetHistory: () => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  history: {
    canUndo$: HistorySignal;
    canRedo$: HistorySignal;
  };
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

export function seedEmptyPageIfNeeded(store: M0Store): boolean;

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
 * Empty `affine:page` seed only in memory (or `seedIfEmpty`). Hub empty
 * after sync throws — do not broadcast a blank page.
 */
export function openWorkspaceDoc(
  workspace: M0Workspace,
  provider: SyncProvider,
  uuid: string,
  options?: { signal?: AbortSignal; seedIfEmpty?: boolean },
): Promise<{
  workspace: M0Workspace;
  store: M0Store;
  docId: string;
  provider: SyncProvider;
}>;

/** Retries `openWorkspaceDoc`. Abort `signal` to stop further connects. */
export function openPageStore(
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
