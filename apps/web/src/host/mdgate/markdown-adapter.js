import {
  MarkdownAdapter,
  docLinkBaseURLMiddleware,
  embedSyncedDocMiddleware,
  titleMiddleware,
} from '@blocksuite/affine/shared/adapters';
import { Text } from '@blocksuite/affine/store';

/**
 * Adapter wiring for the shared exporter (`from-doc.js`).
 * Keep this file `.js` so `tsc` does not follow affine published `.ts`.
 */
export function createMarkdownAdapter(store, workspace) {
  const transformer = store.getTransformer([
    titleMiddleware(workspace.meta.docMetas),
    docLinkBaseURLMiddleware(workspace.id),
    embedSyncedDocMiddleware('content'),
  ]);
  return new MarkdownAdapter(transformer, store.provider);
}

/** One transformer + adapter per Store (hot pane / splice path). */
const adapterByStore = new WeakMap();

export function markdownAdapterFor(store, workspace) {
  const docMetas = workspace?.meta?.docMetas;
  const hit = adapterByStore.get(store);
  if (
    hit &&
    hit.workspaceId === workspace.id &&
    hit.docMetas === docMetas
  ) {
    return hit.adapter;
  }
  const adapter = createMarkdownAdapter(store, workspace);
  adapterByStore.set(store, {
    workspaceId: workspace.id,
    docMetas,
    adapter,
  });
  return adapter;
}

export function addListItem(store, parentId, type, text, extra = {}) {
  return store.addBlock(
    'affine:list',
    { type, text: new Text(text), ...extra },
    parentId,
  );
}

export function addNestedBulletedList(store, noteId, outer, inner) {
  const outerId = store.addBlock(
    'affine:list',
    { type: 'bulleted', text: new Text(outer) },
    noteId,
  );
  const innerId = store.addBlock(
    'affine:list',
    { type: 'bulleted', text: new Text(inner) },
    outerId,
  );
  return { outerId, innerId };
}

export function clearNote(store, note) {
  for (const child of [...note.children]) {
    store.deleteBlock(child.id);
  }
}

export function replaceNoteWithParagraphs(store, note, texts) {
  clearNote(store, note);
  return texts.map((text) =>
    store.addBlock('affine:paragraph', { text: new Text(text) }, note.id),
  );
}

export function addHeading(store, noteId, type, text) {
  return store.addBlock(
    'affine:paragraph',
    { type, text: new Text(text) },
    noteId,
  );
}

export function addParagraph(store, noteId, text) {
  return store.addBlock(
    'affine:paragraph',
    { text: new Text(text) },
    noteId,
  );
}

export function addParagraphAt(store, noteId, text, index) {
  return store.addBlock(
    'affine:paragraph',
    { text: new Text(text) },
    noteId,
    index,
  );
}

export function addImageBlock(store, noteId, sourceId) {
  return store.addBlock('affine:image', { sourceId }, noteId);
}

export function addColoredParagraph(store, noteId, text, color) {
  return store.addBlock(
    'affine:paragraph',
    {
      text: new Text([{ insert: text, attributes: { color } }]),
    },
    noteId,
  );
}

export function paragraphHasColorMark(store, blockId) {
  const block = store.getBlock(blockId);
  const text = block?.model?.text;
  const delta = text?.toDelta?.() ?? [];
  return delta.some((op) => typeof op.attributes?.color === 'string');
}

export function setParagraphText(store, blockId, text) {
  const current = store.getBlock(blockId)?.model?.text;
  if (!current || typeof current.toString !== 'function') {
    throw new Error(`No text on block ${blockId}`);
  }
  const now = current.toString();
  if (typeof current.delete === 'function') current.delete(0, now.length);
  if (typeof current.insert === 'function') current.insert(text, 0);
}

export function setParagraphType(store, blockId, type) {
  store.updateBlock(blockId, { type });
}

export function formatParagraph(store, blockId, attrs) {
  const current = store.getBlock(blockId)?.model?.text;
  if (!current || typeof current.format !== 'function') {
    throw new Error(`No format on block ${blockId}`);
  }
  const len = current.toString().length;
  current.format(0, len, attrs);
}

export function paragraphTextLength(store, blockId) {
  const current = store.getBlock(blockId)?.model?.text;
  return current?.toString?.().length ?? 0;
}

export function addCodeBlock(store, noteId, language, source) {
  return store.addBlock(
    'affine:code',
    { language, text: new Text(source) },
    noteId,
  );
}

export function addLinkParagraph(store, noteId, label, url) {
  return store.addBlock(
    'affine:paragraph',
    {
      text: new Text([{ insert: label, attributes: { link: url } }]),
    },
    noteId,
  );
}

export function addMarksParagraph(store, noteId) {
  return store.addBlock(
    'affine:paragraph',
    {
      text: new Text([
        { insert: 'bold', attributes: { bold: true } },
        { insert: ' ' },
        { insert: 'italic', attributes: { italic: true } },
        { insert: ' ' },
        { insert: 'code', attributes: { code: true } },
      ]),
    },
    noteId,
  );
}

export function addEmbedLinkedDoc(store, noteId, pageId) {
  return store.addBlock('affine:embed-linked-doc', { pageId }, noteId);
}
