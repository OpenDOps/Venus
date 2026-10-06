import * as Y from 'yjs';
import { CATALOG_GUID } from '../ids.js';
import { waitUntilSynced } from '../workspace.js';
import { getNode, listNodes } from './schema.js';
import {
  applyCatalogDrop,
  canCatalogDrop,
  destFromDrop,
} from './drop.js';
import {
  createDoc,
  createFolder,
  deleteNode,
  reparent,
  rename,
  seedOnce,
} from './ops.js';

export function createCatalogDoc() {
  return new Y.Doc({ guid: CATALOG_GUID });
}

/**
 * Drop the catalog socket and destroy the Y.Doc. Disconnect home if `homeId`
 * is passed. Safe to call twice (`isDestroyed`).
 *
 * @param {import('../sync-provider.js').SyncProvider | null | undefined} provider
 * @param {import('yjs').Doc | null | undefined} catalog
 * @param {string} [homeId]
 */
export function disposeCatalog(provider, catalog, homeId) {
  if (provider) {
    provider.disconnect(CATALOG_GUID);
    if (homeId) provider.disconnect(homeId);
  }
  if (catalog && !catalog.isDestroyed) catalog.destroy();
}

/**
 * Plain Y.Doc (`venus:catalog`), not `affine:page`. Connect through the
 * same SyncProvider as pages. Seed-once after hydrate.
 *
 * Pass `{ catalog, alreadyConnected: true }` when the provider already
 * connected this doc in the same wait as home (one handshake, not two).
 *
 * @param {import('../sync-provider.js').SyncProvider} provider
 * @param {unknown} workspace
 * @param {{ signal?: AbortSignal, catalog?: import('yjs').Doc, alreadyConnected?: boolean }} [options]
 */
export async function openCatalog(provider, workspace, options = {}) {
  const owned = !options.catalog;
  const catalog = options.catalog ?? createCatalogDoc();
  const alreadyConnected = Boolean(options.alreadyConnected);
  if (!alreadyConnected) {
    provider.connect(CATALOG_GUID, catalog);
  }
  try {
    if (!alreadyConnected) {
      await waitUntilSynced(provider, options.signal, CATALOG_GUID);
    }
    if (options.signal?.aborted) {
      const err = new Error('workspace create aborted');
      err.name = 'AbortError';
      throw err;
    }
    seedOnce(catalog, workspace);
  } catch (err) {
    if (!alreadyConnected) provider.disconnect(CATALOG_GUID);
    if (owned && !catalog.isDestroyed) catalog.destroy();
    throw err;
  }
  return { catalog, provider, docId: CATALOG_GUID };
}

/**
 * Playwright / Vitest hooks until the tree UI exists (step 5).
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 */
export function attachCatalogTestHooks(catalog, workspace) {
  if (typeof window === 'undefined') return;
  window.__VENUS_CATALOG_OPS__ = {
    snapshot: () => listNodes(catalog),
    getNode: (id) => getNode(catalog, id),
    createDoc: (createAt) => {
      const node = createDoc(catalog, workspace, { createAt });
      return { id: node.id, docId: node.docId, gitPath: node.gitPath };
    },
    createFolder: (createAt, name) => {
      const node = createFolder(catalog, { createAt, name });
      return { id: node.id, gitPath: node.gitPath };
    },
    rename: (id, name) => {
      const node = rename(catalog, workspace, id, name);
      return { gitPath: node.gitPath, name: node.name };
    },
    reparent: (id, parentId) => {
      const node = reparent(catalog, id, { parentId });
      return { gitPath: node.gitPath, parentId: node.parentId };
    },
    drop: (id, parentId) => {
      const dest = destFromDrop(catalog, [id], { parentId });
      const ok = applyCatalogDrop(catalog, [id], dest);
      const node = getNode(catalog, id);
      return {
        ok,
        gitPath: node?.gitPath ?? null,
        parentId: node?.parentId ?? null,
      };
    },
    canDrop: (id, parentId) => canCatalogDrop(catalog, [id], parentId),
    deleteNode: (id) => {
      deleteNode(catalog, id);
    },
  };
  window.__VENUS_CATALOG_READY__ = true;
}

export function detachCatalogTestHooks() {
  if (typeof window === 'undefined') return;
  delete window.__VENUS_CATALOG_OPS__;
  delete window.__VENUS_CATALOG_READY__;
}
