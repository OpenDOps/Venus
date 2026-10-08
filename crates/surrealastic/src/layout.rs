//! Layout: one commit set, shard sets keyed by an integer, queued apply, refill.
//! No page, doc, or field hash. The caller supplies `key`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use serde_json::{json, Value};
use tokio::task::{JoinHandle, JoinSet};

use crate::body::Body;
use crate::cluster::Cluster;
use crate::guard::Entry;
use crate::hooks::Owner;
use crate::store::{self, LayoutRow};
use crate::writer::Writer;

/// One record the caller wants on the shards that `key` lands on.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub key: i64,
    pub item: String,
    pub tag: String,
    pub rids: Vec<String>,
    pub body: Body,
    pub delete: bool,
}

/// A shard a read skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialShard {
    pub shard: i32,
    pub reason: String,
}

/// Rows from the shards that were caught up, and the shards that were not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOut {
    pub rows: String,
    pub partial: Vec<PartialShard>,
}

/// Commit head minus the shard cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardLag {
    pub shard: i32,
    pub behind: i64,
}

struct Pending {
    commit: i64,
    items: Vec<Item>,
}

struct Inner {
    cluster: Cluster,
    writer: Writer,
    db: tokio::sync::Mutex<LayoutRow>,
    owner: Option<Arc<dyn Owner>>,
    queue: tokio::sync::Mutex<BTreeMap<i32, Vec<Pending>>>,
    apply: tokio::sync::Mutex<()>,
    /// Last TCP probe. The write path trusts this and does not dial.
    probes: tokio::sync::Mutex<HashMap<String, bool>>,
}

pub struct Layout {
    inner: Arc<Inner>,
    stop: Arc<AtomicBool>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Layout {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

/// `key % shard_count`, using a non-negative remainder.
pub fn shard_for(key: i64, shard_count: i32) -> i32 {
    key.rem_euclid(shard_count as i64) as i32
}

/// Current shard, and the next layout's shard when that modulus differs.
pub fn shards_for(key: i64, shard_count: i32, shard_count_next: Option<i32>) -> Vec<i32> {
    let mut out = vec![shard_for(key, shard_count)];
    if let Some(next) = shard_count_next {
        let extra = shard_for(key, next);
        if extra != out[0] {
            out.push(extra);
        }
    }
    out
}

pub fn shard_set_id(db_id: &str, epoch: i32, shard: i32) -> String {
    format!("{db_id}:{epoch}:{shard}")
}

/// First commit lsn a refill may read from the log. Never `L0` or below.
pub fn log_tail_start(l0: i64) -> i64 {
    l0 + 1
}

/// Split items into entries of at most `max_docs` and `max_bytes`.
pub fn pack_items(items: &[Item], max_docs: u32, max_bytes: u32) -> Vec<Vec<Item>> {
    let mut packs = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0_u32;
    for item in items {
        let size = item_bytes(item);
        let full = !current.is_empty()
            && (current.len() as u32 >= max_docs || bytes.saturating_add(size) > max_bytes);
        if full {
            packs.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes = bytes.saturating_add(size);
        current.push(item.clone());
    }
    if !current.is_empty() {
        packs.push(current);
    }
    packs
}

fn item_bytes(item: &Item) -> u32 {
    let n = if item.delete {
        item.rids.iter().map(|rid| rid.len() + 8).sum()
    } else {
        item.body.logged().to_string().len()
    };
    n as u32
}

const ITEM_DDL: &str = r#"
DEFINE TABLE IF NOT EXISTS _layout_item SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS item ON _layout_item TYPE string;
DEFINE FIELD IF NOT EXISTS tag ON _layout_item TYPE string;
DEFINE FIELD IF NOT EXISTS rids ON _layout_item TYPE array<string>;
DEFINE FIELD IF NOT EXISTS body ON _layout_item FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS commit ON _layout_item TYPE int;
DEFINE TABLE IF NOT EXISTS _layout_commit SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS items ON _layout_commit FLEXIBLE TYPE array;
"#;

const CURSOR_DDL: &str = r#"
DEFINE TABLE IF NOT EXISTS _layout SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS commit ON _layout TYPE int;
"#;

impl Layout {
    /// Claim the commit set, attach its shard sets, and apply each copy's schema.
    pub async fn open(
        cluster: Cluster,
        db_id: &str,
        holder: &str,
        owner: Option<Arc<dyn Owner>>,
    ) -> anyhow::Result<Self> {
        let row = store::load_layout(cluster.pool(), db_id).await?;
        ensure_schema(&cluster, &row, owner.as_ref()).await?;
        let writer = cluster
            .claim(&row.commit_set, holder)
            .await?
            .context("commit set lease is held")?;
        for shard in 0..span(&row) {
            writer
                .attach(&shard_set_id(&row.db_id, row.epoch, shard))
                .await?;
        }
        let inner = Arc::new(Inner {
            cluster,
            writer,
            db: tokio::sync::Mutex::new(row),
            owner,
            queue: tokio::sync::Mutex::new(BTreeMap::new()),
            apply: tokio::sync::Mutex::new(()),
            probes: tokio::sync::Mutex::new(HashMap::new()),
        });
        rebuild_queue(&inner).await?;
        let stop = Arc::new(AtomicBool::new(false));
        let pump = {
            let inner = Arc::clone(&inner);
            let stop = Arc::clone(&stop);
            tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let shards: Vec<i32> = inner.queue.lock().await.keys().copied().collect();
                    for shard in shards {
                        if inner.apply_queued(shard).await.is_err() {
                            continue;
                        }
                    }
                }
            })
        };
        let probe = {
            let inner = Arc::clone(&inner);
            let stop = Arc::clone(&stop);
            tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    tokio::time::sleep(inner.cluster.config().probe).await;
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    inner.probe_all().await;
                }
            })
        };
        Ok(Self {
            inner,
            stop,
            tasks: vec![pump, probe],
        })
    }

    /// Commit, then apply. The number returned is the last commit lsn.
    /// A job that fits in one entry stays one entry. A larger job is several
    /// entries, each within the entry limits, and the shards apply once.
    pub async fn write(&self, commit_body: Body, items: Vec<Item>) -> anyhow::Result<i64> {
        let docs = self.inner.cluster.config().entry_docs;
        let bytes = self.inner.cluster.config().entry_bytes;
        let chunks = commit_chunks(commit_body, &items, docs, bytes)?;
        let tag = entry_tag(&items);
        let commit_set = self.db().await.commit_set;
        let mut last = 0_i64;
        for (body, _) in chunks {
            last = match self.inner.writer.write(&commit_set, body, &tag).await {
                Ok(lsn) => lsn,
                Err(err) => return Err(commit_error(err)),
            };
        }
        let inner = Arc::clone(&self.inner);
        let _guard = inner.apply.lock().await;
        inner.fan_out(last, &items).await;
        Ok(last)
    }

    pub async fn read(&self, query: &str, min_commit: Option<i64>) -> anyhow::Result<ReadOut> {
        accept_read(query)?;
        let db = self.db().await;
        let mut rows = String::new();
        let mut partial = Vec::new();
        for shard in 0..db.shard_count {
            match self.inner.read_shard(shard, query, min_commit).await {
                Ok(text) => {
                    rows.push_str(&text);
                    rows.push('\n');
                }
                Err(reason) => partial.push(PartialShard { shard, reason }),
            }
        }
        Ok(ReadOut { rows, partial })
    }

    pub async fn lag(&self) -> anyhow::Result<Vec<ShardLag>> {
        let db = self.db().await;
        let head = self.inner.writer.head(&db.commit_set).await;
        let mut out = Vec::new();
        for shard in 0..span(&db) {
            let cursor = self.inner.cursor(shard).await.unwrap_or(0);
            out.push(ShardLag {
                shard,
                behind: head.saturating_sub(cursor),
            });
        }
        Ok(out)
    }

    /// One guarded entry of `DEFINE` / `REMOVE` statements on `set_id`.
    pub async fn define_on(&self, set_id: &str, statements: &[String]) -> anyhow::Result<i64> {
        let mut body = Body::new();
        for statement in statements {
            body = body.define(statement)?;
        }
        let _guard = self.inner.apply.lock().await;
        self.inner.ready(set_id).await?;
        self.inner.writer.write(set_id, body, "define").await
    }

    /// Replay this process's entries onto copies that missed them.
    /// A copy stays `lagging` until replay reaches the head.
    pub async fn catch_up(&self, set_id: &str) -> anyhow::Result<()> {
        self.inner.note_health(set_id, true).await;
        Ok(())
    }

    /// Fill an empty shard from the commit set's item table. A sibling shard is not a source.
    pub async fn refill(&self, shard: i32) -> anyhow::Result<()> {
        self.inner.refill(shard).await
    }

    async fn db(&self) -> LayoutRow {
        self.inner.db.lock().await.clone()
    }
}

