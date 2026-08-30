import { toBase64 } from 'lib0/buffer';
import * as Y from 'yjs';
import {
  createMarkdownAdapter,
  markdownAdapterFor,
} from './markdown-adapter.js';

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

/** pageId in `<!-- venus:doc:… -->` — reject comment breakout / odd ids. */
const SAFE_PAGE_ID = /^[A-Za-z0-9_.:-]+$/;

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

function snapshotDeltaText(value) {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (Array.isArray(value.delta)) {
    return value.delta
      .map((op) => (typeof op.insert === 'string' ? op.insert : ''))
      .join('');
  }
  if (typeof value.toString === 'function') {
    const s = value.toString();
    if (s !== '[object Object]') return s;
  }
  return '';
}

function isEmptyParagraph(model, slice) {
  return model.flavour === 'affine:paragraph' && coreOf(slice) === '';
}

/** Empty body paragraphs: skip per-block adapter (last-N maps the newline). */
function isEmptyTextParagraphSnapshot(snapshot) {
  if (snapshot.flavour !== 'affine:paragraph') return false;
  const type = snapshot.props?.type;
  if (type != null && type !== 'text') return false;
  return snapshotDeltaText(snapshot.props?.text) === '';
}

function indentSlice(slice, listDepth) {
  if (listDepth <= 0) return slice;
  const pad = '  '.repeat(listDepth);
  const endsWithNl = slice.endsWith('\n');
  const lines = slice.replace(/\n+$/, '').split('\n');
  const body = lines.map((line) => (line === '' ? line : pad + line)).join('\n');
  return endsWithNl || slice.length === 0 ? `${body}\n` : body;
}

function placementCandidates(slice) {
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
  return candidates;
}

