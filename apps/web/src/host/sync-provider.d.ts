import type { Doc } from 'yjs';

export type SyncProviderKind = 'memory' | 'octobase' | 'y-websocket';

/**
 * M1 implements OctoBase keck behind this.
 * Do not import OctoBase / y-protocols into the editor host.
 */
export interface SyncProvider {
  readonly kind: SyncProviderKind;
  readonly synced: boolean;
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(): Promise<void>;
  on?(event: 'sync', fn: () => void): () => void;
}

export class MemoryNoopProvider implements SyncProvider {
  readonly kind: 'memory';
  readonly synced: boolean;
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(): Promise<void>;
  on(event: 'sync', fn: () => void): () => void;
}
