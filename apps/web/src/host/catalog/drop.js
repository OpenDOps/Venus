import {
  KIND_FOLDER,
  childrenOf,
  getNode,
  isHome,
} from './schema.js';
import { reparent, setOrder } from './ops.js';

/** Virtual wiki root for headless-tree. Not a catalog node. */
export const WIKI_ROOT_ID = 'wiki:root';

/**
 * @param {string} itemId
 * @returns {string | null}
 */
export function catalogParentId(itemId) {
  if (itemId == null || itemId === WIKI_ROOT_ID) return null;
  return itemId;
}

/**
 * @param {import('yjs').Doc} catalog
 * @param {string} id
 * @param {string | null} newParentId
 */
export function wouldCycle(catalog, id, newParentId) {
  let cur = newParentId;
  const seen = new Set();
  while (cur) {
    if (cur === id) return true;
    if (seen.has(cur)) break;
    seen.add(cur);
    cur = getNode(catalog, cur)?.parentId ?? null;
  }
  return false;
}

/**
 * Dest parent for a drop. Unordered = drop onto that folder; ordered =
 * insert among that folder's children. `wiki:root` → `parentId: null`.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string[]} draggedIds
 * @param {{ parentId: string | null, childIndex?: number }} target
 */
export function destFromDrop(catalog, draggedIds, target) {
  const parentId = target.parentId;
  if (typeof target.childIndex !== 'number') {
    return { parentId };
  }
  const skipped = new Set(draggedIds);
  const siblings = childrenOf(catalog, parentId).filter(
    (n) => !skipped.has(n.id),
  );
  const idx = Math.max(0, Math.min(target.childIndex, siblings.length));
  return {
    parentId,
    afterId: siblings[idx - 1]?.id ?? null,
    beforeId: siblings[idx]?.id ?? null,
  };
}

/**
 * UI gate: home cannot change parent; dest must be a folder or wiki root;
 * drop onto a descendant is rejected.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string[]} draggedIds
 * @param {string | null} destParentId
 */
export function canCatalogDrop(catalog, draggedIds, destParentId) {
  if (!draggedIds.length) return false;
  if (destParentId != null) {
    const parent = getNode(catalog, destParentId);
    if (!parent || parent.kind !== KIND_FOLDER) return false;
  }
  for (const id of draggedIds) {
    const node = getNode(catalog, id);
    if (!node) return false;
    if (isHome(node) && destParentId !== node.parentId) return false;
    if (wouldCycle(catalog, id, destParentId)) return false;
  }
  return true;
}

/**
 * Apply a tree drop. Same-parent → `setOrder`; otherwise `reparent`.
 * Home stays under spec (`canCatalogDrop` false if parent would change).
 * Multi-item: first-to-last, `afterId` walks to the just-placed node so
 * each fractional key sits in a fresh gap (not one dest slot reused).
 * One outer `catalog.transact` — nested `setOrder` / `reparent` txs join.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string[]} draggedIds
 * @param {{ parentId: string | null, afterId?: string | null, beforeId?: string | null }} dest
 * @returns {boolean} false if the UI must not complete the drop
 */
export function applyCatalogDrop(catalog, draggedIds, dest) {
  if (!canCatalogDrop(catalog, draggedIds, dest.parentId)) return false;
  catalog.transact(() => {
    let afterId = dest.afterId ?? null;
    const beforeId = dest.beforeId ?? null;
    const slotted =
      dest.afterId !== undefined || dest.beforeId !== undefined;
    let placed = false;
    for (const id of draggedIds) {
      const node = getNode(catalog, id);
      if (!node) continue;
      const pos =
        slotted || placed ? { afterId, beforeId } : {};
      if (node.parentId === dest.parentId) {
        setOrder(catalog, id, pos);
      } else {
        reparent(catalog, id, { parentId: dest.parentId, ...pos });
      }
      afterId = id;
      placed = true;
    }
  });
  return true;
}
