import { markdownAdapterFor } from './markdown-adapter.js';
import {
  blockMarkdownSlice,
  collectRangedBlocks,
  encodeSidecarClock,
  fromDoc,
} from './from-doc.js';

const SPLICEABLE = new Set(['affine:paragraph']);

function coreOf(slice) {
  return slice.replace(/\n+$/, '');
}

function paragraphType(model) {
  return model.props?.type ?? 'text';
}

/** Infer ATX level from a previous sidecar slice (`# ` / `## ` …). */
function typeFromMarkdownRange(markdown, start, end) {
  const core = markdown.slice(start, end).replace(/\n+$/, '');
  const atx = /^(#{1,6}) /.exec(core);
  if (!atx) return 'text';
  return `h${atx[1].length}`;
}

/** Match full fromDoc placement: non-empty block range is `core + '\\n'`. */
function canonicalRangeSlice(indented) {
  const core = coreOf(indented);
  if (!core) return null;
  return `${core}\n`;
}

function canSplice(store, previous, dirtyIds, ranged) {
  if (!previous?.sidecar?.blocks?.length) return false;
  if (!Array.isArray(dirtyIds) || dirtyIds.length === 0) return false;
  const pageId = store.root?.id;
  if (!pageId || previous.sidecar.blocks[0]?.id !== pageId) return false;
  const currentNote = ranged.map((r) => r.model.id);
  const prevNote = previous.sidecar.blocks
    .filter((b) => b.id !== pageId)
    .map((b) => b.id);
  if (currentNote.length !== prevNote.length) return false;
  for (let i = 0; i < currentNote.length; i += 1) {
    if (currentNote[i] !== prevNote[i]) return false;
  }
  const byId = new Map(ranged.map((r) => [r.model.id, r]));
  const sidecarIds = new Set(previous.sidecar.blocks.map((b) => b.id));
  for (const id of dirtyIds) {
    if (id === pageId) return false;
    if (!sidecarIds.has(id)) return false;
    const item = byId.get(id);
    if (!item || !SPLICEABLE.has(item.model.flavour)) return false;
    const row = previous.sidecar.blocks.find((b) => b.id === id);
    if (!row) return false;
    // Empty-paragraph last-N is a newline in a gap, not ownMarkdown.
    if (coreOf(previous.markdown.slice(row.start, row.end)) === '') {
      return false;
    }
  }
  // titleMiddleware can change a card's link label with 0 Y.Text on this
  // block and without this id in dirtyIds. Do not shift later ranges.
  if (ranged.some((r) => r.model.flavour === 'affine:embed-linked-doc')) {
    return false;
  }
  // h1↔text (and h1↔h2): Y.Text length 0, markdown ± `# `. Structural.
  for (const id of dirtyIds) {
    const item = byId.get(id);
    if (!item || item.model.flavour !== 'affine:paragraph') continue;
    const row = previous.sidecar.blocks.find((b) => b.id === id);
    if (!row) return false;
    const prevType = typeFromMarkdownRange(
      previous.markdown,
      row.start,
      row.end,
    );
    if (prevType !== paragraphType(item.model)) return false;
  }
  return true;
}

/**
 * RAM pane update: splice in-place dirty paragraphs, else full `fromDoc`.
 * `forceFull` is the optimization-off path (same helper, measured in tests).
 */
export async function incrementalFromDoc(
  store,
  workspace,
  previous,
  dirtyIds,
  options = {},
) {
  const ranged = collectRangedBlocks(store.root);
  if (options.forceFull || !canSplice(store, previous, dirtyIds, ranged)) {
    const full = await fromDoc(store, workspace);
    return { ...full, mode: 'full' };
  }

  const adapter = markdownAdapterFor(store, workspace);
  let markdown = previous.markdown;
  const blocks = previous.sidecar.blocks.map((b) => ({ ...b }));
  const dirty = new Set(dirtyIds);

  for (const { model, listDepth } of ranged) {
    if (!dirty.has(model.id)) continue;
    const row = blocks.find((b) => b.id === model.id);
    const oldStart = row.start;
    const oldEnd = row.end;
    const indented = await blockMarkdownSlice(adapter, model, listDepth);
    const slice = canonicalRangeSlice(indented);
    if (!slice) {
      const full = await fromDoc(store, workspace);
      return { ...full, mode: 'full' };
    }
    // Adapter markdown length, UTF-16. Never Y.Text / keystroke count:
    // marks and `# ` change markdown with 0 Y.Text delta.
    const delta = slice.length - (oldEnd - oldStart);
    markdown = `${markdown.slice(0, oldStart)}${slice}${markdown.slice(oldEnd)}`;
    row.end = oldStart + slice.length;
    for (const b of blocks) {
      if (b.id === row.id) continue;
      if (b.start >= oldEnd) {
        b.start += delta;
        b.end += delta;
      }
    }
  }

  return {
    markdown,
    sidecar: {
      docId: previous.sidecar.docId,
      clock: encodeSidecarClock(store.spaceDoc),
      blocks,
    },
    mode: 'splice',
  };
}
