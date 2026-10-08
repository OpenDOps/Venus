# Extraction

**Status:** not started. Step-by-step: [plan.md](./plan.md). Board 2 of the [semantic graph](../README.md#boards). Milestone: [M6 plan](../../M6/plan.md).

Committed pages become one graph. Each page is headings and mentions. `links_to` joins headings inside a page and joins pages to each other. The rules are [extract.md](../extract.md) and [connect.md — direct](../connect.md#direct--no-model). The tables are [store.md](../store.md). The write is one `layout.write` on the cluster from [search-scale](../search-scale/plan.md).

| Pass | Tool | Lands |
| --- | --- | --- |
| Structure | Sidecar blocks | `page`, `heading`, outline edges |
| Links | `venus:doc:` and markdown hrefs | `links_to` inside the file and across pages |
| Glossary | `daachorse` | `mention` kind `term` (and `title` / `heading` by the [M6 dictionary](../../M6/README.md#2-dictionary--daachorse) rules) |
| Symbols | `tree-sitter` on fences | `symbol`, same-page mentions, a narrow cross-page mention |
| Patterns | `regex` | `email`, `phone`, `uuid`, `hash`, `endpoint` |
| NER | `ort`, off unless `graph_meta.ner` | `person`, `org`, `team` mentions |

No network on the static passes. No second markdown parser. Offsets are UTF-8 bytes inside the sidecar slice. The optional NER model is the only model, and it stays off.

**Theme tags are future work.** A fine-tuned DistilBERT can multi-label a heading against a fixed theme list (what subject the heading covers). That is a classification job, which is what a BERT-family encoder is good at. BERT-base is unnecessary for it. An off-the-shelf BERT does not know this wiki’s themes, and it cannot invent theme names. The glossary already tags a heading when the theme word is actually written. A classifier would only add a label when the heading is about that theme and never uses the word. The tag would sit on the heading. It would not be an edge. This board does not train or run that classifier.

Folder `contains` and `transcludes` stay [connect.md](../connect.md). Heading vectors are [board 5](../vectors/plan.md). Model edges are [board 6](../semantic/plan.md). Drawing the graph is [board 3](../graph-view/plan.md).
