import { generateKeyBetween } from 'fractional-indexing';
import { PAGE_DOC_ID } from '../ids.js';
import { seedEmptyPageIfNeeded } from '../workspace.js';
import {
  filenameFromDocname,
  gitNameFromStem,
  sanitizeDocname,
  stemOfGitName,
} from './git-path.js';
import {
  FOLDER_SPEC_ID,
  KIND_DOC,
  KIND_FOLDER,
  childrenIndex,
  childrenOf,
  getNode,
  hasChild,
  isHome,
  nodesMap,
  putNode,
} from './schema.js';

export class CatalogError extends Error {
  /**
   * @param {string} code
   * @param {string} [message]
   */
  constructor(code, message) {
    super(message ?? code);
    this.name = 'CatalogError';
    this.code = code;
  }
}

/**
 * @param {unknown} workspace
 * @param {string} id
 */
function hasWorkspaceDoc(workspace, id) {
  if (typeof workspace?.getDoc === 'function' && workspace.getDoc(id)) {
    return true;
  }
  return Boolean(workspace?.docs?.has?.(id));
}

/**
 * @param {unknown} workspace
 * @param {string} uuid
 */
function ensureWorkspaceDoc(workspace, uuid) {
  if (hasWorkspaceDoc(workspace, uuid)) return;
  const doc = workspace?.createDoc?.(uuid);
  const store = doc?.getStore?.();
  if (store) seedEmptyPageIfNeeded(store);
}

/**
 * @param {unknown} workspace
 * @param {string} docId
 * @param {string} title
 */
function setDocTitle(workspace, docId, title) {
  workspace?.meta?.setDocMeta?.(docId, { title });
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string | null} parentId
 */
