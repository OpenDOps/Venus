import { KIND_DOC, listNodes } from './schema.js';

/** @type {WeakMap<object, Set<string>>} */
const seenByWorkspace = new WeakMap();

/**
 * Card `getStore({ id })` on a doc that has no page root. `root` is truthy
 * so the embed card does not wait forever for `rootAdded`, and `load` does
 * not write blocks. `getStore()` with no args stays the real store so opening
 * the page still syncs an empty Y.Doc.
 *
 * @param {object} workspace
 */
export function installCatalogDocShell(workspace) {
  if (!workspace || workspace.__venusCatalogShell) return;
  const rawGetDoc = workspace.getDoc?.bind(workspace);
  if (typeof rawGetDoc !== 'function') return;
  /** @type {WeakMap<object, object>} */
  const shells = new WeakMap();
  workspace.getDoc = (id) => {
    const doc = rawGetDoc(id);
    if (!doc) return doc;
    let shell = shells.get(doc);
    if (!shell) {
      shell = shellDoc(doc);
      shells.set(doc, shell);
    }
    return shell;
  };
  workspace.__venusCatalogShell = true;
}

/**
 * @param {object} doc
 */
function shellDoc(doc) {
  const rawGetStore = doc.getStore.bind(doc);
  /** @type {object | null} */
  let stubStore = null;
  return new Proxy(doc, {
    get(target, prop, _receiver) {
      if (prop === 'getStore') {
        return (opts) => {
          const store = rawGetStore(opts);
          if (opts?.id && !store.root) {
            if (!stubStore) stubStore = stubStoreOf(store);
            return stubStore;
          }
          return store;
        };
      }
      const value = Reflect.get(target, prop, target);
      if (typeof value === 'function') return value.bind(target);
      return value;
    },
  });
}

/**
 * @param {object} store
 */
function stubStoreOf(store) {
  return new Proxy(store, {
    get(target, prop, _receiver) {
      if (prop === 'root' && !target.root) {
        return { flavour: 'venus:stub', children: [] };
      }
      if (prop === 'load') return () => target;
      const value = Reflect.get(target, prop, target);
      if (typeof value === 'function') return value.bind(target);
      return value;
    },
  });
}

/**
 * One lazy workspace doc per catalog `kind:doc`, with `docMetas.title` set
 * to the catalog name. No seed and no `load` — the Y.Doc stays empty until
 * the page is opened. Docs that leave the catalog are removed, except the
 * one currently open. `beforeRemove(id)` runs first so a live socket on that
 * Y.Doc is closed before the doc goes away.
 *
 * @param {import('yjs').Doc} catalog
 * @param {object} workspace
 * @param {{ keepId?: string, beforeRemove?: (id: string) => void }} [options]
 */
export function projectCatalogDocs(catalog, workspace, options = {}) {
  if (!workspace?.createDoc || !workspace?.meta?.setDocMeta) return;
  installCatalogDocShell(workspace);
  let seen = seenByWorkspace.get(workspace);
  if (!seen) {
    seen = new Set();
    seenByWorkspace.set(workspace, seen);
  }
  const keepId = options.keepId;
  /** @type {Set<string>} */
  const live = new Set();
  for (const node of listNodes(catalog)) {
    if (node.kind !== KIND_DOC) continue;
    const id = node.docId || node.id;
    if (!id) continue;
    live.add(id);
    seen.add(id);
    if (!workspace.docs?.has?.(id)) {
      workspace.createDoc(id);
    }
    workspace.meta.setDocMeta(id, { title: node.name });
  }
  for (const id of seen) {
    if (live.has(id) || id === keepId) continue;
    options.beforeRemove?.(id);
    if (workspace.docs?.has?.(id)) {
      workspace.removeDoc(id);
    }
    seen.delete(id);
  }
}
