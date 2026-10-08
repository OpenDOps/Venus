# Graph view

**Status:** not started. Step-by-step: [plan.md](./plan.md). Parent: [M5 plan](../plan.md) board 3.

A browser view of the graph board 2 writes. Nodes are pages and headings. Edges are `contains`, `same_page`, in-document `links_to`, and cross-page `links_to`. Mentions stay off the first drawing. They are a zoom-in, not the map.

## Renderer

[Sigma.js](https://www.sigmajs.org/) (`sigma` + `graphology`). It draws nodes and edges with WebGL instancing. Pan and zoom are a camera matrix, so a few thousand headings stay at frame rate. Labels are culled by zoom on a 2D layer. That is the expensive part, and Sigma already drops labels that would not be readable.

| Choice | Why it loses |
| --- | --- |
| A hand-written WebGL renderer | A small gain, and picking, culling, and edge batches are ours to rebuild. |
| PixiJS | The fastest general 2D engine. A graph on top of it is still a graph engine we would be writing. |
| Canvas 2D (Cytoscape, vis) | Fine for a sketch. It drops frames once edges are in the thousands. |
| cosmos.gl | Faster force layout for millions of points. Worse for labeled headings and distinct edge kinds. |

Layout is ForceAtlas2 in a web worker (`graphology-layout-forceatlas2`). The main thread only paints. The view is not a page in the wiki.
