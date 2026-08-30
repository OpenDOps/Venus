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

/** Opt-in `?md-demo=1` heading. Idempotent marker — do not change. */
export const SEED_MD_DEMO_H2 = 'Markdown subset';

const DEMO_PNG_B64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

function noteOf(store) {
  return store.root?.children.find((c) => c.flavour === 'affine:note');
}

export function noteHasMarkdownDemo(store) {
  const note = noteOf(store);
  if (!note) return false;
  return note.children.some(
    (c) =>
      c.flavour === 'affine:paragraph' &&
      c.props.type === 'h2' &&
      c.props.text?.toString() === SEED_MD_DEMO_H2,
  );
}

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

/**
 * Append subset + loss/opaque examples after the M0 seed. No-op if the demo
 * H2 is already on the note (Compose persist / second tab). Default seed and
 * goldens stay unchanged — App calls this only when `?md-demo` is set.
 */
export async function seedMarkdownDemo(store) {
  if (noteHasMarkdownDemo(store)) return false;
  const note = noteOf(store);
  if (!note) {
    throw new Error('seedMarkdownDemo: no affine:note');
  }
  const noteId = note.id;

  store.addBlock(
    'affine:paragraph',
    { type: 'h2', text: new Text(SEED_MD_DEMO_H2) },
    noteId,
  );
  store.addBlock(
    'affine:paragraph',
    { type: 'h3', text: new Text('Headings stay paragraph + type') },
    noteId,
  );
  store.addBlock(
    'affine:paragraph',
    { text: new Text('A body paragraph under the H3.') },
    noteId,
  );

  const outerId = store.addBlock(
    'affine:list',
    { type: 'bulleted', text: new Text('outer') },
    noteId,
  );
  store.addBlock(
    'affine:list',
    { type: 'bulleted', text: new Text('inner') },
    outerId,
  );
  store.addBlock(
    'affine:list',
    { type: 'numbered', text: new Text('one') },
    noteId,
  );
  store.addBlock(
    'affine:list',
    { type: 'numbered', text: new Text('two') },
    noteId,
  );
  store.addBlock(
    'affine:list',
    { type: 'todo', text: new Text('a task'), checked: false },
    noteId,
  );

  store.addBlock(
    'affine:code',
    { language: 'javascript', text: new Text('const x = 1;') },
    noteId,
  );
  store.addBlock(
    'affine:paragraph',
    {
      text: new Text([
        { insert: 'docs', attributes: { link: 'https://example.com/path' } },
      ]),
    },
    noteId,
  );
  store.addBlock(
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
  store.addBlock(
    'affine:paragraph',
    {
      text: new Text([
        {
          insert: 'Colored text',
          attributes: { color: 'var(--affine-palette-line-red)' },
        },
      ]),
    },
    noteId,
  );
  store.addBlock('affine:embed-linked-doc', { pageId: 'doc:lease' }, noteId);

  if (store.blobSync?.set) {
    const bytes = Uint8Array.from(atob(DEMO_PNG_B64), (c) => c.charCodeAt(0));
    const file = new File([bytes], 'dot.png', { type: 'image/png' });
    const sourceId = await store.blobSync.set(file);
    store.addBlock('affine:image', { sourceId }, noteId);
  }

  store.resetHistory();
  return true;
}