impl Inner {
    async fn fan_out(self: &Arc<Self>, commit: i64, items: &[Item]) {
        let db = self.db.lock().await.clone();
        let grouped = group_items(items, db.shard_count, db.shard_count_next);
        let mut jobs = Vec::new();
        for shard in 0..span(&db) {
            let set_id = shard_set_id(&db.db_id, db.epoch, shard);
            self.note_health(&set_id, false).await;
            let group = grouped.get(&shard).cloned().unwrap_or_default();
            jobs.push((shard, group));
        }
        let mut set = JoinSet::new();
        for (shard, group) in jobs {
            let inner = Arc::clone(self);
            set.spawn(async move {
                let result = inner.apply_group(shard, commit, &group).await;
                (shard, group, result)
            });
        }
        while let Some(joined) = set.join_next().await {
            let Ok((shard, group, result)) = joined else {
                continue;
            };
            if result.is_err() {
                self.queue
                    .lock()
                    .await
                    .entry(shard)
                    .or_default()
                    .push(Pending {
                        commit,
                        items: group,
                    });
            }
        }
    }

    async fn apply_queued(&self, shard: i32) -> anyhow::Result<()> {
        let pending = {
            let queue = self.queue.lock().await;
            queue.get(&shard).and_then(|v| v.first()).map(|p| Pending {
                commit: p.commit,
                items: p.items.clone(),
            })
        };
        let Some(pending) = pending else {
            return Ok(());
        };
        let _guard = self.apply.lock().await;
        // The write path may have applied it while we waited.
        let still = self
            .queue
            .lock()
            .await
            .get(&shard)
            .and_then(|v| v.first())
            .map(|p| p.commit);
        if still != Some(pending.commit) {
            return Ok(());
        }
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        self.note_health(&set_id, false).await;
        if self
            .apply_group(shard, pending.commit, &pending.items)
            .await
            .is_err()
        {
            return Ok(());
        }
        let mut queue = self.queue.lock().await;
        if let Some(list) = queue.get_mut(&shard) {
            if list.first().map(|p| p.commit) == Some(pending.commit) {
                list.remove(0);
            }
            if list.is_empty() {
                queue.remove(&shard);
            }
        }
        Ok(())
    }

