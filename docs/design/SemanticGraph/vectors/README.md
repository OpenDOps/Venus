# Vector search

**Status:** not started. Step-by-step: [plan.md](./plan.md). Board 5 of the [semantic graph](../README.md#boards). Milestone: [M6 plan](../../M6/plan.md).

Search finds a heading that means the same thing without sharing a word or a `links_to` edge. The model is BGE-M3, in ONNX, in the graph process. It is the only model on this board. The index is HNSW on the search shard that already holds that heading. The same `hkey` places the vector. The graph that extraction wrote stays the structure: lexical hits and vector hits meet on shared heading ids, then one hop of `links_to`, `contains`, and mentions.

A neighbor is a search hit. It is not a `related` edge. BERT-base is not the encoder. Its `[CLS]` vector was not trained so that cosine distance means “same meaning.” A fine-tuned classifier is not the gate for what gets stored.

## What gets a vector

BGE-M3 does not emit a store-or-drop label. The rule uses the outline, then the distance the model already computes.

1. A heading on the [stop list](../../M6/README.md#2-dictionary--daachorse) (`Status`, `Do not`, `Test scenarios`, and the rest of that list) has no vector.
2. A heading’s own prose is the text before its first child heading. Each child heading is its own vector. The parent vector does not repeat the children.
3. A heading with no own prose has no vector. Its children carry the vectors.
4. Own prose of at most 512 tokens (BGE-M3’s tokenizer) is one vector: the heading `text` plus that prose. An unchanged `body_hash` skips the call.
5. Own prose over 512 tokens and no child heading is split on sidecar paragraph and list blocks. Fences, blank blocks, and link-only blocks are skipped. Each remaining block is embedded and compared with the heading vector. Cosine at or above 0.85 is the same meaning, and that block is not stored. Cosine below 0.85 is a second topic: the vector is stored on the same heading id, with the sidecar `blockId` of that paragraph or list. The 0.85 floor is a constant of this board.
6. A sentence is not a vector. The smallest extra block is the paragraph or list the sidecar already named.

This board starts once [extraction](../extraction/plan.md) writes real heading text and [search-scale step 8](../search-scale/plan.md#8-step-query) can merge a shard read. It does not wait on Tantivy or the graph view. The nearest headings it returns are candidates for [board 6](../../M6/plan.md#from-board-5-to-board-6). That board’s model may write a typed edge. This board does not.
