# Venus

![Venus](docs/img/image.png)

**Venus is your project heart.**

The heart is where feelings and wishes live — where you want to go — and aesthetics, the feeling of beauty. It also pushes blood so the body can work.

Venus is that for a project: it **accumulates** feelings and wishes (aims, intention, the spec humans accept). It **pushes specs like blood** through the full cycle — plan, implement, test, review, iterate — so the rest of the body (agents, PRs, code, reviews, tests, publications) can work. Without that pulse, the cycle is motion without a heart.

**Humans keep heart, instinct, faith and intention, while agents do the work.**

That is the product. Venus is the heart of the project — not the board and not the agent. The wiki, the leases, the plans, the code and Hugo publications are how the pulse is enforced, not what to love.

## Practically

**Venus is a spec-driven development tool**: a collaborative wiki that is also a git markdown tree. The spec is first-class. A story is not done until a human accepts the spec diff.

## Run

Node `>=22`, pnpm `10.19.0`. From the repo root:

```bash
pnpm install
pnpm dev
pnpm test
pnpm build
```

What those do, what Vitest covers, and what is not automated yet: **[docs/runbook.md](docs/runbook.md)**. Test catalog: **[docs/scenarios](docs/scenarios/README.md)**. Compose / later Kubernetes: **[docs/devops](docs/devops/README.md)**.

## Docs

- [Runbook](docs/runbook.md) — install, `dev`, `test`, `build`, Compose
- [DevOps](docs/devops/README.md) — Compose stack; Kubernetes later
- [Design index](docs/design/README.md) — map of the design folder
- [Design](docs/design/venus-design.md) — product and data model (lease, freeze, git snapshots)
- [Product plan](docs/product/product-plan.md) — loop shorter than Notion + a PR
- [Pains](docs/product/pains.md) — gaps Venus closes (agent wiki, SDD vs Scrum)
- [Architecture](docs/design/architecture.md) — dataflow (sync, persist, doc export, markdown later)
- [Scenarios](docs/scenarios/README.md) — implemented tests by feature
- [Comparisons](docs/marketing/comparisons.md) — vs PM tools and CodeSpeak
- [Plans](docs/drafts/pre-design/venus-plan.md) — spec → plan → DoD → implement → human-accepted docs
- [Pitch](docs/marketing/pitch.md) — what Venus is (the project heart)