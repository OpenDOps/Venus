import type { Doc } from 'yjs';
import type { SyncProvider } from '../sync-provider.js';
import type { HubWorkerStats } from './shared-worker-socket.js';

export const RECONNECT_BASE_MS: number;
export const RECONNECT_CAP_MS: number;
export function reconnectDelayMs(
  attempt: number,
  random?: () => number,
): number;

export interface HubSocketSource {
  readonly failed: boolean;
  open(url: string): WebSocket;
  stats?(): Promise<HubWorkerStats>;
}

export class VenusHubProvider implements SyncProvider {
  readonly kind: 'venus';
  readonly url: string;
  synced: boolean;
  connection: 'synced' | 'reconnecting';
  /** Per-session generation. Incremented on each `connect`, not a global ready token. */
  _gen: number;
  /** `shared-worker` while the worker holds new sockets; `tab` otherwise. */
  readonly transport: 'shared-worker' | 'tab';
  constructor(url: string, options?: { sockets?: HubSocketSource | null });
  connect(docId: string, ydoc: Doc): void;
  disconnect(docId: string): void;
  nextStep2(docId: string): Promise<void>;
  confirmApplied(docId: string): Promise<void>;
  whenReady(docId?: string): Promise<void>;
  on(event: 'sync' | 'connection', fn: () => void): () => void;
  hubStats(): Promise<HubWorkerStats | null>;
}
