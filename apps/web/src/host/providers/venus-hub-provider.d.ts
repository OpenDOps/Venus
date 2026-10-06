import type { Doc } from 'yjs';
import type { SyncProvider } from '../sync-provider.js';

export class VenusHubProvider implements SyncProvider {
  readonly kind: 'venus';
  readonly url: string;
  synced: boolean;
  /** Per-session generation. Incremented on each `connect`, not a global ready token. */
  _gen: number;
  constructor(url: string);
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(docId?: string): Promise<void>;
  on(event: 'sync', fn: () => void): () => void;
}
