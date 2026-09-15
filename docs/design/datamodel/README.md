# Data model

**Status:** design. Product + data design (goal, hub stack, lease, git, agentic): [venus-design.md](../venus-design.md). Wire: [CRDT](../CRDT/README.md) (M1 keck; product [M3.0 hub](../M3.0/README.md)). Pin then git convert: [LiveSnapshot](../LiveSnapshot/README.md). Markdown projection: [MDGate](../MDGate/README.md) (RAM pane in [M2](../M2/README.md); git sidecar in M3). Workspace index (after SHA): [LifeIndexing](../Agents/LifeIndexing.md).

This folder is **what is stored where**. Markdown is a **projection** of a block tree, not a second replica.

## Two persistences

| Persistence | Holds | Engine (prototype) |
|---|---|---|
| **CRDT** | Live published pages, catalog, review session, per-commit Before/After, blobs | Yjs in the browser; **Venus hub** + **Postgres** (`crdt_*`). M1: keck + `jwst`. |
| **Git** | Share history: folders, `.md`, sidecar ids, assets, commit messages | One repo on disk; Venus is the only v1 writer |

Postgres is **not** a markdown store. Git is **not** the live page. Cloud **does not keep keck** after M3.0; spaces and git layout stay.

```text
browsers ──Yjs──► hub ──► Postgres     published, catalog, review, Before/After, blobs
                          │
                     pin + fromDoc
                          ▼
                    wiki/  git          .md + .venus/ids + .venus/pages.yaml + assets
                          │
                     last_flushed
                          ▼
                    LifeIndexing        gists / tags / graphs at that SHA
                                        (async; not the spec)
```

## Invariants

1. Published blocks contain only **accepted** content. No hunks, rationales, or lease fields on that tree.
2. Every BlockSuite page (published or review After/Before) is one **hub space**, created **once**. Same `space_id` is never minted independently on two devices.
3. **Path is not identity.** Catalog `docId` (space id) is. Git path can change (`git mv`).
4. Review After/Before are real Y.Docs on the hub, not tab RAM. After-edits after submit are a **new** comment-commit (`parentCommitId`), not a rewrite of the parent.
5. Git commits are either **snapshot** (WYSIWYG, autocomment) or **comment-commit** (required why). Venus writes them; clones do not `git push` into the live tree in v1.

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | This map |
| [crdt.md](./crdt.md) | Spaces: published, catalog, review, Before/After, blobs, Postgres |
| [git.md](./git.md) | `wiki/` tree, sidecar, two commit classes, `pages.yaml` (pages + folders projection) |
| [page-identity.md](./page-identity.md) | uuid, docname, POSIX filename, DB `page_identity`, YAML. Not a new component. |

M1 only implements **one** published space (`venus-m0` / `doc:home`). Catalog and extra page spaces start at [M4](../M4/README.md). Review spaces start at M5–M6. The layout below is the product model those milestones fill in.
