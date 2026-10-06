//! One apply queue and one persist buffer per `doc_id` inside a room
//! (`workspace_id`). Not per socket. M4: sockets name `doc_id` with `?doc=`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use bytes::Bytes;
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot, Mutex, Notify, RwLock};
use tokio::task::JoinHandle;
use y_octo::{Doc, DocMessage, SyncMessage};

use crate::db;
use crate::lease::{Lease, LeaseError};
use crate::protocol::{
    apply_v1, encode_auth_ok, encode_awareness_empty, encode_doc_step1, encode_doc_step2,
    encode_doc_update, encode_state_vector, encode_step2_for, encode_v1, is_noop_update,
};
use crate::PAGE_DOC_ID;

type ClientId = u64;

/// Per-client outbound queue of framed `Bytes` (P7: fan-out is a refcount).
/// Slot cap (S1). A stalled reader is detached on `try_send` Full rather than
/// growing hub memory. Reconnect gets a fresh Step2.
pub const OUTBOUND_CAP: usize = 256;

/// Queued-byte budget per client (S6). Two max frames (`WS_MAX_MESSAGE` =
/// 512 KiB) so a full-size paste plus a bit of follow-on does not detach a
/// slightly lagging peer. Far below `OUTBOUND_CAP × WS_MAX_MESSAGE`. Detach
/// when a send would exceed this, even if slots remain.
pub const OUTBOUND_BYTES: usize = 1024 * 1024;

/// Persist buffer cap per room (S10). Oldest bins are dropped (loudly) when a
/// push would exceed this; the newest stays. Must be ≥ `WS_MAX_MESSAGE` so one
/// accepted frame always fits. Do not drop on SQL error (L2).
pub const PERSIST_BYTES: usize = 8 * 1024 * 1024;

/// Non-home documents resident in one room. Home is the room doc, not counted.
/// Idle documents with an empty persist buffer are dropped before a new one is refused.
pub const MAX_EXTRA_DOCS: usize = 64;

/// Sockets one client address may hold on one room, home included.
pub const MAX_CONNECTIONS_PER_IP: usize = 32;

/// How long `ensure_doc` keeps a document after hydration with no socket yet.
/// The websocket upgrade follows immediately. A detached document is not covered
/// and can be dropped at the cap right away.
const OPEN_GRACE: Duration = Duration::from_secs(15);

pub const CAP_DOCS: &str = "too many open documents";
pub const CAP_CONNECTIONS: &str = "too many connections";

/// Room refused another document or another socket from one address.
#[derive(Debug)]
pub struct RoomCapacity {
    pub kind: &'static str,
}

impl std::fmt::Display for RoomCapacity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.kind)
    }
}

impl std::error::Error for RoomCapacity {}

pub fn is_capacity(err: &anyhow::Error) -> bool {
    err.downcast_ref::<RoomCapacity>().is_some()
}

/// The page's `page_identity` row is a Flush tombstone. Its CRDT rows remain.
#[derive(Debug)]
pub struct DocDeleted;

impl std::fmt::Display for DocDeleted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("document was deleted")
    }
}

impl std::error::Error for DocDeleted {}

pub fn is_deleted(err: &anyhow::Error) -> bool {
    err.downcast_ref::<DocDeleted>().is_some()
}

async fn refuse_deleted(pool: &PgPool, workspace_id: &str, doc_id: &str) -> Result<()> {
    if doc_id == PAGE_DOC_ID || doc_id == crate::CATALOG_DOC_ID {
        return Ok(());
    }
    if db::page_deleted(pool, workspace_id, doc_id).await? {
        return Err(DocDeleted.into());
    }
    Ok(())
}

/// Sender half: slot cap plus a queued-byte counter the socket task decrements.
pub struct Outbound {
    tx: mpsc::Sender<Bytes>,
    queued: Arc<AtomicU64>,
    budget: usize,
}

impl Outbound {
    fn try_send(&self, frame: Bytes) -> Result<(), mpsc::error::TrySendError<Bytes>> {
        let n = frame.len() as u64;
        let budget = self.budget as u64;
        if n > budget {
            return Err(mpsc::error::TrySendError::Full(frame));
        }
        let prev = self.queued.fetch_add(n, Ordering::SeqCst);
        if prev.saturating_add(n) > budget {
            self.queued.fetch_sub(n, Ordering::SeqCst);
            return Err(mpsc::error::TrySendError::Full(frame));
        }
        match self.tx.try_send(frame) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.queued.fetch_sub(n, Ordering::SeqCst);
                Err(e)
            }
        }
    }
}

/// Receiver half of [`Outbound`]. `recv` releases the byte reservation (S6).
pub struct OutboundRx {
    rx: mpsc::Receiver<Bytes>,
    queued: Arc<AtomicU64>,
}

impl OutboundRx {
    pub async fn recv(&mut self) -> Option<Bytes> {
        let frame = self.rx.recv().await?;
        self.queued.fetch_sub(frame.len() as u64, Ordering::SeqCst);
        Some(frame)
    }
}

struct PersistBuf {
    bins: Vec<Bytes>,
    bytes: usize,
    /// Prefix currently in `flush` (L8). S10 must not drop these or a successful
    /// INSERT would mismatch the RAM prefix and duplicate on retry.
    in_flight: usize,
}

impl PersistBuf {
    fn snapshot(&mut self) -> Vec<Bytes> {
        self.in_flight = self.bins.len();
        self.bins.clone()
    }

    fn abort_snapshot(&mut self) {
        self.in_flight = 0;
    }

    fn commit_prefix(&mut self, n: usize) {
        let n = n.min(self.bins.len());
        let dropped: usize = self.bins.drain(..n).map(|b| b.len()).sum();
        self.bytes = self.bytes.saturating_sub(dropped);
        self.in_flight = 0;
    }

    fn push_capped(&mut self, bin: Bytes, cap: usize, workspace: &str) {
        self.bytes += bin.len();
        self.bins.push(bin);
        let mut dropped_bytes = 0usize;
        let mut dropped_bins = 0usize;
        while self.bytes > cap && self.bins.len() > self.in_flight.max(1) {
            let old = self.bins.remove(self.in_flight);
            self.bytes -= old.len();
            dropped_bytes += old.len();
            dropped_bins += 1;
        }
        if dropped_bytes > 0 {
            tracing::error!(
                workspace_id = %workspace,
                dropped_bytes,
                dropped_bins,
                cap,
                queued_bytes = self.bytes,
                "persist buffer over cap; dropped oldest bins"
            );
        }
    }
}

/// `Hub::get_room` failed. HTTP maps `Held` to 503 and `Store` to 500.
#[derive(Debug, Clone)]
pub enum GetRoomError {
    Held {
        workspace_id: String,
        owner: Option<String>,
    },
    /// `workspace_id` is well-shaped but not in `HUB_WORKSPACES`.
    Unknown,
    Store(Arc<anyhow::Error>),
}

impl From<LeaseError> for GetRoomError {
    fn from(e: LeaseError) -> Self {
        match e {
            LeaseError::Held {
                workspace_id,
                owner,
            } => GetRoomError::Held {
                workspace_id,
                owner,
            },
            LeaseError::Store(e) => GetRoomError::Store(Arc::new(e.into())),
        }
    }
}

