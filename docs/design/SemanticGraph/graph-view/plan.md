# Graph view


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **planId**    | `m6-graph-view`                                                 |
| **Milestone** | [M6](../../M6/plan.md). Board 3 of the [semantic graph](../README.md#boards). |
| **Duration**  | About 2 weeks after extraction writes `links_to`. |
| **Board**     | State file added when this board opens.                        |


The extractor stores pages, headings, and `links_to` inside a document and across documents. This board draws that graph. It does not publish a wiki page and it does not ship chat.

Renderer: [Sigma.js](https://www.sigmajs.org/) on WebGL. See [README](./README.md).

## Where the board is

Not started. Next is [step 1](#1-step-read), after [extraction step 3](../extraction/plan.md#3-step-links).

## Gate

| Steps | When |
| --- | --- |
| **1–4** | When `dependsOn` is done. |


## Exit

1. `venus-graph` returns a bounded subgraph: pages, headings, `contains`, `same_page`, and `links_to`.
2. The browser draws it with Sigma.js. Pan and zoom do not rebuild the scene on the CPU.
3. ForceAtlas2 runs in a worker. The frame loop only uploads positions.
4. A picked node shows its heading or page, its in-document links, and its cross-page links.


## Non-goals

| Later | Why not here |
| --- | --- |
| Every mention as a node | The map is pages and headings. Mentions are a later layer. |
| Semantic edges | [Board 6](../semantic/plan.md). Draw them when they exist, as another edge kind. |
| A published wiki page | The view reads the graph. It is not a `wiki/*.md` page. |
| 3D | WebGL 2D is enough and keeps labels readable. |


## Steps summary

| # | id | Proves |
| --- | --- | --- |
| [1](#1-step-read) | `step-read` | **pending.** A bounded subgraph for one wiki, with in-document and cross-page edges labeled. |
| [2](#2-step-draw) | `step-draw` | **pending.** Sigma.js WebGL view. Pan, zoom, pick. |
| [3](#3-step-layout) | `step-layout` | **pending.** ForceAtlas2 in a worker. Paint stays on the main thread. |
| [4](#4-step-focus) | `step-focus` | **pending.** Pick a heading. See links inside its page and links to other pages. |


<a id="1-step-read"></a>

### 1. step-read

| | |
| --- | --- |
| **n** | 1 |
| **id** | `step-read` |
| **title** | Bounded subgraph |
| **dependsOn** | extraction `step-links` |
| **kind** | implement |
| **status** | **pending** |

#### Work

A read on `venus-graph` returns nodes and edges for the view. Nodes are `page` and `heading`. Edges are `contains`, `same_page`, and `links_to`. Each `links_to` carries whether the target is in the same `docId` or another page.

Default scope is one page plus its neighbors one hop out. A wiki-wide call is capped (node budget, then edge budget) and says when it truncated. Unresolved hrefs are omitted from the drawing. They stay in the store.

#### Do not

- Stream every mention.
- Read SurrealDB from the browser. The browser talks to `venus-graph`.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Two pages | A `links_to` from page A to page B appears once, marked cross-page. |
| In document | A `#` link between two headings of A appears once, marked same page. |
| Cap | A request over the budget returns a truncated flag and no more than the budget. |

- **How:** `cargo test -p venus-graph --test graph_view`.


<a id="2-step-draw"></a>

### 2. step-draw

| | |
| --- | --- |
| **n** | 2 |
| **id** | `step-draw` |
| **title** | Sigma.js WebGL view |
| **dependsOn** | `step-read` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Mount Sigma on a `graphology` graph built from step 1. Nodes and edges render on one WebGL context. In-document `links_to` and cross-page `links_to` are two edge kinds. `contains` and `same_page` are two more.

Pan and zoom update the camera. Picking uses Sigma’s WebGL picking buffer. Labels draw only for nodes that are large enough on screen.

#### Do not

- Draw edges with a canvas 2D path per frame.
- Put the view inside the published wiki.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Draw | Two linked fixture pages show both nodes and one cross-page edge. |
| In document | Two headings in one page show the `#` link between them. |
| Pick | Clicking a node reports that node’s id. |

- **How:** the web package’s graph-view test, plus a check that the canvas is a WebGL context.


<a id="3-step-layout"></a>

### 3. step-layout

| | |
| --- | --- |
| **n** | 3 |
| **id** | `step-layout` |
| **title** | ForceAtlas2 off the main thread |
| **dependsOn** | `step-draw` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Run ForceAtlas2 in a web worker. The worker posts positions. Sigma reads them and paints. Dragging a node pins it. Stopping the layout leaves the last positions.

A page’s headings start near that page so an in-document link is short and a cross-page link is the long edge.

#### Do not

- Run the force loop on the UI thread.
- Recompute layout on every camera move.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Worker | Positions arrive from the worker. The main thread does not call ForceAtlas2. |
| Pin | A dragged node keeps its pin when the next positions arrive. |

- **How:** the web package’s graph-view test.


<a id="4-step-focus"></a>

### 4. step-focus

| | |
| --- | --- |
| **n** | 4 |
| **id** | `step-focus` |
| **title** | Focus one heading |
| **dependsOn** | `step-layout` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Selecting a node loads that page or heading and lists two groups: links whose target has the same `docId`, and links whose target is another page. Choosing a row moves the camera to that node and, if it was outside the loaded hop, asks step 1 for the next hop.

#### Do not

- Draw the whole wiki when the user picked one heading.
- Resolve an unresolved href in the view.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Split | The focus list for a heading shows its in-document target and its cross-page target in different groups. |
| Hop | Choosing a neighbor that was not loaded fetches that hop and draws it. |

- **How:** the web package’s graph-view test.
