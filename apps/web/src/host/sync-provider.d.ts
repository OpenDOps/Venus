import type { Doc } from 'yjs';

export type SyncProviderKind = 'memory' | 'venus' | 'y-websocket';

/**
 * Live hub is `VenusHubProvider` (`kind: 'venus'`).
 * Do not import y-protocols into the editor host.
 */
export interface SyncProvider {
  readonly kind: SyncProviderKind;
  readonly synced: boolean;
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  /** Omit `docId` = every session (boot). Pass `docId` so a page open does not wait on catalog. */
  whenReady(docId?: string): Promise<void>;
  on?(event: 'sync', fn: () => void): () => void;
}

export class MemoryNoopProvider implements SyncProvider {
  readonly kind: 'memory';
  readonly synced: boolean;
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(docId?: string): Promise<void>;
  on(event: 'sync', fn: () => void): () => void;
}