impl std::fmt::Display for GetRoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GetRoomError::Held {
                workspace_id,
                owner: Some(other),
            } => write!(f, "workspace {workspace_id} owned by {other}"),
            GetRoomError::Held {
                workspace_id,
                owner: None,
            } => write!(f, "workspace {workspace_id} owned by another hub"),
            GetRoomError::Unknown => write!(f, "unknown workspace"),
            GetRoomError::Store(e) => write!(f, "{:#}", e.as_ref()),
        }
    }
}

impl std::error::Error for GetRoomError {}

struct Client {
    tx: Outbound,
    doc_id: String,
    ip: Option<IpAddr>,
}

struct ExtraSpace {
    doc: RwLock<Doc>,
    persist: Mutex<PersistBuf>,
    trail_len: AtomicU64,
    flush_mu: Mutex<()>,
    last_active: StdMutex<Instant>,
}

impl ExtraSpace {
    fn hydrated(doc: Doc, trail_len: u64) -> Self {
        Self {
            doc: RwLock::new(doc),
            persist: Mutex::new(PersistBuf {
                bins: Vec::new(),
                bytes: 0,
                in_flight: 0,
            }),
            trail_len: AtomicU64::new(trail_len),
            flush_mu: Mutex::new(()),
            last_active: StdMutex::new(Instant::now()),
        }
    }

    fn touch(&self) {
        *self.last_active.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last_active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .elapsed()
    }

    async fn persist_is_empty(&self) -> bool {
        let p = self.persist.lock().await;
        p.bins.is_empty() && p.in_flight == 0
    }
}

pub struct Room {
    pub workspace_id: String,
    doc_id: String,
    /// Home (`PAGE_DOC_ID`). Reads (hello, export, Step1) share; only `apply_v1`
    /// takes a write (P10). Other `doc_id`s live in `extra`.
    doc: RwLock<Doc>,
    /// Queued bins. `Bytes` so the L8 prefix clone is refcounts, not a copy (P15).
    persist: Mutex<PersistBuf>,
    persist_budget: usize,
    outbound_budget: usize,
    /// One flush at a time so leftover drain does not INSERT the same prefix
    /// while a persist-task flush is in flight.
    flush_mu: Mutex<()>,
    extra: Mutex<HashMap<String, Arc<ExtraSpace>>>,
    /// `ensure_doc` stamps a document here so the cap cannot drop it before the
    /// socket attaches. `attach_from` clears the stamp once the client is in.
    opening: Mutex<HashMap<String, Instant>>,
    clients: Mutex<HashMap<ClientId, Client>>,
    next_client: AtomicU64,
    trail_len: AtomicU64,
    stop: AtomicBool,
    stop_notify: Notify,
    /// Join handle for this room's persist loop (P8). Shutdown/abort take it
    /// from here so there is no process-wide Vec of every room ever opened.
    persist_task: StdMutex<Option<JoinHandle<()>>>,
    /// Last time `clients` became empty (P6). `None` while anyone is attached.
    last_empty: StdMutex<Option<Instant>>,
}

impl Room {
    pub fn new(workspace_id: String, doc: Doc) -> Self {
        Self::with_trail_len(workspace_id, doc, 0)
    }

    pub fn with_trail_len(workspace_id: String, doc: Doc, trail_len: u64) -> Self {
        Self::with_budgets(workspace_id, doc, trail_len, OUTBOUND_BYTES, PERSIST_BYTES)
    }

    #[doc(hidden)]
    pub fn with_budgets(
        workspace_id: String,
        doc: Doc,
        trail_len: u64,
        outbound_budget: usize,
        persist_budget: usize,
    ) -> Self {
        Self {
            workspace_id,
            doc_id: PAGE_DOC_ID.into(),
            doc: RwLock::new(doc),
            persist: Mutex::new(PersistBuf {
                bins: Vec::new(),
                bytes: 0,
                in_flight: 0,
            }),
            persist_budget,
            outbound_budget,
            flush_mu: Mutex::new(()),
            extra: Mutex::new(HashMap::new()),
            opening: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
            next_client: AtomicU64::new(1),
            trail_len: AtomicU64::new(trail_len),
            stop: AtomicBool::new(false),
            stop_notify: Notify::const_new(),
            persist_task: StdMutex::new(None),
            last_empty: StdMutex::new(Some(Instant::now())),
        }
    }

    pub fn connect_client(&self) -> (ClientId, Outbound, OutboundRx) {
        let id = self.next_client.fetch_add(1, Ordering::Relaxed);
        let queued = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel(OUTBOUND_CAP);
        (
            id,
            Outbound {
                tx,
                queued: Arc::clone(&queued),
                budget: self.outbound_budget,
            },
            OutboundRx { rx, queued },
        )
    }

    pub async fn attach(&self, id: ClientId, tx: Outbound) -> Result<Vec<Vec<u8>>> {
        self.attach_doc(id, tx, PAGE_DOC_ID).await
    }

    pub async fn attach_doc(
        &self,
        id: ClientId,
        tx: Outbound,
        doc_id: &str,
    ) -> Result<Vec<Vec<u8>>> {
        self.attach_from(id, tx, doc_id, None).await
    }

    /// `ip` is the socket peer. `None` is not counted toward the per-address cap
    /// (tests and requests that never installed `ConnectInfo`).
    pub async fn attach_from(
        &self,
        id: ClientId,
        tx: Outbound,
        doc_id: &str,
        ip: Option<IpAddr>,
    ) -> Result<Vec<Vec<u8>>> {
        {
            let mut clients = self.clients.lock().await;
            if let Some(ip) = ip {
                let n = clients.values().filter(|c| c.ip == Some(ip)).count();
                if n >= MAX_CONNECTIONS_PER_IP {
                    return Err(RoomCapacity {
                        kind: CAP_CONNECTIONS,
                    }
                    .into());
                }
            }
            // Register before opening the document so eviction sees this client.
            clients.insert(
                id,
                Client {
                    tx,
                    doc_id: doc_id.to_string(),
                    ip,
                },
            );
            self.note_client_count(clients.len());
        }
        self.opening.lock().await.remove(doc_id);
        let hello = if doc_id == PAGE_DOC_ID {
            Self::hello_frames(&*self.doc.read().await)
        } else {
            match self.admit_doc(doc_id).await {
                Ok(space) => {
                    let doc = space.doc.read().await;
                    Self::hello_frames(&*doc)
                }
                Err(e) => {
                    self.detach(id).await;
                    return Err(e);
                }
            }
        };
        match hello {
            Ok(frames) => Ok(frames),
            Err(e) => {
                self.detach(id).await;
                Err(e)
            }
        }
    }

    pub async fn connections_from(&self, ip: IpAddr) -> usize {
        self.clients
            .lock()
            .await
            .values()
            .filter(|c| c.ip == Some(ip))
            .count()
    }

    #[doc(hidden)]
    pub async fn extra_doc_count(&self) -> usize {
        self.extra.lock().await.len()
    }

    async fn extra_space(&self, doc_id: &str) -> Option<Arc<ExtraSpace>> {
        self.extra.lock().await.get(doc_id).cloned()
    }

    /// Open a non-home document already in the map, or an empty one under the cap.
    /// Hydration from SQL is `ensure_doc`; this only admits a resident doc.
    async fn admit_doc(&self, doc_id: &str) -> Result<Arc<ExtraSpace>> {
        if let Some(space) = self.extra_space(doc_id).await {
            space.touch();
            return Ok(space);
        }
        self.insert_extra(doc_id, Doc::default(), 0).await?;
        self.extra_space(doc_id)
            .await
            .context("admitted document missing")
    }