function escapeRegExp(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Per-block `fromBlockSnapshot` of a numbered item is always `1.`; the full
 * `fromDocSnapshot` uses `2.`, `3.`, … Same body, different marker.
 */
const NUMBERED_ITEM = /^((?:  )*)(\d+)\.([\s\S]*)$/;

function matchNumberedAt(markdown, slice, i) {
  const m = NUMBERED_ITEM.exec(slice);
  if (!m) return null;
  const pad = m[1];
  const rest = m[3];
  if (!markdown.startsWith(pad, i)) return null;
  const afterPad = i + pad.length;
  const num = /^(\d+)\./.exec(markdown.slice(afterPad));
  if (!num) return null;
  const restAt = afterPad + num[0].length;
  if (!markdown.startsWith(rest, restAt)) return null;
  return { start: i, end: restAt + rest.length };
}

function placeNumberedByIndexOf(markdown, slice, from) {
  const m = NUMBERED_ITEM.exec(slice);
  if (!m) return null;
  const pad = m[1];
  const rest = m[3];
  const re = new RegExp(`${escapeRegExp(pad)}\\d+\\.${escapeRegExp(rest)}`);
  const idx = markdown.slice(from).search(re);
  if (idx === -1) return null;
  const start = from + idx;
  const hit = markdown.slice(start).match(re);
  if (!hit) return null;
  return { start, end: start + hit[0].length };
}

function placeByIndexOf(markdown, candidates, from) {
  for (const candidate of candidates) {
    const idx = markdown.indexOf(candidate, from);
    if (idx !== -1) {
      return { start: idx, end: idx + candidate.length };
    }
  }
  for (const candidate of candidates) {
    const numbered = placeNumberedByIndexOf(markdown, candidate, from);
    if (numbered) return numbered;
  }
  return null;
}

/**
 * Prefer matching at the cursor (skip stringify-gap newlines). Falls back
 * to forward indexOf if the next bytes are not the slice (opaque gaps).
 */
function placeFromCursor(markdown, slice, from) {
  const candidates = placementCandidates(slice);
  let i = from;
  while (i <= markdown.length) {
    for (const candidate of candidates) {
      if (markdown.startsWith(candidate, i)) {
        return { start: i, end: i + candidate.length };
      }
      const numbered = matchNumberedAt(markdown, candidate, i);
      if (numbered) return numbered;
    }
    if (i < markdown.length && markdown[i] === '\n') {
      i += 1;
      continue;
    }
    break;
  }
  return placeByIndexOf(markdown, candidates, from);
}

/**
 * Venus linked-doc comment. Adapter emits `[title](url)` only.
 */
function venusDocComment(pageId) {
  return `<!-- venus:doc:${pageId} -->`;
}

function isSafePageId(pageId) {
  return (
    typeof pageId === 'string' &&
    pageId.length > 0 &&
    pageId.length <= 128 &&
    SAFE_PAGE_ID.test(pageId)
  );
}

function urlMentionsPageId(url, pageId) {
  const path = url.split(/[?#]/, 1)[0];
  return path === pageId || path.endsWith(`/${pageId}`);
}

function findLinkedDocInsert(markdown, pageId, from) {
  let search = from;
  while (search < markdown.length) {
    const open = markdown.indexOf('](', search);
    if (open === -1) return null;
    const close = markdown.indexOf(')', open + 2);
    if (close === -1) return null;
    const url = markdown.slice(open + 2, close);
    if (urlMentionsPageId(url, pageId)) {
      const nl = markdown.indexOf('\n', close);
      return {
        at: nl === -1 ? markdown.length : nl + 1,
        missingNl: nl === -1,
      };
    }
    search = open + 2;
  }
  return null;
}

function withVenusLinkedDocComment(slice, pageId) {
  if (!isSafePageId(pageId)) return slice;
  const comment = venusDocComment(pageId);
  if (slice.includes(comment)) return slice;
  const core = slice.replace(/\n+$/, '');
  return `${core}\n${comment}\n`;
}

function injectVenusLinkedDocComments(markdown, models) {
  const inserts = [];
  let searchFrom = 0;
  for (const { model } of models) {
    if (model.flavour !== 'affine:embed-linked-doc') continue;
    const pageId = model.props?.pageId;
    if (!isSafePageId(pageId)) continue;
    const comment = venusDocComment(pageId);
    const existing = markdown.indexOf(comment, searchFrom);
    if (existing !== -1) {
      searchFrom = existing + comment.length;
      continue;
    }
    const loc = findLinkedDocInsert(markdown, pageId, searchFrom);
    if (!loc) continue;
    inserts.push({ ...loc, comment });
    searchFrom = loc.at;
  }
  if (inserts.length === 0) return markdown;
  const chunks = [];
  let cursor = 0;
  for (const ins of inserts) {
    chunks.push(markdown.slice(cursor, ins.at));
    if (ins.missingNl) chunks.push('\n');
    chunks.push(`${ins.comment}\n`);
    cursor = ins.at;
  }
  chunks.push(markdown.slice(cursor));
  return chunks.join('');
}

/**
 * Own markdown for one block: nested ranged children are exported as their
 * own sidecar rows, so they must not appear in the parent slice.
 */
async function ownMarkdownFromSnapshot(adapter, snapshot) {
  if (!snapshot) return '';
  const own = {
    ...snapshot,
    children: (snapshot.children ?? []).filter(
      (c) => !RANGE_FLAVOURS.has(c.flavour),
    ),
  };
  const result = await adapter.fromBlockSnapshot({
    snapshot: own,
    assets: adapter.job.assetsManager,
  });
  const file = result?.file ?? '';
  if (snapshot.flavour === 'affine:embed-linked-doc') {
    return withVenusLinkedDocComment(file, snapshot.props?.pageId);
  }
  return file;
}

async function ownMarkdown(adapter, model) {
  const snapshot = adapter.job.blockToSnapshot(model);
  return ownMarkdownFromSnapshot(adapter, snapshot);
}

function placePageTitle(pageSnap, markdown) {
  if (!pageSnap || pageSnap.flavour !== 'affine:page') return null;
  const title = snapshotDeltaText(pageSnap.props?.title);
  if (!title) return null;
  const heading = `# ${title}\n`;
  if (!markdown.startsWith(heading)) {
    throw new Error(
      `Page title ${JSON.stringify(title)} not at start of fromDoc: ${JSON.stringify(markdown.slice(0, 80))}`,
    );
  }
  return { id: pageSnap.id, start: 0, end: heading.length };
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
      placeFromCursor(markdown, indented, cursor) ??
      placeFromCursor(markdown, slice, cursor);
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

export function collectRangedBlocks(root) {
  const out = [];
  if (root) collectRanged(root, out);
  return out;
}

/**
 * Adapter markdown for one ranged block, nested ranged children stripped,
 * list indent applied. Used by full `fromDoc` placement and by RAM splice.
 */
export async function blockMarkdownSlice(adapter, model, listDepth) {
  const slice = await ownMarkdown(adapter, model);
  return indentSlice(slice, listDepth);
}

export function encodeSidecarClock(ydoc) {
  return toBase64(Y.encodeStateVector(ydoc));
}

/**
 * Shared exporter: one frozen `docToSnapshot` + `fromDocSnapshot`, then
 * parallel per-block slices from that snapshot (empty text paragraphs skip
 * the adapter). Page title `# …` maps to `affine:page`. Extra blank lines
 * between note blocks are not in any range. Linked-doc cards get
 * `<!-- venus:doc:<pageId> -->` after the adapter link (post-process).
 */
export async function fromDoc(store, workspace) {
  const adapter = markdownAdapterFor(store, workspace);
  const docSnapshot = adapter.job.docToSnapshot(store);
  if (!docSnapshot?.blocks) {
    throw new Error('MarkdownAdapter job.docToSnapshot returned undefined');
  }

  const models = [];
  collectRanged(docSnapshot.blocks, models);

  const result = await adapter.fromDocSnapshot({
    snapshot: docSnapshot,
    assets: adapter.job.assetsManager,
  });
  if (!result) {
    throw new Error('MarkdownAdapter.fromDocSnapshot returned undefined');
  }

  const markdown = injectVenusLinkedDocComments(result.file, models);
  const titleRange = placePageTitle(docSnapshot.blocks, markdown);

  const items = await Promise.all(
    models.map(async ({ model, listDepth }) => {
      const slice = isEmptyTextParagraphSnapshot(model)
        ? ''
        : await ownMarkdownFromSnapshot(adapter, model);
      return { model, listDepth, slice };
    }),
  );

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
