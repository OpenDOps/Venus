import type { SyncProvider } from '../sync-provider.js';

export const KEEP_PAGE_SESSIONS: number;

export class PageSessions {
  constructor(provider: SyncProvider, options?: { keep?: number });
  has(docId: string): boolean;
  ids(): string[];
  touch(docId: string): void;
  release(docId: string): void;
  dispose(): void;
}