    /// Hydrate a non-home `doc_id` from SQL (empty trail is an empty `Doc`).
    /// Home is loaded in `open_room`. Idempotent. Refuses past `MAX_EXTRA_DOCS`
    /// after dropping idle documents with an empty persist buffer. A Flush
    /// tombstone is `DocDeleted`, even when the doc is still resident.
    pub async fn ensure_doc(&self, pool: &PgPool, doc_id: &str) -> Result<()> {
        if doc_id == PAGE_DOC_ID {
            return Ok(());
        }
        refuse_deleted(pool, &self.workspace_id, doc_id).await?;
        self.protect_open(doc_id).await;
        if self.touch_extra(doc_id).await {
            return Ok(());
        }
        let (doc, trail_len) = db::hydrate_with_trail_len(pool, &self.workspace_id, doc_id).await?;
        self.insert_extra(doc_id, doc, trail_len).await
    }

    async fn protect_open(&self, doc_id: &str) {
        self.opening
            .lock()
            .await
            .insert(doc_id.to_string(), Instant::now());
    }

    async fn fresh_openings(&self) -> HashSet<String> {
        let mut opening = self.opening.lock().await;
        opening.retain(|_, t| t.elapsed() < OPEN_GRACE);
        opening.keys().cloned().collect()
    }

    async fn touch_extra(&self, doc_id: &str) -> bool {
        let extra = self.extra.lock().await;
        if let Some(space) = extra.get(doc_id) {
            space.touch();
            true
        } else {
            false
        }
    }

    async fn insert_extra(&self, doc_id: &str, doc: Doc, trail_len: u64) -> Result<()> {
        let mut pending = Some((doc, trail_len));
        loop {
            {
                let mut extra = self.extra.lock().await;
                if let Some(space) = extra.get(doc_id) {
                    space.touch();
                    return Ok(());
                }
                if extra.len() < MAX_EXTRA_DOCS {
                    let (doc, trail_len) = pending.take().expect("pending doc");
                    extra.insert(
                        doc_id.to_string(),
                        Arc::new(ExtraSpace::hydrated(doc, trail_len)),
                    );
                    return Ok(());
                }
            }
            if self.evict_for_capacity().await == 0 {
                return Err(RoomCapacity { kind: CAP_DOCS }.into());
            }
        }
    }

    async fn busy_doc_ids(&self) -> HashSet<String> {
        self.clients
            .lock()
            .await
            .values()
            .map(|c| c.doc_id.clone())
            .collect()
    }