    async fn apply_group(&self, shard: i32, commit: i64, items: &[Item]) -> anyhow::Result<()> {
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        if !self.writer.has_in_sync(&set_id).await {
            anyhow::bail!("no in-sync copy");
        }
        let docs = self.cluster.config().entry_docs;
        let bytes = self.cluster.config().entry_bytes;
        let packed = pack_items(items, docs, bytes);
        let packs = if packed.is_empty() {
            vec![Vec::new()]
        } else {
            packed
        };
        for (index, pack) in packs.iter().enumerate() {
            let last = index + 1 == packs.len();
            let body = shard_body(pack, commit, last)?;
            let tag = entry_tag(pack);
            self.writer.write(&set_id, body, &tag).await?;
        }
        Ok(())
    }

    async fn ready(&self, set_id: &str) -> anyhow::Result<bool> {
        self.note_health(set_id, false).await;
        Ok(self.writer.has_in_sync(set_id).await)
    }

    /// A healthy socket stays up. TCP runs only when `probe` is set: the timer
    /// and an explicit catch-up. A write uses the last answer and does not dial.
    async fn note_health(&self, set_id: &str, probe: bool) {
        let Ok(copies) = self.cluster.copies(set_id).await else {
            return;
        };
        let head = self.writer.head(set_id).await;
        for copy in copies {
            let known = self.probes.lock().await.get(&copy.node_id).copied();
            if probe {
                let ready = node_ready(&copy.url).await;
                self.probes.lock().await.insert(copy.node_id.clone(), ready);
                if !ready {
                    self.mark_down(set_id, &copy.node_id).await;
                    continue;
                }
            } else if known == Some(false) {
                self.mark_down(set_id, &copy.node_id).await;
                continue;
            }
            let state = self.writer.copy_state(set_id, &copy.node_id).await;
            let applied_mem = self
                .writer
                .copy_applied(set_id, &copy.node_id)
                .await
                .unwrap_or(0);
            if state.as_deref() == Some("in_sync") && applied_mem >= head {
                continue;
            }
            // A lagging copy waits for the probe. The write path does not dial it.
            if !probe && state.as_deref() != Some("in_sync") {
                continue;
            }
            let node = copy.node_id.clone();
            let url = copy.url.clone();
            let recovered = tokio::time::timeout(
                Duration::from_secs(8),
                self.recover_copy(set_id, &node, &url, head),
            )
            .await;
            if !matches!(recovered, Ok(Ok(()))) {
                self.cluster.disconnect(&node).await;
                self.probes.lock().await.insert(node, false);
            }
        }
    }

    /// Dial once, replay to the head, and mark the copy in sync when it arrives.
    async fn recover_copy(
        &self,
        set_id: &str,
        node: &str,
        url: &str,
        head: i64,
    ) -> anyhow::Result<()> {
        self.cluster.disconnect(node).await;
        let set = store::load_set(self.cluster.pool(), set_id).await?;
        let conn = self.cluster.conn(node, url).await?;
        let applied = crate::link::state_of(&conn, &set.namespace, &set.database)
            .await
            .map(|state| state.0)
            .unwrap_or(0);
        if applied < head {
            let reached = self.replay_copy(set_id, node, url).await?;
            if reached < head {
                anyhow::bail!("still behind");
            }
        }
        self.writer.revive(set_id, node).await?;
        Ok(())
    }

    async fn mark_down(&self, set_id: &str, node: &str) {
        self.cluster.disconnect(node).await;
        if self.writer.copy_state(set_id, node).await.as_deref() != Some("lagging") {
            let _ = self.writer.force_lagging(set_id, node).await;
        }
    }

    async fn probe_all(&self) {
        let db = self.db.lock().await.clone();
        self.note_health(&db.commit_set, true).await;
        for shard in 0..span(&db) {
            let set_id = shard_set_id(&db.db_id, db.epoch, shard);
            self.note_health(&set_id, true).await;
        }
    }

