import { CATALOG_GUID, PAGE_DOC_ID } from '../ids.js';

/** The open page plus the one visited before it stay connected. */
export const KEEP_PAGE_SESSIONS = 2;

/**
 * Live sockets for recently opened pages (not home, not the catalog: those
 * stay connected for the app's lifetime). A kept page keeps receiving remote
 * updates while hidden, so a revisit binds its Store with no handshake.
 *
 * Release a page when it falls out of the recent set, leaves the catalog,
 * or the app unmounts.
 */
export class PageSessions {
  /**
   * @param {import('../sync-provider.js').SyncProvider} provider
   * @param {{ keep?: number }} [options]
   */
  constructor(provider, options = {}) {
    this._provider = provider;
    this._keep = Math.max(1, options.keep ?? KEEP_PAGE_SESSIONS);
    /** Oldest first. @type {string[]} */
    this._ids = [];
  }

  /** @param {string} docId */
  has(docId) {
    return this._ids.includes(docId);
  }

  /** Kept page ids, oldest first. */
  ids() {
    return [...this._ids];
  }

  /**
   * Record `docId` (already connected and synced) as the most recent page.
   * Disconnects the oldest pages past the limit; never `docId`.
   *
   * @param {string} docId
   */
  touch(docId) {
    if (docId === PAGE_DOC_ID || docId === CATALOG_GUID) return;
    this._ids = this._ids.filter((id) => id !== docId);
    this._ids.push(docId);
    while (this._ids.length > this._keep) {
      const oldest = /** @type {string} */ (this._ids.shift());
      this._provider.disconnect(oldest);
    }
  }

  /**
   * Disconnect a kept page (deleted from the catalog, or its Store is gone).
   *
   * @param {string} docId
   */
  release(docId) {
    if (!this.has(docId)) return;
    this._ids = this._ids.filter((id) => id !== docId);
    this._provider.disconnect(docId);
  }

  dispose() {
    const ids = this._ids;
    this._ids = [];
    for (const id of ids) this._provider.disconnect(id);
  }
}
