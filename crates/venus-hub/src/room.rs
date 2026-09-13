//! One apply queue and one persist buffer per room (`workspace_id`), not per socket.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

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

/// Queued-byte budget per client (S6). Far below `OUTBOUND_CAP × 4 MiB`
/// (`WS_MAX_MESSAGE`). Detach when a send would exceed this, even if slots remain.
pub const OUTBOUND_BYTES: usize = 8 * 1024 * 1024;

/// Persist buffer cap per room (S10). Oldest bins are dropped (loudly) when a
/// push would exceed this; the newest stays. Must be ≥ `WS_MAX_MESSAGE` so one
/// accepted frame always fits. Do not drop on SQL error (L2).
pub const PERSIST_BYTES: usize = 8 * 1024 * 1024;

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
            GetRoomError::Store(e) => write!(f, "{:#}", e.as_ref()),
        }
    }
}

impl std::error::Error for GetRoomError {}

pub struct Room {
    pub workspace_id: String,
    doc_id: String,
    /// One `Doc` per room. Reads (hello, export, Step1) share; only `apply_v1`
    /// takes a write (P10). Not a second doc or a snapshot cache.
    doc: RwLock<Doc>,
    /// Queued bins. `Bytes` so the L8 prefix clone is refcounts, not a copy (P15).
    persist: Mutex<PersistBuf>,
    persist_budget: usize,
    outbound_budget: usize,
    /// One flush at a time so leftover drain does not INSERT the same prefix
    /// while a persist-task flush is in flight.
    flush_mu: Mutex<()>,
    clients: Mutex<HashMap<ClientId, Outbound>>,
    next_client: AtomicU64,
    trail_len: AtomicU64,
    stop: AtomicBool,
    stop_notify: Notify,
    /// Join handle for this room's persist loop (P8). Shutdown/abort take it
    /// from here so there is no process-wide Vec of every room ever opened.
    persist_task: StdMutex<Option<JoinHandle<()>>>,
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
            clients: Mutex::new(HashMap::new()),
            next_client: AtomicU64::new(1),
            trail_len: AtomicU64::new(trail_len),
            stop: AtomicBool::new(false),
            stop_notify: Notify::const_new(),
            persist_task: StdMutex::new(None),
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
        let hello = Self::hello_frames(&*self.doc.read().await)?;
        self.clients.lock().await.insert(id, tx);
        Ok(hello)
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
        self.clients.lock().await.remove(&id);
    }

    pub async fn handle_binary(&self, from: ClientId, bytes: &[u8]) {
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
                let doc = self.doc.read().await;
                let update = encode_step2_for(&doc, &sv)?;
                drop(doc);
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

    async fn apply_and_fanout(&self, from: ClientId, bin: Vec<u8>) -> Result<()> {
        if is_noop_update(&bin) {
            return Ok(());
        }
        // Frame first (L21): a framing error must not leave the update in RAM
        // and the persist buffer with no peer told.
        let frame = Bytes::from(encode_doc_update(bin.clone())?);
        {
            let mut doc = self.doc.write().await;
            apply_v1(&mut doc, &bin)?;
        }
        {
            let mut buf = self.persist.lock().await;
            buf.push_capped(Bytes::from(bin), self.persist_budget, &self.workspace_id);
        }
        self.broadcast_except(from, frame).await;
        Ok(())
    }

    async fn send_to(&self, id: ClientId, frame: Bytes) {
        let mut clients = self.clients.lock().await;
        let lagged = match clients.get(&id) {
            Some(tx) => tx.try_send(frame).is_err(),
            None => false,
        };
        if lagged {
            clients.remove(&id);
            tracing::warn!(
                workspace = %self.workspace_id,
                client = id,
                "outbound full, over budget, or closed; detach"
            );
        }
    }

    async fn broadcast_except(&self, from: ClientId, frame: Bytes) {
        let mut dead = Vec::new();
        {
            let clients = self.clients.lock().await;
            for (id, tx) in clients.iter() {
                if *id == from {
                    continue;
                }
                if tx.try_send(frame.clone()).is_err() {
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
        }
    }

    pub async fn encode_live(&self) -> Result<Vec<u8>> {
        let doc = self.doc.read().await;
        encode_v1(&doc)
    }

    pub async fn flush(&self, pool: &PgPool) -> Result<()> {
        let _gate = self.flush_mu.lock().await;
        let bins = {
            let mut buf = self.persist.lock().await;
            buf.snapshot()
        };
        if bins.is_empty() {
            return Ok(());
        }
        let views: Vec<&[u8]> = bins.iter().map(|b| b.as_ref()).collect();
        if let Err(e) = db::flush_updates(pool, &self.workspace_id, &self.doc_id, &views)
            .await
            .context("persist flush")
        {
            self.persist.lock().await.abort_snapshot();
            return Err(e);
        }
        {
            let mut buf = self.persist.lock().await;
            let n = bins.len();
            if buf.bins.len() >= n && buf.bins[..n] == bins[..] {
                buf.commit_prefix(n);
            } else {
                buf.abort_snapshot();
                tracing::warn!(
                    workspace = %self.workspace_id,
                    expected = n,
                    actual = buf.bins.len(),
                    "persist prefix mismatch after flush; leaving buffer"
                );
            }
            self.trail_len.fetch_add(n as u64, Ordering::SeqCst);
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
        if self.trail_len() < threshold as u64 {
            return Ok(false);
        }
        let out = db::compact(pool, &self.workspace_id, &self.doc_id, threshold).await?;
        self.trail_len
            .store(out.trail_len.max(0) as u64, Ordering::SeqCst);
        Ok(out.merged)
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
        self.persist.lock().await.bytes
    }
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
}

impl Hub {
    pub fn new(
        pool: PgPool,
        lease: Lease,
        persist_interval: Duration,
        compact_after: i64,
    ) -> Arc<Self> {
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
        })
    }

    fn shutting_down_err() -> GetRoomError {
        GetRoomError::Store(Arc::new(anyhow::Error::msg("hub is shutting down")))
    }

    pub async fn get_room(self: &Arc<Self>, workspace_id: &str) -> Result<Arc<Room>, GetRoomError> {
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
        if let Some(room) = self.live_room(workspace_id).await {
            return room.encode_live().await;
        }
        loop {
            let follow = {
                let mut flights = self.exporting.lock().unwrap_or_else(|e| e.into_inner());
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
                    Ok(Ok(bytes)) => return Ok(bytes),
                    Ok(Err(e)) => return Err(anyhow::Error::msg(e.to_string())),
                    Err(_) => continue,
                }
            }

            let mut lead = ExportLead::new(&self.exporting, workspace_id);
            if let Some(room) = self.live_room(workspace_id).await {
                let result = encode_to_export(room.encode_live().await);
                lead.complete(&result);
                return export_to_result(result);
            }
            self.export_sql_attempts.fetch_add(1, Ordering::SeqCst);
            let result = encode_to_export(db::default_page_export(&self.pool, workspace_id).await);
            lead.complete(&result);
            return export_to_result(result);
        }
    }

    pub async fn workspace_ids(&self) -> Vec<String> {
        self.rooms.lock().await.keys().cloned().collect()
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
