import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { SEED_H1, SEED_H2, seedMarkdownDemo } from '../seed.js';
import { MemoryNoopProvider } from '../sync-provider.js';
import { PAGE_DOC_ID, WORKSPACE_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import { fromDoc } from './from-doc.js';
import {
  addEmbedLinkedDoc,
  addListItem,
  addNestedBulletedList,
  clearNote,
  createMarkdownAdapter,
  replaceNoteWithParagraphs,
} from './markdown-adapter.js';

const here = dirname(fileURLToPath(import.meta.url));
const seedGoldenPath = join(here, 'goldens/seed.fromDoc.md');

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

test('Seed fromDoc: MemoryNoopProvider seed contains Why Venus and Empty host', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );

  expect(markdown).toContain(SEED_H1);
  expect(markdown).toContain(SEED_H2);
  expect(markdown.endsWith('\n') && !markdown.endsWith('\n\n')).toBe(true);
  expect(markdown).toBe(readFileSync(seedGoldenPath, 'utf8'));
  expect(sidecar.docId).toBe(PAGE_DOC_ID);
  expect(sidecar.clock.length).toBeGreaterThan(0);
  expect(markdown).not.toMatch(/<!--\s*id:/);
  expect(sidecar.blocks[0]?.id).toBe(session.store.root!.id);
  expect(markdown.slice(sidecar.blocks[0].start, sidecar.blocks[0].end)).toBe(
    '# Venus\n',
  );

  const adapter = createMarkdownAdapter(session.store, session.workspace);
  const imported = await adapter.toDoc({ file: markdown });
  expect(imported, 'toDoc of seed markdown must not fail').toBeDefined();
});

test('recon: list fromDoc form (one sidecar row per list item)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  const { outerId, innerId } = addNestedBulletedList(
    session.store,
    note.id,
    'outer',
    'inner',
  );

  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );
  // 0.22.4 remark-gfm: `*`, not `-`. Nested item is two-space indented.
  expect(markdown).toMatch(/^\* outer$/m);
  expect(markdown).toMatch(/^  \* inner$/m);

  const outer = sidecar.blocks.find((b) => b.id === outerId);
  const inner = sidecar.blocks.find((b) => b.id === innerId);
  expect(outer).toBeDefined();
  expect(inner).toBeDefined();
  expect(markdown.slice(outer!.start, outer!.end).replace(/\n+$/, '')).toBe(
    '* outer',
  );
  expect(markdown.slice(inner!.start, inner!.end).replace(/\n+$/, '')).toBe(
    '  * inner',
  );
});

test('numbered list: per-block 1. still places 2. in the full file', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  clearNote(session.store, note);
  const one = addListItem(session.store, note.id, 'numbered', 'one');
  const two = addListItem(session.store, note.id, 'numbered', 'two');

  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );
  expect(markdown).toMatch(/^1\. one$/m);
  expect(markdown).toMatch(/^2\. two$/m);

  const row1 = sidecar.blocks.find((b) => b.id === one);
  const row2 = sidecar.blocks.find((b) => b.id === two);
  expect(markdown.slice(row1!.start, row1!.end).replace(/\n+$/, '')).toBe(
    '1. one',
  );
  expect(markdown.slice(row2!.start, row2!.end).replace(/\n+$/, '')).toBe(
    '2. two',
  );
});

test('seedMarkdownDemo fromDoc does not throw', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  expect(await seedMarkdownDemo(session.store)).toBe(true);
  const { markdown } = await fromDoc(session.store, session.workspace);
  expect(markdown).toContain('2. two');
  expect(markdown).toContain('Markdown subset');
});

test('side-ids: three paragraphs map CRDT ids to slices; no ids in the body', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
    'Charlie',
  ]);

  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );

  expect(sidecar.docId).toBe(PAGE_DOC_ID);
  const notes = noteRanges(session.store, sidecar);
  expect(notes.map((b) => b.id)).toEqual(ids);
  expect(notes).toHaveLength(3);

  const cores = ['Alpha', 'Bravo', 'Charlie'];
  for (let i = 0; i < 3; i += 1) {
    const { start, end } = notes[i];
    expect(markdown.slice(start, end).replace(/\n+$/, '')).toBe(cores[i]);
    expect(end).toBeGreaterThan(start);
    if (i > 0) {
      expect(start).toBeGreaterThanOrEqual(notes[i - 1].end);
    }
  }
  expect(markdown).not.toMatch(/<!--/);
  for (const id of ids) {
    expect(markdown).not.toContain(`id:${id}`);
  }
});

