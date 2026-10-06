import type { Doc } from 'yjs';
import type { SyncProvider } from '../sync-provider.js';

export function createCatalogDoc(): Doc;

export function disposeCatalog(
  provider: SyncProvider | null | undefined,
  catalog: Doc | null | undefined,
  homeId?: string,
): void;

export function openCatalog(
  provider: SyncProvider,
  workspace: unknown,
  options?: {
    signal?: AbortSignal;
    catalog?: Doc;
    alreadyConnected?: boolean;
  },
): Promise<{ catalog: Doc; provider: SyncProvider; docId: string }>;

export function attachCatalogTestHooks(
  catalog: Doc,
  workspace: unknown,
  provider?: SyncProvider | null,
  env?: Record<string, unknown>,
): void;
export function detachCatalogTestHooks(): void;

export function installPageTestHooks(
  hooks: {
    openDoc: (docId: string) => void;
    openDocId: string;
    openVector: () => string;
    insertLinkedDoc: (pageId: string) => string;
  },
  env?: Record<string, unknown>,
): void;
export function clearPageTestHooks(): void;
