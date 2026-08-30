import { expect, test } from 'vitest';
import {
  SEED_H1,
  SEED_H1_BODY,
  SEED_H2,
  SEED_H2_BODY,
  SEED_SPACER_COUNT,
  SEED_TITLE,
} from './seed.js';
import { createM0Workspace } from './workspace.js';

function noteChildren(
  store: Awaited<ReturnType<typeof createM0Workspace>>['store'],
) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  expect(note).toBeDefined();
  return note!.children;
}

test('visible structure: title, H1, H2', async () => {
  const { store } = await createM0Workspace();
  expect(store.root?.props.title?.toString()).toBe(SEED_TITLE);

  const children = noteChildren(store);
  expect(children[0]?.props.type ?? 'text').toBe('text');
  expect(children[0]?.props.text?.toString() ?? '').toBe('');

  const headings = children.filter((c) =>
    ['h1', 'h2'].includes(c.props.type ?? ''),
  );
  expect(headings.map((c) => c.props.type)).toEqual(['h1', 'h2']);
  expect(headings.map((c) => c.props.text?.toString())).toEqual([
    SEED_H1,
    SEED_H2,
  ]);

  const texts = children.map((c) => c.props.text?.toString() ?? '');
  expect(texts).toContain(SEED_H1_BODY);
  expect(texts).toContain(SEED_H2_BODY);
  expect(children.length).toBe(3 + SEED_SPACER_COUNT + 2);
});

test('constructor undo does not delete the page tree', async () => {
  const { store } = await createM0Workspace();
  expect(store.canUndo).toBe(false);
  store.undo();
  expect(store.root?.flavour).toBe('affine:page');
  expect(store.root?.props.title?.toString()).toBe(SEED_TITLE);
  const headings = noteChildren(store).filter((c) =>
    ['h1', 'h2'].includes(c.props.type ?? ''),
  );
  expect(headings.map((c) => c.props.text?.toString())).toEqual([
    SEED_H1,
    SEED_H2,
  ]);
});