    /// Replay until the copy reaches the head. Missing entries come from an
    /// in-sync copy's `_repl_log` when this process no longer holds them.
    async fn replay_copy(&self, set_id: &str, node: &str, url: &str) -> anyhow::Result<i64> {
        let set = store::load_set(self.cluster.pool(), set_id).await?;
        let conn = match self.cluster.conn(node, url).await {
            Ok(conn) => conn,
            Err(err) => {
                self.cluster.disconnect(node).await;
                return Err(err);
            }
        };
        let (mut applied, prev_fence) =
            match crate::link::state_of(&conn, &set.namespace, &set.database).await {
                Ok(state) => state,
                Err(err) => {
                    self.cluster.disconnect(node).await;
                    return Err(err);
                }
            };
        let head = self.writer.head(set_id).await;
        let mut entries = self.writer.remembered(set_id).await;
        let covered = entries
            .iter()
            .map(|entry| entry.lsn)
            .max()
            .unwrap_or(applied);
        if covered < head {
            if let Ok(more) = self
                .fetch_log(set_id, node, applied + 1, head, prev_fence)
                .await
            {
                for entry in more {
                    if !entries.iter().any(|have| have.lsn == entry.lsn) {
                        entries.push(entry);
                    }
                }
                entries.sort_by_key(|entry| entry.lsn);
            }
        }
        for entry in &entries {
            if entry.lsn <= applied {
                continue;
            }
            match crate::link::apply(&conn, &set.namespace, &set.database, entry).await {
                Ok(reply)
                    if matches!(
                        reply.status,
                        crate::guard::ApplyStatus::Ok | crate::guard::ApplyStatus::Already
                    ) =>
                {
                    applied = entry.lsn;
                }
                Ok(_) => break,
                Err(_) => {
                    self.cluster.disconnect(node).await;
                    anyhow::bail!("replay failed");
                }
            }
        }
        self.writer.note_caught_up(set_id, node, applied).await;
        self.writer.trim_remembered(set_id).await;
        Ok(applied)
    }

    /// Read `_repl_log` from another in-sync copy of this same set.
    async fn fetch_log(
        &self,
        set_id: &str,
        skip: &str,
        from: i64,
        to: i64,
        mut prev_fence: i64,
    ) -> anyhow::Result<Vec<Entry>> {
        if from > to {
            return Ok(Vec::new());
        }
        let copies = self.cluster.copies(set_id).await?;
        let Some(source) = copies
            .iter()
            .find(|copy| copy.node_id != skip && copy.state == "in_sync")
        else {
            return Ok(Vec::new());
        };
        let set = store::load_set(self.cluster.pool(), set_id).await?;
        let conn = self.cluster.conn(&source.node_id, &source.url).await?;
        let batch = self.cluster.config().catchup_batch.max(1) as i64;
        let mut out = Vec::new();
        let mut next = from;
        while next <= to {
            let end = (next + batch - 1).min(to);
            let sql = format!("SELECT * FROM _repl_log:{next}..={end};");
            let value = crate::link::query_json(&conn, &set.namespace, &set.database, &sql).await?;
            let rows = entries_from_log(&value, prev_fence);
            let Some(last) = rows.last() else {
                break;
            };
            if last.lsn < next {
                break;
            }
            prev_fence = last.fence;
            next = last.lsn + 1;
            out.extend(rows);
        }
        Ok(out)
    }

    async fn read_shard(
        &self,
        shard: i32,
        query: &str,
        min_commit: Option<i64>,
    ) -> Result<String, String> {
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        let copies = self
            .cluster
            .copies(&set_id)
            .await
            .map_err(|err| err.to_string())?;
        let set = store::load_set(self.cluster.pool(), &set_id)
            .await
            .map_err(|err| err.to_string())?;
        let mut why = "behind".to_string();
        for copy in copies {
            if !node_ready(&copy.url).await {
                continue;
            }
            let script = match min_commit {
                Some(min) => format!(
                    "LET $n = (SELECT VALUE commit FROM ONLY _layout:cursor) ?? 0; IF type::int($n) < {min} {{ THROW \"behind\"; }}; {query}"
                ),
                None => query.to_string(),
            };
            for attempt in 0..2 {
                let conn = match self.cluster.conn(&copy.node_id, &copy.url).await {
                    Ok(conn) => conn,
                    Err(_) => break,
                };
                match crate::link::query_json(&conn, &set.namespace, &set.database, &script).await {
                    Ok(value) => {
                        let text = match value {
                            Value::String(text) => text,
                            other => other.to_string(),
                        };
                        return Ok(text);
                    }
                    Err(err) if err.to_string().contains("behind") => {
                        why = "behind".to_string();
                        break;
                    }
                    Err(_) if attempt == 0 => {
                        self.cluster.disconnect(&copy.node_id).await;
                    }
                    Err(err) => {
                        why = err.to_string();
                        break;
                    }
                }
            }
        }
        Err(why)
    }

    async fn cursor(&self, shard: i32) -> anyhow::Result<i64> {
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        let copies = self.cluster.copies(&set_id).await?;
        let set = store::load_set(self.cluster.pool(), &set_id).await?;
        let mut best = 0_i64;
        for copy in copies {
            if !node_ready(&copy.url).await {
                continue;
            }
            for attempt in 0..2 {
                let Ok(conn) = self.cluster.conn(&copy.node_id, &copy.url).await else {
                    break;
                };
                match crate::link::query_json(
                    &conn,
                    &set.namespace,
                    &set.database,
                    "SELECT commit FROM _layout:cursor;",
                )
                .await
                {
                    Ok(value) => {
                        if let Some(n) = json_i64(&value, "commit") {
                            best = best.max(n);
                        }
                        break;
                    }
                    Err(_) if attempt == 0 => self.cluster.disconnect(&copy.node_id).await,
                    Err(_) => break,
                }
            }
        }
        Ok(best)
    }

