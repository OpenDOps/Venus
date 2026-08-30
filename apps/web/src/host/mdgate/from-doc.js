import { toBase64 } from 'lib0/buffer';
import * as Y from 'yjs';
import { createMarkdownAdapter } from './markdown-adapter.js';

/**
 * Flavours that get a sidecar range from the note walk. `affine:page` is
 * not in this set — `fromBlock` of the page includes the whole tree.
 * The title heading (`titleMiddleware`) is mapped separately onto `root.id`.
 * Nested `affine:list` children are walked as their own items.
 */
const RANGE_FLAVOURS = new Set([
  'affine:paragraph',
  'affine:list',
  'affine:code',
  'affine:divider',
  'affine:image',
  'affine:embed-linked-doc',
]);

function collectRanged(model, out, listDepth = 0) {
  if (RANGE_FLAVOURS.has(model.flavour)) {
    out.push({
      model,
      listDepth: model.flavour === 'affine:list' ? listDepth : 0,
    });
  }
  const nextDepth =
    model.flavour === 'affine:list' ? listDepth + 1 : listDepth;
  for (const child of model.children ?? []) {
    collectRanged(child, out, nextDepth);
  }
}

function coreOf(slice) {
  return slice.replace(/\n+$/, '');
}

function isEmptyParagraph(model, slice) {
  return model.flavour === 'affine:paragraph' && coreOf(slice) === '';
}

function indentSlice(slice, listDepth) {
  if (listDepth <= 0) return slice;
  const pad = '  '.repeat(listDepth);
  const endsWithNl = slice.endsWith('\n');
  const lines = slice.replace(/\n+$/, '').split('\n');
  const body = lines.map((line) => (line === '' ? line : pad + line)).join('\n');
  return endsWithNl || slice.length === 0 ? `${body}\n` : body;
}

function placeNonEmpty(markdown, slice, from) {
  const candidates = [];
  const seen = new Set();
  const push = (s) => {
    if (s && !seen.has(s)) {
      seen.add(s);
      candidates.push(s);
    }
  };
  push(slice);
  const core = coreOf(slice);
  if (core) {
    push(`${core}\n`);
    push(core);
  }
  for (const candidate of candidates) {
    const idx = markdown.indexOf(candidate, from);
    if (idx !== -1) {
      return { start: idx, end: idx + candidate.length };
    }
  }
  return null;
}

/**
 * Venus linked-doc comment. Adapter emits `[title](url)` only.
 */
function venusDocComment(pageId) {
  return `<!-- venus:doc:${pageId} -->`;
}

function withVenusLinkedDocComment(slice, pageId) {
  if (!pageId) return slice;
  const comment = venusDocComment(pageId);
  if (slice.includes(comment)) return slice;
  const core = slice.replace(/\n+$/, '');
  return `${core}\n${comment}\n`;
}

function injectVenusLinkedDocComments(markdown, models) {
  let result = markdown;
  let cursor = 0;
  for (const { model } of models) {
    if (model.flavour !== 'affine:embed-linked-doc') continue;
    const pageId = model.props?.pageId;
    if (!pageId) continue;
    const comment = venusDocComment(pageId);
    const existing = result.indexOf(comment, cursor);
    if (existing !== -1) {
      cursor = existing + comment.length;
      continue;
    }
    const idAt = result.indexOf(String(pageId), cursor);
    if (idAt === -1) continue;
    const lineEnd = result.indexOf('\n', idAt);
    if (lineEnd === -1) {
      result = `${result}\n${comment}\n`;
      cursor = result.length;
      continue;
    }
    result =
      result.slice(0, lineEnd + 1) + `${comment}\n` + result.slice(lineEnd + 1);
    cursor = lineEnd + 1 + comment.length + 1;
  }
  return result;
}

/**
 * Own markdown for one block: nested ranged children are exported as their
 * own sidecar rows, so they must not appear in the parent slice.
 */
async function ownMarkdown(adapter, model) {
  const snapshot = adapter.job.blockToSnapshot(model);
  if (!snapshot) return '';
  const own = {
    ...snapshot,
    children: (snapshot.children ?? []).filter(
      (c) => !RANGE_FLAVOURS.has(c.flavour),
    ),
  };
  const result = await adapter.fromBlockSnapshot({ snapshot: own });
  const file = result?.file ?? '';
  if (model.flavour === 'affine:embed-linked-doc') {
    return withVenusLinkedDocComment(file, model.props?.pageId);
  }
  return file;
}

function placePageTitle(root, markdown) {
  if (!root || root.flavour !== 'affine:page') return null;
  const title = root.props?.title?.toString() ?? '';
  if (!title) return null;
  const heading = `# ${title}\n`;
  if (!markdown.startsWith(heading)) {
    throw new Error(
      `Page title ${JSON.stringify(title)} not at start of fromDoc: ${JSON.stringify(markdown.slice(0, 80))}`,
    );
  }
  return { id: root.id, start: 0, end: heading.length };
}

