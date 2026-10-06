import { PAGE_DOC_ID } from '../ids.js';
import { projectCatalogDocs } from './project-docs.js';
import { repairDuplicateGitNames } from './repair-git-names.js';
import { repairCatalogStructure } from './repair-structure.js';
import { getNode, nodesMap } from './schema.js';

/** One queued repair per catalog so a repair write cannot schedule itself forever. */
const repairQueued = new WeakSet();

/**
 * Repair runs after the observer returns. A catalog transact inside
 * `observeDeep` re-enters Yjs.
 *
 * @param {import('yjs').Doc} catalog
 */
function scheduleGitNameRepair(catalog) {
  if (repairQueued.has(catalog)) return;
  repairQueued.add(catalog);
  queueMicrotask(() => {
    repairQueued.delete(catalog);
    repairCatalogStructure(catalog);
    repairDuplicateGitNames(catalog);
  });
}

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
 * One `run` per animation frame. A second `schedule` before the frame
 * fires shares that call.
 *
 * @param {() => void} run
 * @param {(cb: () => void) => unknown} [raf]
 * @param {(id: unknown) => void} [cancel]
 */
export function batchOnAnimationFrame(
  run,
  raf = requestAnimationFrame,
  cancel = cancelAnimationFrame,
) {
  /** @type {unknown} */
  let frame = null;
  return {
    schedule() {
      if (frame != null) return;
      frame = raf(() => {
        frame = null;
        run();
      });
    },
    cancel() {
      if (frame == null) return;
      cancel(frame);
      frame = null;
    },
  };
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
 *   onRemoveDoc?: (id: string) => void,
 * }} hooks
 * @param {object} [workspace] when set, project catalog docs into it
 * @returns {() => void}
 */
export function listenCatalogHost(catalog, getOpenDocId, hooks, workspace) {
  const chrome = () => {
    if (workspace) {
      projectCatalogDocs(catalog, workspace, {
        keepId: getOpenDocId(),
        beforeRemove: hooks.onRemoveDoc,
      });
    }
    applyCatalogHostChrome(catalog, getOpenDocId(), hooks);
    scheduleGitNameRepair(catalog);
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
