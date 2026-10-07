//! One fenced writer. One queue per set. One entry in flight per copy.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use tokio::sync::{mpsc, Mutex, oneshot};
use tokio::task::JoinHandle;

use crate::body::Body;
use crate::cluster::Cluster;
use crate::guard::{ApplyReply, ApplyStatus, Entry};
use crate::hooks::Owner;
use crate::store::{self, SetRow};

struct CopyRt {
    applied: i64,
    state: String,
    behind_since: Option<Instant>,
    tx: mpsc::UnboundedSender<Job>,
}

struct SetRt {
    row: SetRow,
    head: i64,
    head_fence: i64,
    copies: HashMap<String, CopyRt>,
}

struct Job {
    entry: Entry,
    reply: oneshot::Sender<anyhow::Result<ApplyReply>>,
}

pub struct Writer {
    cluster: Cluster,
    lease: String,
    holder: String,
    fence: i64,
    lease_until: Arc<Mutex<f64>>,
    stopped: Arc<AtomicBool>,
    sets: Arc<Mutex<HashMap<String, SetRt>>>,
    renew: Option<JoinHandle<()>>,
    #[cfg_attr(not(feature = "fault"), allow(dead_code))]
    sends: Arc<AtomicU64>,
    #[cfg_attr(not(feature = "fault"), allow(dead_code))]
    peak: Arc<Mutex<HashMap<String, u32>>>,
    #[allow(dead_code)]
    inflight: Arc<Mutex<HashMap<String, u32>>>,
    lagged_at: Arc<Mutex<HashMap<String, i64>>>,
    owner: Option<Arc<dyn Owner>>,
    log: Arc<Mutex<HashMap<String, Vec<Entry>>>>,
}

fn transport(err: &anyhow::Error) -> bool {
    let mut text = String::new();
    for cause in err.chain() {
        text.push_str(&cause.to_string());
        text.push(' ');
    }
    let text = text.to_ascii_lowercase();
    ["timed out", "timeout", "connection", "os error", "refused", "reset", "closed", "transport"]
        .iter()
        .any(|needle| text.contains(needle))
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(task) = self.renew.take() {
            task.abort();
        }
    }
}