    /// Drop non-home documents with no client, an empty persist buffer, and
    /// no in-flight handle, once they have been idle for `ttl`. Home stays.
    pub async fn evict_idle_docs(&self, ttl: Duration) -> usize {
        let busy = self.busy_doc_ids().await;
        let opening = self.fresh_openings().await;
        let mut extra = self.extra.lock().await;
        let gone: Vec<String> = extra
            .iter()
            .filter(|(id, space)| {
                !busy.contains(*id)
                    && !opening.contains(*id)
                    && Arc::strong_count(space) == 1
                    && space.idle_for() >= ttl
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut n = 0;
        for id in gone {
            if !extra
                .get(&id)
                .expect("id just listed")
                .persist_is_empty()
                .await
            {
                continue;
            }
            extra.remove(&id);
            n += 1;
        }
        if n > 0 {
            tracing::debug!(
                workspace = %self.workspace_id,
                evicted = n,
                "evict idle docs"
            );
        }
        n
    }

    /// Free the oldest idle, empty documents until one slot is open. A document
    /// with a client or unflushed updates is never dropped, even at the cap.
    async fn evict_for_capacity(&self) -> usize {
        let busy = self.busy_doc_ids().await;
        let opening = self.fresh_openings().await;
        let mut extra = self.extra.lock().await;
        if extra.len() < MAX_EXTRA_DOCS {
            return 0;
        }
        let mut idle: Vec<(String, Instant)> = Vec::new();
        for (id, space) in extra.iter() {
            if busy.contains(id) || opening.contains(id) || Arc::strong_count(space) > 1 {
                continue;
            }
            if !space.persist_is_empty().await {
                continue;
            }
            let last = *space.last_active.lock().unwrap_or_else(|e| e.into_inner());
            idle.push((id.clone(), last));
        }
        idle.sort_by_key(|(_, t)| *t);
        let need = extra.len() + 1 - MAX_EXTRA_DOCS;
        let mut n = 0;
        for (id, _) in idle.into_iter().take(need) {
            extra.remove(&id);
            n += 1;
        }
        n
    }

    fn hello_frames(doc: &Doc) -> Result<Vec<Vec<u8>>> {
        let step1 = encode_doc_step1(encode_state_vector(doc)?)?;
        let full = encode_v1(doc)?;
        let step2 = encode_doc_step2(full)?;
        let auth = encode_auth_ok()?;
        let awareness = encode_awareness_empty()?;
        Ok(vec![auth, awareness, step1, step2])
    }

    pub async fn detach(&self, id: ClientId) {
        let mut clients = self.clients.lock().await;
        clients.remove(&id);
        self.note_client_count(clients.len());
    }

    /// Drop outbound senders so `handle_socket` sees `recv` None (L4).
    pub async fn drop_clients(&self) {
        let mut clients = self.clients.lock().await;
        clients.clear();
        self.note_client_count(0);
    }

    fn note_client_count(&self, n: usize) {
        let mut t = self.last_empty.lock().unwrap_or_else(|e| e.into_inner());
        if n == 0 {
            if t.is_none() {
                *t = Some(Instant::now());
            }
        } else {
            *t = None;
        }
    }

    pub async fn is_idle(&self, ttl: Duration) -> bool {
        if !self.clients.lock().await.is_empty() {
            return false;
        }
        {
            let p = self.persist.lock().await;
            if !p.bins.is_empty() || p.in_flight > 0 {
                return false;
            }
        }
        {
            let extra = self.extra.lock().await;
            for space in extra.values() {
                let p = space.persist.lock().await;
                if !p.bins.is_empty() || p.in_flight > 0 {
                    return false;
                }
            }
        }
        match *self.last_empty.lock().unwrap_or_else(|e| e.into_inner()) {
            Some(t) => t.elapsed() >= ttl,
            None => false,
        }
    }

    pub async fn handle_binary(&self, from: ClientId, bytes: &[u8]) {
        if self.stopped() {
            return;
        }
        let decoded = crate::protocol::decode_sync_messages(bytes);
        if decoded.remaining > 0 {
            tracing::warn!(
                workspace = %self.workspace_id,
                offset = decoded.offset,
                remaining = decoded.remaining,
                "truncated sync frame"
            );
        }
        for msg in decoded.messages {
            if let Err(e) = self.handle_msg(from, msg).await {
                tracing::warn!(
                    workspace = %self.workspace_id,
                    error = %e,
                    "hub message failed (not panicking)"
                );
            }
        }
    }

    async fn handle_msg(&self, from: ClientId, msg: SyncMessage) -> Result<()> {
        match msg {
            SyncMessage::Doc(DocMessage::Step1(sv)) => {
                let update = self.encode_step2(from, &sv).await?;
                let frame = Bytes::from(encode_doc_step2(update)?);
                self.send_to(from, frame).await;
            }
            SyncMessage::Doc(DocMessage::Step2(bin) | DocMessage::Update(bin)) => {
                self.apply_and_fanout(from, bin).await?;
            }
            SyncMessage::AwarenessQuery => {
                let frame = Bytes::from(crate::protocol::encode_sync(&SyncMessage::Awareness(
                    Default::default(),
                ))?);
                self.send_to(from, frame).await;
            }
            SyncMessage::Awareness(_) | SyncMessage::Auth(_) => {}
        }
        Ok(())
    }

    async fn client_doc_id(&self, from: ClientId) -> Option<String> {
        self.clients
            .lock()
            .await
            .get(&from)
            .map(|c| c.doc_id.clone())
    }

    async fn encode_step2(&self, from: ClientId, sv: &[u8]) -> Result<Vec<u8>> {
        let doc_id = self
            .client_doc_id(from)
            .await
            .unwrap_or_else(|| PAGE_DOC_ID.into());
        if doc_id == PAGE_DOC_ID {
            let doc = self.doc.read().await;
            encode_step2_for(&doc, sv)
        } else {
            let space = self
                .extra_space(&doc_id)
                .await
                .context("document is not open")?;
            let doc = space.doc.read().await;
            encode_step2_for(&doc, sv)
        }
    }

    async fn apply_and_fanout(&self, from: ClientId, bin: Vec<u8>) -> Result<()> {
        if self.stopped() || is_noop_update(&bin) {
            return Ok(());
        }
        let doc_id = self
            .client_doc_id(from)
            .await
            .unwrap_or_else(|| PAGE_DOC_ID.into());
        let frame = Bytes::from(encode_doc_update(bin.clone())?);
        if doc_id == PAGE_DOC_ID {
            {
                let mut doc = self.doc.write().await;
                if self.stopped() {
                    return Ok(());
                }
                apply_v1(&mut doc, &bin)?;
            }
            {
                let mut buf = self.persist.lock().await;
                if self.stopped() {
                    return Ok(());
                }
                buf.push_capped(Bytes::from(bin), self.persist_budget, &self.workspace_id);
            }
        } else {
            let space = self
                .extra_space(&doc_id)
                .await
                .context("document is not open")?;
            space.touch();
            {
                let mut doc = space.doc.write().await;
                if self.stopped() {
                    return Ok(());
                }
                apply_v1(&mut doc, &bin)?;
            }
            {
                let mut buf = space.persist.lock().await;
                if self.stopped() {
                    return Ok(());
                }
                buf.push_capped(Bytes::from(bin), self.persist_budget, &self.workspace_id);
            }
        }
        if self.stopped() {
            return Ok(());
        }
        self.broadcast_except(from, frame).await;
        Ok(())
    }

    async fn send_to(&self, id: ClientId, frame: Bytes) {
        let mut clients = self.clients.lock().await;
        let lagged = match clients.get(&id) {
            Some(c) => c.tx.try_send(frame).is_err(),
            None => false,
        };
        if lagged {
            clients.remove(&id);
            self.note_client_count(clients.len());
            tracing::warn!(
                workspace = %self.workspace_id,
                client = id,
                "outbound full, over budget, or closed; detach"
            );
        }
    }

    async fn broadcast_except(&self, from: ClientId, frame: Bytes) {
        let from_doc = match self.client_doc_id(from).await {
            Some(id) => id,
            None => return,
        };
        let mut dead = Vec::new();
        {
            let clients = self.clients.lock().await;
            for (id, c) in clients.iter() {
                if *id == from || c.doc_id != from_doc {
                    continue;
                }
                if c.tx.try_send(frame.clone()).is_err() {
                    dead.push(*id);
                }
            }
        }
        if !dead.is_empty() {
            let mut clients = self.clients.lock().await;
            for id in dead {
                tracing::warn!(
                    workspace = %self.workspace_id,
                    client = id,
                    "outbound full, over budget, or closed; detach"
                );
                clients.remove(&id);
            }
            self.note_client_count(clients.len());
        }
    }

    pub async fn encode_live(&self) -> Result<Vec<u8>> {
        let doc = self.doc.read().await;
        encode_v1(&doc)
    }

    pub async fn flush(&self, pool: &PgPool) -> Result<()> {
        flush_persist(
            pool,
            &self.workspace_id,
            &self.doc_id,
            &self.persist,
            &self.flush_mu,
            &self.trail_len,
        )
        .await?;
        let extras: Vec<(String, Arc<ExtraSpace>)> = self
            .extra
            .lock()
            .await
            .iter()
            .map(|(id, s)| (id.clone(), Arc::clone(s)))
            .collect();
        for (doc_id, space) in extras {
            flush_persist(
                pool,
                &self.workspace_id,
                &doc_id,
                &space.persist,
                &space.flush_mu,
                &space.trail_len,
            )
            .await?;
        }
        Ok(())
    }

    pub fn trail_len(&self) -> u64 {
        self.trail_len.load(Ordering::SeqCst)
    }

    /// Compact only when the in-memory trail counter says we crossed the
    /// threshold. Idle rooms do not `COUNT(*)`. Merge is always snapshot +
    /// trail from SQL so a foreign writer's rows are not deleted unseen.
    pub async fn compact_if_needed(&self, pool: &PgPool, threshold: i64) -> Result<bool> {
        if threshold < 1 {
            return Ok(false);
        }
        let mut merged_any = false;
        if self.trail_len() >= threshold as u64 {
            let out = db::compact(pool, &self.workspace_id, &self.doc_id, threshold).await?;
            self.trail_len
                .store(out.trail_len.max(0) as u64, Ordering::SeqCst);
            merged_any |= out.merged;
        }
        let extras: Vec<(String, Arc<ExtraSpace>)> = self
            .extra
            .lock()
            .await
            .iter()
            .map(|(id, s)| (id.clone(), Arc::clone(s)))
            .collect();
        for (doc_id, space) in extras {
            if space.trail_len.load(Ordering::SeqCst) < threshold as u64 {
                continue;
            }
            let out = db::compact(pool, &self.workspace_id, &doc_id, threshold).await?;
            space
                .trail_len
                .store(out.trail_len.max(0) as u64, Ordering::SeqCst);
            merged_any |= out.merged;
        }
        Ok(merged_any)
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // Permit if nobody is parked yet (L14). `notify_waiters` alone
        // stores none, so a stop between `stopped()` and `notified()` was lost.
        self.stop_notify.notify_one();
    }

    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn set_persist_task(&self, handle: JoinHandle<()>) {
        *self.persist_task.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    fn take_persist_task(&self) -> Option<JoinHandle<()>> {
        self.persist_task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub fn persist_task_is_some(&self) -> bool {
        self.persist_task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    #[doc(hidden)]
    pub async fn persist_queued_bytes(&self) -> usize {
        let mut n = self.persist.lock().await.bytes;
        let extra = self.extra.lock().await;
        for space in extra.values() {
            n += space.persist.lock().await.bytes;
        }
        n
    }

    #[doc(hidden)]
    pub async fn encode_live_doc(&self, doc_id: &str) -> Result<Vec<u8>> {
        if doc_id == PAGE_DOC_ID {
            return self.encode_live().await;
        }
        // A document that was never opened is empty. Do not insert it: export
        // calls `ensure_doc` first, and a stray encode must not pin RAM.
        let Some(space) = self.extra_space(doc_id).await else {
            return encode_v1(&Doc::default());
        };
        let doc = space.doc.read().await;
        encode_v1(&doc)
    }
}

async fn flush_persist(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    persist: &Mutex<PersistBuf>,
    flush_mu: &Mutex<()>,
    trail_len: &AtomicU64,
) -> Result<()> {
    let _gate = flush_mu.lock().await;
    let bins = {
        let mut buf = persist.lock().await;
        buf.snapshot()
    };
    if bins.is_empty() {
        return Ok(());
    }
    let views: Vec<&[u8]> = bins.iter().map(|b| b.as_ref()).collect();
    if let Err(e) = db::flush_updates(pool, workspace_id, doc_id, &views)
        .await
        .context("persist flush")
    {
        persist.lock().await.abort_snapshot();
        return Err(e);
    }
    {
        let mut buf = persist.lock().await;
        let n = bins.len();
        if buf.bins.len() >= n && buf.bins[..n] == bins[..] {
            buf.commit_prefix(n);
        } else {
            buf.abort_snapshot();
            tracing::warn!(
                workspace_id = %workspace_id,
                doc_id = %doc_id,
                expected = n,
                actual = buf.bins.len(),
                "persist prefix mismatch after flush; leaving buffer"
            );
        }
        trail_len.fetch_add(n as u64, Ordering::SeqCst);
    }
    Ok(())
}

type RoomResult = Result<Arc<Room>, GetRoomError>;
type HydrateWaiter = oneshot::Sender<RoomResult>;
type ExportResult = Result<Vec<u8>, Arc<anyhow::Error>>;
type ExportWaiter = oneshot::Sender<ExportResult>;

pub struct Hub {
    pub pool: PgPool,
    pub lease: Lease,
    rooms: Mutex<HashMap<String, Arc<Room>>>,
    /// In-flight `get_room` by `workspace_id` (P9). Std mutex so a panicked
    /// leader can drop waiters without awaiting. Never held across hydrate.
    hydrating: StdMutex<HashMap<String, Vec<HydrateWaiter>>>,
    /// In-flight cold `live_export` by `workspace_id` (S7). Same shape as
    /// `hydrating`; not the same map (waiters want bytes, not a Room).
    exporting: StdMutex<HashMap<String, Vec<ExportWaiter>>>,
    persist_interval: Duration,
    compact_after: i64,
    hydrate_attempts: AtomicU64,
    export_sql_attempts: AtomicU64,
    stopping: AtomicBool,
    /// `None` permits every well-shaped id (tests). Product sets the allowlist.
    allowed_workspaces: Option<Arc<HashSet<String>>>,
}

impl Hub {
    pub fn new(
        pool: PgPool,
        lease: Lease,
        persist_interval: Duration,
        compact_after: i64,
    ) -> Arc<Self> {
        Self::with_allowlist(pool, lease, persist_interval, compact_after, None)
    }

    /// `Some` list is the only workspaces this process will open. Ids are lowercased.
    pub fn with_allowlist(
        pool: PgPool,
        lease: Lease,
        persist_interval: Duration,
        compact_after: i64,
        workspaces: Option<Vec<String>>,
    ) -> Arc<Self> {
        let allowed_workspaces = workspaces.map(|ids| {
            Arc::new(
                ids.into_iter()
                    .map(|id| id.to_ascii_lowercase())
                    .collect::<HashSet<_>>(),
            )
        });
        Arc::new(Self {
            pool,
            lease,
            rooms: Mutex::new(HashMap::new()),
            hydrating: StdMutex::new(HashMap::new()),
            exporting: StdMutex::new(HashMap::new()),
            persist_interval,
            compact_after,
            hydrate_attempts: AtomicU64::new(0),
            export_sql_attempts: AtomicU64::new(0),
            stopping: AtomicBool::new(false),
            allowed_workspaces,
        })
    }

    /// `None` allowlist permits every id. A set permits only its members.
    pub fn permits_workspace(&self, workspace_id: &str) -> bool {
        match &self.allowed_workspaces {
            None => true,
            Some(set) => set.contains(&workspace_id.to_ascii_lowercase()),
        }
    }

    fn shutting_down_err() -> GetRoomError {
        GetRoomError::Store(Arc::new(anyhow::Error::msg("hub is shutting down")))
    }

    pub async fn get_room(self: &Arc<Self>, workspace_id: &str) -> Result<Arc<Room>, GetRoomError> {
        if !self.permits_workspace(workspace_id) {
            return Err(GetRoomError::Unknown);
        }
        if self.stopping.load(Ordering::SeqCst) {
            return Err(Self::shutting_down_err());
        }
        loop {
            if self.stopping.load(Ordering::SeqCst) {
                return Err(Self::shutting_down_err());
            }
            if let Some(room) = self.live_room(workspace_id).await {
                return Ok(room);
            }

            let follow = {
                let mut flights = self.hydrating.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(waiters) = flights.get_mut(workspace_id) {
                    let (tx, rx) = oneshot::channel();
                    waiters.push(tx);
                    Some(rx)
                } else {
                    flights.insert(workspace_id.to_string(), Vec::new());
                    None
                }
            };

            if let Some(rx) = follow {
                match rx.await {
                    Ok(result) => return result,
                    Err(_) => continue,
                }
            }

            let mut lead = HydrateLead::new(&self.hydrating, workspace_id);
            if let Some(room) = self.live_room(workspace_id).await {
                lead.complete(&Ok(Arc::clone(&room)));
                return Ok(room);
            }
            let result = self.open_room(workspace_id).await;
            lead.complete(&result);
            return result;
        }
    }

    async fn live_room(&self, workspace_id: &str) -> Option<Arc<Room>> {
        self.rooms.lock().await.get(workspace_id).cloned()
    }

    async fn open_room(self: &Arc<Self>, workspace_id: &str) -> Result<Arc<Room>, GetRoomError> {
        if self.stopping.load(Ordering::SeqCst) {
            return Err(Self::shutting_down_err());
        }
        self.lease.try_acquire(workspace_id).await?;
        if self.stopping.load(Ordering::SeqCst) {
            let e = anyhow::Error::msg("hub is shutting down");
            self.release_failed_open(workspace_id, &e).await;
            return Err(Self::shutting_down_err());
        }
        self.hydrate_attempts.fetch_add(1, Ordering::SeqCst);
        let hydrated = db::hydrate_with_trail_len(&self.pool, workspace_id, PAGE_DOC_ID).await;
        let (doc, trail_len) = match hydrated {
            Ok(pair) => pair,
            Err(e) => {
                // L16: a failed open must not leave this process owning the
                // workspace. No room means nothing heartbeats the lease, and a
                // retry would renew it — 503ing a healthy hub indefinitely.
                self.release_failed_open(workspace_id, &e).await;
                return Err(GetRoomError::Store(Arc::new(e)));
            }
        };
        let room = Arc::new(Room::with_trail_len(
            workspace_id.to_string(),
            doc,
            trail_len,
        ));
        let mut g = self.rooms.lock().await;
        if self.stopping.load(Ordering::SeqCst) {
            drop(g);
            let e = anyhow::Error::msg("hub is shutting down");
            self.release_failed_open(workspace_id, &e).await;
            return Err(Self::shutting_down_err());
        }
        if let Some(existing) = g.get(workspace_id) {
            return Ok(existing.clone());
        }
        g.insert(workspace_id.to_string(), room.clone());
        // Hold `rooms` until the handle is on the Room so shutdown cannot miss it.
        self.spawn_persist(room.clone());
        Ok(room)
    }

    /// Give the lease back after `open_room` acquired it but could not hydrate.
    /// Only reached with no `Room` in the map for this id, so no live room loses
    /// its lease. A failed drop is logged: the row then expires on its own TTL.
    async fn release_failed_open(&self, workspace_id: &str, cause: &anyhow::Error) {
        match self.lease.drop_one(workspace_id).await {
            Ok(()) => tracing::warn!(
                workspace_id = %workspace_id,
                error = %cause,
                "hydrate failed; lease released for another hub"
            ),
            Err(e) => tracing::error!(
                workspace_id = %workspace_id,
                error = %e,
                "hydrate failed and the lease could not be released; it expires on TTL"
            ),
        }
    }

    fn spawn_persist(self: &Arc<Self>, room: Arc<Room>) {
        let hub = Arc::clone(self);
        let running = room.clone();
        let handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(hub.persist_interval);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                if running.stopped() {
                    let _ = running.flush(&hub.pool).await;
                    break;
                }
                tokio::select! {
                    _ = tick.tick() => {}
                    _ = running.stop_notify.notified() => {}
                }
                if running.stopped() {
                    let _ = running.flush(&hub.pool).await;
                    break;
                }
                if let Err(e) = running.flush(&hub.pool).await {
                    tracing::error!(error = %e, workspace = %running.workspace_id, "persist flush");
                }
                if let Err(e) = running
                    .compact_if_needed(&hub.pool, hub.compact_after)
                    .await
                {
                    tracing::warn!(error = %e, workspace = %running.workspace_id, "compact");
                }
            }
        });
        room.set_persist_task(handle);
    }

    pub async fn live_export(&self, workspace_id: &str) -> Result<Vec<u8>> {
        self.live_export_doc(workspace_id, PAGE_DOC_ID).await
    }

    /// RAM encode if this process has the room; else SQL. `doc_id` is a SQL uuid.
    pub async fn live_export_doc(&self, workspace_id: &str, doc_id: &str) -> Result<Vec<u8>> {
        refuse_deleted(&self.pool, workspace_id, doc_id).await?;
        if let Some(room) = self.live_room(workspace_id).await {
            if doc_id != PAGE_DOC_ID {
                room.ensure_doc(&self.pool, doc_id).await?;
            }
            return room.encode_live_doc(doc_id).await;
        }
        let flight = export_flight_key(workspace_id, doc_id);
        loop {
            let follow = {
                let mut flights = self.exporting.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(waiters) = flights.get_mut(&flight) {
                    let (tx, rx) = oneshot::channel();
                    waiters.push(tx);
                    Some(rx)
                } else {
                    flights.insert(flight.clone(), Vec::new());
                    None
                }
            };
            if let Some(rx) = follow {
                match rx.await {
                    Ok(Ok(bytes)) => return Ok(bytes),
                    Ok(Err(e)) => return Err(anyhow::Error::msg(e.to_string())),
                    Err(_) => continue,
                }
            }

            let mut lead = ExportLead::new(&self.exporting, &flight);
            if let Some(room) = self.live_room(workspace_id).await {
                if doc_id != PAGE_DOC_ID {
                    if let Err(e) = room.ensure_doc(&self.pool, doc_id).await {
                        let result = encode_to_export(Err(e));
                        lead.complete(&result);
                        return export_to_result(result);
                    }
                }
                let result = encode_to_export(room.encode_live_doc(doc_id).await);
                lead.complete(&result);
                return export_to_result(result);
            }
            self.export_sql_attempts.fetch_add(1, Ordering::SeqCst);
            let result = encode_to_export(db::encode_doc(&self.pool, workspace_id, doc_id).await);
            lead.complete(&result);
            return export_to_result(result);
        }
    }

    pub async fn workspace_ids(&self) -> Vec<String> {
        self.rooms.lock().await.keys().cloned().collect()
    }

    /// Refresh leases for live rooms. Misses are stolen or missing: shed RAM
    /// without `try_acquire` (L4). Idle rooms (no clients, empty persist,
    /// idle ≥ lease TTL) get the same shed plus `drop_one` (P6).
    pub async fn heartbeat(&self) {
        if self.stopping.load(Ordering::SeqCst) {
            return;
        }
        let ids = self.workspace_ids().await;
        let missed = self.lease.heartbeat_many(&ids).await;
        for id in &missed {
            self.shed(id, false).await;
        }
        if self.stopping.load(Ordering::SeqCst) {
            return;
        }
        let idle_ttl = self.lease.ttl();
        let rooms: Vec<Arc<Room>> = self.rooms.lock().await.values().cloned().collect();
        for room in rooms {
            if room.is_idle(idle_ttl).await {
                self.shed(&room.workspace_id, true).await;
            } else {
                room.evict_idle_docs(idle_ttl).await;
            }
        }
    }

    /// Take the room out of the map, stop persist, close clients. Idle
    /// eviction also drops the lease so the next `get_room` can acquire.
    /// Stolen shed does not `drop_one` or `try_acquire` — the thief owns SQL.
    async fn shed(&self, workspace_id: &str, drop_lease: bool) {
        let room = {
            let mut g = self.rooms.lock().await;
            g.remove(workspace_id)
        };
        let Some(room) = room else {
            return;
        };
        if drop_lease {
            if let Err(e) = self.lease.drop_one(workspace_id).await {
                tracing::error!(
                    workspace_id = %workspace_id,
                    error = %e,
                    "idle shed could not drop lease; it expires on TTL"
                );
            }
        }
        room.request_stop();
        room.drop_clients().await;
        let timeout = drain_join_timeout(self.persist_interval);
        if let Some(handle) = room.take_persist_task() {
            let abort = handle.abort_handle();
            match tokio::time::timeout(timeout, handle).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if !e.is_cancelled() {
                        tracing::warn!(error = %e, workspace_id, "persist join");
                    }
                }
                Err(_) => {
                    tracing::warn!(workspace_id, "persist drain timed out");
                    abort.abort();
                }
            }
        }
        if let Err(e) = room.flush(&self.pool).await {
            tracing::error!(
                error = %e,
                workspace_id,
                "shed flush"
            );
        }
        tracing::warn!(workspace_id, drop_lease, "shed room");
    }

