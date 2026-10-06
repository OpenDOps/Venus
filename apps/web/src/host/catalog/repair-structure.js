import { generateKeyBetween } from 'fractional-indexing';
import { WIKI_ROOT_ID } from './drop.js';
import { childrenIndex, childrenOf, compareNodes, nodesMap, putNode } from './schema.js';
import { repairedParent } from './structure.js';

/** Virtual tree folder. Not a catalog node. */
export const UNFILED_ID = 'wiki:unfiled';

/**
 * @param {import('yjs').Doc} catalog
 * @returns {Map<string, string | null>}
 */
export function parentLinks(catalog) {
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

/**
 * Nodes whose parent was deleted. Descendants of those nodes stay nested
 * under them. Sorted like other siblings.
 *
 * @param {import('yjs').Doc} catalog
 * @param {ReturnType<typeof import('./schema.js').childrenIndex>} [index]
 */
export function orphanNodes(catalog, index) {
  const links = parentLinks(catalog);
  const grouped = index ?? childrenIndex(catalog);
  return [...grouped.values()]
    .flat()
    .filter((node) => node.parentId && !links.has(node.parentId))
    .sort(compareNodes);
}

/**
 * Child ids for the headless tree. Orphans hang under {@link UNFILED_ID}.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string} itemId
 * @param {ReturnType<typeof import('./schema.js').childrenIndex>} index
 */
export function catalogTreeChildIds(catalog, itemId, index) {
  if (itemId === UNFILED_ID) {
    return orphanNodes(catalog, index).map((node) => node.id);
  }
  const parentId = itemId === WIKI_ROOT_ID ? null : itemId;
  const ids = childrenOf(catalog, parentId, index).map((node) => node.id);
  if (itemId === WIKI_ROOT_ID && orphanNodes(catalog, index).length > 0) {
    ids.push(UNFILED_ID);
  }
  return ids;
}

/**
 * Move orphans to the root, and break each cycle at its greatest id by
 * moving that node to the root. Idempotent. Every tab computes the same
 * parents, so concurrent repairs converge.
 *
 * @param {import('yjs').Doc} catalog
 * @returns {boolean} true when a parent was rewritten
 */
export function repairCatalogStructure(catalog) {
  const links = parentLinks(catalog);
  const ids = [...links.keys()].filter((id) => repairedParent(id, links) !== links.get(id));
  if (ids.length === 0) return false;
  ids.sort();
  const root = childrenOf(catalog, null);
  let last = root.length > 0 ? root[root.length - 1].order : null;
  catalog.transact(() => {
    for (const id of ids) {
      const order = generateKeyBetween(last, null);
      last = order;
      putNode(catalog, { id, parentId: null, order });
    }
  });
  return true;
}
