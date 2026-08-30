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