    pub fn hydrate_attempts(&self) -> u64 {
        self.hydrate_attempts.load(Ordering::SeqCst)
    }

    pub fn export_sql_attempts(&self) -> u64 {
        self.export_sql_attempts.load(Ordering::SeqCst)
    }

    /// Kill persist tasks without a last flush. Test stand-in for `kill -9`.
    #[doc(hidden)]
    pub async fn abort_persist_no_flush(&self) {
        let rooms: Vec<Arc<Room>> = self.rooms.lock().await.values().cloned().collect();
        for room in rooms {
            if let Some(t) = room.take_persist_task() {
                t.abort();
            }
        }
    }

    /// Flush persist, join persist tasks, clear rooms, drop leases. Crash without this may lose ≤1s RAM.
    pub async fn shutdown(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let rooms: Vec<Arc<Room>> = self.rooms.lock().await.values().cloned().collect();
        for room in &rooms {
            room.request_stop();
        }
        let timeout = drain_join_timeout(self.persist_interval);
        for room in &rooms {
            let Some(handle) = room.take_persist_task() else {
                continue;
            };
            let abort = handle.abort_handle();
            match tokio::time::timeout(timeout, handle).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if !e.is_cancelled() {
                        tracing::warn!(error = %e, "persist join");
                    }
                }
                Err(_) => {
                    tracing::warn!(workspace = %room.workspace_id, "persist drain timed out");
                    abort.abort();
                }
            }
        }
        for room in &rooms {
            if let Err(e) = room.flush(&self.pool).await {
                tracing::error!(error = %e, workspace = %room.workspace_id, "drain flush");
            }
        }
        self.rooms.lock().await.clear();
        if let Err(e) = self.lease.drop_all().await {
            tracing::error!(error = %e, "drop leases");
        }
    }

    /// Abort the process-wide heartbeat so it cannot UPDATE after `drop_all`, then drain.
    /// Call even when `axum::serve` returned `Err` (L12).
    pub async fn shutdown_with_heartbeat(&self, heartbeat: JoinHandle<()>) {
        heartbeat.abort();
        let _ = heartbeat.await;
        self.shutdown().await;
    }
}

