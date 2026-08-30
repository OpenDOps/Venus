# Round-trippable subset and loss

**Status:** design. Exact **bytes** of `fromDoc` are recon Actual (fill after the first Vitest golden on BlockSuite **0.22.4**). Types and loss classes below are the product contract. Fixtures: [fixtures.md](./fixtures.md). Exporter: [README.md](./README.md). Apply: [apply.md](./apply.md).

This is what agents and git may treat as source. Everything else is WYSIWYG-only or **opaque**.

## Round-trippable types (v1)

Pin: `@blocksuite/affine` **0.22.4** ([api-map.md](../api-map.md)). There is **no** `affine:heading` flavour — headings are `affine:paragraph` with `type` `h1`…`h6` (same as [seed](../../../apps/web/src/host/seed.js)).

| Flavour / form | Markdown shape (intent) | In sidecar |
|---|---|---|
| `affine:paragraph` `type` text (default) | Body paragraph | One range per block |
| `affine:paragraph` `type` `h1`…`h6` | ATX heading `#` … `######` | One range |
| `affine:list` (bulleted / numbered / todo if adapter emits it) | `-` / `1.` / `- [ ]` lines; nested lists indent | One range per **list item** (not the list container, unless recon proves otherwise) |
| `affine:code` | Fenced block with language if set | One range including fences |
| Inline **link** on paragraph/list/heading text | `[text](url)` | Inside the parent block’s range |
| `affine:embed-linked-doc` | Human path + page id comment (below) | One range |

**Linked-doc stable form** (post-process if the adapter omits the id):

```markdown
[Lease freeze](./crdt/lease.md)
<!-- venus:doc:<docId> -->
```

Import: `venus:doc:` first, path second ([venus-design](../venus-design.md#cross-document-references)).

### Round-trip bar

For a Store that contains **only** the types above:

```text
fromDoc → toDoc (or toDocSnapshot + load) → fromDoc
```

The second markdown is **byte-identical** to the first, except [whitespace](#whitespace-rules). Sidecar **ids** on the second export match the first for every block the writer did not delete.

`toDoc` here is only to prove the adapter’s parse of **our** export. Apply to the live page still uses [markdown vs `T0`](./apply.md), not whole-file replace.

## Whitespace rules

Pin these after recon; until then tests compare with the listed exemptions.

| Rule | Meaning |
|---|---|
| **Offsets** | Sidecar `start`/`end` are **UTF-16 code units** (JS / CodeMirror), half-open `[start, end)`. |
| **Block slice** | Range covers that block’s markdown, including its **terminating newline** if `fromDoc` emits one. |
| **Gaps** | Extra blank lines **between** blocks belong to **neither** range. A text diff in a gap is an **insert** hunk (`afterBlockId` = previous block). |
| **EOF** | At most one trailing newline on the file. A second `fromDoc` must not flip `\n` vs `\n\n` at EOF (or list the flip here as exempt). |
| **Empty paragraph** | An empty `affine:paragraph` is a stable form (recon: ` \n` vs `\n`). Whatever `fromDoc` emits once is the golden. |
| **No smart quotes / Unicode NFC** | Do not normalize. Round-trip is the adapter’s bytes. |

If recon finds the adapter unstable on empty paragraphs or list markers, **cut that case from the subset** or list it under [loss](#loss-wysiwyg-only) — do not hide jitter in tests.

## Opaque blocks

Anything `fromDoc` cannot name as a subset type is **opaque**: HTML comment, raw HTML, or a raw BlockSuite dump — recon picks one form and goldens it.

| Likely opaque on 0.22.4 (confirm in recon) | Why |
|---|---|
| `affine:image` / page image | Blob hash vs `blob:` URL; not a faithful git file unless we also write `assets/` (M3). Until then: opaque or named-lossy. |
| `affine:surface` and edgeless | v1 page mode unused; must not rewrite on a markdown commit that did not touch it. |
| `affine:embed-synced-doc` | Transclusion; not the linked-doc card form. |
| Bookmark / embed / database / unknown flavours | Adapter drop or rewrite. |

**Apply:** if the markdown diff does not overlap an opaque block’s sidecar range, that id is a **no-op**. A commit must not “pretty-print” opaque regions.

## Loss (WYSIWYG-only)

Documented so agents are not told to round-trip them. They may appear in WYSIWYG and in git as **omitted** or **flattened**.

| Feature | What git/markdown keeps |
|---|---|
| Text color, background, highlight | Plain text (marks dropped) |
| Other marks the adapter drops (recon list) | Dropped |
| Column / min-width / note display mode | Not in markdown |
| Image pixels | Blob in Postgres; git `assets/` only after M3 flush |
| Exact CRDT history / undo stack | Never |

Loss is **not** a hunk. `fromDoc` of a colored paragraph that exports as plain text, then `fromDoc` again, must be stable **as that plain text**. Do not apply ops that strip color on an untouched block (opaque/no-op). If color is on a **subset** paragraph and export drops it, that block is **lossy subset**: still one sidecar id; markdown edit of the text still `updateBlock` text, not a license to rewrite neighbours.

## Out of subset (do not invent)

- Custom fences (` ```venus-step `, YAML-in-markdown as schema) until this subset is boring ([venus-plan](../../drafts/pre-design/venus-plan.md)).
- Markdown-as-Y.Text.
- Per-block `<!-- id:b1 -->` in the body (sidecar is the id map).

## Recon checklist (M2 first coding day)

Fill Actual in [api-map.md](../api-map.md) (adapter import path, `fromDoc` / `toDoc` names) during [M2 step 1](../M2/plan.md#1-step-recon-adapter) and append here:

1. Golden `fromDoc` of seed note (h1, paragraphs, spacers, h2).
2. List item vs list container: which ids get ranges.
3. Opaque form for `affine:image`.
4. Whether bold/italic/inline code survive `fromDoc → toDoc → fromDoc`.

Whitespace Actual **wins** over the intent table if they disagree; update this file, do not weaken fixtures.
