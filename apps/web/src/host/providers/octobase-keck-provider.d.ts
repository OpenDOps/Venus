import type { Doc } from 'yjs';
import type { SyncProvider } from '../sync-provider.js';

export class OctoBaseKeckProvider implements SyncProvider {
  readonly kind: 'octobase';
  readonly url: string;
  synced: boolean;
  constructor(url: string);
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(): Promise<void>;
  on(event: 'sync', fn: () => void): () => void;
}
