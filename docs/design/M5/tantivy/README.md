# Tantivy search

**Status:** optional, not started. Step-by-step: [plan.md](./plan.md). Parent: [M5 plan](../plan.md) board 4. Why this index: [Compared with Elasticsearch](../README.md#compared-with-elasticsearch).

The search-node index is replaced. A shard copy becomes a Tantivy index. The layout still replicates an item (key, tag, statements) and the shard applies that item to Tantivy instead of SurrealDB `SEARCH`.

| Stays | Changes |
| --- | --- |
| Graph process on SurrealDB | Search nodes no longer run SurrealDB |
| Commit set, equal copies, fence, `lsn` | The shard body is a Tantivy update |
| Router: hedge, failover, `min_commit`, hydrate | The shard read is a Tantivy query |
| Whole-term BM25 | Prefix search, fuzzy / edit-distance search, and segment indexing |

Indexing is the Lucene pattern Elasticsearch uses, without the JVM. Documents buffer in memory. A flush writes one immutable segment: terms sorted once, postings written sequentially, deletes as a bitset. Background merges fold small segments into larger ones. Search opens the new segment on commit. That is why a bulk index is fast: the hot path is a sequential write, and the old segment is never rewritten in place.

Tantivy is that same writer (`IndexWriter`, in-memory buffer, commit, `LogMergePolicy`). It can go faster than Elasticsearch here because the durable log already exists on the commit set. The index does not also fsync a translog, parse JSON on a JVM, or store a full `_source`. There is no garbage-collection pause during the flush. Hydrate still reads the graph, so the segment stores postings and ids, not a second copy of the page. A pack of items becomes part of the buffer. A commit publishes a segment. Committing every pack would give up the batching, so the writer flushes on size, not on each `layout.write`.

`mention.norm` exact lookup stays on the graph. This board does not start until [search-scale step 8](../search-scale/plan.md#8-step-query) has a router to re-aim and [extraction](../extraction/plan.md) is writing real heading and mention text.
