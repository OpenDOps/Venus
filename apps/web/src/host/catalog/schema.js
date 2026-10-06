import * as Y from 'yjs';
import { joinGitNames } from './git-path.js';
import { repairedParent } from './structure.js';
import { PAGE_DOC_ID } from '../ids.js';

export const NODES_KEY = 'nodes';
export const FOLDER_SPEC_ID = 'folder:spec';
export const KIND_FOLDER = 'folder';
export const KIND_DOC = 'doc';

/**
 * @param {import('yjs').Doc} catalog
 * @returns {import('yjs').Map<import('yjs').Map<unknown>>}
 */
export function nodesMap(catalog) {
  return catalog.getMap(NODES_KEY);
}

/**
 * POSIX leaf on the Y.Map. Falls back to the last segment of a leftover
 * `gitPath` so maps written before L3 still join correctly.
 *
 * @param {import('yjs').Map<unknown>} ymap
 */
export function storedGitName(ymap) {
  const stored = ymap.get('gitName');
  if (typeof stored === 'string' && stored.length > 0) return stored;
  const path = ymap.get('gitPath');
  if (typeof path === 'string' && path.length > 0) {
    return path.split('/').pop() ?? '';
  }
  return '';
}

/**
 * @param {import('yjs').Map<unknown> | undefined | null} ymap
 */
export function readNode(ymap) {
  if (!ymap || typeof ymap.get !== 'function') return null;
  const parentRaw = ymap.get('parentId');
  const gitName = storedGitName(ymap);
  /** @type {{ id: string, kind: string, name: string, parentId: string | null, order: string, gitName: string, gitPath: string, docId?: string }} */
  const node = {
    id: /** @type {string} */ (ymap.get('id')),
    kind: /** @type {string} */ (ymap.get('kind')),
    name: /** @type {string} */ (ymap.get('name')),
    parentId: parentRaw == null ? null : /** @type {string} */ (parentRaw),
    order: /** @type {string} */ (ymap.get('order')),
    gitName,
    gitPath: gitName,
  };
  const docId = ymap.get('docId');
  if (typeof docId === 'string') node.docId = docId;
  return node;
}

/**
 * `gitPath` from ancestor `gitName`s. Cycle-safe (returns the looping
 * node's leaf and does not recurse).
 *
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {Map<string, string>} [memo]
 * @param {Set<string>} [visiting]
 */
/**
 * @param {import('yjs').Doc} catalog
 * @returns {Map<string, string | null>}
 */
function parentLinks(catalog) {
  /** @type {Map<string, string | null>} */
  const links = new Map();
  nodesMap(catalog).forEach((value, key) => {
    const id = value.get('id');
    const nodeId = typeof id === 'string' && id ? id : String(key);
    const parent = value.get('parentId');
    links.set(nodeId, parent == null ? null : String(parent));
  });
  return links;
}

