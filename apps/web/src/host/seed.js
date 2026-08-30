import { Text } from '@blocksuite/affine/store';

export const SEED_TITLE = 'Venus';
export const SEED_H1 = 'Why Venus';
export const SEED_H1_BODY =
  'A thin host around BlockSuite: one workspace, one page.';
export const SEED_H2 = 'Empty host';
export const SEED_H2_BODY =
  'Type in a real page on localhost before later milestones attach.';

/** Empty paragraphs between H1 and H2 so clicking H2 in the outline must scroll. */
export const SEED_SPACER_COUNT = 24;

/**
 * Headings after the empty paragraph. 0.22.4 has no `affine:heading` flavour —
 * headings are `affine:paragraph` with `type` h1/h2 (api-map).
 */
export function seedHomeNote(store, noteId) {
  store.addBlock(
    'affine:paragraph',
    { type: 'h1', text: new Text(SEED_H1) },
    noteId,
  );
  store.addBlock(
    'affine:paragraph',
    { text: new Text(SEED_H1_BODY) },
    noteId,
  );
  for (let i = 0; i < SEED_SPACER_COUNT; i++) {
    store.addBlock('affine:paragraph', {}, noteId);
  }
  store.addBlock(
    'affine:paragraph',
    { type: 'h2', text: new Text(SEED_H2) },
    noteId,
  );
  store.addBlock(
    'affine:paragraph',
    { text: new Text(SEED_H2_BODY) },
    noteId,
  );
}