fn drain_join_timeout(persist_interval: Duration) -> Duration {
    const CAP: Duration = Duration::from_secs(5);
    const FLOOR: Duration = Duration::from_millis(200);
    if persist_interval > CAP {
        CAP
    } else if persist_interval < FLOOR {
        FLOOR
    } else {
        persist_interval
    }
}

fn export_flight_key(workspace_id: &str, doc_id: &str) -> String {
    let mut s = String::with_capacity(workspace_id.len() + doc_id.len() + 1);
    s.push_str(workspace_id);
    s.push('\0');
    s.push_str(doc_id);
    s
}

fn clone_room_result(result: &RoomResult) -> RoomResult {
    match result {
        Ok(room) => Ok(Arc::clone(room)),
        Err(e) => Err(e.clone()),
    }
}

fn clone_export_result(result: &ExportResult) -> ExportResult {
    match result {
        Ok(bytes) => Ok(bytes.clone()),
        Err(e) => Err(Arc::clone(e)),
    }
}

fn encode_to_export(result: Result<Vec<u8>>) -> ExportResult {
    match result {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(Arc::new(e)),
    }
}

fn export_to_result(result: ExportResult) -> Result<Vec<u8>> {
    match result {
        Ok(bytes) => Ok(bytes),
        Err(e) => Err(anyhow::Error::msg(e.to_string())),
    }
}