    async fn refill(&self, shard: i32) -> anyhow::Result<()> {
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        let commit = store::load_set(self.cluster.pool(), &db.commit_set).await?;
        let l0 = commit_head(&self.cluster, &commit).await?;
        // The tail, when there is one, starts at L0 + 1. The item table is the source.
        let _tail = log_tail_start(l0);
        ensure_one(&self.cluster, &set_id, false, self.owner.as_ref()).await?;
        refuse_filled(&self.cluster, &set_id).await?;
        self.writer.reset_progress(&set_id).await;
        for copy in self.cluster.copies(&set_id).await? {
            self.writer.revive(&set_id, &copy.node_id).await?;
        }
        let stored = read_item_table(&self.cluster, &commit).await?;
        let mut items = Vec::new();
        for row in stored {
            if shards_for(row.key, db.shard_count, db.shard_count_next).contains(&shard) {
                items.push(row.into_item());
            }
        }
        let docs = self.cluster.config().entry_docs;
        let bytes = self.cluster.config().entry_bytes;
        let packs = if items.is_empty() {
            vec![Vec::new()]
        } else {
            pack_items(&items, docs, bytes)
        };
        for (index, pack) in packs.iter().enumerate() {
            let last = index + 1 == packs.len();
            let body = shard_body(pack, l0, last)?;
            self.writer.write(&set_id, body, "refill").await?;
        }
        let head = self.writer.head(&db.commit_set).await;
        if head > l0 {
            let later = read_commits(&self.cluster, &commit, log_tail_start(l0), head).await?;
            for (commit_lsn, committed) in later {
                let group: Vec<Item> = committed
                    .into_iter()
                    .filter(|item| {
                        shards_for(item.key, db.shard_count, db.shard_count_next).contains(&shard)
                    })
                    .collect();
                self.apply_group(shard, commit_lsn, &group).await?;
            }
        }
        Ok(())
    }
}

struct StoredItem {
    key: i64,
    item: String,
    tag: String,
    rids: Vec<String>,
    body: Body,
    #[allow(dead_code)]
    commit: i64,
}

impl StoredItem {
    fn into_item(self) -> Item {
        Item {
            key: self.key,
            item: self.item,
            tag: self.tag,
            rids: self.rids,
            body: self.body,
            delete: false,
        }
    }
}

fn group_items(items: &[Item], count: i32, next: Option<i32>) -> BTreeMap<i32, Vec<Item>> {
    let mut map: BTreeMap<i32, Vec<Item>> = BTreeMap::new();
    for item in items {
        for shard in shards_for(item.key, count, next) {
            map.entry(shard).or_default().push(item.clone());
        }
    }
    map
}

fn push_manifest(mut body: Body, items: &[Item]) -> anyhow::Result<Body> {
    let listed: Vec<Value> = items
        .iter()
        .map(|item| {
            json!({
                "key": item.key,
                "item": item.item,
                "tag": item.tag,
                "rids": item.rids,
                "body": item.body.logged(),
                "delete": item.delete,
            })
        })
        .collect();
    let param = body.bind_value(Value::Array(listed));
    body.statement(&format!(
        "UPSERT type::thing('_layout_commit', $lsn) CONTENT {{ items: ${param} }};"
    ))
}

fn push_item(body: Body, item: &Item) -> anyhow::Result<Body> {
    if item.delete {
        return body.statement(&format!("DELETE _layout_item:{};", item.key));
    }
    let mut body = body;
    let p_item = body.bind_value(json!(item.item));
    let p_tag = body.bind_value(json!(item.tag));
    let p_rids = body.bind_value(json!(item.rids));
    let p_body = body.bind_value(item.body.logged());
    let key = item.key;
    body.statement(&format!(
        "LET $rows = (SELECT item FROM _layout_item:{key}); LET $have = $rows[0].item; IF $have != NONE AND $have != ${p_item} {{ THROW \"key collision\"; }}; UPSERT _layout_item:{key} CONTENT {{ item: ${p_item}, tag: ${p_tag}, rids: ${p_rids}, body: ${p_body}, commit: $lsn }};"
    ))
}

/// One chunk when the job fits. Otherwise each piece stays within the entry limits.
/// A single statement or item that is already over the byte cap is emitted alone.
fn commit_chunks(
    commit_body: Body,
    items: &[Item],
    max_docs: u32,
    max_bytes: u32,
) -> anyhow::Result<Vec<(Body, Vec<Item>)>> {
    let max_docs = max_docs.max(1);
    let max_bytes = max_bytes.max(1);
    if (items.len() as u32) <= max_docs {
        let whole = finish_chunk(commit_body.clone(), items)?;
        if whole.byte_len() <= max_bytes {
            return Ok(vec![(whole, items.to_vec())]);
        }
    }
    let mut chunks = Vec::new();
    let mut prefix = commit_body.split_bytes(max_bytes);
    let mut carry = prefix.pop().unwrap_or_else(Body::new);
    for piece in prefix {
        chunks.push((push_manifest(piece, &[])?, Vec::new()));
    }
    let mut batch = Vec::new();
    for item in items {
        let mut trial = batch.clone();
        trial.push(item.clone());
        if !batch.is_empty() && !chunk_fits(&carry, &trial, max_docs, max_bytes)? {
            chunks.push((finish_chunk(std::mem::take(&mut carry), &batch)?, batch));
            batch = Vec::new();
        }
        if batch.is_empty() && !chunk_fits(&carry, std::slice::from_ref(item), max_docs, max_bytes)?
        {
            if !carry.is_empty() {
                chunks.push((push_manifest(std::mem::take(&mut carry), &[])?, Vec::new()));
            }
            chunks.push((
                finish_chunk(Body::new(), std::slice::from_ref(item))?,
                vec![item.clone()],
            ));
            continue;
        }
        batch.push(item.clone());
    }
    if !batch.is_empty() || chunks.is_empty() {
        chunks.push((finish_chunk(carry, &batch)?, batch));
    } else if !carry.is_empty() {
        chunks.push((push_manifest(carry, &[])?, Vec::new()));
    }
    Ok(chunks)
}

