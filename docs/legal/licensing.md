# Venus licensing overview

How Venus may use BlockSuite, y-octo, Yjs, and OctoBase — open source vs commercial, and what that implies for a hosted cloud service.

This is an engineering summary of public license texts as of 29 August 2026. It is **not legal advice**. Confirm with counsel before shipping a paid or closed product.

## Decision

- **BlockSuite, Yjs, and y-octo** are acceptable in open-source and commercial Venus.
- **OctoBase is AGPL-3.0.** [M1](../design/M1/README.md) used keck as a prototype merge buffer. **[M3.0](../design/M3.0/README.md) replaces keck** with a Venus-owned **hub** (new files MIT/Apache). Do not ship keck on the product path after M3.0.
- If someone still runs a **public** hosted keck (M1 image, overlays): treat it as an AGPL network program and publish that keck source. A **closed** SaaS that includes keck is the gray/combined-work case. This is **not legal advice.**

The live contract is **Yjs update binaries + spaces (docs) + blobs**. M1 implemented that with keck. After M3.0 the hub implements it. There is no Hocuspocus / nbstore cloud target.

## Current licenses

| Piece | License | Copyleft | Venus use |
|---|---|---|---|
| **BlockSuite** (`@blocksuite/affine`, `@blocksuite/store`, adapters) | [MPL-2.0](https://github.com/toeverything/blocksuite) | Weak, **file-level** | Editor in prototype and cloud |
| **Yjs** | MIT | None | Browser CRDT (BlockSuite already uses it) |
| **y-octo** | [MIT](https://crates.io/crates/y-octo) | None | **Required** M3.0+ hub merge; convert-worker hydrate. OK in cloud |
| **OctoBase** | [AGPL-3.0](https://github.com/toeverything/OctoBase) | Strong, including **network / SaaS** | **M1 keck only.** Product after M3.0 does not ship it. If you still run the M1 image: disclose overlays |

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

“Venus talks to unmodified OctoBase over WebSocket, so they are two programs” is a **gray** argument. Dirty **notify** from keck to Venus is a tighter coupling than unmodified WS. Do not assume a closed Venus app is safe without counsel.

If OctoBase stays in a **public** cloud: publish **keck** source (including overlays). Venus application files are not automatically AGPL; do not treat that as a court holding.

## y-octo and Yjs (MIT)

Both may be used in prototype and cloud, including closed commercial products, with the usual MIT notice. y-octo is the Yjs-compatible Rust engine; it is **not** AGPL. OctoBase is the AGPL workspace/sync wrapper around it.

## Product shapes

| Venus you ship | BlockSuite | y-octo / Yjs | OctoBase keck |
|---|---|---|---|
| M1 prototype (Compose `octobase`) | Yes | Yes | **Yes** — AGPL image |
| Product after M3.0 (Compose `hub`) | Yes | Yes | **No** as a shipped binary |
| Open source MIT/Apache hub, no keck | Yes | Yes | **No** |
| Open source, AGPL, OctoBase inside | Yes | Yes | **Yes** (not the product path) |
| Hosted cloud **with** leftover keck | Yes | Yes | **Yes** — disclose keck source |
| Closed SaaS **with** keck | Yes | Yes | **Gray** — counsel; conservative publish keck |

## Product collab after M3.0

```text
BlockSuite Store (Y.Doc)
        │  Yjs update binary  (AFFiNE WS)
        ▼
Venus hub  (hosted Docker; MIT/Apache Venus code)
        │  persist ~1s; SQL trigger → dirty
        ▼
Postgres crdt_* + blobs  (hosted)
        └── Venus snapshotter beside the hub (not inside it)
```

Do not call OctoBase **block** APIs from the web bundle. Depend on Yjs binaries, space ids, blobs, and the dirty trigger. M1 keck recon: [octobase.md](../design/LiveSnapshot/octobase.md).

## Venus’s own license

- **Hub** source (M3.0+) is Venus-owned (MIT/Apache on new files). It is not an OctoBase fork.
- Hosted **keck** (including overlays) is AGPL-shaped if you still run it: offer that source to users of the service.
- Venus files that are not OctoBase may use another license; a closed hosted **combined** product **with keck** is gray ([above](#octobase-agpl-30)).
- Do not plan around OctoBase relicensing to MPL.

## References

- [BlockSuite](https://github.com/toeverything/blocksuite) — MPL-2.0
- [OctoBase](https://github.com/toeverything/OctoBase) — AGPL-3.0; README notes planned MPL switch post-production
- [y-octo](https://github.com/toeverything/y-octo) / [crates.io](https://crates.io/crates/y-octo) — MIT
- [MPL 2.0 FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- Product/stack context: [venus-design.md](../design/venus-design.md), [venus-implementation-plan.md](../design/venus-implementation-plan.md)
