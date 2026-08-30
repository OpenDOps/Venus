import { expect, test } from 'vitest';
import { MemoryNoopProvider } from '../sync-provider.js';
import { createM0Workspace } from '../workspace.js';
import { fromDoc } from './from-doc.js';
import {
  addEmbedLinkedDoc,
  addHeading,
  addParagraph,
  addParagraphAt,
  clearNote,
  formatParagraph,
  paragraphTextLength,
  replaceNoteWithParagraphs,
  setParagraphText,
  setParagraphType,
} from './markdown-adapter.js';
import { incrementalFromDoc } from './splice.js';

type HostStore = Awaited<ReturnType<typeof createM0Workspace>>['store'];

function noteOf(store: HostStore) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  expect(note).toBeDefined();
  return note!;
}

function median(values: number[]) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

async function timeMs(fn: () => Promise<unknown>) {
  const t0 = performance.now();
  await fn();
  return performance.now() - t0;
}

test('incr-inplace: splice of one paragraph equals full fromDoc and shifts later ranges', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
    'Charlie',
  ]);
  const previous = await fromDoc(session.store, session.workspace);
  const b1 = previous.sidecar.blocks.find((b) => b.id === ids[0]);
  const b2 = previous.sidecar.blocks.find((b) => b.id === ids[1]);
  expect(b1).toBeDefined();
  expect(b2).toBeDefined();
  const b2StartBefore = b2!.start;

  setParagraphText(session.store, ids[0], 'Alpha edited');
  const spliced = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [ids[0]],
  );
  const full = await fromDoc(session.store, session.workspace);

  expect(spliced.mode).toBe('splice');
  expect(spliced.markdown).toBe(full.markdown);
  expect(spliced.sidecar.blocks).toEqual(full.sidecar.blocks);
  expect(spliced.sidecar.blocks.find((b) => b.id === ids[0])?.id).toBe(ids[0]);
  const b2After = spliced.sidecar.blocks.find((b) => b.id === ids[1]);
  expect(b2After!.start).toBeGreaterThan(b2StartBefore);
  expect(b2After!.start - b2StartBefore).toBe(
    spliced.sidecar.blocks.find((b) => b.id === ids[0])!.end - b1!.end,
  );
});

test.each([
  ['bold', { bold: true }, '**Plain**'],
  ['italic', { italic: true }, '*Plain*'],
  ['code', { code: true }, '`Plain`'],
  [
    'link',
    { link: 'https://example.com/path' },
    '[Plain](https://example.com/path)',
  ],
] as const)(
  'incr-marks: %s grows markdown without Y.Text length change; splice uses adapter slice',
  async (_name, attrs, needle) => {
    const session = await createM0Workspace(new MemoryNoopProvider());
    const note = noteOf(session.store);
    clearNote(session.store, note);
    const ids = replaceNoteWithParagraphs(session.store, note, ['Plain']);
    const previous = await fromDoc(session.store, session.workspace);
    const yLenBefore = paragraphTextLength(session.store, ids[0]);
    const mdLenBefore = previous.markdown.length;

    formatParagraph(session.store, ids[0], attrs);
    expect(paragraphTextLength(session.store, ids[0])).toBe(yLenBefore);

    const spliced = await incrementalFromDoc(
      session.store,
      session.workspace,
      previous,
      [ids[0]],
    );
    const full = await fromDoc(session.store, session.workspace);

    expect(spliced.mode).toBe('splice');
    expect(spliced.markdown).toBe(full.markdown);
    expect(spliced.markdown.length).toBeGreaterThan(mdLenBefore);
    expect(spliced.markdown).toContain(needle);
    const yDelta = paragraphTextLength(session.store, ids[0]) - yLenBefore;
    const mdDelta = spliced.markdown.length - mdLenBefore;
    expect(yDelta).toBe(0);
    expect(mdDelta).not.toBe(yDelta);
    expect(spliced.sidecar.blocks).toEqual(full.sidecar.blocks);
  },
);

test('incr-inplace: heading text splice still uses adapter slice (type unchanged)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  clearNote(session.store, note);
  const id = addHeading(session.store, note.id, 'h1', 'Hello');
  addParagraphAt(session.store, note.id, 'Bravo', 1);
  const previous = await fromDoc(session.store, session.workspace);
  setParagraphText(session.store, id, 'Hello there');
  const spliced = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [id],
  );
  const full = await fromDoc(session.store, session.workspace);
  expect(spliced.mode).toBe('splice');
  expect(spliced.markdown).toBe(full.markdown);
  expect(spliced.markdown).toContain('# Hello there');
  expect(spliced.sidecar.blocks).toEqual(full.sidecar.blocks);
});

