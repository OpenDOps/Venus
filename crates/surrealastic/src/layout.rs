//! Layout: one commit set, shard sets keyed by an integer, queued apply, refill.
//! No page, doc, or field hash. The caller supplies `key`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::body::Body;
use crate::cluster::Cluster;
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
    history: tokio::sync::Mutex<Vec<(i64, Vec<Item>)>>,
    apply: tokio::sync::Mutex<()>,
}

pub struct Layout {
    inner: Arc<Inner>,
    stop: Arc<AtomicBool>,
    pump: Option<JoinHandle<()>>,
}

impl Drop for Layout {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(task) = self.pump.take() {
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
            history: tokio::sync::Mutex::new(Vec::new()),
            apply: tokio::sync::Mutex::new(()),
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
        Ok(Self {
            inner,
            stop,
            pump: Some(pump),
        })
    }

    /// Commit, then apply. The number returned is the commit set's lsn.
    pub async fn write(&self, commit_body: Body, items: Vec<Item>) -> anyhow::Result<i64> {
        let mut body = commit_body;
        for item in &items {
            body = push_item(body, item)?;
        }
        let commit = match self
            .inner
            .writer
            .write(self.db().await.commit_set.as_str(), body, "layout")
            .await
        {
            Ok(lsn) => lsn,
            Err(err) => return Err(commit_error(err)),
        };
        self.inner
            .history
            .lock()
            .await
            .push((commit, items.clone()));
        let _guard = self.inner.apply.lock().await;
        self.inner.fan_out(commit, &items).await;
        Ok(commit)
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
    pub async fn catch_up(&self, set_id: &str) -> anyhow::Result<()> {
        self.inner.catch_up(set_id).await
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
    async fn fan_out(&self, commit: i64, items: &[Item]) {
        let db = self.db.lock().await.clone();
        let grouped = group_items(items, db.shard_count, db.shard_count_next);
        for (shard, group) in grouped {
            if self.apply_group(shard, commit, &group).await.is_err() {
                self.queue.lock().await.entry(shard).or_default().push(Pending {
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
        if !self.ready(&set_id).await? {
            anyhow::bail!("no in-sync copy");
        }
        let docs = self.cluster.config().entry_docs;
        let bytes = self.cluster.config().entry_bytes;
        let packs = pack_items(items, docs, bytes);
        for (index, pack) in packs.iter().enumerate() {
            let last = index + 1 == packs.len();
            let body = shard_body(pack, commit, last)?;
            self.writer.write(&set_id, body, "layout").await?;
        }
        Ok(())
    }

    async fn ready(&self, set_id: &str) -> anyhow::Result<bool> {
        let copies = self.cluster.copies(set_id).await?;
        let head = self.writer.head(set_id).await;
        let mut any = false;
        for copy in copies {
            if !node_ready(&copy.url).await {
                self.writer.force_lagging(set_id, &copy.node_id).await?;
                continue;
            }
            let set = store::load_set(self.cluster.pool(), set_id).await?;
            self.cluster.disconnect(&copy.node_id).await;
            let conn = self.cluster.conn(&copy.node_id, &copy.url).await?;
            let (applied, _) = crate::link::state_of(&conn, &set.namespace, &set.database)
                .await
                .unwrap_or((0, 0));
            if applied < head {
                self.replay_copy(set_id, &copy.node_id, &copy.url).await?;
            }
            self.writer.revive(set_id, &copy.node_id).await?;
            any = true;
        }
        Ok(any)
    }

    async fn replay_copy(&self, set_id: &str, node: &str, url: &str) -> anyhow::Result<()> {
        let set = store::load_set(self.cluster.pool(), set_id).await?;
        let entries = self.writer.remembered(set_id).await;
        self.cluster.disconnect(node).await;
        let conn = self.cluster.conn(node, url).await?;
        let (mut applied, _) = crate::link::state_of(&conn, &set.namespace, &set.database).await?;
        for entry in &entries {
            if entry.lsn <= applied {
                continue;
            }
            let reply = crate::link::apply(&conn, &set.namespace, &set.database, entry).await?;
            if matches!(
                reply.status,
                crate::guard::ApplyStatus::Ok | crate::guard::ApplyStatus::Already
            ) {
                applied = entry.lsn;
            }
        }
        self.writer.note_caught_up(set_id, node, applied).await;
        Ok(())
    }

    async fn catch_up(&self, set_id: &str) -> anyhow::Result<()> {
        let copies = self.cluster.copies(set_id).await?;
        for copy in copies {
            if !node_ready(&copy.url).await {
                continue;
            }
            let set = store::load_set(self.cluster.pool(), set_id).await?;
            self.cluster.disconnect(&copy.node_id).await;
            let conn = self.cluster.conn(&copy.node_id, &copy.url).await?;
            let (applied, _) =
                crate::link::state_of(&conn, &set.namespace, &set.database).await?;
            let head = self.writer.head(set_id).await;
            if applied < head {
                self.replay_copy(set_id, &copy.node_id, &copy.url).await?;
            }
            self.writer.revive(set_id, &copy.node_id).await?;
        }
        Ok(())
    }

    async fn read_shard(
        &self,
        shard: i32,
        query: &str,
        min_commit: Option<i64>,
    ) -> Result<String, String> {
        let db = self.db.lock().await.clone();
        let set_id = shard_set_id(&db.db_id, db.epoch, shard);
        let copies = self.cluster.copies(&set_id).await.map_err(|err| err.to_string())?;
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
        let history = self.history.lock().await.clone();
        for (commit_lsn, committed) in history {
            if commit_lsn <= l0 {
                continue;
            }
            let group: Vec<Item> = committed
                .into_iter()
                .filter(|item| {
                    shards_for(item.key, db.shard_count, db.shard_count_next).contains(&shard)
                })
                .collect();
            if group.is_empty() {
                continue;
            }
            self.apply_group(shard, commit_lsn, &group).await?;
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
    if err.to_string().to_ascii_lowercase().contains("key collision") {
        anyhow::anyhow!("key collision")
    } else {
        err
    }
}

fn accept_read(query: &str) -> anyhow::Result<()> {
    let lower = query.to_ascii_lowercase();
    for word in ["upsert", "delete", "create", "define", "remove", "update", "relate"] {
        if lower.contains(word) {
            anyhow::bail!("read is a query");
        }
    }
    Ok(())
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
        ensure_one(cluster, &shard_set_id(&row.db_id, row.epoch, shard), false, owner).await?;
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
    let stored = read_item_table(&inner.cluster, &commit).await?;
    for shard in 0..span(&db) {
        let cursor = inner.cursor(shard).await.unwrap_or(0);
        if cursor >= head {
            continue;
        }
        let mut by_commit: BTreeMap<i64, Vec<Item>> = BTreeMap::new();
        for row in &stored {
            if row.commit <= cursor {
                continue;
            }
            if !shards_for(row.key, db.shard_count, db.shard_count_next).contains(&shard) {
                continue;
            }
            by_commit.entry(row.commit).or_default().push(Item {
                key: row.key,
                item: row.item.clone(),
                tag: row.tag.clone(),
                rids: row.rids.clone(),
                body: row.body.clone(),
                delete: false,
            });
        }
        let mut queue = inner.queue.lock().await;
        for (commit_lsn, items) in by_commit {
            queue.entry(shard).or_default().push(Pending {
                commit: commit_lsn,
                items,
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

async fn read_item_table(cluster: &Cluster, set: &crate::store::SetRow) -> anyhow::Result<Vec<StoredItem>> {
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
                item: row.get("item").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                tag: row.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string(),
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
        Value::Array(rows) => rows.iter().find_map(|row| row.get(field)).or_else(|| rows.first()),
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
        &format!("USE NS {namespace}; REMOVE DATABASE IF EXISTS {database};"),
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
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));
    if stream
        .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => buf.starts_with(b"HTTP"),
        _ => false,
    }
}
