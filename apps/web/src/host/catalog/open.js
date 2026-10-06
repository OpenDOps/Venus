import * as Y from 'yjs';
import { CATALOG_GUID } from '../ids.js';
import { testidsFromEnv } from '../providers/from-env.js';
import { waitUntilSynced } from '../workspace.js';
import { getNode, listNodes } from './schema.js';
import {
  applyCatalogDrop,
  canCatalogDrop,
  destFromDrop,
} from './drop.js';
import { createPublishedDoc } from './create-published.js';
import {
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
 * Playwright hooks. Installed only when `VITE_TESTIDS` is set. A flag that
 * is off clears any hooks already installed.
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 * @param {import('../sync-provider.js').SyncProvider | null | undefined} [provider]
 * @param {Record<string, unknown>} [env]
 */
export function attachCatalogTestHooks(catalog, workspace, provider, env = import.meta.env) {
  if (typeof window === 'undefined') return;
  if (!testidsFromEnv(env)) {
    detachCatalogTestHooks();
    return;
  }
  window.__VENUS_CATALOG_OPS__ = {
    snapshot: () => listNodes(catalog),
    getNode: (id) => getNode(catalog, id),
    createDoc: (createAt) => {
      return createPublishedDoc(catalog, workspace, provider, {
        createAt,
      }).then((node) => ({
        id: node.id,
        docId: node.docId,
        gitPath: node.gitPath,
      }));
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

/**
 * Page-open hooks used by Playwright. Same `VITE_TESTIDS` gate as the
 * catalog hooks. Off clears a previous install.
 *
 * @param {{
 *   openDoc: (docId: string) => void,
 *   openDocId: string,
 *   openVector: () => string,
 *   insertLinkedDoc: (pageId: string) => string,
 * }} hooks
 * @param {Record<string, unknown>} [env]
 */
export function installPageTestHooks(hooks, env = import.meta.env) {
  if (typeof window === 'undefined') return;
  if (!testidsFromEnv(env)) {
    clearPageTestHooks();
    return;
  }
  window.__VENUS_OPEN_DOC__ = hooks.openDoc;
  window.__VENUS_OPEN_DOC_ID__ = hooks.openDocId;
  window.__VENUS_OPEN_VECTOR__ = hooks.openVector;
  window.__VENUS_INSERT_LINKED_DOC__ = hooks.insertLinkedDoc;
}

export function clearPageTestHooks() {
  if (typeof window === 'undefined') return;
  delete window.__VENUS_OPEN_DOC__;
  delete window.__VENUS_OPEN_DOC_ID__;
  delete window.__VENUS_OPEN_VECTOR__;
  delete window.__VENUS_INSERT_LINKED_DOC__;
}