function requireFolderParent(catalog, parentId) {
  if (parentId == null) return null;
  const parent = getNode(catalog, parentId);
  if (!parent || parent.kind !== KIND_FOLDER) {
    throw new CatalogError('invalid_parent', 'createAt / parentId must be a folder or null');
  }
  return parent;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {string | null} newParentId
 */
function assertNotCycle(catalog, id, newParentId) {
  let cur = newParentId;
  const seen = new Set();
  while (cur) {
    if (cur === id) {
      throw new CatalogError('invalid_parent', 'reparent would cycle');
    }
    if (seen.has(cur)) break;
    seen.add(cur);
    cur = getNode(catalog, cur)?.parentId ?? null;
  }
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string | null} parentId
 * @param {string} [exceptId]
 * @param {ReturnType<typeof childrenIndex>} [index]
 */
function siblingFilenames(catalog, parentId, exceptId, index) {
  const taken = new Set();
  for (const child of childrenOf(catalog, parentId, index)) {
    if (child.id === exceptId) continue;
    taken.add(stemOfGitName(child.gitName, /** @type {'folder' | 'doc'} */ (child.kind)));
  }
  return taken;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string | null} destParentId
 * @param {string} neighborId
 */
function requireDestSibling(catalog, destParentId, neighborId) {
  const neighbor = getNode(catalog, neighborId);
  if (!neighbor) {
    throw new CatalogError('invalid_parent', 'afterId / beforeId must exist');
  }
  if (neighbor.parentId !== destParentId) {
    throw new CatalogError(
      'invalid_parent',
      'afterId / beforeId must be children of the destination folder',
    );
  }
  return neighbor;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string | null} parentId
 * @param {{ afterId?: string | null, beforeId?: string | null, exceptId?: string, index?: ReturnType<typeof childrenIndex> }} [opts]
 */
function orderBetween(catalog, parentId, opts = {}) {
  if (opts.afterId || opts.beforeId) {
    const afterNode = opts.afterId
      ? requireDestSibling(catalog, parentId, opts.afterId)
      : null;
    const beforeNode = opts.beforeId
      ? requireDestSibling(catalog, parentId, opts.beforeId)
      : null;
    const after = afterNode?.order ?? null;
    const before = beforeNode?.order ?? null;
    if (after != null && before != null && after === before) {
      const siblings = childrenOf(catalog, parentId, opts.index).filter(
        (n) => n.id !== opts.exceptId,
      );
      const nextDistinct = siblings.find((n) => n.order > after)?.order ?? null;
      return generateKeyBetween(after, nextDistinct);
    }
    return generateKeyBetween(after, before);
  }
  const siblings = childrenOf(catalog, parentId, opts.index).filter(
    (n) => n.id !== opts.exceptId,
  );
  const last = siblings.at(-1);
  return generateKeyBetween(last?.order ?? null, null);
}

/**
 * Seed folder:spec + doc:home if missing. Fixed ids (not “map is empty”).
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} [workspace]
 */
export function seedOnce(catalog, workspace) {
  const specMissing = !getNode(catalog, FOLDER_SPEC_ID);
  const homeMissing = !getNode(catalog, PAGE_DOC_ID);
  if (!specMissing && !homeMissing) return;

  catalog.transact(() => {
    const index = childrenIndex(catalog);
    if (!getNode(catalog, FOLDER_SPEC_ID)) {
      const taken = siblingFilenames(catalog, null, undefined, index);
      const filename = filenameFromDocname('spec', {
        fallback: 'spec',
        taken,
      });
      putNode(catalog, {
        id: FOLDER_SPEC_ID,
        kind: KIND_FOLDER,
        name: 'spec',
        parentId: null,
        order: orderBetween(catalog, null, { index }),
        gitName: gitNameFromStem(filename, KIND_FOLDER),
      });
    }
    if (!getNode(catalog, PAGE_DOC_ID)) {
      const taken = siblingFilenames(catalog, FOLDER_SPEC_ID, undefined, index);
      const filename = filenameFromDocname('home', {
        fallback: PAGE_DOC_ID,
        taken,
      });
      putNode(catalog, {
        id: PAGE_DOC_ID,
        kind: KIND_DOC,
        name: 'home',
        parentId: FOLDER_SPEC_ID,
        order: orderBetween(catalog, FOLDER_SPEC_ID, { index }),
        docId: PAGE_DOC_ID,
        gitName: gitNameFromStem(filename, KIND_DOC),
      });
    }
  });

  if (homeMissing) setDocTitle(workspace, PAGE_DOC_ID, 'home');
}

/**
 * Mint a page id. Does not write the catalog or the page body.
 * Live creates use this, seed on the hub, then `commitNewDoc`.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string | null | undefined} createAt
 */
export function mintDocId(catalog, createAt) {
  if (createAt === undefined) {
    throw new CatalogError('invalid_parent', 'createDoc requires createAt');
  }
  requireFolderParent(catalog, createAt);
  return crypto.randomUUID();
}

/**
 * Insert the catalog node. `gitName` and `order` are taken at this moment
 * so a sibling created while the page seed was syncing is visible.
 * The page body must already exist when this is the live path.
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 * @param {{ id: string, createAt: string | null }} stage
 */
export function commitNewDoc(catalog, workspace, stage) {
  const { id, createAt } = stage;
  const index = childrenIndex(catalog);
  const filename = filenameFromDocname(id, {
    fallback: id,
    taken: siblingFilenames(catalog, createAt, undefined, index),
  });
  const gitName = gitNameFromStem(filename, KIND_DOC);
  const order = orderBetween(catalog, createAt, { index });
  catalog.transact(() => {
    putNode(catalog, {
      id,
      kind: KIND_DOC,
      name: id,
      parentId: createAt,
      order,
      docId: id,
      gitName,
    });
  });
  setDocTitle(workspace, id, id);
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', 'createDoc failed');
  return node;
}

/**
 * Catalog node plus a local empty page. No socket.
 * Product creates go through `createPublishedDoc`, which seeds on the hub
 * before this node's insert.
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 * @param {{ createAt: string | null }} options
 */
export function createDoc(catalog, workspace, options) {
  const createAt = options?.createAt;
  const id = mintDocId(catalog, createAt);
  ensureWorkspaceDoc(workspace, id);
  return commitNewDoc(catalog, workspace, {
    id,
    createAt: createAt ?? null,
  });
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {{ createAt: string | null, name: string }} options
 */
export function createFolder(catalog, options) {
  const createAt = options?.createAt;
  if (createAt === undefined) {
    throw new CatalogError('invalid_parent', 'createFolder requires createAt');
  }
  requireFolderParent(catalog, createAt);
  const uuid = crypto.randomUUID();
  const id = `folder:${uuid}`;
  const name = sanitizeDocname(options.name) || uuid;
  const index = childrenIndex(catalog);
  const filename = filenameFromDocname(name, {
    fallback: uuid,
    taken: siblingFilenames(catalog, createAt, undefined, index),
  });
  const gitName = gitNameFromStem(filename, KIND_FOLDER);
  const order = orderBetween(catalog, createAt, { index });
  catalog.transact(() => {
    putNode(catalog, {
      id,
      kind: KIND_FOLDER,
      name,
      parentId: createAt,
      order,
      gitName,
    });
  });
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', 'createFolder failed');
  return node;
}

/**
 * Commit a docname. Empty after sanitize is a no-op (keep previous name).
 * Tree UI may show `''` while the input is focused.
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 * @param {string} id
 * @param {string} name
 */
export function rename(catalog, workspace, id, name) {
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', `unknown node ${id}`);
  const nextName = sanitizeDocname(name);
  if (nextName === '') return node;
  if (nextName === node.name) return node;
  const index = childrenIndex(catalog);
  const filename = filenameFromDocname(nextName, {
    fallback: node.kind === KIND_DOC ? (node.docId ?? node.id) : node.id.replace(/^folder:/, ''),
    taken: siblingFilenames(catalog, node.parentId, id, index),
  });
  const gitName = gitNameFromStem(filename, /** @type {'folder' | 'doc'} */ (node.kind));
  catalog.transact(() => {
    putNode(catalog, { id, name: nextName, gitName });
  });
  if (node.kind === KIND_DOC) {
    setDocTitle(workspace, node.docId ?? id, nextName);
  }
  const next = getNode(catalog, id);
  if (!next) throw new CatalogError('invalid_parent', 'rename failed');
  return next;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {{ parentId: string | null, afterId?: string | null, beforeId?: string | null }} dest
 */
export function reparent(catalog, id, dest) {
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', `unknown node ${id}`);
  if (isHome(node)) {
    throw new CatalogError('home_protected', 'home_protected');
  }
  const parentId = dest.parentId;
  requireFolderParent(catalog, parentId);
  assertNotCycle(catalog, id, parentId);
  const index = childrenIndex(catalog);
  const filename = filenameFromDocname(node.name, {
    fallback: node.kind === KIND_DOC ? (node.docId ?? node.id) : node.id.replace(/^folder:/, ''),
    taken: siblingFilenames(catalog, parentId, id, index),
  });
  const gitName = gitNameFromStem(filename, /** @type {'folder' | 'doc'} */ (node.kind));
  const order = orderBetween(catalog, parentId, {
    afterId: dest.afterId,
    beforeId: dest.beforeId,
    exceptId: id,
    index,
  });
  catalog.transact(() => {
    putNode(catalog, { id, parentId, order, gitName });
  });
  const next = getNode(catalog, id);
  if (!next) throw new CatalogError('invalid_parent', 'reparent failed');
  return next;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {{ afterId?: string | null, beforeId?: string | null }} pos
 */
export function setOrder(catalog, id, pos = {}) {
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', `unknown node ${id}`);
  const order = orderBetween(catalog, node.parentId, {
    afterId: pos.afterId,
    beforeId: pos.beforeId,
    exceptId: id,
    index: childrenIndex(catalog),
  });
  catalog.transact(() => {
    putNode(catalog, { id, order });
  });
  const next = getNode(catalog, id);
  if (!next) throw new CatalogError('invalid_parent', 'setOrder failed');
  return next;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 */
export function deleteNode(catalog, id) {
  const node = getNode(catalog, id);
  if (!node) throw new CatalogError('invalid_parent', `unknown node ${id}`);
  if (isHome(node)) {
    throw new CatalogError('home_protected', 'home_protected');
  }
  if (hasChild(catalog, id)) {
    throw new CatalogError('node_not_empty', 'node_not_empty');
  }
  catalog.transact(() => {
    nodesMap(catalog).delete(id);
  });
}
