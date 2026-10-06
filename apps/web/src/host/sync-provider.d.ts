import type { Doc } from 'yjs';

export type SyncProviderKind = 'memory' | 'venus' | 'y-websocket';

/**
 * Live hub is `VenusHubProvider` (`kind: 'venus'`).
 * Do not import y-protocols into the editor host.
 */
export interface SyncProvider {
  readonly kind: SyncProviderKind;
  readonly synced: boolean;
  /** `reconnecting` after a live socket drops. Memory stays unset. */
  readonly connection?: 'synced' | 'reconnecting';
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  /** Omit `docId` = every session (boot). Pass `docId` so a page open does not wait on catalog. */
  whenReady(docId?: string): Promise<void>;
  /**
   * Resolve on the next sync step 2. Live page create uses this to drop the
   * handshake reply that arrives after `whenReady`.
   */
  nextStep2?(docId: string): Promise<void>;
  /**
   * Resolve when the hub's next sync step 2 contains this doc's page seed.
   * Live page create calls this after the seed update is already on the socket.
   */
  confirmApplied?(docId: string): Promise<void>;
  on?(event: 'sync' | 'connection', fn: () => void): () => void;
}

export class MemoryNoopProvider implements SyncProvider {
  readonly kind: 'memory';
  readonly synced: boolean;
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  whenReady(docId?: string): Promise<void>;
  on(event: 'sync', fn: () => void): () => void;
}
