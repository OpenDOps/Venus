import { PAGE_DOC_ID } from '../ids.js';
import { KIND_DOC, getNode, isHome } from './schema.js';

/**
 * Catalog id to open, or home if missing / not a doc.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string | null | undefined} requestedId
 */
export function resolveOpenDocId(catalog, requestedId) {
  if (requestedId == null || requestedId === '' || isHome(requestedId)) {
    return PAGE_DOC_ID;
  }
  const node = getNode(catalog, requestedId);
  if (!node || node.kind !== KIND_DOC) return PAGE_DOC_ID;
  return node.docId ?? node.id;
}