test('incr-fallback: heading type h1↔text is full fromDoc (Y.Text length 0)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Hello',
    'Bravo',
  ]);
  const previous = await fromDoc(session.store, session.workspace);
  const yLenBefore = paragraphTextLength(session.store, ids[0]);
  const mdLenBefore = previous.markdown.length;

  setParagraphType(session.store, ids[0], 'h1');
  expect(paragraphTextLength(session.store, ids[0])).toBe(yLenBefore);

  const result = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [ids[0]],
  );
  const full = await fromDoc(session.store, session.workspace);

  expect(result.mode).toBe('full');
  expect(result.markdown).toBe(full.markdown);
  expect(result.markdown).toContain('# Hello');
  expect(result.markdown.length).toBeGreaterThan(mdLenBefore);
  expect(result.sidecar.blocks).toEqual(full.sidecar.blocks);
});

test('incr-fallback: linked-doc card on the page blocks splice (title middleware)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  const ids = replaceNoteWithParagraphs(session.store, note, ['Alpha']);
  addEmbedLinkedDoc(session.store, note.id, 'doc:lease');
  const previous = await fromDoc(session.store, session.workspace);

  setParagraphText(session.store, ids[0], 'Alpha edited');
  const result = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [ids[0]],
  );
  const full = await fromDoc(session.store, session.workspace);

  expect(result.mode).toBe('full');
  expect(result.markdown).toBe(full.markdown);
  expect(result.markdown).toContain('<!-- venus:doc:doc:lease -->');
  expect(result.sidecar.blocks).toEqual(full.sidecar.blocks);
});

test('incr-fallback: addBlock before b1 does not splice; full fromDoc still side-shift', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  const [b1] = replaceNoteWithParagraphs(session.store, note, [
    'Alpha',
    'Bravo',
  ]);
  const previous = await fromDoc(session.store, session.workspace);
  const b1First = previous.sidecar.blocks.find((b) => b.id === b1);
  expect(b1First).toBeDefined();

  const inserted = addParagraphAt(session.store, note.id, 'Zed', 0);
  const result = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [inserted, b1],
  );
  expect(result.mode).toBe('full');
  expect(result.sidecar.blocks.find((b) => b.id === inserted)).toBeDefined();

  const b1Second = result.sidecar.blocks.find((b) => b.id === b1);
  expect(b1Second!.id).toBe(b1);
  expect(b1Second!.start).toBeGreaterThan(b1First!.start);
});

test('incr-fallback: empty paragraph last-N does not splice', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const note = noteOf(session.store);
  clearNote(session.store, note);
  const emptyId = addParagraph(session.store, note.id, '');
  addParagraph(session.store, note.id, 'Bravo');
  const previous = await fromDoc(session.store, session.workspace);
  const emptyRow = previous.sidecar.blocks.find((b) => b.id === emptyId);
  expect(emptyRow).toBeDefined();
  expect(
    previous.markdown.slice(emptyRow!.start, emptyRow!.end).replace(/\n+$/, ''),
  ).toBe('');

  setParagraphText(session.store, emptyId, 'Hello');
  const result = await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [emptyId],
  );
  const full = await fromDoc(session.store, session.workspace);

  expect(result.mode).toBe('full');
  expect(result.markdown).toBe(full.markdown);
  expect(result.markdown).toContain('Hello');
});

test('incr-perf: measure splice on vs full fromDoc off for one in-place edit', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const n = 48;
  const texts = Array.from({ length: n }, (_, i) => `Para-${String(i).padStart(2, '0')}`);
  const ids = replaceNoteWithParagraphs(
    session.store,
    noteOf(session.store),
    texts,
  );
  const previous = await fromDoc(session.store, session.workspace);
  const dirty = ids[Math.floor(n / 2)];
  setParagraphText(session.store, dirty, `${texts[Math.floor(n / 2)]} edited`);

  await incrementalFromDoc(session.store, session.workspace, previous, [dirty]);
  await incrementalFromDoc(
    session.store,
    session.workspace,
    previous,
    [dirty],
    { forceFull: true },
  );

  const runs = 3;
  const spliceMs: number[] = [];
  const fullMs: number[] = [];
  let spliced;
  let forced;
  for (let i = 0; i < runs; i += 1) {
    spliceMs.push(
      await timeMs(async () => {
        spliced = await incrementalFromDoc(
          session.store,
          session.workspace,
          previous,
          [dirty],
        );
      }),
    );
    fullMs.push(
      await timeMs(async () => {
        forced = await incrementalFromDoc(
          session.store,
          session.workspace,
          previous,
          [dirty],
          { forceFull: true },
        );
      }),
    );
  }

  const spliceMedian = median(spliceMs);
  const fullMedian = median(fullMs);
  console.info(
    `incr-perf n=${n} paragraphs, 1 dirty: splice(on) median ${spliceMedian.toFixed(1)}ms [${spliceMs.map((x) => x.toFixed(1)).join(', ')}] vs full(off) median ${fullMedian.toFixed(1)}ms [${fullMs.map((x) => x.toFixed(1)).join(', ')}]`,
  );

  expect(spliced!.mode).toBe('splice');
  expect(forced!.mode).toBe('full');
  expect(spliced!.markdown).toBe(forced!.markdown);
  expect(spliced!.sidecar.blocks).toEqual(forced!.sidecar.blocks);
  expect(spliceMedian).toBeLessThan(fullMedian);
});
