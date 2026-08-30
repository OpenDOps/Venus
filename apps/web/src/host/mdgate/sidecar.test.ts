import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { MemoryNoopProvider } from '../sync-provider.js';
import { createM0Workspace } from '../workspace.js';
import { fromDoc } from './from-doc.js';
import {
  addColoredParagraph,
  addImageBlock,
  addParagraph,
  addParagraphAt,
  clearNote,
  paragraphHasColorMark,
  replaceNoteWithParagraphs,
} from './markdown-adapter.js';

const here = dirname(fileURLToPath(import.meta.url));
const goldens = join(here, 'goldens');
const dotPng = join(here, '../../../e2e/fixtures/dot.png');

type HostStore = Awaited<ReturnType<typeof createM0Workspace>>['store'];

function noteOf(store: HostStore) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  expect(note).toBeDefined();
  return note!;
}

function noteRanges(
  store: HostStore,
  sidecar: { blocks: { id: string; start: number; end: number }[] },
) {
  const pageId = store.root!.id;
  expect(sidecar.blocks[0]?.id).toBe(pageId);
  return sidecar.blocks.slice(1);
}

function golden(name: string) {
  return readFileSync(join(goldens, name), 'utf8');
}

function noPerBlockIdComments(markdown: string) {
  expect(markdown).not.toMatch(/<!--\s*id:/);
}

test('side-stable: two fromDoc calls with no Store mutation keep ids and ranges', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
    'Charlie',
  ]);

  const first = await fromDoc(session.store, session.workspace);
  const second = await fromDoc(session.store, session.workspace);

  expect(second.markdown).toBe(first.markdown);
  expect(second.sidecar).toEqual(first.sidecar);
  expect(noteRanges(session.store, first.sidecar).map((b) => b.id)).toEqual(
    ids,
  );
  noPerBlockIdComments(first.markdown);
  for (const id of ids) {
    expect(first.markdown).not.toContain(`id:${id}`);
  }
});

test('side-shift: addBlock before b1 keeps b1 id and increases b1.start', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  const [b1] = replaceNoteWithParagraphs(session.store, note, [
    'Alpha',
    'Bravo',
  ]);

  const first = await fromDoc(session.store, session.workspace);
  const b1First = first.sidecar.blocks.find((b) => b.id === b1);
  expect(b1First).toBeDefined();

  addParagraphAt(session.store, note.id, 'Zed', 0);

  const second = await fromDoc(session.store, session.workspace);
  const b1Second = second.sidecar.blocks.find((b) => b.id === b1);
  expect(b1Second).toBeDefined();
  expect(b1Second!.id).toBe(b1);
  expect(b1Second!.start).toBeGreaterThan(b1First!.start);
  expect(second.markdown.slice(b1Second!.start, b1Second!.end).replace(/\n+$/, '')).toBe(
    'Alpha',
  );
  noPerBlockIdComments(second.markdown);
});

test('opaque-untouched: affine:image slice is byte-equal across two fromDocs', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  clearNote(session.store, note);
  const paraId = addParagraph(session.store, note.id, 'Hello paragraph');
  const file = new File([readFileSync(dotPng)], 'dot.png', {
    type: 'image/png',
  });
  const sourceId = await session.store.blobSync.set(file);
  const imageId = addImageBlock(session.store, note.id, sourceId);

  const first = await fromDoc(session.store, session.workspace);
  const second = await fromDoc(session.store, session.workspace);

  expect(first.markdown).toBe(golden('opaque-image.md'));
  expect(second.markdown).toBe(first.markdown);

  const img1 = first.sidecar.blocks.find((b) => b.id === imageId);
  const img2 = second.sidecar.blocks.find((b) => b.id === imageId);
  expect(img1).toBeDefined();
  expect(img2).toBeDefined();
  const slice1 = first.markdown.slice(img1!.start, img1!.end);
  const slice2 = second.markdown.slice(img2!.start, img2!.end);
  expect(slice1).toBe(slice2);
  expect(slice1.replace(/\n+$/, '')).toBe('![dot.png](assets/dot.png)');

  const para = first.sidecar.blocks.find((b) => b.id === paraId);
  expect(para).toBeDefined();
  expect(first.markdown.slice(para!.start, para!.end).replace(/\n+$/, '')).toBe(
    'Hello paragraph',
  );
  noPerBlockIdComments(first.markdown);
});

test('loss-color: color mark flattens to plain text and is stable on re-export', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  clearNote(session.store, note);
  const color = 'var(--affine-palette-line-red)';
  const id = addColoredParagraph(
    session.store,
    note.id,
    'Colored text',
    color,
  );
  expect(
    paragraphHasColorMark(session.store, id),
    'AffineTextAttributes.color must remain on the CRDT (lossy subset, not skipped)',
  ).toBe(true);

  const first = await fromDoc(session.store, session.workspace);
  const second = await fromDoc(session.store, session.workspace);

  expect(first.markdown).toBe(golden('loss-color.md'));
  expect(second.markdown).toBe(first.markdown);
  expect(first.markdown).toContain('Colored text');
  expect(first.markdown).not.toContain(color);
  noPerBlockIdComments(first.markdown);
});