fn chunk_fits(
    prefix: &Body,
    items: &[Item],
    max_docs: u32,
    max_bytes: u32,
) -> anyhow::Result<bool> {
    if items.len() as u32 > max_docs {
        return Ok(false);
    }
    Ok(finish_chunk(prefix.clone(), items)?.byte_len() <= max_bytes)
}

fn finish_chunk(mut body: Body, items: &[Item]) -> anyhow::Result<Body> {
    for item in items {
        body = push_item(body, item)?;
    }
    push_manifest(body, items)
}

fn entries_from_log(value: &Value, prev_fence: i64) -> Vec<Entry> {
    let rows = match value {
        Value::Array(rows) => rows.clone(),
        Value::Object(_) => vec![value.clone()],
        _ => return Vec::new(),
    };
    let mut parsed = Vec::new();
    for row in rows {
        let Some(lsn) = record_key(row.get("id")) else {
            continue;
        };
        let fence = json_i64(&row, "fence").unwrap_or(0);
        let tag = row
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or("layout")
            .to_string();
        let at = match row.get("at") {
            Some(Value::String(text)) => text.clone(),
            Some(other) => other.to_string().trim_matches('"').to_string(),
            None => String::new(),
        };
        let body = row
            .get("body")
            .and_then(|value| Body::from_logged(value).ok())
            .unwrap_or_default();
        parsed.push(Entry {
            lsn,
            prev: lsn.saturating_sub(1),
            prev_fence: 0,
            fence,
            tag,
            at,
            body,
        });
    }
    parsed.sort_by_key(|entry| entry.lsn);
    let mut fence_before = prev_fence;
    for entry in &mut parsed {
        entry.prev = entry.lsn.saturating_sub(1);
        entry.prev_fence = fence_before;
        fence_before = entry.fence;
    }
    parsed
}

fn shard_body(items: &[Item], commit: i64, cursor: bool) -> anyhow::Result<Body> {
    let mut body = Body::new();
    for item in items {
        if item.delete {
            for rid in &item.rids {
                body = body.delete(rid);
            }
        } else {
            body = body.append(&item.body);
        }
    }
    if cursor {
        body = body.upsert("_layout:cursor", json!({ "commit": commit }));
    }
    Ok(body)
}

fn commit_error(err: anyhow::Error) -> anyhow::Error {
    if err
        .to_string()
        .to_ascii_lowercase()
        .contains("key collision")
    {
        anyhow::anyhow!("key collision")
    } else {
        err
    }
}

fn accept_read(query: &str) -> anyhow::Result<()> {
    let sql = query.trim().trim_end_matches(';').trim();
    if sql.is_empty() || sql.contains(';') {
        anyhow::bail!("read is one select");
    }
    let lower = sql.to_ascii_lowercase();
    let Some(after) = lower.strip_prefix("select") else {
        anyhow::bail!("read is one select");
    };
    if after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
        anyhow::bail!("read is one select");
    }
    Ok(())
}

/// One shared item tag is the log entry tag. A mixed or empty pack stays `layout`.
fn entry_tag(items: &[Item]) -> String {
    let Some(first) = items.first() else {
        return "layout".to_string();
    };
    if first.tag.is_empty() || items.iter().any(|item| item.tag != first.tag) {
        return "layout".to_string();
    }
    first.tag.clone()
}

fn span(row: &LayoutRow) -> i32 {
    row.shard_count.max(row.shard_count_next.unwrap_or(0))
}

async fn ensure_schema(
    cluster: &Cluster,
    row: &LayoutRow,
    owner: Option<&Arc<dyn Owner>>,
) -> anyhow::Result<()> {
    ensure_one(cluster, &row.commit_set, true, owner).await?;
    for shard in 0..span(row) {
        ensure_one(
            cluster,
            &shard_set_id(&row.db_id, row.epoch, shard),
            false,
            owner,
        )
        .await?;
    }
    Ok(())
}

async fn ensure_one(
    cluster: &Cluster,
    set_id: &str,
    commit_set: bool,
    owner: Option<&Arc<dyn Owner>>,
) -> anyhow::Result<()> {
    cluster.ensure(set_id).await?;
    let set = store::load_set(cluster.pool(), set_id).await?;
    let copies = cluster.copies(set_id).await?;
    let mut extra = if commit_set { ITEM_DDL } else { CURSOR_DDL }.to_string();
    if let Some(owner) = owner {
        for statement in owner.schema(set_id).await? {
            extra.push_str(&statement);
            if !statement.trim_end().ends_with(';') {
                extra.push(';');
            }
            extra.push('\n');
        }
    }
    for copy in copies {
        let conn = cluster.conn(&copy.node_id, &copy.url).await?;
        crate::link::raw(&conn, &set.namespace, &set.database, &extra)
            .await
            .with_context(|| format!("schema {set_id}"))?;
    }
    Ok(())
}