test('gaps: extra blank line between two paragraphs is in neither range', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
  ]);

  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );

  const notes = noteRanges(session.store, sidecar);
  expect(notes).toHaveLength(2);
  const [a, b] = notes;
  expect(a.end).toBeLessThan(b.start);
  const between = markdown.slice(a.end, b.start);
  expect(between).toMatch(/^\n+$/);
  expect(markdown.slice(a.start, a.end).replace(/\n+$/, '')).toBe('Alpha');
  expect(markdown.slice(b.start, b.end).replace(/\n+$/, '')).toBe('Bravo');
});

test('empty paragraph vs stringify gap: last-N newline is the empty block', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);

  replaceNoteWithParagraphs(session.store, note, ['Alpha', 'Bravo']);
  const two = await fromDoc(session.store, session.workspace);
  expect(two.markdown).toBe('# Venus\n\nAlpha\n\nBravo\n');
  const [alpha2, bravo2] = noteRanges(session.store, two.sidecar);
  expect(two.markdown.slice(alpha2.end, bravo2.start)).toBe('\n');

  const ids = replaceNoteWithParagraphs(session.store, note, [
    'Alpha',
    '',
    'Bravo',
  ]);
  const withEmpty = await fromDoc(session.store, session.workspace);
  expect(withEmpty.markdown).toBe('# Venus\n\nAlpha\n\n\n\nBravo\n');
  const notes = noteRanges(session.store, withEmpty.sidecar);
  expect(notes.map((b) => b.id)).toEqual(ids);
  const [alpha, empty, bravo] = notes;
  expect(withEmpty.markdown.slice(alpha.start, alpha.end)).toBe('Alpha\n');
  expect(withEmpty.markdown.slice(empty.start, empty.end)).toBe('\n');
  expect(withEmpty.markdown.slice(bravo.start, bravo.end)).toBe('Bravo\n');
  // Interval after Alpha is `\n\n\n`; last-N maps only the last `\n` to the
  // empty paragraph. The leading `\n\n` are stringify gaps (neither range).
  expect(withEmpty.markdown.slice(alpha.end, empty.start)).toBe('\n\n');
  expect(empty.end).toBe(bravo.start);
});

test('duplicate paragraphs place in document order (cursor, not first indexOf)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Hello',
    'Hello',
  ]);

  const { markdown, sidecar } = await fromDoc(
    session.store,
    session.workspace,
  );
  const notes = noteRanges(session.store, sidecar);
  expect(notes.map((b) => b.id)).toEqual(ids);
  expect(markdown.slice(notes[0].start, notes[0].end).replace(/\n+$/, '')).toBe(
    'Hello',
  );
  expect(markdown.slice(notes[1].start, notes[1].end).replace(/\n+$/, '')).toBe(
    'Hello',
  );
  expect(notes[1].start).toBeGreaterThanOrEqual(notes[0].end);
});

test('linked-doc comment injects at the adapter URL, not a prose substring', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  replaceNoteWithParagraphs(session.store, note, ['mentions doc:lease here']);
  addEmbedLinkedDoc(session.store, note.id, 'doc:lease');

  const { markdown } = await fromDoc(session.store, session.workspace);
  const comment = '<!-- venus:doc:doc:lease -->';
  const prose = markdown.indexOf('mentions doc:lease here');
  const commentAt = markdown.indexOf(comment);
  const linkAt = markdown.indexOf('](');
  expect(prose).toBeGreaterThanOrEqual(0);
  expect(linkAt).toBeGreaterThan(prose);
  expect(commentAt).toBeGreaterThan(linkAt);
  expect(markdown).toMatch(
    new RegExp(
      `\\[untitled\\]\\(\\.\\/workspace\\/${WORKSPACE_ID}\\/doc:lease\\)\\n<!-- venus:doc:doc:lease -->`,
    ),
  );
});