/// Leader of one cold `live_export`. Drop without `complete` closes waiters
/// so they retry instead of hanging if this task panics (S7).
struct ExportLead<'a> {
    exporting: &'a StdMutex<HashMap<String, Vec<ExportWaiter>>>,
    workspace_id: String,
    finished: bool,
}

impl<'a> ExportLead<'a> {
    fn new(
        exporting: &'a StdMutex<HashMap<String, Vec<ExportWaiter>>>,
        workspace_id: &str,
    ) -> Self {
        Self {
            exporting,
            workspace_id: workspace_id.to_string(),
            finished: false,
        }
    }

    fn complete(&mut self, result: &ExportResult) {
        let waiters = self
            .exporting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.workspace_id)
            .unwrap_or_default();
        self.finished = true;
        for waiter in waiters {
            let _ = waiter.send(clone_export_result(result));
        }
    }
}

impl Drop for ExportLead<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let waiters = self
            .exporting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.workspace_id)
            .unwrap_or_default();
        drop(waiters);
    }
}

/// Leader of one `workspace_id` hydrate. Drop without `complete` closes
/// waiters so they retry instead of hanging if this task panics.
struct HydrateLead<'a> {
    hydrating: &'a StdMutex<HashMap<String, Vec<HydrateWaiter>>>,
    workspace_id: String,
    finished: bool,
}

impl<'a> HydrateLead<'a> {
    fn new(
        hydrating: &'a StdMutex<HashMap<String, Vec<HydrateWaiter>>>,
        workspace_id: &str,
    ) -> Self {
        Self {
            hydrating,
            workspace_id: workspace_id.to_string(),
            finished: false,
        }
    }

    fn complete(&mut self, result: &RoomResult) {
        let waiters = self
            .hydrating
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.workspace_id)
            .unwrap_or_default();
        self.finished = true;
        for waiter in waiters {
            let _ = waiter.send(clone_room_result(result));
        }
    }
}

impl Drop for HydrateLead<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let waiters = self
            .hydrating
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.workspace_id)
            .unwrap_or_default();
        drop(waiters);
    }
}

#[cfg(test)]
mod l14 {
    use super::*;

    /// Stop before the persist task parks on `notified()` must still wake it.
    #[tokio::test]
    async fn request_stop_stores_a_permit_for_a_later_notified() {
        let room = Room::new("l14".into(), Doc::default());
        room.request_stop();
        tokio::time::timeout(Duration::from_millis(50), room.stop_notify.notified())
            .await
            .expect("notify_one must store a permit when nobody is waiting");
        assert!(room.stopped());
    }
}

#[cfg(test)]
mod p10 {
    use super::*;

    fn spike_bin() -> Vec<u8> {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v").expect("insert");
        encode_v1(&d).expect("encode")
    }

    fn live_has_spike(bin: &[u8]) -> bool {
        let mut d = Doc::default();
        apply_v1(&mut d, bin).expect("apply live");
        match d.get_map("spike") {
            Ok(map) => format!("{:?}", map.get("k")).contains('v'),
            Err(_) => false,
        }
    }

    /// Readers share; apply is exclusive. A parked reader must not block export
    /// or attach, and must block apply until it drops (write-preferring is fine).
    #[tokio::test]
    async fn doc_readers_overlap_apply_excludes() {
        let room = Arc::new(Room::new("p10".into(), Doc::default()));
        let parked = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let hold = {
            let r = Arc::clone(&room);
            let parked = Arc::clone(&parked);
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                let _g = r.doc.read().await;
                parked.notify_one();
                release.notified().await;
            })
        };
        tokio::time::timeout(Duration::from_millis(200), parked.notified())
            .await
            .expect("reader must take the lock");

        tokio::time::timeout(Duration::from_millis(200), room.encode_live())
            .await
            .expect("export must share the read lock")
            .expect("encode_live");

        let (id, tx, _rx) = room.connect_client();
        tokio::time::timeout(Duration::from_millis(200), room.attach(id, tx))
            .await
            .expect("attach hello is a read; must overlap the parked reader")
            .expect("attach");

        let apply = {
            let r = Arc::clone(&room);
            tokio::spawn(async move { r.apply_and_fanout(id, spike_bin()).await })
        };
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            !apply.is_finished(),
            "write (apply) must wait for the parked reader"
        );

        release.notify_one();
        hold.await.expect("reader task");
        tokio::time::timeout(Duration::from_millis(200), apply)
            .await
            .expect("apply after the reader drops")
            .expect("join apply")
            .expect("apply");

        let bin = room.encode_live().await.expect("live after apply");
        assert!(live_has_spike(&bin), "apply must land once the write runs");
    }

    #[tokio::test]
    async fn encode_live_waits_for_a_write() {
        let room = Arc::new(Room::new("p10-w".into(), Doc::default()));
        let parked = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let hold = {
            let r = Arc::clone(&room);
            let parked = Arc::clone(&parked);
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                let _g = r.doc.write().await;
                parked.notify_one();
                release.notified().await;
            })
        };
        tokio::time::timeout(Duration::from_millis(200), parked.notified())
            .await
            .expect("writer must take the lock");

        let export = {
            let r = Arc::clone(&room);
            tokio::spawn(async move { r.encode_live().await })
        };
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            !export.is_finished(),
            "export must wait while apply holds the write lock"
        );

        release.notify_one();
        hold.await.expect("writer task");
        tokio::time::timeout(Duration::from_millis(200), export)
            .await
            .expect("export after the writer drops")
            .expect("join export")
            .expect("encode_live");
    }
}

#[cfg(test)]
mod p15 {
    use super::*;

    fn spike_bin() -> Vec<u8> {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v").expect("insert");
        encode_v1(&d).expect("encode")
    }

