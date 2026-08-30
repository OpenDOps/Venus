# Data model

**Status:** design. Product rules (lease, freeze, two git classes, views): [venus-design.md](../venus-design.md). Wire and keck: [CRDT](../CRDT/README.md). Pin then git convert: [LiveSnapshot](../LiveSnapshot/README.md). Markdown projection: [MDGate](../MDGate/README.md) (RAM pane in [M2](../M2/README.md); git sidecar in M3).

This folder is **what is stored where**. Markdown is a **projection** of a block tree, not a second replica.

## Two persistences

| Persistence | Holds | Engine (prototype) |
|---|---|---|
| **CRDT** | Live published pages, catalog, review session, per-commit Before/After, blobs | Yjs in the browser; OctoBase keck + **Postgres** |
| **Git** | Share history: folders, `.md`, sidecar ids, assets, commit messages | One repo on disk; Venus is the only v1 writer |

Postgres is **not** a markdown store. Git is **not** the live page. Cloud later replaces keck; the **spaces and git layout** stay.

```text
browsers ──Yjs──► keck ──► Postgres     published, catalog, review, Before/After, blobs
                          │
                     pin + fromDoc
                          ▼
                    wiki/  git          .md + .venus/ids + assets
```

## Invariants

1. Published blocks contain only **accepted** content. No hunks, rationales, or lease fields on that tree.
2. Every BlockSuite page (published or review After/Before) is one **OctoBase space**, created **once**. Same `space_id` is never minted independently on two devices.
3. **Path is not identity.** Catalog `docId` (space id) is. Git path can change (`git mv`).
4. Review After/Before are real Y.Docs in OctoBase, not tab RAM. After-edits after submit are a **new** comment-commit (`parentCommitId`), not a rewrite of the parent.
5. Git commits are either **snapshot** (WYSIWYG, autocomment) or **comment-commit** (required why). Venus writes them; clones do not `git push` into the live tree in v1.

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | This map |
| [crdt.md](./crdt.md) | Spaces: published, catalog, review, Before/After, blobs, Postgres |
| [git.md](./git.md) | `wiki/` tree, sidecar, two commit classes |

M1 only implements **one** published space (`venus-m0` / `doc:home`). Catalog, review, and extra page spaces start at M3–M6. The layout below is the product model those milestones fill in.
