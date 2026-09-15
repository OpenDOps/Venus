# Round-trippable subset and loss

**Status:** design + **step-1–4** Actuals (seed + `rt-*` goldens; sidecar ranges; linked-doc comment post-process; opaque image + loss-color). Types and loss classes below are the product contract; whitespace bytes follow the goldens. Fixtures: [fixtures.md](./fixtures.md). Exporter: [README.md](./README.md). Pane: [live-pane.md](./live-pane.md). Pin convert: [pin-convert.md](./pin-convert.md). Apply: [apply.md](./apply.md).

This is what agents and git may treat as source. Everything else is WYSIWYG-only or **opaque**.

## Round-trippable types (v1)

Pin: `@blocksuite/affine` **0.22.4** ([api-map.md](../api-map.md)). There is **no** `affine:heading` flavour — headings are `affine:paragraph` with `type` `h1`…`h6` (same as [seed](../../../apps/web/src/host/seed.js)).

| Flavour / form | Markdown shape (intent) | In sidecar |
|---|---|---|
| `affine:page` title (`titleMiddleware`) | ATX h1 `# ${title}` at file start | **Actual:** one range on `store.root.id`; slice `# Venus\n` for the seed. Not a note paragraph. |
| `affine:paragraph` `type` text (default) | Body paragraph | One range per block |
| `affine:paragraph` `type` `h1`…`h6` | ATX heading `#` … `######` | One range |
| `affine:list` (bulleted / numbered / todo if adapter emits it) | GFM `*` / `1.` / `- [ ]` (Actual bulleted is `*`, not `-`); nested lists indent two spaces | **Actual:** one range per list **item** (nested inner is its own row, slice `  * inner`). No list-container flavour / extra sidecar id. |
| `affine:code` | Fenced block with language if set | One range including fences |
| Inline **link** on paragraph/list/heading text | `[text](url)` | Inside the parent block’s range |
| `affine:embed-linked-doc` | Adapter link + Venus id comment (below) | **Actual:** one range including `<!-- venus:doc:<pageId> -->` |

**Linked-doc stable form** (post-process in `from-doc.js`; adapter emits only a link):

```markdown
[untitled](./workspace/venus-m0/doc:lease)
<!-- venus:doc:doc:lease -->
```

**Actual (M2, no catalog):** `titleMiddleware` has no meta for a synthetic `pageId`, so the link text is `untitled`. `docLinkBaseURLMiddleware` in Node uses base `.` → URL `./workspace/<workspace.id>/<pageId>`. Venus appends `<!-- venus:doc:<pageId> -->` after that line. That form is the **oracle golden** (`rt-linked-doc.md`). It is **not** what `wiki/` stores after the catalog exists.

**Actual (M4 git / product):** same two lines, but human path is catalog:

```markdown
[protocol](protocol.md)
<!-- venus:doc:doc:protocol -->
```

