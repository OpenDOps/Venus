import { WIKI_ROOT_ID } from './drop.js';
import { getNode } from './schema.js';

/**
 * Selection ids that still exist in the catalog (wiki root is virtual).
 *
 * @param {import('yjs').Doc} catalog
 * @param {string[]} selectedIds
 */
export function keepSelectedIds(catalog, selectedIds) {
  return selectedIds.filter(
    (id) => id === WIKI_ROOT_ID || getNode(catalog, id, { gitPath: false }),
  );
}

/**
 * True when the rename target was deleted / remote-removed.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string | null | undefined} renamingId
 */
export function isRenamingGone(catalog, renamingId) {
  if (renamingId == null || renamingId === '' || renamingId === WIKI_ROOT_ID) {
    return false;
  }
  return !getNode(catalog, renamingId, { gitPath: false });
}

/**
 * After rebuild: drop selection / abort rename if those ids left the catalog.
 *
 * @param {{
 *   getState: () => { selectedItems?: string[], renamingItem?: string | null },
 *   setSelectedItems: (ids: string[]) => void,
 *   abortRenaming: () => void,
 * }} tree
 * @param {import('yjs').Doc} catalog
 */
export function pruneGoneTreeItems(tree, catalog) {
  const selected = tree.getState().selectedItems ?? [];
  const keep = keepSelectedIds(catalog, selected);
  if (keep.length !== selected.length) tree.setSelectedItems(keep);
  if (isRenamingGone(catalog, tree.getState().renamingItem)) {
    tree.abortRenaming();
  }
}