function assignEmptyParagraphs(markdown, items, placed, leadingStart) {
  let i = 0;
  while (i < items.length) {
    if (placed[i] !== 'empty') {
      i += 1;
      continue;
    }
    const group = [];
    while (i < items.length && placed[i] === 'empty') {
      group.push(i);
      i += 1;
    }
    const prev = group[0] - 1;
    const next = group[group.length - 1] + 1;
    const intervalStart =
      prev >= 0 && placed[prev] !== 'empty' ? placed[prev].end : leadingStart;
    const intervalEnd =
      next < items.length && placed[next] !== 'empty'
        ? placed[next].start
        : markdown.length;
    const region = markdown.slice(intervalStart, intervalEnd);
    const nl = [];
    for (let k = 0; k < region.length; k += 1) {
      if (region[k] === '\n') nl.push(intervalStart + k);
    }
    if (nl.length < group.length) {
      throw new Error(
        `Need ${group.length} empty-paragraph newline(s), found ${nl.length} in ${JSON.stringify(region)}`,
      );
    }
    const take = nl.slice(nl.length - group.length);
    for (let g = 0; g < group.length; g += 1) {
      const start = take[g];
      placed[group[g]] = { start, end: start + 1 };
    }
  }
}

function buildRanges(markdown, items, titleRange) {
  const leadingStart = titleRange ? titleRange.end : 0;
  const placed = new Array(items.length);
  let cursor = leadingStart;
  for (let i = 0; i < items.length; i += 1) {
    const { model, slice, listDepth } = items[i];
    if (isEmptyParagraph(model, slice)) {
      placed[i] = 'empty';
      continue;
    }
    const indented = indentSlice(slice, listDepth);
    const loc =
      placeNonEmpty(markdown, indented, cursor) ??
      placeNonEmpty(markdown, slice, cursor);
    if (!loc) {
      throw new Error(
        `Could not place block ${model.id} (${model.flavour}) slice=${JSON.stringify(indented)} at ${cursor} near ${JSON.stringify(markdown.slice(cursor, cursor + 80))}`,
      );
    }
    placed[i] = loc;
    cursor = loc.end;
  }
  assignEmptyParagraphs(markdown, items, placed, leadingStart);
  const noteBlocks = items.map((item, i) => ({
    id: item.model.id,
    start: placed[i].start,
    end: placed[i].end,
  }));
  return titleRange ? [titleRange, ...noteBlocks] : noteBlocks;
}

/** Yjs state vector of `store.spaceDoc`, lib0 base64. Sidecar `clock` Actual. */
export function encodeSidecarClock(ydoc) {
  return toBase64(Y.encodeStateVector(ydoc));
}

/**
 * Shared exporter: `MarkdownAdapter.fromDoc` plus a RAM sidecar.
 * Page title `# …` maps to `affine:page` (`root.id`). Extra blank lines
 * between note blocks are not in any range. Linked-doc cards get
 * `<!-- venus:doc:<pageId> -->` after the adapter link (post-process).
 */
export async function fromDoc(store, workspace) {
  const adapter = createMarkdownAdapter(store, workspace);
  const result = await adapter.fromDoc(store);
  if (!result) {
    throw new Error('MarkdownAdapter.fromDoc returned undefined');
  }

  const models = [];
  if (store.root) collectRanged(store.root, models);
  const markdown = injectVenusLinkedDocComments(result.file, models);
  const titleRange = placePageTitle(store.root, markdown);

  const items = [];
  for (const { model, listDepth } of models) {
    const slice = await ownMarkdown(adapter, model);
    items.push({ model, listDepth, slice });
  }

  const blocks = buildRanges(markdown, items, titleRange);
  const docId = store.doc?.id ?? store.id;
  return {
    markdown,
    sidecar: {
      docId,
      clock: encodeSidecarClock(store.spaceDoc),
      blocks,
    },
  };
}

function setPageTitle(store, title) {
  const current = store.root?.props?.title;
  if (!current || typeof current.toString !== 'function') return;
  const now = current.toString();
  if (now === title) return;
  if (typeof current.delete === 'function' && typeof current.insert === 'function') {
    current.delete(0, now.length);
    current.insert(title, 0);
  }
}

/**
 * `fromDoc` → `toDoc` of the **note body** (page-title heading stripped so
 * the adapter does not parse `# Venus` as a note H1) → restore page title →
 * `fromDoc`. Whole-file `toDoc` of a Venus export mints `Untitled` and treats
 * the title line as a heading — that is not the round-trip we gold.
 */
export async function roundTripFromDoc(store, workspace) {
  const first = await fromDoc(store, workspace);
  const pageId = store.root?.id;
  const title = store.root?.props?.title?.toString() ?? '';
  const firstNote = first.sidecar.blocks.find((b) => b.id !== pageId);
  const noteMd = firstNote
    ? first.markdown.slice(firstNote.start)
    : '';
  const adapter = createMarkdownAdapter(store, workspace);
  const imported = await adapter.toDoc({ file: noteMd });
  if (!imported) {
    throw new Error('MarkdownAdapter.toDoc returned undefined');
  }
  setPageTitle(imported, title);
  const importedWorkspace = {
    id: workspace.id,
    meta: {
      docMetas: [{ id: imported.doc?.id ?? imported.id, title }],
    },
  };
  const second = await fromDoc(imported, importedWorkspace);
  return { first, imported, second };
}