Link text = target catalog `name`. Href = POSIX relative from the exporting file’s `gitPath` to the target `gitPath` (same dir → `protocol.md`; nested → `../design/protocol.md`). Comment `pageId` = live `affine:embed-linked-doc` `pageId` = catalog `docId` (guid). Import / restore: `venus:doc:` first, catalog `gitPath` second ([venus-design](../venus-design.md#cross-document-references)). A Flush that `git mv`s also rewrites hrefs in the same cut so that SHA’s clone links match the folders. Do not write `./workspace/<ws>/…` into git.

### Linked-doc export vs toDoc

`toDoc` of the export does **not** restore `affine:embed-linked-doc`. Adapter **import** of linked docs is a footnote JSON definition, not `[title](url)` + HTML comment. Measured second `fromDoc` after `toDoc`: empty-label link `[](./workspace/venus-m0/doc:lease)` and the comment becomes an `html` fenced code block. **M2 exit** is the export golden (`rt-linked-doc.md`). Recreating the card from `venus:doc:` is apply (M6), not adapter `toDoc`.

### Round-trip bar

For a Store that contains **only** the types above:

```text
fromDoc → toDoc (note body) → restore page title → fromDoc
```

**Actual (0.22.4):** `MarkdownAdapter.toDoc` of a **whole** Venus export mints page title `Untitled` and parses `# Venus` as a note H1. Helper `roundTripFromDoc` therefore `toDoc`s only the markdown **after** the page-title sidecar range, then writes the original title back onto the imported page. Goldens: `apps/web/src/host/mdgate/goldens/rt-*.md`. Tests: `roundtrip.test.ts`.

The second markdown is **byte-identical** to the first for `rt-paragraph`, `rt-headings`, `rt-list`, `rt-code`, `rt-link`, `rt-marks`. **`rt-linked-doc` is export-only** — see [linked-doc remainder](#linked-doc-export-vs-todoc).

`toDoc` here is only to prove the adapter’s parse of **our** note export. Apply to the live page still uses [markdown vs `T0`](./apply.md), not whole-file replace.

## Whitespace rules

Pin these after recon; until then tests compare with the listed exemptions.

| Rule | Meaning |
|---|---|
| **Offsets** | Sidecar `start`/`end` are **UTF-16 code units** (JS / CodeMirror), half-open `[start, end)`. |
| **Block slice** | Range covers that block’s markdown, including its **terminating newline** if `fromDoc` emits one. |
| **Gaps** | Extra blank lines **between** blocks belong to **neither** range. A diff in a gap that adds **non-newline** markdown is an **insert** hunk (`afterBlockId` = previous block). A diff that only adds/removes `\n` in a gap is **not** a hunk ([empty / stringify](#empty-paragraphs-and-stringify-gaps)). |
| **EOF** | **Actual (seed golden):** exactly one trailing `\n`. A second `fromDoc` must not flip `\n` vs `\n\n` at EOF. |
| **Empty paragraph** | **Actual:** an empty `affine:paragraph` is a **blank line** (not ` \n`). Sidecar: **last-N** — in the newline run before the next non-empty block, the last N `\n`s are the N empty ids (one each). Remark may emit **extra** `\n`s around them; those extras are gaps. Apply: [empty / stringify](#empty-paragraphs-and-stringify-gaps). |
| **No smart quotes / Unicode NFC** | Do not normalize. Round-trip is the adapter’s bytes. |

If recon finds the adapter unstable on empty paragraphs or list markers, **cut that case from the subset** or list it under [loss](#loss-wysiwyg-only) — do not hide jitter in tests.

## Empty paragraphs and stringify gaps

Measured on 0.22.4 (`from-doc.test.ts`): two text paras export as `Alpha\n\nBravo\n` (one gap `\n` between ranges). The same note with an empty para between them exports as `Alpha\n\n\n\nBravo\n`. One empty CRDT block is **not** one extra byte. Remark wraps it; last-N maps only the **last** `\n` of that run to the empty id; the other `\n`s stay gaps.

**Do not** post-process `fromDoc` to squeeze those blanks (second dialect). **Do not** invent a marker for empty paras.

| Layer | What to do |
|---|---|
| **M2 export** | Keep last-N. Golden the exact bytes. Empty paras still get sidecar ids so typing *text* on that `\n` is `updateBlock` of that id. |
| **M6 apply** | **Newline-only** edits in gaps (and extra blanks around empties) are **no hunk** — same family as `ap-noop-ws`. **Insert** only when a gap gains **non-newline** content (`Hello`, `#`, `*`, fence, …). Do not `addBlock` / `deleteBlock` empty paras because an agent added or removed blank lines. Seed spacers (24 empties) must survive a “tidy the markdown” commit. |
| **Agents** | Do not tell them “add a blank line to create an empty paragraph.” Empty paras are WYSIWYG (outline scroll). Filling the mapped `\n` with text is a normal paragraph edit. |

Creating or deleting empty CRDT paragraphs from markdown blank-line count is **out of v1 apply**. That is lossy subset (like color): the id exists; blank-line jitter is not a hunk.

## Opaque blocks

Anything `fromDoc` cannot name as a subset type is **opaque**: HTML comment, raw HTML, or a raw BlockSuite dump — recon picks one form and goldens it.

| Flavour | Actual on 0.22.4 | Why opaque |
|---|---|---|
| `affine:image` / page image | GFM `![dot.png](assets/dot.png)` when the store blob is a `File` named `dot.png` (golden `opaque-image.md`). Sidecar: one range covering that line. `fromDoc` twice: slice byte-equal. | Path names a git `assets/` file we do **not** write in M2. Pixels stay in the blob store. Git flush of `assets/` is M3. |
| `affine:surface` and edgeless | Not measured in M2 (page mode unused). | Must not rewrite on a markdown commit that did not touch it. |
| `affine:embed-synced-doc` | Not measured in M2. | Transclusion; not the linked-doc card form. |
| Bookmark / embed / database / unknown flavours | Not measured in M2. | Adapter drop or rewrite. |

**Apply:** if the markdown diff does not overlap an opaque block’s sidecar range, that id is a **no-op**. A commit must not “pretty-print” opaque regions.

## Loss (WYSIWYG-only)

Documented so agents are not told to round-trip them. They may appear in WYSIWYG and in git as **omitted** or **flattened**.

| Feature | What git/markdown keeps |
|---|---|
| Text color, background, highlight | Plain text. **Actual:** `AffineTextAttributes.color` is stored on the CRDT (`paragraphHasColorMark`); markdown adapter has no color matcher. Golden `loss-color.md` is the same bytes as a plain paragraph (`Colored text`). Second `fromDoc` equals first. |
| Empty-paragraph **count** vs extra blank lines in git | WYSIWYG empties kept; blank-line-only markdown diffs are not hunks |
| Other marks the adapter drops (recon: `color`, `background`, `mention`) | Dropped. **Kept on 0.22.4:** bold, italic, inline code (`rt-marks.md`); also strike / underline / link if present |
| Column / min-width / note display mode | Not in markdown |
| Image pixels | Blob in Postgres; git `assets/` only after M3 flush |
| Exact CRDT history / undo stack | Never |

Loss is **not** a hunk. `fromDoc` of a colored paragraph that exports as plain text, then `fromDoc` again, must be stable **as that plain text**. Do not apply ops that strip color on an untouched block (opaque/no-op). If color is on a **subset** paragraph and export drops it, that block is **lossy subset**: still one sidecar id; markdown edit of the text still `updateBlock` text, not a license to rewrite neighbours.

## Out of subset (do not invent)

- Custom fences (` ```venus-step `, YAML-in-markdown as schema) until this subset is boring ([venus-plan](../../drafts/pre-design/venus-plan.md)).
- Markdown-as-Y.Text.
- Per-block `<!-- id:b1 -->` in the body (sidecar is the id map).

## Recon checklist (M2 first coding day)

Filled in [M2 step 1](../M2/plan.md#1-step-recon-adapter). Adapter Actuals: [api-map.md](../api-map.md#names--markdown-adapter). Golden: `apps/web/src/host/mdgate/goldens/seed.fromDoc.md`.

1. **Golden `fromDoc` of seed note** — done. Page title is `# Venus` (`titleMiddleware`), then the empty leading paragraph as blank lines, then `# Why Venus`, body, 24 spacer blank-line runs, `## Empty host`, body. Vitest: `from-doc.test.ts` **Seed fromDoc**. **Sidecar:** `# Venus\n` is `affine:page` (`root.id`). Empty paragraphs take the last N newlines before the next non-empty block. Extra blank lines between two text paragraphs are gaps (neither range). An empty para between two texts is **not** one extra `\n` in the file — remark emits extra blanks around it; last-N maps only the last `\n` to the empty id.
2. **List item vs list container** — markdown form: nested bullets stringify as `* outer` / `  * inner`. **Sidecar Actual:** one range per `affine:list` item (outer `* outer`, inner `  * inner`). Not a list container. Vitest: `from-doc.test.ts` recon list.
3. **Opaque form for `affine:image`** — **done** in [step 4](../M2/plan.md#4-step-sidecar). Node: `blobSync.set(new File(dot.png))` + `addBlock('affine:image', { sourceId })`. Export: `![dot.png](assets/dot.png)`. Golden: `opaque-image.md`. Vitest: `sidecar.test.ts` **opaque-untouched**.
4. **Bold/italic/inline code** `fromDoc → toDoc → fromDoc` — **done.** They survive. Golden: `rt-marks.md` (`**bold** *italic* \`code\``). Vitest: `roundtrip.test.ts` **rt-marks**.

Whitespace Actual **wins** over the intent table if they disagree; update this file, do not weaken fixtures.
