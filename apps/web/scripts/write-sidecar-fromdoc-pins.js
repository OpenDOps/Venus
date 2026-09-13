#!/usr/bin/env node
/**
 * Write crates/venus-sidecar/tests/fixtures/*.yjs pins for M3 step-rust-adapter
 * identity tests (M2 goldens + large file). Re-run after adapter helpers change:
 *
 *   pnpm --filter @venus/web exec node ./scripts/write-sidecar-fromdoc-pins.js
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = join(here, '..');
const repoRoot = join(webRoot, '../..');
const outDir = join(repoRoot, 'crates/venus-sidecar/tests/fixtures');

const LARGE_PARAS = 2000;
const LARGE_PAD = 'xy'.repeat(130);

const server = await createServer({
  configFile: join(webRoot, 'vite.config.ts'),
  root: webRoot,
  cacheDir: join(webRoot, 'node_modules/.vite-write-fromdoc-pins'),
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'error',
});

function noteOf(store) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  if (!note) throw new Error('no affine:note');
  return note;
}

async function pinSession(workspace, sync, pinMod, build) {
  const session = await workspace.createM0Workspace(
    new sync.MemoryNoopProvider(),
  );
  const note = noteOf(session.store);
  await build(session, note);
  const { bytes } = pinMod.pinYjsBytes(session.store.spaceDoc);
  if (bytes.byteLength <= 2) {
    throw new Error(`pin too small: ${bytes.byteLength} bytes`);
  }
  return bytes;
}

function writePin(name, bytes) {
  const out = join(outDir, name);
  writeFileSync(out, bytes);
  process.stderr.write(`wrote ${bytes.byteLength} bytes to ${out}\n`);
}

let code = 1;
try {
  mkdirSync(outDir, { recursive: true });
  const workspace = await server.ssrLoadModule('/src/host/workspace.js');
  const sync = await server.ssrLoadModule('/src/host/sync-provider.js');
  const pinMod = await server.ssrLoadModule('/src/host/mdgate/pin-from-doc.js');
  const adapter = await server.ssrLoadModule(
    '/src/host/mdgate/markdown-adapter.js',
  );

  const write = async (name, build) => {
    const bytes = await pinSession(workspace, sync, pinMod, build);
    writePin(name, bytes);
  };

  await write('rt-paragraph.yjs', async (session, note) => {
    adapter.replaceNoteWithParagraphs(session.store, note, ['Hello paragraph']);
  });

  await write('rt-headings.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addHeading(session.store, note.id, 'h1', 'Why Venus');
    adapter.addParagraph(
      session.store,
      note.id,
      'A thin host around BlockSuite: one workspace, one page.',
    );
    adapter.addHeading(session.store, note.id, 'h2', 'Empty host');
  });

  await write('rt-list.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addNestedBulletedList(session.store, note.id, 'outer', 'inner');
  });

  await write('rt-code.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addCodeBlock(session.store, note.id, 'javascript', 'const x = 1;');
  });

  await write('rt-link.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addLinkParagraph(
      session.store,
      note.id,
      'docs',
      'https://example.com/path',
    );
  });

  await write('rt-marks.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addMarksParagraph(session.store, note.id);
  });

  await write('rt-linked-doc.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addEmbedLinkedDoc(session.store, note.id, 'doc:lease');
  });

  await write('loss-color.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addColoredParagraph(
      session.store,
      note.id,
      'Colored text',
      'var(--affine-palette-line-red)',
    );
  });

  await write('opaque-image.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    adapter.addParagraph(session.store, note.id, 'Hello paragraph');
    adapter.addImageBlock(session.store, note.id, 'unresolved-source');
  });

  await write('large-home.yjs', async (session, note) => {
    adapter.clearNote(session.store, note);
    const texts = [];
    for (let i = 0; i < LARGE_PARAS; i += 1) {
      texts.push(`Paragraph-${String(i).padStart(4, '0')} ${LARGE_PAD}`);
    }
    adapter.replaceNoteWithParagraphs(session.store, note, texts);
  });

  code = 0;
} catch (err) {
  process.stderr.write(err instanceof Error ? `${err.stack}\n` : `${err}\n`);
} finally {
  await Promise.race([
    server.close(),
    new Promise((resolve) => {
      setTimeout(resolve, 2000);
    }),
  ]).catch(() => {});
  process.exit(code);
}