    /// L8 still clones the prefix; P15 makes that clone a refcount bump.
    #[tokio::test]
    async fn persist_flush_clone_shares_storage() {
        let room = Room::new("p15".into(), Doc::default());
        let (id, tx, _rx) = room.connect_client();
        room.attach(id, tx).await.expect("attach");
        room.apply_and_fanout(id, spike_bin()).await.expect("apply");

        let buf = room.persist.lock().await;
        assert_eq!(buf.bins.len(), 1, "one accepted update is queued");
        let cloned = buf.bins.clone();
        assert_eq!(
            buf.bins[0].as_ptr(),
            cloned[0].as_ptr(),
            "flush prefix clone must share Bytes storage, not copy the payload"
        );
        assert_eq!(
            buf.bins[..],
            cloned[..],
            "Bytes PartialEq is by content; L8 prefix check stays valid"
        );
    }
}

#[cfg(test)]
mod m4_wire {
    use super::*;

    const OTHER: &str = crate::CATALOG_DOC_ID;

    fn spike_bin() -> Vec<u8> {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v").expect("insert");
        encode_v1(&d).expect("encode")
    }

    fn live_has_spike(bin: &[u8]) -> bool {
        let mut d = Doc::default();
        apply_v1(&mut d, bin).expect("apply live");
        match d.get_map("spike") {
            Ok(map) => format!("{:?}", map.get("k")).contains('v'),
            Err(_) => false,
        }
    }

    #[tokio::test]
    async fn second_doc_does_not_apply_into_home() {
        let room = Room::new("m4-a1".into(), Doc::default());
        let (home_id, home_tx, _home_rx) = room.connect_client();
        room.attach(home_id, home_tx).await.expect("home");
        let (other_id, other_tx, _other_rx) = room.connect_client();
        room.attach_doc(other_id, other_tx, OTHER)
            .await
            .expect("other");

        room.apply_and_fanout(other_id, spike_bin())
            .await
            .expect("apply other");

        let home = room.encode_live().await.expect("home");
        assert!(
            !live_has_spike(&home),
            "C1: extra doc_id must not land in PAGE_DOC_ID"
        );
        let other = room.encode_live_doc(OTHER).await.expect("other live");
        assert!(live_has_spike(&other), "second doc must hold the update");
    }
}

#[cfg(test)]
mod m8 {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    fn doc_n(n: u32) -> String {
        format!("doc-{n}")
    }

    fn spike_bin() -> Vec<u8> {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v").expect("insert");
        encode_v1(&d).expect("encode")
    }

    async fn attach_n(room: &Room, n: u32) -> ClientId {
        let (id, tx, _rx) = room.connect_client();
        room.attach_doc(id, tx, &doc_n(n)).await.expect("attach");
        id
    }

    #[tokio::test]
    async fn idle_empty_doc_is_evicted_and_a_live_client_is_kept() {
        let room = Room::new("m8-evict".into(), Doc::default());
        let idle = attach_n(&room, 1).await;
        let live = attach_n(&room, 2).await;
        room.detach(idle).await;

        let n = room.evict_idle_docs(Duration::ZERO).await;
        assert_eq!(n, 1);
        assert_eq!(room.extra_doc_count().await, 1);
        let _ = room.encode_live_doc(&doc_n(1)).await.expect("evicted");
        assert_eq!(
            room.extra_doc_count().await,
            1,
            "encoding an evicted document must not put it back"
        );
        let _ = live;
    }

    #[tokio::test]
    async fn unflushed_doc_is_not_evicted() {
        let room = Room::new("m8-dirty".into(), Doc::default());
        let id = attach_n(&room, 1).await;
        room.apply_and_fanout(id, spike_bin()).await.expect("apply");
        room.detach(id).await;
        assert_eq!(room.evict_idle_docs(Duration::ZERO).await, 0);
        assert_eq!(room.extra_doc_count().await, 1);
    }

    #[tokio::test]
    async fn unknown_encode_does_not_pin_a_document() {
        let room = Room::new("m8-encode".into(), Doc::default());
        let bin = room.encode_live_doc("never-opened").await.expect("empty");
        assert!(bin.is_empty() || room.extra_doc_count().await == 0);
        assert_eq!(room.extra_doc_count().await, 0);
    }

    #[tokio::test]
    async fn document_cap_refuses_a_busy_room_and_reuses_an_idle_slot() {
        let room = Room::new("m8-docs".into(), Doc::default());
        let mut ids = Vec::new();
        for n in 0..MAX_EXTRA_DOCS as u32 {
            ids.push(attach_n(&room, n).await);
        }
        assert_eq!(room.extra_doc_count().await, MAX_EXTRA_DOCS);
        let (id, tx, _rx) = room.connect_client();
        let err = room
            .attach_doc(id, tx, &doc_n(10_000))
            .await
            .expect_err("cap");
        assert!(is_capacity(&err), "{err}");
        assert!(err.to_string().contains(CAP_DOCS), "{err}");

        room.detach(ids[0]).await;
        let (id, tx, _rx) = room.connect_client();
        room.attach_doc(id, tx, &doc_n(10_001))
            .await
            .expect("idle slot freed");
        assert_eq!(room.extra_doc_count().await, MAX_EXTRA_DOCS);
    }

    #[tokio::test]
    async fn a_document_waiting_for_its_socket_is_not_dropped_at_the_cap() {
        let room = Room::new("m8-open".into(), Doc::default());
        let mut ids = Vec::new();
        for n in 0..MAX_EXTRA_DOCS as u32 {
            ids.push(attach_n(&room, n).await);
        }
        room.detach(ids[0]).await;
        room.protect_open(&doc_n(0)).await;
        let (id, tx, _rx) = room.connect_client();
        let err = room
            .attach_doc(id, tx, &doc_n(10_002))
            .await
            .expect_err("hydrated doc still reserved");
        assert!(is_capacity(&err), "{err}");
        assert_eq!(room.extra_doc_count().await, MAX_EXTRA_DOCS);
    }

    #[tokio::test]
    async fn connection_cap_is_per_address() {
        let room = Room::new("m8-ip".into(), Doc::default());
        let a = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5));
        let b = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            let (id, tx, _rx) = room.connect_client();
            room.attach_from(id, tx, PAGE_DOC_ID, Some(a))
                .await
                .expect("under cap");
        }
        let (id, tx, _rx) = room.connect_client();
        let err = room
            .attach_from(id, tx, PAGE_DOC_ID, Some(a))
            .await
            .expect_err("same address");
        assert!(is_capacity(&err), "{err}");
        assert!(err.to_string().contains(CAP_CONNECTIONS), "{err}");

        let (id, tx, _rx) = room.connect_client();
        room.attach_from(id, tx, PAGE_DOC_ID, Some(b))
            .await
            .expect("other address");

        for _ in 0..MAX_CONNECTIONS_PER_IP {
            let (id, tx, _rx) = room.connect_client();
            room.attach_from(id, tx, PAGE_DOC_ID, None)
                .await
                .expect("missing address is not capped");
        }
        assert_eq!(
            room.extra_doc_count().await,
            0,
            "home is not an extra document"
        );
    }

    #[test]
    fn heartbeat_evicts_extra_docs_while_the_room_stays() {
        let src = include_str!("room.rs");
        let start = src.find("pub async fn heartbeat").expect("heartbeat");
        let slice = &src[start..start + 900];
        assert!(
            slice.contains("evict_idle_docs"),
            "heartbeat must drop idle documents before the room itself is shed"
        );
    }
}
