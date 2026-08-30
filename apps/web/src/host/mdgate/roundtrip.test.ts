import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { MemoryNoopProvider } from '../sync-provider.js';
import { createM0Workspace } from '../workspace.js';
import { roundTripFromDoc } from './from-doc.js';
import {
  addCodeBlock,
  addEmbedLinkedDoc,
  addHeading,
  addLinkParagraph,
  addMarksParagraph,
  addNestedBulletedList,
  addParagraph,
  clearNote,
  replaceNoteWithParagraphs,
} from './markdown-adapter.js';

const here = dirname(fileURLToPath(import.meta.url));
const goldens = join(here, 'goldens');

type HostStore = Awaited<ReturnType<typeof createM0Workspace>>['store'];

function noteOf(store: HostStore) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  expect(note).toBeDefined();
  return note!;
}

function headingTypes(store: {
  root: {
    children?: {
      flavour: string;
      children?: { flavour: string; props?: { type?: string } }[];
    }[];
  } | null;
}) {
  const note = store.root?.children?.find((c) => c.flavour === 'affine:note');
  return (note?.children ?? [])
    .filter((c) => c.flavour === 'affine:paragraph')
    .map((c) => c.props?.type ?? 'text');
}

async function sessionNote() {
  const session = await createM0Workspace(new MemoryNoopProvider());
  return { session, note: noteOf(session.store) };
}

function golden(name: string) {
  return readFileSync(join(goldens, name), 'utf8');
}

test('rt-paragraph: one paragraph round-trips to golden', async () => {
  const { session, note } = await sessionNote();
  replaceNoteWithParagraphs(session.store, note, ['Hello paragraph']);
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-paragraph.md'));
  expect(second.markdown).toBe(first.markdown);
});

test('rt-headings: h1/h2 types survive toDoc; markdown matches golden', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  addHeading(session.store, note.id, 'h1', 'Why Venus');
  addParagraph(
    session.store,
    note.id,
    'A thin host around BlockSuite: one workspace, one page.',
  );
  addHeading(session.store, note.id, 'h2', 'Empty host');
  const { first, second, imported } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-headings.md'));
  expect(second.markdown).toBe(first.markdown);
  expect(headingTypes(imported)).toEqual(['h1', 'text', 'h2']);
});

test('rt-list: nested bullets golden; one sidecar row per item', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  const { outerId, innerId } = addNestedBulletedList(
    session.store,
    note.id,
    'outer',
    'inner',
  );
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-list.md'));
  expect(second.markdown).toBe(first.markdown);

  const pageId = session.store.root!.id;
  const notes = first.sidecar.blocks.filter((b) => b.id !== pageId);
  expect(notes.map((b) => b.id)).toEqual([outerId, innerId]);
  expect(first.markdown.slice(notes[0].start, notes[0].end).replace(/\n+$/, '')).toBe(
    '* outer',
  );
  expect(first.markdown.slice(notes[1].start, notes[1].end).replace(/\n+$/, '')).toBe(
    '  * inner',
  );
});

test('rt-code: fenced javascript survives round-trip', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  addCodeBlock(session.store, note.id, 'javascript', 'const x = 1;');
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-code.md'));
  expect(second.markdown).toBe(first.markdown);
  expect(first.markdown).toContain('```javascript');
  expect(first.markdown).toContain('const x = 1;');
});

test('rt-link: inline URL round-trips to golden', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  addLinkParagraph(
    session.store,
    note.id,
    'docs',
    'https://example.com/path',
  );
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-link.md'));
  expect(second.markdown).toBe(first.markdown);
  expect(first.markdown).toContain('[docs](https://example.com/path)');
});

test('rt-linked-doc: export has path and venus:doc comment; toDoc does not restore the card', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  const blockId = addEmbedLinkedDoc(session.store, note.id, 'doc:lease');
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-linked-doc.md'));
  expect(first.markdown).toContain('<!-- venus:doc:doc:lease -->');
  expect(first.markdown).toContain('doc:lease');
  const range = first.sidecar.blocks.find((b) => b.id === blockId);
  expect(range).toBeDefined();
  expect(first.markdown.slice(range!.start, range!.end)).toContain(
    '<!-- venus:doc:doc:lease -->',
  );
  // Adapter import of the comment is an html code fence, not embed-linked-doc.
  expect(second.markdown).not.toBe(first.markdown);
  expect(second.markdown).toContain('```html');
});

test('rt-marks: bold, italic, and inline code survive round-trip', async () => {
  const { session, note } = await sessionNote();
  clearNote(session.store, note);
  addMarksParagraph(session.store, note.id);
  const { first, second } = await roundTripFromDoc(
    session.store,
    session.workspace,
  );
  expect(first.markdown).toBe(golden('rt-marks.md'));
  expect(second.markdown).toBe(first.markdown);
});