async fn rebuild_queue(inner: &Inner) -> anyhow::Result<()> {
    let db = inner.db.lock().await.clone();
    let head = inner.writer.head(&db.commit_set).await;
    if head == 0 {
        return Ok(());
    }
    let commit = store::load_set(inner.cluster.pool(), &db.commit_set).await?;
    for shard in 0..span(&db) {
        let cursor = inner.cursor(shard).await.unwrap_or(0);
        if cursor >= head {
            continue;
        }
        let commits = read_commits(&inner.cluster, &commit, cursor + 1, head).await?;
        let mut queue = inner.queue.lock().await;
        for (commit_lsn, items) in commits {
            let mine: Vec<Item> = items
                .into_iter()
                .filter(|item| {
                    shards_for(item.key, db.shard_count, db.shard_count_next).contains(&shard)
                })
                .collect();
            queue.entry(shard).or_default().push(Pending {
                commit: commit_lsn,
                items: mine,
            });
        }
    }
    Ok(())
}

async fn commit_head(cluster: &Cluster, set: &crate::store::SetRow) -> anyhow::Result<i64> {
    let copies = cluster.copies(&set.set_id).await?;
    let mut best = 0_i64;
    for copy in copies {
        if !node_ready(&copy.url).await {
            continue;
        }
        let conn = cluster.conn(&copy.node_id, &copy.url).await?;
        let (applied, _) = crate::link::state_of(&conn, &set.namespace, &set.database).await?;
        if applied > best {
            best = applied;
        }
    }
    Ok(best)
}

async fn refuse_filled(cluster: &Cluster, set_id: &str) -> anyhow::Result<()> {
    let set = store::load_set(cluster.pool(), set_id).await?;
    for copy in cluster.copies(set_id).await? {
        let conn = cluster.conn(&copy.node_id, &copy.url).await?;
        let (applied, _) = crate::link::state_of(&conn, &set.namespace, &set.database)
            .await
            .unwrap_or((0, 0));
        if applied > 0 {
            anyhow::bail!("refill refuses a database that still has rows");
        }
        let value = crate::link::query_json(
            &conn,
            &set.namespace,
            &set.database,
            "SELECT * FROM _layout:cursor;",
        )
        .await
        .unwrap_or(Value::Null);
        if json_present(&value) {
            anyhow::bail!("refill refuses a database that still has rows");
        }
    }
    Ok(())
}

async fn read_commits(
    cluster: &Cluster,
    set: &crate::store::SetRow,
    from: i64,
    to: i64,
) -> anyhow::Result<BTreeMap<i64, Vec<Item>>> {
    let mut out = BTreeMap::new();
    if from > to {
        return Ok(out);
    }
    let batch = cluster.config().catchup_batch.max(1);
    let copies = cluster.copies(&set.set_id).await?;
    let copy = copies
        .into_iter()
        .find(|copy| copy.state == "in_sync")
        .context("commit set has no in-sync copy")?;
    let conn = cluster.conn(&copy.node_id, &copy.url).await?;
    let mut offset = 0_u32;
    loop {
        let sql = format!(
            "SELECT items, <string> id AS id FROM _layout_commit ORDER BY id LIMIT {batch} START {offset};"
        );
        let value = crate::link::query_json(&conn, &set.namespace, &set.database, &sql).await?;
        let rows = match value {
            Value::Array(rows) => rows,
            Value::Null => break,
            Value::String(_) => break,
            other => vec![other],
        };
        if rows.is_empty() {
            break;
        }
        let count = rows.len();
        for row in rows {
            let lsn = record_key(row.get("id")).unwrap_or(0);
            if lsn < from || lsn > to {
                continue;
            }
            out.insert(lsn, items_from(row.get("items")));
        }
        if (count as u32) < batch {
            break;
        }
        offset += count as u32;
    }
    Ok(out)
}

fn items_from(value: Option<&Value>) -> Vec<Item> {
    let Some(Value::Array(rows)) = value else {
        return Vec::new();
    };
    rows.iter().filter_map(|row| item_from(row).ok()).collect()
}