impl Writer {
    pub(crate) async fn start(
        cluster: Cluster,
        set_id: &str,
        holder: &str,
        fence: i64,
        lease_until: f64,
    ) -> anyhow::Result<Self> {
        let set = store::load_set(cluster.pool(), set_id).await?;
        let copies = store::load_copies(cluster.pool(), set_id).await?;
        let sends = Arc::new(AtomicU64::new(0));
        let inflight = Arc::new(Mutex::new(HashMap::new()));
        let peak = Arc::new(Mutex::new(HashMap::new()));
        let mut head = 0_i64;
        let mut head_fence = 0_i64;
        let mut copy_rt = HashMap::new();
        for copy in &copies {
            let (applied, applied_fence) = match cluster.conn(&copy.node_id, &copy.url).await {
                Ok(conn) => crate::link::state_of(&conn, &set.namespace, &set.database)
                    .await
                    .unwrap_or((copy.applied_lsn, 0)),
                Err(_) => (copy.applied_lsn, 0),
            };
            if (applied, applied_fence) > (head, head_fence) {
                head = applied;
                head_fence = applied_fence;
            }
            let (tx, rx) = mpsc::unbounded_channel();
            spawn_lane(
                cluster.clone(),
                set.namespace.clone(),
                set.database.clone(),
                copy.node_id.clone(),
                copy.url.clone(),
                rx,
                Arc::clone(&sends),
                Arc::clone(&inflight),
                Arc::clone(&peak),
            );
            copy_rt.insert(
                copy.node_id.clone(),
                CopyRt {
                    applied,
                    state: copy.state.clone(),
                    behind_since: None,
                    tx,
                },
            );
        }
        let lease = set.lease.clone();
        let sets = Arc::new(Mutex::new(HashMap::from([(
            set_id.to_string(),
            SetRt {
                row: set,
                head,
                head_fence,
                copies: copy_rt,
            },
        )])));
        let lease_until = Arc::new(Mutex::new(lease_until));
        let stopped = Arc::new(AtomicBool::new(false));
        let renew = spawn_renew(
            cluster.clone(),
            lease.clone(),
            holder.to_string(),
            Arc::clone(&lease_until),
            Arc::clone(&stopped),
        );
        Ok(Self {
            cluster,
            lease,
            holder: holder.to_string(),
            fence,
            lease_until,
            stopped,
            sets,
            renew: Some(renew),
            sends,
            peak,
            inflight,
            lagged_at: Arc::new(Mutex::new(HashMap::new())),
            owner: None,
            log: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Another set on this writer's lease. Shard sets share the commit set's lease.
    pub(crate) async fn attach(&self, set_id: &str) -> anyhow::Result<()> {
        if self.sets.lock().await.contains_key(set_id) {
            return Ok(());
        }
        let set = store::load_set(self.cluster.pool(), set_id).await?;
        if set.lease != self.lease {
            anyhow::bail!("set lease does not match this writer");
        }
        let copies = store::load_copies(self.cluster.pool(), set_id).await?;
        let mut copy_rt = HashMap::new();
        let mut head = 0_i64;
        let mut head_fence = 0_i64;
        for copy in &copies {
            let (applied, applied_fence) = match self.cluster.conn(&copy.node_id, &copy.url).await {
                Ok(conn) => crate::link::state_of(&conn, &set.namespace, &set.database)
                    .await
                    .unwrap_or((copy.applied_lsn, 0)),
                Err(_) => (copy.applied_lsn, 0),
            };
            if (applied, applied_fence) > (head, head_fence) {
                head = applied;
                head_fence = applied_fence;
            }
            let (tx, rx) = mpsc::unbounded_channel();
            spawn_lane(
                self.cluster.clone(),
                set.namespace.clone(),
                set.database.clone(),
                copy.node_id.clone(),
                copy.url.clone(),
                rx,
                Arc::clone(&self.sends),
                Arc::clone(&self.inflight),
                Arc::clone(&self.peak),
            );
            copy_rt.insert(
                copy.node_id.clone(),
                CopyRt {
                    applied,
                    state: copy.state.clone(),
                    behind_since: None,
                    tx,
                },
            );
        }
        self.sets.lock().await.insert(
            set_id.to_string(),
            SetRt {
                row: set,
                head,
                head_fence,
                copies: copy_rt,
            },
        );
        Ok(())
    }

    pub(crate) async fn head(&self, set_id: &str) -> i64 {
        self.sets
            .lock()
            .await
            .get(set_id)
            .map(|set| set.head)
            .unwrap_or(0)
    }

    pub(crate) async fn remembered(&self, set_id: &str) -> Vec<Entry> {
        self.log
            .lock()
            .await
            .get(set_id)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) async fn force_lagging(&self, set_id: &str, node: &str) -> anyhow::Result<()> {
        self.mark_lagging(set_id, node, 0).await
    }

    pub(crate) async fn note_caught_up(&self, set_id: &str, node: &str, applied: i64) {
        self.note_applied(set_id, node, applied).await;
    }

    pub(crate) async fn revive(&self, set_id: &str, node: &str) -> anyhow::Result<()> {
        {
            let mut sets = self.sets.lock().await;
            let set = sets.get_mut(set_id).context("set is not on this writer")?;
            let copy = set.copies.get_mut(node).context("copy is not on this set")?;
            copy.state = "in_sync".to_string();
            copy.behind_since = None;
        }
        store::set_copy_state(self.cluster.pool(), set_id, node, "in_sync").await?;
        Ok(())
    }

    /// After a database is removed. The next entry starts at lsn 1.
    pub(crate) async fn reset_progress(&self, set_id: &str) {
        let mut sets = self.sets.lock().await;
        if let Some(set) = sets.get_mut(set_id) {
            set.head = 0;
            set.head_fence = 0;
            for copy in set.copies.values_mut() {
                copy.applied = 0;
                copy.state = "in_sync".to_string();
                copy.behind_since = None;
            }
        }
        self.log.lock().await.remove(set_id);
    }

    pub fn fence(&self) -> i64 {
        self.fence
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    pub fn set_owner(&mut self, owner: Arc<dyn Owner>) {
        self.owner = Some(owner);
    }

    pub fn has_owner(&self) -> bool {
        self.owner.is_some()
    }

    pub fn stop_renew(&self) {
        if let Some(task) = &self.renew {
            task.abort();
        }
    }

    /// Renew. The fence does not move while this holder keeps the lease.
    pub async fn renew(&self) -> anyhow::Result<Option<i64>> {
        let secs = self.cluster.config().lease.as_secs_f64();
        match store::claim_lease(self.cluster.pool(), &self.lease, &self.holder, secs).await? {
            Some((fence, until)) => {
                *self.lease_until.lock().await = until;
                Ok(Some(fence))
            }
            None => {
                self.stopped.store(true, Ordering::Relaxed);
                Ok(None)
            }
        }
    }

    #[cfg(feature = "fault")]
    pub fn sends(&self) -> u64 {
        self.sends.load(Ordering::Relaxed)
    }

    #[cfg(feature = "fault")]
    pub async fn peak_in_flight(&self, node_id: &str) -> u32 {
        self.peak.lock().await.get(node_id).copied().unwrap_or(0)
    }

    #[cfg(feature = "fault")]
    pub async fn lagged_at(&self, node_id: &str) -> Option<i64> {
        self.lagged_at.lock().await.get(node_id).copied()
    }

    pub async fn write(&self, set_id: &str, body: Body, tag: &str) -> anyhow::Result<i64> {
        crate::body::accept_sql(&body.sql())?;
        if self.stopped.load(Ordering::Relaxed) {
            anyhow::bail!("writer stopped");
        }
        let now = epoch();
        if now >= *self.lease_until.lock().await {
            anyhow::bail!("lease expired");
        }

        let entry = Entry {
            lsn: 0,
            prev: 0,
            prev_fence: 0,
            fence: self.fence,
            tag: tag.to_string(),
            at: iso_now(),
            body,
        };
        let (entry, targets, lags) = {
            let mut sets = self.sets.lock().await;
            let set = sets.get_mut(set_id).context("set is not on this writer")?;
            if set.row.lease != self.lease {
                anyhow::bail!("set lease does not match this writer");
            }
            let lsn = set.head + 1;
            let prev = set.head;
            let prev_fence = set.head_fence;
            set.head = lsn;
            set.head_fence = self.fence;
            let mut entry = entry;
            entry.lsn = lsn;
            entry.prev = prev;
            entry.prev_fence = prev_fence;

            let lag_entries = self.cluster.config().lag_entries;
            let lag_for = self.cluster.config().lag;
            let mut targets = Vec::new();
            let mut lags = Vec::new();
            for (node, copy) in set.copies.iter_mut() {
                if copy.state != "in_sync" {
                    continue;
                }
                let behind = lsn.saturating_sub(copy.applied);
                if copy.applied < prev && copy.behind_since.is_none() {
                    copy.behind_since = Some(Instant::now());
                }
                let timed = copy
                    .behind_since
                    .map(|t| t.elapsed() >= lag_for)
                    .unwrap_or(false);
                if behind >= lag_entries as i64 || (behind > 1 && timed) {
                    copy.state = "lagging".to_string();
                    lags.push(node.clone());
                    continue;
                }
                targets.push((node.clone(), copy.tx.clone()));
            }
            (entry, targets, lags)
        };

        for node in &lags {
            store::set_copy_state(self.cluster.pool(), set_id, node, "lagging").await?;
            self.lagged_at.lock().await.entry(node.clone()).or_insert(entry.lsn);
        }

        let ack = {
            let sets = self.sets.lock().await;
            sets.get(set_id).map(|s| s.row.ack).unwrap_or(1)
        };
        if targets.is_empty() {
            self.rollback_head(set_id, &entry).await;
            anyhow::bail!("no in-sync copy");
        }
        let mut pending = targets.len();
        let mut ok = 0_i32;
        let (tx, mut rx) = mpsc::channel(pending);
        for (node, lane) in targets {
            let (reply_tx, reply_rx) = oneshot::channel();
            lane.send(Job {
                entry: entry.clone(),
                reply: reply_tx,
            })
            .context("lane closed")?;
            let tx = tx.clone();
            let node = node.clone();
            tokio::spawn(async move {
                let result = reply_rx.await.unwrap_or_else(|e| Err(anyhow::anyhow!(e)));
                let _ = tx.send((node, result)).await;
            });
        }
        drop(tx);
        while pending > 0 && ok < ack {
            let Some((node, result)) = rx.recv().await else { break };
            pending -= 1;
            match result {
                Ok(reply) if matches!(reply.status, ApplyStatus::Ok | ApplyStatus::Already) => {
                    self.note_applied(set_id, &node, reply.applied_lsn).await;
                    ok += 1;
                }
                Ok(reply) if reply.status == ApplyStatus::Fenced => {
                    self.stopped.store(true, Ordering::Relaxed);
                    anyhow::bail!("fenced");
                }
                Err(err) if transport(&err) => {
                    self.mark_lagging(set_id, &node, entry.lsn).await?;
                }
                Err(err) => {
                    if ok == 0 {
                        self.rollback_head(set_id, &entry).await;
                    }
                    return Err(err);
                }
                Ok(_) => {
                    self.mark_lagging(set_id, &node, entry.lsn).await?;
                }
            }
        }
        if ok < ack {
            if ok == 0 {
                self.rollback_head(set_id, &entry).await;
            }
            anyhow::bail!("acked {ok} of {ack}");
        }
        self.log
            .lock()
            .await
            .entry(set_id.to_string())
            .or_default()
            .push(entry.clone());
        Ok(entry.lsn)
    }

    async fn rollback_head(&self, set_id: &str, entry: &Entry) {
        let mut sets = self.sets.lock().await;
        if let Some(set) = sets.get_mut(set_id) {
            if set.head == entry.lsn {
                set.head = entry.prev;
                set.head_fence = entry.prev_fence;
            }
        }
    }

    async fn note_applied(&self, set_id: &str, node: &str, applied: i64) {
        let mut sets = self.sets.lock().await;
        if let Some(set) = sets.get_mut(set_id) {
            if let Some(copy) = set.copies.get_mut(node) {
                copy.applied = applied;
                // 1s behind means a second with no progress, not a second of being one write short.
                copy.behind_since = if copy.applied >= set.head {
                    None
                } else {
                    Some(Instant::now())
                };
            }
        }
    }

    async fn mark_lagging(&self, set_id: &str, node: &str, lsn: i64) -> anyhow::Result<()> {
        {
            let mut sets = self.sets.lock().await;
            if let Some(set) = sets.get_mut(set_id) {
                if let Some(copy) = set.copies.get_mut(node) {
                    copy.state = "lagging".to_string();
                }
            }
        }
        store::set_copy_state(self.cluster.pool(), set_id, node, "lagging").await?;
        self.lagged_at.lock().await.entry(node.to_string()).or_insert(lsn);
        Ok(())
    }
}

fn spawn_lane(
    cluster: Cluster,
    namespace: String,
    database: String,
    node_id: String,
    url: String,
    mut rx: mpsc::UnboundedReceiver<Job>,
    sends: Arc<AtomicU64>,
    inflight: Arc<Mutex<HashMap<String, u32>>>,
    peak: Arc<Mutex<HashMap<String, u32>>>,
) {
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            enter(&inflight, &peak, &node_id).await;
            sends.fetch_add(1, Ordering::Relaxed);
            let result = match cluster.conn(&node_id, &url).await {
                Ok(conn) => crate::link::apply(&conn, &namespace, &database, &job.entry).await,
                Err(err) => Err(err),
            };
            leave(&inflight, &node_id).await;
            let _ = job.reply.send(result);
        }
    });
}

async fn enter(
    inflight: &Mutex<HashMap<String, u32>>,
    peak: &Mutex<HashMap<String, u32>>,
    node: &str,
) {
    let now = {
        let mut map = inflight.lock().await;
        let n = map.entry(node.to_string()).or_insert(0);
        *n += 1;
        *n
    };
    let mut peak = peak.lock().await;
    let slot = peak.entry(node.to_string()).or_insert(0);
    if now > *slot {
        *slot = now;
    }
}

async fn leave(inflight: &Mutex<HashMap<String, u32>>, node: &str) {
    let mut map = inflight.lock().await;
    if let Some(n) = map.get_mut(node) {
        *n = n.saturating_sub(1);
    }
}

fn spawn_renew(
    cluster: Cluster,
    lease: String,
    holder: String,
    lease_until: Arc<Mutex<f64>>,
    stopped: Arc<AtomicBool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(cluster.config().renew).await;
            if stopped.load(Ordering::Relaxed) {
                break;
            }
            let secs = cluster.config().lease.as_secs_f64();
            match store::claim_lease(cluster.pool(), &lease, &holder, secs).await {
                Ok(Some((_, until))) => *lease_until.lock().await = until,
                Ok(None) => {
                    stopped.store(true, Ordering::Relaxed);
                    break;
                }
                Err(_) => {
                    stopped.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    })
}

fn epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Days from Unix epoch to a civil date. Howard Hinnant's algorithm.
fn civil(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let ss = secs.rem_euclid(86_400) as u32;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m as u32, d as u32, ss / 3600, (ss / 60) % 60, ss % 60)
}