export function gitPathOf(catalog, id, memo = new Map(), visiting = new Set(), links = null) {
  if (memo.has(id)) return /** @type {string} */ (memo.get(id));
  const ymap = nodesMap(catalog).get(id);
  if (!ymap) {
    memo.set(id, '');
    return '';
  }
  const parents = links ?? parentLinks(catalog);
  const gitName = storedGitName(ymap);
  if (visiting.has(id)) return gitName;
  visiting.add(id);
  const parentId = repairedParent(id, parents);
  const path = parentId
    ? joinGitNames(gitPathOf(catalog, parentId, memo, visiting, parents), gitName)
    : gitName;
  visiting.delete(id);
  memo.set(id, path);
  return path;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {NonNullable<ReturnType<typeof readNode>>} node
 * @param {Map<string, string>} [memo]
 * @param {Set<string>} [visiting]
 */
function withGitPath(catalog, node, memo, visiting) {
  node.gitPath = gitPathOf(catalog, node.id, memo, visiting);
  return node;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {{ gitPath?: boolean }} [opts] `gitPath: false` skips ancestor join (tree loader).
 */
export function getNode(catalog, id, opts) {
  const node = readNode(nodesMap(catalog).get(id));
  if (!node) return null;
  if (opts?.gitPath === false) return node;
  return withGitPath(catalog, node);
}

/**
 * @param {import('yjs').Doc} catalog
 */
export function listNodes(catalog) {
  /** @type {NonNullable<ReturnType<typeof readNode>>[]} */
  const out = [];
  const memo = new Map();
  const visiting = new Set();
  nodesMap(catalog).forEach((value) => {
    const node = readNode(value);
    if (node) out.push(withGitPath(catalog, node, memo, visiting));
  });
  return out;
}

/**
 * Sibling sort: fractional `order`, then `id` so equal keys are a total order.
 *
 * @param {{ order: string, id: string }} a
 * @param {{ order: string, id: string }} b
 */
export function compareNodes(a, b) {
  if (a.order < b.order) return -1;
  if (a.order > b.order) return 1;
  if (a.id < b.id) return -1;
  if (a.id > b.id) return 1;
  return 0;
}

/**
 * One scan of `nodes`, grouped by `parentId`, sorted. Does not join
 * `gitPath` — tree and sibling lists need `order` / `id` / `name` / `kind`.
 * Use `getNode` / `listNodes` when Flush or tests need the joined path.
 *
 * @param {import('yjs').Doc} catalog
 * @returns {Map<string | null, NonNullable<ReturnType<typeof readNode>>[]>}
 */
export function childrenIndex(catalog) {
  /** @type {Map<string | null, NonNullable<ReturnType<typeof readNode>>[]>} */
  const byParent = new Map();
  nodesMap(catalog).forEach((value) => {
    const node = readNode(value);
    if (!node) return;
    const key = node.parentId;
    let bucket = byParent.get(key);
    if (!bucket) {
      bucket = [];
      byParent.set(key, bucket);
    }
    bucket.push(node);
  });
  for (const bucket of byParent.values()) {
    bucket.sort(compareNodes);
  }
  return byParent;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string | null} parentId
 * @param {Map<string | null, NonNullable<ReturnType<typeof readNode>>[]>} [index]
 */
export function childrenOf(catalog, parentId, index) {
  const idx = index ?? childrenIndex(catalog);
  return idx.get(parentId) ?? [];
}

/**
 * True if any node has `parentId === id`. Pass the rebuild `childrenIndex`
 * to skip a second map scan (toolbar). Without `index`, stops at the first hit.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string | null} parentId
 * @param {Map<string | null, NonNullable<ReturnType<typeof readNode>>[]>} [index]
 */
export function hasChild(catalog, parentId, index) {
  if (index) return (index.get(parentId)?.length ?? 0) > 0;
  for (const value of nodesMap(catalog).values()) {
    if (readNode(value)?.parentId === parentId) return true;
  }
  return false;
}

/**
 * @param {string | { id?: string, docId?: string } | null | undefined} nodeOrId
 */
export function isHome(nodeOrId) {
  if (nodeOrId == null) return false;
  if (typeof nodeOrId === 'string') return nodeOrId === PAGE_DOC_ID;
  return nodeOrId.id === PAGE_DOC_ID || nodeOrId.docId === PAGE_DOC_ID;
}

/**
 * @param {import('yjs').Map<unknown>} ymap
 * @param {Record<string, unknown>} fields
 */
export function writeNodeFields(ymap, fields) {
  for (const [key, value] of Object.entries(fields)) {
    if (value === undefined || key === 'gitPath') continue;
    ymap.set(key, value);
  }
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {Record<string, unknown> & { id: string }} fields
 */
export function putNode(catalog, fields) {
  const nodes = nodesMap(catalog);
  let ymap = nodes.get(fields.id);
  if (!ymap) {
    ymap = new Y.Map();
    nodes.set(fields.id, ymap);
  }
  writeNodeFields(ymap, fields);
  if (Object.prototype.hasOwnProperty.call(fields, 'gitName') && ymap.has('gitPath')) {
    ymap.delete('gitPath');
  }
}
