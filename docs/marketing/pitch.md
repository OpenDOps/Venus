# Venus pitch

**English** · [Русский](./pitch.ru.md)

![Venus](/docs/img/image.png)

**Venus is spec-driven development** — a way to build software when agents write the code.

Humans keep intention. Agents do the work. The spec is the record. A story is not done until that spec is current — or you skip it because contract did not move.

Venus is the tool for that way of work. It is an **LLM wiki**: one specification that people can write together like Notion, and that agents can clone, grep, and edit as markdown in git. Those are not two products. They are one spec.

## The gap

Nice-looking collaborative WYSIWYG is for people. Agents do not live there. They live in **folders of markdown in a git repo**.

Today that split is a hole:

- Notion and Google Docs: the room can write together. Export is a dump. Agents cannot clone an honest tree. If an agent types the page, the agent becomes the record.
- Confluence: technical design lives there. Heavy. Dump → ChatGPT → Cursor. People who do not use markdown cannot edit the copy.
- Cursor: excellent at **writing** markdown and at **code**. Terrible at **accepting a document**. Preview has no diff. You review commits in source, then switch preview/source. That is a pain.

Venus closes the gap.

## How it works

**Brainstorm.** The team is on a call in a collaborative editor — Notion-like on purpose, so product, marketing, and engineering actually show up. They write the spec together. They can **ask agents to help write it**. Humans still type the room. The agent does not become the page by typing live.

**Snapshot.** Venus continuously projects that live page into **git markdown**. Agents clone the same spec the room just wrote. No dump. No paste.

**Agent edit.** When an agent changes the spec, it edits **markdown**, not the live CRDT. It proposes a **git commit**. A human accepts, adjusts, or rejects. That is Cursor’s loop, aimed at the wiki: checkout, diff, comment, accept — except you review the **rendered** change, not a source-only mark.

**Why on every decision.** Those commits are real git. Each agent proposal carries a **comment: why it took that decision**. You see the diffs in WYSIWYG (After / Before / Diff). You are not switching preview and source to understand meaning.

**Ask with context.** In the background, an LLM indexes the wiki as a **semantic graph with a time axis** — how parts of the spec bind to each other, and how they evolved. Select a span, ask something: the pack includes related pages **and** the history of why this is designed this way, who decided, which commit to revert.

**Plan, DoD, run.** The LLM wiki becomes a **plan**. Definition of done is **tests** — scenarios live in the wiki, not a ticket checklist. The plan runs. Each step is covered by tests. A human can review each step.

**Review against the spec.** An automated reviewer takes the **code graph** and the **wiki graph** and enriches the PR: not only “are there bugs?”, but **does this match the spec**, and should docs ship with it.

**Publish.** After the loop, the adjusted spec is still the wiki. **Hugo** builds **public docs** from accepted git only — not a second site that drifts.

```text
call (WYSIWYG)  →  git markdown snapshots  →  agent proposes commit
                                              →  human accepts meaning (rendered diff + why)
                                              →  semantic graph (related + history)
                                              →  plan + DoD tests in the wiki
                                              →  run; human reviews steps
                                              →  PR review bound to spec
                                              →  Hugo from accepted git
```

## Unique

What Venus *is*. Not a better editor, board, or copilot.

| | |
|---|---|
| **LLM wiki, not a human wiki with AI** | The spec exists so agents can work. WYSIWYG is how humans join and bless meaning. |
| **One spec, two shapes** | Live collab page **and** cloneable git markdown. Same document. Not an export. |
| **Cursor for the wiki** | Agent edits markdown, proposes a commit. You accept on **WYSIWYG diffs**, with a **why**. Cursor preview cannot do that. |
| **Agents cannot publish** | They draft. A human accepts the meaning. The live page stays frozen while they work. |
| **Graph with time** | Related clauses **and** the timeline of decisions — not search, not chat history. |
| **Plan is spec** | DoD is tests in the wiki. The runner is that plan. Humans review steps. |
| **PR knows the spec** | Code graph + wiki graph. Landed work bound to the plan bound to the docs. |
| **Docs are the spec** | Hugo publishes accepted git. Public documentation does not drift from the wiki. |

## Why it stands out

Spec-driven development and “LLM wiki” are claimed elsewhere. The join is not.

| | They | Venus |
|---|---|---|
| **Notion** | The page *is* the product. Agent types it. No cloneable markdown tree. Most people never set up Agent. | Same gestures so the room shows up. Object is git spec. Agent proposes; you accept. |
| **Confluence** | Home of technical design. Dump to markdown. Not a collab call. | Lightweight room + honest files. Design *is* the product. |
| **Cursor** | Won code and writing markdown. Docs preview has **no diff**. Accept lives in source. Chat is why. | Same accept loop on the wiki, in WYSIWYG. Why is on the commit. Cursor stays the IDE if you want it. |
| **GitBook / Tina** | Git markdown is truth. Collab is branches. No Notion-like room. | Room **and** git. People who do not use markdown still write the spec. |
| **Linear / Jira** | Ticket is truth. Spec is a field or a link. Done = card closed. Optional AI you never turn on. | Spec is truth. Board is a mirror. Done = spec current or skip. |
| **CodeSpeak** | Chat becomes requirements on **code**. | The wiki is the spec. Chat packs the graph; it does not replace accept. |
| **PR bots** | Hunt bugs. Do not know the plan or the docs. You had to install them. | Review is in the loop: code graph + wiki graph, spec + docs-to-ship. |

Do not buy Venus for multiplayer typing, a kanban, or a cheaper model. Buy it if agents will implement from a spec, and that spec must stay one object for the room, the git clone, the plan, the PR, and the public docs.

## Read next

| | |
|---|---|
| Design (what the spec *is*) | [venus-design.md](../design/venus-design.md) |
| Unique catalog | [unique-features.md](../product/unique-features.md) |
| Vs tools (full tables) | [comparisons.md](./comparisons.md) |
| Bound chat / why graph | [agentic-comparison.md](./agentic-comparison.md) |
| Order and keep/cut | [product-plan.md](../product/product-plan.md) |
