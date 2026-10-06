import type { Doc } from 'yjs';
import type { CatalogNode } from './schema.js';
import type { SyncProvider } from '../sync-provider.js';

export function createPublishedDoc(
  catalog: Doc,
  workspace: unknown,
  provider: SyncProvider | null | undefined,
  options: { createAt: string | null; signal?: AbortSignal },
): Promise<CatalogNode>;
