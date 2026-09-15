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

export function attachCatalogTestHooks(catalog: Doc, workspace: unknown): void;
export function detachCatalogTestHooks(): void;
