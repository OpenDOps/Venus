# M2 — Markdown projection

Read-only markdown pane on the **synced Store**: `MarkdownAdapter.fromDoc` + RAM sidecar, [live-pane](../MDGate/live-pane.md) loop, **highlight.js** source paint. **Export fixture suite** green ([fixtures.md](../MDGate/fixtures.md) M2 rows). **No git, lease, apply, catalog, or editable markdown.**

**Status:** in progress. Board: [M2.state.yaml](./M2.state.yaml) — steps 1–3 `done`; 4–8 `pending`.

This folder is the implementation contract for the M2 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). The wiki plan runner does not exist yet, so the plan lives here. Contract: [MDGate](../MDGate/README.md). Stores: [datamodel](../datamodel/README.md) (projection is RAM; Postgres stays Yjs; git is M3). Dataflow: [architecture.md](../architecture.md#markdown-projection-add-here-before-coding-m2).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M2.state.yaml](./M2.state.yaml) | Board: step status only |

Shared: [api-map.md](../api-map.md) (adapter Actuals filled in `step-recon-adapter`). Words: [glossary.md](../glossary.md). Subset: [subset.md](../MDGate/subset.md).

**Depends on:** [M1](../M1/README.md) closed (2026-08-30). Host already syncs `doc:home`; `mount-editor` must stay unaware of markdown.

**Exit:** [fixtures](../MDGate/fixtures.md) **export** rows green; WYSIWYG and markdown stay aligned on one client; pane is replaceable (no caret).

**Next:** [M3 — Git snapshotter](../venus-implementation-plan.md#m3--git-snapshotter-week) ([LiveSnapshot](../LiveSnapshot/README.md)). Same `fromDoc` helper on a **pin**. Do not start apply (`ap-*`) until [M6](../venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks).