fn item_from(row: &Value) -> anyhow::Result<Item> {
    let body = match row.get("body") {
        Some(value) if !value.is_null() => Body::from_logged(value)?,
        _ => Body::new(),
    };
    let rids = row
        .get("rids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok(Item {
        key: json_i64(row, "key").context("key")?,
        item: row
            .get("item")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tag: row
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        rids,
        body,
        delete: row.get("delete").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

async fn read_item_table(
    cluster: &Cluster,
    set: &crate::store::SetRow,
) -> anyhow::Result<Vec<StoredItem>> {
    let batch = cluster.config().catchup_batch.max(1);
    let copies = cluster.copies(&set.set_id).await?;
    let copy = copies
        .into_iter()
        .find(|copy| copy.state == "in_sync")
        .context("commit set has no in-sync copy")?;
    let conn = cluster.conn(&copy.node_id, &copy.url).await?;
    let mut offset = 0_u32;
    let mut out = Vec::new();
    loop {
        let sql = format!(
            "SELECT item, tag, rids, body, commit, <string> id AS id FROM _layout_item ORDER BY id LIMIT {batch} START {offset};"
        );
        let value = crate::link::query_json(&conn, &set.namespace, &set.database, &sql).await?;
        let rows = match value {
            Value::Array(rows) => rows,
            Value::Null => break,
            other => vec![other],
        };
        if rows.is_empty() {
            break;
        }
        let count = rows.len();
        for row in rows {
            let key = json_i64(&row, "key")
                .or_else(|| record_key(row.get("id")))
                .context("item key")?;
            let body = Body::from_logged(row.get("body").unwrap_or(&Value::Null))?;
            let rids = row
                .get("rids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            out.push(StoredItem {
                key,
                item: row
                    .get("item")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tag: row
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rids,
                body,
                commit: json_i64(&row, "commit").unwrap_or(0),
            });
        }
        if (count as u32) < batch {
            break;
        }
        offset += count as u32;
    }
    Ok(out)
}

fn json_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(rows) => !rows.is_empty(),
        Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

fn record_key(id: Option<&Value>) -> Option<i64> {
    let id = id?;
    if let Some(n) = id.as_i64() {
        return Some(n);
    }
    if let Some(text) = id.as_str() {
        let num = text.rsplit(':').next().unwrap_or("");
        let num = num.trim_matches(|c: char| !c.is_ascii_digit() && c != '-');
        return num.parse().ok();
    }
    json_i64(id, "id").or_else(|| json_i64(id, "Number"))
}

fn json_i64(value: &Value, field: &str) -> Option<i64> {
    let found = match value {
        Value::Array(rows) => rows
            .iter()
            .find_map(|row| row.get(field))
            .or_else(|| rows.first()),
        other => other.get(field),
    };
    match found {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

pub async fn remove_database(
    cluster: &Cluster,
    node_id: &str,
    url: &str,
    namespace: &str,
    database: &str,
) -> anyhow::Result<()> {
    let conn = cluster.conn(node_id, url).await?;
    crate::link::exec(
        &conn,
        &format!(
            "USE NS {}; REMOVE DATABASE IF EXISTS {};",
            crate::guard::quote_ident(namespace),
            crate::guard::quote_ident(database)
        ),
    )
    .await?;
    Ok(())
}

async fn node_ready(url: &str) -> bool {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || probe(&url))
        .await
        .unwrap_or(false)
}

fn probe(url: &str) -> bool {
    let addr = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let Ok(addr) = addr.parse::<SocketAddr>() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return false;
    };
    let _ = stream.set_nonblocking(true);
    let started = std::time::Instant::now();
    let deadline = Duration::from_millis(400);
    let request = b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n";
    let mut sent = 0_usize;
    let mut buf = [0_u8; 16];
    loop {
        if started.elapsed() > deadline {
            return false;
        }
        if sent < request.len() {
            match stream.write(&request[sent..]) {
                Ok(0) => return false,
                Ok(n) => sent += n,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                Err(_) => return false,
            }
            continue;
        }
        match stream.read(&mut buf) {
            Ok(n) if n > 0 => return buf.starts_with(b"HTTP"),
            Ok(_) => return false,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::accept_read;

    #[test]
    fn read_is_one_select() {
        assert!(accept_read("SELECT * FROM row").is_ok());
        assert!(accept_read("select * from row;").is_ok());
        assert!(accept_read("SELECT * FROM committed").is_ok());
        for sql in [
            "DELETE row",
            "INSERT INTO row",
            "SELECT * FROM row; SELECT * FROM row",
            "UPDATE row SET n = 1",
        ] {
            assert!(accept_read(sql).is_err(), "{sql}");
        }
    }

    #[test]
    fn a_small_commit_stays_one_entry() {
        let item = sample_item(1);
        let chunks =
            super::commit_chunks(super::Body::new(), &[item], 256, 4 * 1024 * 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1.len(), 1);
    }

    #[test]
    fn a_large_commit_splits_on_the_doc_cap() {
        let items: Vec<_> = (0..300).map(sample_item).collect();
        let chunks = super::commit_chunks(super::Body::new(), &items, 256, u32::MAX).unwrap();
        assert_eq!(chunks.len(), 2, "{}", chunks.len());
        assert_eq!(chunks[0].1.len(), 256);
        assert_eq!(chunks[1].1.len(), 44);
        for (body, packed) in &chunks {
            assert!(body.byte_len() > 0);
            assert!(packed.len() <= 256);
        }
    }

    #[test]
    fn one_oversized_item_is_still_emitted() {
        let item = sample_item(7);
        let chunks = super::commit_chunks(super::Body::new(), &[item], 256, 1).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1.len(), 1);
    }

    #[test]
    fn log_rows_keep_order_and_the_previous_fence() {
        let value = serde_json::json!([
            {"id": "_repl_log:8", "fence": 4, "tag": "layout", "at": "2026-10-08T00:00:00Z", "body": {"statements": ["UPSERT row:1 SET n = 1;"], "params": {}}},
            {"id": "_repl_log:7", "fence": 3, "tag": "layout", "at": "2026-10-08T00:00:00Z", "body": {"statements": ["UPSERT row:1 SET n = 1;"], "params": {}}}
        ]);
        let entries = super::entries_from_log(&value, 2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].lsn, 7);
        assert_eq!(entries[0].prev, 6);
        assert_eq!(entries[0].prev_fence, 2);
        assert_eq!(entries[1].prev_fence, 3);
        assert_eq!(entries[1].fence, 4);
    }

    fn sample_item(n: i64) -> super::Item {
        super::Item {
            key: n,
            item: format!("item:{n}"),
            tag: "job".into(),
            rids: vec![format!("row:{n}")],
            body: super::Body::new().upsert(&format!("row:{n}"), serde_json::json!({"n": n})),
            delete: false,
        }
    }
}
