# Extraction

**Status:** not started. Step-by-step: [plan.md](./plan.md). Parent: [M5 plan](../plan.md) board 2.

Committed pages become one graph. Each page is headings and mentions. `links_to` joins headings inside a page and joins pages to each other. The rules are [extract.md](../extract.md) and [connect.md — direct](../connect.md#direct--no-model). The tables are [store.md](../store.md). The write is one `layout.write` on the cluster from [search-scale](../search-scale/plan.md).

| Pass | Tool | Lands |
| --- | --- | --- |
| Structure | Sidecar blocks | `page`, `heading`, outline edges |
| Links | `venus:doc:` and markdown hrefs | `links_to` inside the file and across pages |
| Glossary | `daachorse` | `mention` kind `term` (and `title` / `heading` by the [M6 dictionary](../../M6/README.md#2-dictionary--daachorse) rules) |
| Symbols | `tree-sitter` on fences | `symbol`, same-page mentions, a narrow cross-page mention |
| Patterns | `regex` | `email`, `phone`, `uuid`, `hash`, `endpoint` |
| NER | `ort`, off unless `graph_meta.ner` | `person`, `org`, `team` mentions |

No network. No model. No second markdown parser. Offsets are UTF-8 bytes inside the sidecar slice.

Folder `contains`, `transcludes`, and semantic edges stay [connect.md](../connect.md). Drawing the graph is [board 3](../graph-view/plan.md).
