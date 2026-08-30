import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { MemoryNoopProvider } from '../sync-provider.js';
import { createM0Workspace, hydrateM0FromUpdate } from '../workspace.js';
import { fromDoc } from './from-doc.js';
import {
  replaceNoteWithParagraphs,
  setParagraphText,
} from './markdown-adapter.js';
import { fromPinnedBytes, pinThenFromDoc, pinYjsBytes } from './pin-from-doc.js';
import { incrementalFromDoc } from './splice.js';

const here = dirname(fileURLToPath(import.meta.url));

type HostStore = Awaited<ReturnType<typeof createM0Workspace>>['store'];

function noteOf(store: HostStore) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  expect(note).toBeDefined();
  return note!;
}

test('hydrateM0FromUpdate does not seed; block ids match the pin', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const pageId = session.store.root!.id;
  const pin = pinYjsBytes(session.store.spaceDoc);
  const cloned = hydrateM0FromUpdate(pin.bytes);
  expect(cloned.store.root!.id).toBe(pageId);
  expect(cloned.docId).toBe('doc:home');
});

test('pin-then-fromDoc equals live fromDoc at pin time; ids preserved', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
  ]);
  const live = await fromDoc(session.store, session.workspace);
  const pinned = await pinThenFromDoc(session.store, session.workspace);

  expect(pinned.pin.bytes.byteLength).toBeGreaterThan(2);
  expect(pinned.pin.clock).toBe(live.sidecar.clock);
  expect(pinned.markdown).toBe(live.markdown);
  expect(pinned.sidecar.blocks).toEqual(live.sidecar.blocks);
  expect(pinned.sidecar.docId).toBe('doc:home');
  expect(pinned.sidecar.clock).toBe(pinned.pin.clock);
  expect(pinned.sidecar.blocks.map((b) => b.id)).toEqual([
    session.store.root!.id,
    ...ids,
  ]);
});

test('live mutation after pin does not change convert of that pin', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const [b1] = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
  ]);
  const pinned = await pinThenFromDoc(session.store, session.workspace);

  setParagraphText(session.store, b1, 'Alpha edited');
  const liveAfter = await fromDoc(session.store, session.workspace);
  expect(liveAfter.markdown).toContain('Alpha edited');
  expect(pinned.markdown).not.toContain('Alpha edited');
  expect(pinned.markdown).toContain('Alpha');

  const again = await fromPinnedBytes(pinned.pin.bytes, {
    clock: pinned.pin.clock,
    docId: 'doc:home',
  });
  expect(again.markdown).toBe(pinned.markdown);
  expect(again.sidecar.blocks).toEqual(pinned.sidecar.blocks);
  expect(again.sidecar.clock).toBe(pinned.pin.clock);
});

test('spliced RAM sidecar is not the pin convert', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const ids = replaceNoteWithParagraphs(session.store, noteOf(session.store), [
    'Alpha',
    'Bravo',
    'Charlie',
  ]);
  const pinned = await pinThenFromDoc(session.store, session.workspace);

  setParagraphText(session.store, ids[0], 'Alpha edited');
  const spliced = await incrementalFromDoc(
    session.store,
    session.workspace,
    { markdown: pinned.markdown, sidecar: pinned.sidecar },
    [ids[0]],
  );
  expect(spliced.mode).toBe('splice');
  expect(spliced.markdown).not.toBe(pinned.markdown);

  const convert = await fromPinnedBytes(pinned.pin.bytes, {
    clock: pinned.pin.clock,
    docId: 'doc:home',
  });
  expect(convert.markdown).toBe(pinned.markdown);
  expect(convert.sidecar.blocks).toEqual(pinned.sidecar.blocks);
});

test('empty pin bytes throw; pin-from-doc does not import splice', () => {
  expect(() => hydrateM0FromUpdate(new Uint8Array([0, 0]))).toThrow(/empty/);
  const src = readFileSync(join(here, 'pin-from-doc.js'), 'utf8');
  expect(src).not.toMatch(/from ['\"]\.\/splice\.js['\"]/);
  expect(src).toContain("from './from-doc.js'");
  const pane = readFileSync(join(here, 'mount-md-pane.js'), 'utf8');
  expect(pane).not.toMatch(/pin-from-doc/);
  expect(pane).not.toMatch(/pinThenFromDoc/);
});
