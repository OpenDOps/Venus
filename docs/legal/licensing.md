# Venus licensing overview

How Venus may use BlockSuite, y-octo, Yjs, and OctoBase — open source vs commercial, and what that implies for a hosted cloud service.

This is an engineering summary of public license texts as of 29 August 2026. It is **not legal advice**. Confirm with counsel before shipping a paid or closed product.

## Decision

- **BlockSuite, Yjs, and y-octo** are acceptable in both an open-source Venus and a later commercial/cloud Venus.
- **OctoBase is AGPL-3.0.** It is acceptable **for prototyping** (local / single-server, source available).
- **OctoBase should be replaced** before Venus is offered as a **cloud service** (hosted SaaS, closed or MIT/Apache product). Do not couple the cloud architecture to OctoBase APIs.

The live contract Venus actually needs is **Yjs update binaries + spaces (docs) + blobs**. OctoBase is one implementation of that contract, not the product.

Replacement targets for cloud: **Yjs + y-websocket or Hocuspocus** (or equivalent), with **y-octo** as an optional MIT Rust helper for merge/snapshots. Keep a swappable provider so the prototype can use OctoBase without painting the cloud into AGPL.

## Current licenses

| Piece | License | Copyleft | Venus use |
|---|---|---|---|
| **BlockSuite** (`@blocksuite/affine`, `@blocksuite/store`, adapters) | [MPL-2.0](https://github.com/toeverything/blocksuite) | Weak, **file-level** | Editor in prototype and cloud |
| **Yjs** | MIT | None | Browser CRDT (BlockSuite already uses it) |
| **y-octo** | [MIT](https://crates.io/crates/y-octo) | None | Server merge / snapshots; OK in cloud |
| **OctoBase** | [AGPL-3.0](https://github.com/toeverything/OctoBase) | Strong, including **network / SaaS** | Prototype only; replace for cloud |

Toeverything’s OctoBase README states they will switch to MPL (or looser) after production-ready. That has not happened (still pre-1.0). **Do not plan the cloud license around a relicensing with no date.**

There is no public paid “OctoBase commercial license.” AFFiNE Cloud / Enterprise is a different product.

AFFiNE Community Edition is MIT; Venus does **not** take the AFFiNE app shell, so that MIT does not cover OctoBase.

## BlockSuite (MPL-2.0)

MPL-2.0 is designed so a product can **use** the library without open-sourcing the whole app.

Allowed:

- Open-source Venus (MIT, Apache-2.0, MPL, GPL, AGPL).
- Paid / closed-source Venus.
- Hosted Venus.

Required:

- Keep MPL notices on BlockSuite files.
- If Venus **modifies** BlockSuite source files, those files stay MPL, and recipients (including users of a distributed binary or, in practice, anyone you owe source to under MPL) must be able to get that modified source.
- New Venus files (lease, git, review UI, catalog) may use any license, as long as they are **separate files**, not pasted into MPL files.

Not required: open-sourcing the entire product merely because `@blocksuite/affine` is imported.

## OctoBase (AGPL-3.0)

AGPL does **not** forbid selling software. It forbids keeping the **combined program** closed when you distribute it or let users interact with it over a network.

Typical reading:

| Situation | OctoBase |
|---|---|
| Private prototype, no public SaaS | Allowed |
| Open-source Venus under **AGPL-3.0** that includes OctoBase | Allowed (users of the service get source) |
| MIT/Apache Venus that **embeds** OctoBase in one binary/product | Not compatible |
| Closed-source or closed-source SaaS that includes OctoBase | What AGPL is meant to block, unless the whole combined work is published under AGPL |

“Venus talks to unmodified OctoBase over WebSocket, so they are two programs” is a **gray** argument. Do not bet a closed cloud on it.

If OctoBase stays in a **public** cloud, the conservative path is: license that server stack **AGPL-3.0** and publish source. Venus’s product decision is instead to **replace OctoBase** for cloud so Venus itself can be MIT/Apache (or closed) without that coupling.

## y-octo and Yjs (MIT)

Both may be used in prototype and cloud, including closed commercial products, with the usual MIT notice. y-octo is the Yjs-compatible Rust engine; it is **not** AGPL. OctoBase is the AGPL workspace/sync wrapper around it.

## Product shapes

| Venus you ship | BlockSuite | y-octo / Yjs | OctoBase |
|---|---|---|---|
| Prototype (local / single-server) | Yes | Yes | **Yes** (convenience) |
| Open source, AGPL, self-hosted, OctoBase still inside | Yes | Yes | Yes, but not the cloud plan |
| Open source, MIT/Apache | Yes | Yes | **No** as a product dependency |
| Commercial cloud (hosted), source public or closed | Yes | Yes | **Replace** |
| Closed-source app / closed SaaS | Yes | Yes | **Replace** |

## What “replace OctoBase” means

Keep the same client contract:

```text
BlockSuite Store (Y.Doc)
        │  Yjs update binary
        ▼
sync provider (swappable)
        │
        ▼
server: persist updates + snapshots + blobs
        └── Venus: snapshot at lease, markdown export (y-octo OK here)
```

| Phase | Sync / store |
|---|---|
| Prototype | OctoBase (WebSocket, spaces, blobs) **or** `y-websocket` if OctoBase’s JS provider is too raw |
| Cloud | Not OctoBase. Prefer **Hocuspocus** or **y-websocket** (MIT) + own persistence (Postgres, S3 blobs). Optional **y-octo** for merge/compact/snapshot |

Do not call OctoBase-specific block APIs from Venus. Depend on Yjs binaries, doc ids, and blobs so the swap is a provider change.

## Venus’s own license

- If the repo still contains OctoBase as a shipped dependency: treat the **combined** server as AGPL-shaped, or isolate OctoBase so it is clearly prototype-only and not in the cloud image.
- If OctoBase is out of the cloud stack: Venus may be **Apache-2.0 or MIT** (BlockSuite remains MPL on its own files).
- Prefer choosing Venus’s license **after** the cloud sync component is picked, not around OctoBase’s hoped-for MPL relicensing.

## References

- [BlockSuite](https://github.com/toeverything/blocksuite) — MPL-2.0
- [OctoBase](https://github.com/toeverything/OctoBase) — AGPL-3.0; README notes planned MPL switch post-production
- [y-octo](https://github.com/toeverything/y-octo) / [crates.io](https://crates.io/crates/y-octo) — MIT
- [MPL 2.0 FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- Product/stack context: [venus-design.md](../design/venus-design.md), [venus-implementation-plan.md](../design/venus-implementation-plan.md)
