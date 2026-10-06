import { PAGE_DOC_ID } from '../ids.js';
import { getNode, nodesMap } from './schema.js';

/**
 * Header title for the open catalog id. Missing node → empty string.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string} openDocId
 */
export function openNodeTitle(catalog, openDocId) {
  return getNode(catalog, openDocId)?.name ?? '';
}

/**
 * Auto-switch to home only when the open extra page is gone from catalog.
 *
 * @param {import('yjs').Doc} catalog
 * @param {string} openDocId
 */
export function shouldAutoHome(catalog, openDocId) {
  return openDocId !== PAGE_DOC_ID && !getNode(catalog, openDocId);
}

/**
 * Apply title + missing-open chrome from the current open id (no tree rebuild).
 *
 * @param {import('yjs').Doc} catalog
 * @param {string} openDocId
 * @param {{ onTitle: (title: string) => void, onMissingOpen: () => void }} hooks
 */
export function applyCatalogHostChrome(catalog, openDocId, hooks) {
  hooks.onTitle(openNodeTitle(catalog, openDocId));
  if (shouldAutoHome(catalog, openDocId)) hooks.onMissingOpen();
}

/**
 * One `nodesMap.observeDeep`. `getOpenDocId` is read on each event so the
 * subscription does not rebind on page switch. Initial chrome runs without
 * `onChange` (tree rebuild is for mutations only).
 *
 * @param {import('yjs').Doc} catalog
 * @param {() => string} getOpenDocId
 * @param {{
 *   onTitle: (title: string) => void,
 *   onMissingOpen: () => void,
 *   onChange: () => void,
 * }} hooks
 * @returns {() => void}
 */
export function listenCatalogHost(catalog, getOpenDocId, hooks) {
  const chrome = () => {
    applyCatalogHostChrome(catalog, getOpenDocId(), hooks);
  };
  const onDeep = () => {
    chrome();
    hooks.onChange();
  };
  chrome();
  const nodes = nodesMap(catalog);
  nodes.observeDeep(onDeep);
  return () => nodes.unobserveDeep(onDeep);
}
