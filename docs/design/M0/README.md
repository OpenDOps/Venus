# M0 — Empty host

Vite + React host. One BlockSuite workspace, one page, page editor, in-page outline. **No sync.**

This folder is the implementation contract for the M0 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). The wiki plan runner does not exist yet, so the plan lives here (not under `wiki/plans/`).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, steps, DoD, file layout, order of work |
| [api-map.md](./api-map.md) | Design names → installed BlockSuite exports (fill in step-recon) |
| [M0.state.yaml](./M0.state.yaml) | Board: step status only |

**Exit:** `pnpm --filter @venus/web dev` opens a page you can type in; headings appear in the outline; click a heading and the editor scrolls; refresh loses the text (memory-only).

**Next:** [M1 — OctoBase loop](../venus-implementation-plan.md#m1--octobase-loop-week).
