//! M3.0 step-store: Postgres hydrate / flush. No WebSocket.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tokio::sync::OnceCell;
use tower::ServiceExt;
use venus_hub::blobs::blob_hash;
use venus_hub::config::database_url_from_env;
use venus_hub::db;
use venus_hub::http::{router, AppState};
use venus_hub::lease::Lease;
use venus_hub::protocol::{apply_v1, encode_doc_update, encode_v1};
use venus_hub::room::{GetRoomError, Hub, Room, OUTBOUND_BYTES};
use venus_hub::{DEFAULT_WORKSPACE_ID, PAGE_DOC_ID};
use y_octo::Doc;

/// `yjs@13.6.32` `Y.encodeStateAsUpdate` after `doc.getMap('spike').set('k','v')`.
const YJS_SPIKE_KV_HEX: &str = "0101fc92c5bf0c002801057370696b65016b0177017600";

struct TestPg {
    database_url: String,
    _container: Option<
        testcontainers_modules::testcontainers::ContainerAsync<
            testcontainers_modules::postgres::Postgres,
        >,
    >,
}

static PG: OnceCell<TestPg> = OnceCell::const_new();

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn spike_bin() -> Vec<u8> {
    from_hex(YJS_SPIKE_KV_HEX)
}

fn spike_kv(key: &str, val: &str) -> Vec<u8> {
    let d = Doc::default();
    let mut map = d.get_or_create_map("spike").expect("map");
    map.insert(key.to_string(), val).expect("insert");
    encode_v1(&d).expect("encode")
}

fn map_has(doc: &Doc, key: &str, needle: &str) -> bool {
    match doc.get_map("spike") {
        Ok(map) => format!("{:?}", map.get(key)).contains(needle),
        Err(_) => false,
    }
}

fn map_has_v(doc: &Doc) -> bool {
    map_has(doc, "k", "v")
}

fn assert_opaque_yjs(bin: &[u8]) {
    assert!(!bin.is_empty(), "expected Yjs update v1 bytes");
    assert!(
        bin[0] != b'{' && bin[0] != b'[',
        "bytes were stored as JSON blocks, not Yjs update v1: {:?}",
        String::from_utf8_lossy(&bin[..bin.len().min(32)])
    );
}

async fn start_pg() -> TestPg {
    if let Ok(database_url) = database_url_from_env() {
        let pool = db::connect(&database_url)
            .await
            .expect("connect env postgres");
        db::migrate(&pool).await.expect("migrate");
        return TestPg {
            database_url,
            _container: None,
        };
    }

    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::{runners::AsyncRunner, ImageExt};

    let container = Postgres::default()
        .with_tag("16".to_string())
        .start()
        .await
        .expect("start postgres:16 (Docker or set DATABASE_URL / POSTGRES_*)");
    let host = container
        .get_host()
        .await
        .expect("postgres host")
        .to_string();
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("postgres port")
        .to_string();
    let database_url: String = [
        "postgres://postgres:postgres@",
        host.as_str(),
        ":",
        port.as_str(),
        "/postgres?sslmode=disable",
    ]
    .concat();
    let pool = db::connect(&database_url)
        .await
        .expect("connect testcontainers postgres");
    db::migrate(&pool).await.expect("migrate");
    TestPg {
        database_url,
        _container: Some(container),
    }
}

async fn connect_fresh() -> PgPool {
    let pg = PG.get_or_init(start_pg).await;
    db::connect(&pg.database_url).await.expect("connect")
}

#[tokio::test]
async fn connect_with_honors_pool_settings() {
    let url = PG.get_or_init(start_pg).await.database_url.clone();
    let pool = db::connect_with(
        &url,
        &db::PoolSettings {
            max_connections: 2,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(5),
        },
    )
    .await
    .expect("connect_with");
    assert_eq!(pool.options().get_max_connections(), 2);
    assert_eq!(pool.options().get_min_connections(), 0);
}

fn unique_workspace() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed) as u128;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mixed = t ^ (n << 48);
    let mut id = String::with_capacity(36);
    // Trailing `format!()` is `()` to rust-analyzer on this toolchain.
    let _ = std::fmt::Write::write_fmt(
        &mut id,
        format_args!(
            "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
            (mixed >> 96) as u32,
            (mixed >> 80) as u16,
            (mixed >> 64) as u16 & 0x0fff,
            (mixed >> 48) as u16 & 0x0fff,
            mixed as u64 & 0x0000_ffff_ffff_ffff
        ),
    );
    id
}

fn uuid_like() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

#[tokio::test]
async fn push_update_then_get_doc_round_trip() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();
    assert_opaque_yjs(&bin);

    db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
        .await
        .expect("pushUpdate");

    let stored: (Vec<u8>,) =
        sqlx::query_as("SELECT bin FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid")
            .bind(&ws)
            .bind(PAGE_DOC_ID)
            .fetch_one(&pool)
            .await
            .expect("crdt_update row");
    assert_eq!(
        stored.0, bin,
        "persist must keep opaque Yjs bytes, not JSON"
    );

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID).await.expect("getDoc");
    assert_opaque_yjs(&out);

    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply getDoc (same as Y.applyUpdate v1)");
    assert!(map_has_v(&b), "getMap('spike').get('k') should be v");
}

#[tokio::test]
async fn get_doc_after_new_connection_keeps_value() {
    let ws = unique_workspace();
    let bin = spike_bin();

    {
        let pool = connect_fresh().await;
        db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
            .await
            .expect("pushUpdate");
        pool.close().await;
    }

    let pool = connect_fresh().await;
    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after new connection");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(
        map_has_v(&b),
        "hydrate must come from Postgres, not a kept RAM doc"
    );
}

#[tokio::test]
async fn advisory_lock_key_folds_both_uuids() {
    let pool = connect_fresh().await;
    let ws_a = unique_workspace();
    let ws_b = unique_workspace();
    let doc_b = unique_workspace();
    let (a,): (i64,) = sqlx::query_as("SELECT venus_doc_lock_key($1::uuid, $2::uuid)")
        .bind(&ws_a)
        .bind(PAGE_DOC_ID)
        .fetch_one(&pool)
        .await
        .expect("lock a");
    let (a2,): (i64,) = sqlx::query_as("SELECT venus_doc_lock_key($1::uuid, $2::uuid)")
        .bind(&ws_a)
        .bind(PAGE_DOC_ID)
        .fetch_one(&pool)
        .await
        .expect("lock a again");
    let (b,): (i64,) = sqlx::query_as("SELECT venus_doc_lock_key($1::uuid, $2::uuid)")
        .bind(&ws_b)
        .bind(PAGE_DOC_ID)
        .fetch_one(&pool)
        .await
        .expect("lock b");
    let (c,): (i64,) = sqlx::query_as("SELECT venus_doc_lock_key($1::uuid, $2::uuid)")
        .bind(&ws_a)
        .bind(&doc_b)
        .fetch_one(&pool)
        .await
        .expect("lock other doc");
    assert_eq!(a, a2, "same pair must be stable");
    assert_ne!(a, b, "distinct workspace_id must fold differently");
    assert_ne!(a, c, "distinct doc_id must fold differently");
}

/// L13: a garbage trail row must not fail hydrate; both good keys survive.
#[tokio::test]
async fn get_doc_skips_corrupt_trail_bin_keeps_good_keys() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let first = spike_bin();
    let garbage = b"not-a-yjs-update".to_vec();
    let second = {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("other".to_string(), "w").expect("insert");
        encode_v1(&d).expect("encode")
    };

    db::push_update(&pool, &ws, PAGE_DOC_ID, &first)
        .await
        .expect("first");
    sqlx::query("INSERT INTO crdt_update (workspace_id, doc_id, bin) VALUES ($1::uuid, $2::uuid, $3)")
        .bind(&ws)
        .bind(PAGE_DOC_ID)
        .bind(&garbage)
        .execute(&pool)
        .await
        .expect("garbage row");
    db::push_update(&pool, &ws, PAGE_DOC_ID, &second)
        .await
        .expect("second");

    let (doc, trail_len) = db::hydrate_with_trail_len(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("hydrate must skip the bad bin");
    assert_eq!(
        trail_len, 3,
        "skipped bins still count in trail_len so compact can merge past them"
    );
    assert!(map_has_v(&doc), "first good key must survive");
    assert!(
        map_has(&doc, "other", "w"),
        "update after the garbage row must still apply"
    );

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("get_doc after garbage trail");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply get_doc");
    assert!(map_has_v(&b), "get_doc must keep spike.k=v");
    assert!(
        map_has(&b, "other", "w"),
        "get_doc must keep the key after the garbage row"
    );

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid AND bin = $3",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .bind(&garbage)
    .fetch_one(&pool)
    .await
    .expect("garbage still stored");
    assert_eq!(n, 1, "hydrate must not DELETE the bad row");
}

/// L1: compact merges the trail; get_doc still has spike.k=v.
#[tokio::test]
async fn compact_merges_trail_get_doc_keeps_value() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();
    const THRESHOLD: i64 = 32;
    for _ in 0..THRESHOLD {
        db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
            .await
            .expect("pushUpdate");
    }

    let did = db::compact(&pool, &ws, PAGE_DOC_ID, THRESHOLD)
        .await
        .expect("compact");
    assert!(did.merged, "trail length ≥ threshold must compact");
    assert_eq!(
        did.trail_len, 0,
        "compact must delete merged crdt_update rows"
    );

    let (trail,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("trail count");
    assert_eq!(trail, 0, "compact must delete merged crdt_update rows");

    let (snap,): (Vec<u8>,) =
        sqlx::query_as("SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid")
            .bind(&ws)
            .bind(PAGE_DOC_ID)
            .fetch_one(&pool)
            .await
            .expect("crdt_snapshot row");
    assert_opaque_yjs(&snap);

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after compact");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(
        map_has_v(&b),
        "compact must keep spike.k=v in the snapshot, not drop the trail"
    );
}

/// L1: hydrate during compact must not glue an old snapshot to an empty trail.
#[tokio::test(flavor = "multi_thread")]
async fn get_doc_during_compact_does_not_drop_value() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();
    const THRESHOLD: i64 = 32;
    for _ in 0..THRESHOLD {
        db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
            .await
            .expect("pushUpdate");
    }

    let mut hydrators = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        let ws = ws.clone();
        hydrators.push(tokio::spawn(async move {
            for _ in 0..16 {
                let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
                    .await
                    .expect("get_doc during compact");
                let mut d = Doc::default();
                apply_v1(&mut d, &out).expect("apply");
                assert!(map_has_v(&d), "hydrate vs compact must not drop spike.k=v");
            }
        }));
    }

    let compacted = db::compact(&pool, &ws, PAGE_DOC_ID, THRESHOLD)
        .await
        .expect("compact");
    assert!(compacted.merged);

    for h in hydrators {
        h.await.expect("hydrator");
    }

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after race");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(map_has_v(&b), "value must remain after concurrent compact");
}

/// P11: an INSERT after the read tx commits must skip the write so we do not
/// DELETE a prefix against a trail that has moved. The next compact merges.
#[tokio::test]
async fn compact_skips_write_when_trail_moves_during_merge() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();
    const THRESHOLD: i64 = 8;
    for _ in 0..THRESHOLD {
        db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
            .await
            .expect("pushUpdate");
    }

    let inject_pool = pool.clone();
    let inject_ws = ws.clone();
    let extra = bin.clone();
    let first = db::compact_after_load(&pool, &ws, PAGE_DOC_ID, THRESHOLD, move |_max| {
        let pool = inject_pool;
        let ws = inject_ws;
        async move {
            db::push_update(&pool, &ws, PAGE_DOC_ID, &extra)
                .await
                .expect("insert during merge gap");
        }
    })
    .await
    .expect("compact with moved trail");
    assert!(
        !first.merged,
        "MAX(seq) changed in the merge gap; must not write"
    );
    assert_eq!(
        first.trail_len,
        THRESHOLD + 1,
        "skip must leave the whole trail, including the new row"
    );

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("trail still present");
    assert_eq!(n, THRESHOLD + 1);

    let retry = db::compact(&pool, &ws, PAGE_DOC_ID, THRESHOLD)
        .await
        .expect("retry compact");
    assert!(retry.merged, "stable trail must merge on the next attempt");
    assert_eq!(retry.trail_len, 0);

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after retry");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(map_has_v(&b), "retry compact must keep spike.k=v");
}

/// L2: SQL error must put bins back; the next flush still hydrates spike.k=v.
#[tokio::test]
async fn flush_put_back_on_sql_error_then_succeeds() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let room = Room::new(ws.clone(), Doc::default());
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");
    let frame = encode_doc_update(spike_bin()).expect("encode Update");
    room.handle_binary(id, &frame).await;

    let dead = db::connect(&PG.get().expect("PG started").database_url)
        .await
        .expect("reconnect");
    dead.close().await;
    room.flush(&dead)
        .await
        .expect_err("flush against a closed pool must fail");

    room.flush(&pool).await.expect("retry flush after put-back");
    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after retry flush");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(
        map_has_v(&b),
        "bins dropped on SQL error would never reach Postgres"
    );
}

/// L8: aborting flush while it waits on SQL must leave the persist buffer.
/// A 1-connection pool with the only connection held blocks `flush_updates`
/// after the clone (old `mem::take` would already have emptied the Vec).
#[tokio::test(flavor = "multi_thread")]
async fn flush_abort_during_sql_then_leftover_succeeds() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    sqlx::query("DROP TRIGGER IF EXISTS venus_test_l8_sleep_trg ON crdt_update")
        .execute(&pool)
        .await
        .ok();

    let url = PG.get().expect("PG started").database_url.clone();
    let tight = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("1-conn pool");
    let hold = tight.acquire().await.expect("hold the only connection");

    let room = Arc::new(Room::new(ws.clone(), Doc::default()));
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");
    room.handle_binary(id, &encode_doc_update(spike_bin()).unwrap())
        .await;

    let flush_room = Arc::clone(&room);
    let flush_pool = tight.clone();
    let handle = tokio::spawn(async move { flush_room.flush(&flush_pool).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !handle.is_finished(),
        "flush must be waiting for the held pool connection"
    );
    handle.abort();
    let join = handle.await;
    assert!(
        join.as_ref().map_or_else(|e| e.is_cancelled(), |_| false),
        "aborted flush must be cancelled, not Ok: {join:?}"
    );
    drop(hold);

    let missing = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after abort");
    let mut aborted = Doc::default();
    apply_v1(&mut aborted, &missing).expect("apply");
    assert!(
        !map_has_v(&aborted),
        "INSERT must not have run while the pool connection was held"
    );

    room.flush(&pool)
        .await
        .expect("leftover flush after abort must still have the bins");
    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after leftover flush");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(
        map_has_v(&b),
        "mem::take before INSERT would lose the batch on abort"
    );
}

/// P4: one statement for the whole batch. `seq` must follow array order, and
/// the dirty trigger must still fire per row (`dirty.clock` = last seq).
#[tokio::test]
async fn flush_batch_keeps_array_order_and_marks_dirty() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    // Opaque to flush_updates; distinct bytes are what makes order visible.
    let bins: Vec<Vec<u8>> = (1..=5u8).map(|i| vec![i; i as usize]).collect();

    let views: Vec<&[u8]> = bins.iter().map(|b| b.as_slice()).collect();
    let seqs = db::flush_updates(&pool, &ws, PAGE_DOC_ID, &views)
        .await
        .expect("flush batch");
    assert_eq!(seqs.len(), bins.len(), "one row per bin");
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "seq must ascend in array order: {seqs:?}"
    );

    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT bin FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid ORDER BY seq",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_all(&pool)
    .await
    .expect("trail rows");
    let stored: Vec<Vec<u8>> = rows.into_iter().map(|r| r.0).collect();
    assert_eq!(
        stored, bins,
        "hydrate reads ORDER BY seq; order must survive"
    );

    let (clock,): (i64,) =
        sqlx::query_as("SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid")
            .bind(&ws)
            .bind(PAGE_DOC_ID)
            .fetch_one(&pool)
            .await
            .expect("dirty row");
    assert_eq!(
        clock,
        *seqs.last().expect("seqs"),
        "trigger is FOR EACH ROW; the last row wins"
    );
}

/// P3: below threshold, compact_if_needed must not hit SQL (closed pool is fine).
#[tokio::test]
async fn compact_if_needed_skips_sql_when_trail_short() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let room = Room::new(ws.clone(), Doc::default());
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");
    room.handle_binary(id, &encode_doc_update(spike_bin()).unwrap())
        .await;
    room.flush(&pool).await.expect("flush one bin");
    assert_eq!(room.trail_len(), 1);

    let dead = db::connect(&PG.get().expect("PG started").database_url)
        .await
        .expect("reconnect");
    dead.close().await;
    let merged = room
        .compact_if_needed(&dead, 32)
        .await
        .expect("short trail must skip compact SQL");
    assert!(!merged);
    assert_eq!(room.trail_len(), 1);
}

/// `compact_after < 1` must not restore pre-P3 "compact every tick".
#[tokio::test]
async fn compact_if_needed_skips_when_threshold_below_one() {
    let _pool = connect_fresh().await;
    let ws = unique_workspace();
    let room = Room::with_trail_len(ws, Doc::default(), 99);
    let dead = db::connect(&PG.get().expect("PG started").database_url)
        .await
        .expect("reconnect");
    dead.close().await;
    let merged = room
        .compact_if_needed(&dead, 0)
        .await
        .expect("threshold 0 must skip, not compact");
    assert!(!merged);
    assert_eq!(room.trail_len(), 99);
}
#[tokio::test]
async fn compact_if_needed_sql_rebuild_merges_trail() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let room = Room::new(ws.clone(), Doc::default());
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");
    const THRESHOLD: i64 = 32;
    let frame = encode_doc_update(spike_bin()).expect("encode");
    for _ in 0..THRESHOLD {
        room.handle_binary(id, &frame).await;
    }
    room.flush(&pool).await.expect("flush trail");
    assert_eq!(room.trail_len(), THRESHOLD as u64);

    let merged = room
        .compact_if_needed(&pool, THRESHOLD)
        .await
        .expect("compact_if_needed");
    assert!(merged, "trail_len ≥ threshold must compact");
    assert_eq!(room.trail_len(), 0);

    let (trail,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("trail count");
    assert_eq!(trail, 0);

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after SQL compact");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(map_has_v(&b), "SQL rebuild compact must keep spike.k=v");
}

/// L7: a trail row this process never applied must survive compact (SQL merge).
#[tokio::test]
async fn compact_if_needed_keeps_foreign_trail_row() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let room = Room::new(ws.clone(), Doc::default());
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");
    const THRESHOLD: i64 = 32;
    let frame = encode_doc_update(spike_bin()).expect("encode");
    for _ in 0..THRESHOLD {
        room.handle_binary(id, &frame).await;
    }
    room.flush(&pool).await.expect("flush own trail");

    let foreign = {
        let d = Doc::default();
        let mut map = d.get_or_create_map("spike").expect("map");
        map.insert("other".to_string(), "foreign").expect("insert");
        encode_v1(&d).expect("encode foreign")
    };
    db::push_update(&pool, &ws, PAGE_DOC_ID, &foreign)
        .await
        .expect("foreign owner flushed");

    let merged = room
        .compact_if_needed(&pool, THRESHOLD)
        .await
        .expect("compact_if_needed");
    assert!(merged);

    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after compact with foreign row");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(map_has_v(&b), "own spike.k=v must remain");
    assert!(
        map_has(&b, "other", "foreign"),
        "foreign trail row must be merged into the snapshot, not deleted"
    );
}

const HELLO_BLOB: &[u8] = b"hello-blob";
const HELLO_BLOB_HASH: &str = "V6JWxl21rxj4oEx6Qt8mwwCd0BOB-6qux7Qy_DDUyNA=";

/// P5: HEAD uses octet_length; GET still loads bytes. Known BlockSuite hash.
#[tokio::test]
async fn blob_put_get_head_len_without_loading_body() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let hash = blob_hash(HELLO_BLOB);
    assert_eq!(hash, HELLO_BLOB_HASH);

    let existed = db::put_blob(&pool, &ws, &hash, HELLO_BLOB)
        .await
        .expect("put_blob");
    assert!(!existed, "first put must insert");

    let got = db::get_blob(&pool, &ws, &hash)
        .await
        .expect("get_blob")
        .expect("row");
    assert_eq!(got, HELLO_BLOB);

    let len = db::blob_len(&pool, &ws, &hash)
        .await
        .expect("blob_len")
        .expect("row");
    assert_eq!(len, HELLO_BLOB.len() as i64);

    let missing = db::blob_len(&pool, &ws, "no-such-hash")
        .await
        .expect("missing len");
    assert!(missing.is_none());

    let lease = Lease::new(pool.clone(), "owner-blob", Duration::from_secs(20));
    let hub = Hub::new(pool, lease, Duration::from_secs(1), 32);
    let app = router(AppState::new(hub));
    let uri = ["/api/blobs/", ws.as_str(), "/", hash.as_str()].concat();

    let head = app
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri(&uri)
                .body(Body::empty())
                .expect("HEAD"),
        )
        .await
        .expect("HEAD blob");
    assert_eq!(head.status(), StatusCode::OK);
    let cl = head
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .expect("Content-Length");
    assert_eq!(cl, "10");
    let head_body = head
        .into_body()
        .collect()
        .await
        .expect("HEAD body")
        .to_bytes();
    assert!(head_body.is_empty(), "HEAD must not send the blob body");

    let get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .expect("GET"),
        )
        .await
        .expect("GET blob");
    assert_eq!(get.status(), StatusCode::OK);
    let cc = get
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .expect("Cache-Control");
    assert_eq!(cc, "public, max-age=31536000, immutable");
    let etag = get
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .expect("ETag");
    let mut quoted = String::from("\"");
    quoted.push_str(hash.as_str());
    quoted.push('"');
    assert_eq!(etag, quoted);
    let get_body = get
        .into_body()
        .collect()
        .await
        .expect("GET body")
        .to_bytes();
    assert_eq!(get_body.as_ref(), HELLO_BLOB);

    let not_mod = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .header(header::IF_NONE_MATCH, &quoted)
                .body(Body::empty())
                .expect("GET inm"),
        )
        .await
        .expect("GET If-None-Match");
    assert_eq!(not_mod.status(), StatusCode::NOT_MODIFIED);
    let nm_body = not_mod
        .into_body()
        .collect()
        .await
        .expect("304 body")
        .to_bytes();
    assert!(nm_body.is_empty(), "304 must not send the blob");

    let miss = app
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri(["/api/blobs/", ws.as_str(), "/missing"].concat())
                .body(Body::empty())
                .expect("HEAD miss"),
        )
        .await
        .expect("HEAD missing");
    assert_eq!(miss.status(), StatusCode::NOT_FOUND);
}

fn cors_preflight(origin: &str, method: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri(["/api/blobs/", DEFAULT_WORKSPACE_ID, "/x"].concat())
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
        .body(Body::empty())
        .expect("OPTIONS")
}

/// S9: listed origin can preflight DELETE; PUT is not allowed; request Origin is not echoed.
#[tokio::test]
async fn cors_preflight_is_allowlist_not_any_method() {
    let pool = connect_fresh().await;
    let lease = Lease::new(
        pool.clone(),
        ["s9-cors-", &uuid_like()].concat(),
        Duration::from_secs(20),
    );
    let hub = Hub::new(pool, lease, Duration::from_secs(1), 32);
    let app = router(AppState::new(hub));

    let allowed = "http://localhost:5174";
    let del = app
        .clone()
        .oneshot(cors_preflight(allowed, "DELETE"))
        .await
        .expect("DELETE preflight");
    assert_eq!(del.status(), StatusCode::OK);
    let acao = del
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok())
        .expect("Allow-Origin");
    assert_eq!(acao, allowed);
    let methods = del
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_uppercase();
    assert!(methods.contains("DELETE"), "{methods}");
    assert!(methods.contains("POST"), "{methods}");
    assert!(methods.contains("GET"), "{methods}");
    assert!(methods.contains("HEAD"), "{methods}");
    assert!(!methods.contains("PUT"), "{methods}");
    assert!(!methods.contains("PATCH"), "{methods}");

    let put = app
        .clone()
        .oneshot(cors_preflight(allowed, "PUT"))
        .await
        .expect("PUT preflight");
    let put_methods = put
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_uppercase();
    assert!(
        !put_methods.contains("PUT"),
        "PUT must not be in Allow-Methods: {put_methods}"
    );

    let evil = "http://evil.example";
    let foreign = app
        .oneshot(cors_preflight(evil, "DELETE"))
        .await
        .expect("foreign preflight");
    let echoed = foreign
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert_ne!(echoed, Some(evil), "must not echo the request Origin");
}

/// S9: empty `HUB_CORS_ORIGINS` ships no CORS (same-origin nginx).
#[tokio::test]
async fn cors_empty_list_has_no_allow_origin() {
    let pool = connect_fresh().await;
    let lease = Lease::new(
        pool.clone(),
        ["s9-empty-", &uuid_like()].concat(),
        Duration::from_secs(20),
    );
    let hub = Hub::new(pool, lease, Duration::from_secs(1), 32);
    let app = router(AppState::with_cors(hub, Vec::new()));
    let pre = app
        .oneshot(cors_preflight("http://localhost:5173", "DELETE"))
        .await
        .expect("OPTIONS");
    assert!(
        pre.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "empty origin list must not set Allow-Origin"
    );
}

/// S2: garbage `{workspace_id}` is 400 before blob/export SQL.
#[tokio::test]
async fn bad_workspace_id_blob_and_export_are_400() {
    let pool = connect_fresh().await;
    let lease = Lease::new(pool.clone(), "owner-s2-blob", Duration::from_secs(20));
    let hub = Hub::new(pool.clone(), lease, Duration::from_secs(1), 32);
    let app = router(AppState::new(hub));

    let post = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/blobs/bad%20id")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(HELLO_BLOB.to_vec()))
                .expect("POST"),
        )
        .await
        .expect("POST blob");
    assert_eq!(post.status(), StatusCode::BAD_REQUEST);
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM blob WHERE workspace_id::text = $1")
        .bind("bad id")
        .fetch_one(&pool)
        .await
        .expect("blob count");
    assert_eq!(n, 0, "invalid id must not insert a blob row");

    let export = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/block/bad%20id/export")
                .body(Body::empty())
                .expect("GET export"),
        )
        .await
        .expect("GET export");
    assert_eq!(export.status(), StatusCode::BAD_REQUEST);
}

async fn lease_xmin(pool: &PgPool, workspace_id: &str) -> String {
    let (xmin,): (String,) =
        sqlx::query_as("SELECT xmin::text FROM workspace_lease WHERE workspace_id = $1::uuid")
            .bind(workspace_id)
            .fetch_one(pool)
            .await
            .expect("lease xmin");
    xmin
}

/// L10 + P12: a statement error is not a steal; stolen/missing ids are the
/// difference against `RETURNING` of one `UPDATE … ANY($2)`.
#[tokio::test]
async fn heartbeat_many_continues_after_per_id_sql_error() {
    let pool = connect_fresh().await;
    let fail_id = unique_workspace();
    let ok_id = unique_workspace();
    let missing_id = unique_workspace();
    let owner = ["l10-owner-", &uuid_like()].concat();
    let other_owner = ["l10-other-", &uuid_like()].concat();
    let fn_name = ["l10_fail_", &uuid_like()].concat();
    let lease = Lease::new(pool.clone(), owner, Duration::from_secs(20));
    let other = Lease::new(pool.clone(), other_owner, Duration::from_secs(20));
    lease.try_acquire(&fail_id).await.expect("acquire fail id");
    lease.try_acquire(&ok_id).await.expect("acquire ok id");

    let fail_lit = fail_id.replace('\'', "''");
    sqlx::query(&format!(
        "CREATE FUNCTION {fn_name}() RETURNS trigger AS $$
             BEGIN
               IF NEW.workspace_id = '{fail_lit}' THEN
                 RAISE EXCEPTION 'l10 injected heartbeat failure';
               END IF;
               RETURN NEW;
             END;
             $$ LANGUAGE plpgsql"
    ))
    .execute(&pool)
    .await
    .expect("create fail function");
    sqlx::query(&format!(
        "CREATE TRIGGER {fn_name} BEFORE UPDATE ON workspace_lease
         FOR EACH ROW EXECUTE FUNCTION {fn_name}()"
    ))
    .execute(&pool)
    .await
    .expect("create fail trigger");

    let ok_xmin_before = lease_xmin(&pool, &ok_id).await;
    let fail_xmin_before = lease_xmin(&pool, &fail_id).await;
    let missed = lease
        .heartbeat_many(&[fail_id.clone(), ok_id.clone()])
        .await;

    let drop_trigger = sqlx::query(&format!(
        "DROP TRIGGER IF EXISTS {fn_name} ON workspace_lease"
    ))
    .execute(&pool)
    .await;
    let drop_fn = sqlx::query(&format!("DROP FUNCTION IF EXISTS {fn_name}()"))
        .execute(&pool)
        .await;
    drop_trigger.expect("drop trigger");
    drop_fn.expect("drop function");

    assert!(
        missed.is_empty(),
        "SQL error must not be reported as a steal: {missed:?}"
    );
    assert_eq!(
        lease_xmin(&pool, &fail_id).await,
        fail_xmin_before,
        "failed UPDATE must not extend the first lease"
    );
    assert_eq!(
        lease_xmin(&pool, &ok_id).await,
        ok_xmin_before,
        "one-statement heartbeat rolls back the whole tick on SQL error"
    );

    sqlx::query(
        "UPDATE workspace_lease SET lease_until = now() - interval '1 second' WHERE workspace_id = $1::uuid",
    )
    .bind(&fail_id)
    .execute(&pool)
    .await
    .expect("expire fail id");
    other
        .try_acquire(&fail_id)
        .await
        .expect("other owner steals fail id");

    let ok_xmin_before = lease_xmin(&pool, &ok_id).await;
    let missed = lease
        .heartbeat_many(&[fail_id.clone(), ok_id.clone(), missing_id.clone()])
        .await;
    assert_eq!(
        missed,
        vec![fail_id.clone(), missing_id.clone()],
        "one statement returns stolen and missing ids, not the ones we still own"
    );
    assert_ne!(
        lease_xmin(&pool, &ok_id).await,
        ok_xmin_before,
        "owned workspace must refresh in the same statement"
    );
}

/// P14: the leftover btree matching the primary key must not survive migrate.
#[tokio::test]
async fn migrate_drops_duplicate_crdt_update_index() {
    let pool = connect_fresh().await;
    let (dup,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM pg_indexes WHERE indexname = 'crdt_update_ws_doc_seq'",
    )
    .fetch_one(&pool)
    .await
    .expect("dup count");
    let (pkey,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM pg_indexes WHERE indexname = 'crdt_update_pkey'",
    )
    .fetch_one(&pool)
    .await
    .expect("pkey count");
    assert_eq!(dup, 0, "crdt_update_ws_doc_seq is a duplicate of the PK");
    assert_eq!(pkey, 1, "the primary key index stays");
}

/// L17: a snapshot bin that will not apply must not brick the room. Hydrate
/// falls back to the trail; compact stays fail-closed so the row survives for
/// an operator instead of being overwritten by a trail-only rebuild.
#[tokio::test]
async fn hydrate_skips_corrupt_snapshot_and_keeps_the_row() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let garbage = b"not-a-yjs-snapshot".to_vec();
    sqlx::query(
        "INSERT INTO crdt_snapshot (workspace_id, doc_id, bin, clock) VALUES ($1::uuid, $2::uuid, $3, 0)",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .bind(&garbage)
    .execute(&pool)
    .await
    .expect("garbage snapshot");
    db::push_update(&pool, &ws, PAGE_DOC_ID, &spike_bin())
        .await
        .expect("trail on top of the bad snapshot");

    let (doc, trail_len) = db::hydrate_with_trail_len(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("hydrate must skip the bad snapshot, not 500 forever");
    assert_eq!(trail_len, 1);
    assert!(
        map_has_v(&doc),
        "trail must still hydrate when the snapshot is unreadable"
    );

    assert!(
        db::compact(&pool, &ws, PAGE_DOC_ID, 1).await.is_err(),
        "compact must stay fail-closed on a corrupt snapshot"
    );
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_snapshot WHERE workspace_id = $1::uuid AND bin = $2",
    )
    .bind(&ws)
    .bind(&garbage)
    .fetch_one(&pool)
    .await
    .expect("snapshot count");
    assert_eq!(
        n, 1,
        "the bad snapshot is an operator's call: do not delete or rebuild over it"
    );
}

/// L16: `open_room` acquires the lease before hydrating. If hydrate fails the
/// lease must go back, or nothing heartbeats it and retries renew it — 503ing a
/// healthy hub for a workspace no one serves. Lease pool is live, hub pool is
/// closed, so `try_acquire` succeeds and hydrate cannot.
#[tokio::test]
async fn get_room_hydrate_failure_releases_the_lease() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let owner = ["l16-owner-", &uuid_like()].concat();

    let dead = db::connect(&PG.get().expect("PG started").database_url)
        .await
        .expect("reconnect");
    dead.close().await;
    let hub = Hub::new(
        dead,
        Lease::new(pool.clone(), owner.clone(), Duration::from_secs(20)),
        Duration::from_secs(1),
        32,
    );

    assert!(
        hub.get_room(&ws).await.is_err(),
        "hydrate on a closed pool must fail"
    );

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM workspace_lease WHERE workspace_id = $1::uuid AND owner = $2",
    )
    .bind(&ws)
    .bind(&owner)
    .fetch_one(&pool)
    .await
    .expect("lease count");
    assert_eq!(n, 0, "a failed open must not keep owning the workspace");

    let other_owner = ["l16-other-", &uuid_like()].concat();
    let other = Lease::new(pool.clone(), other_owner, Duration::from_secs(20));
    other
        .try_acquire(&ws)
        .await
        .expect("another hub must not wait out a TTL for a failed open");
    other.drop_all().await.expect("cleanup");
}

/// L12: abort the heartbeat task, then drop leases (same path as a serve error).
#[tokio::test]
async fn shutdown_with_heartbeat_aborts_then_drops_lease() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let owner = ["l12-owner-", &uuid_like()].concat();
    let lease = Lease::new(pool.clone(), owner.clone(), Duration::from_secs(20));
    let hub = Hub::new(pool.clone(), lease.clone(), Duration::from_secs(1), 32);
    lease.try_acquire(&ws).await.expect("acquire");

    let hb_lease = lease.clone();
    let hb_ids = vec![ws.clone()];
    let heartbeat = tokio::spawn(async move {
        loop {
            let _ = hb_lease.heartbeat_many(&hb_ids).await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    hub.shutdown_with_heartbeat(heartbeat).await;
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM workspace_lease WHERE owner = $1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .expect("lease count");
    assert_eq!(
        n, 0,
        "serve-error drain must drop leases after aborting heartbeat"
    );
}

/// P9: N concurrent `get_room` on one cold `workspace_id` share one `Arc<Room>`.
/// P8: that room owns the persist `JoinHandle` (abort takes it; no Hub Vec).
#[tokio::test]
async fn concurrent_get_room_single_flight_per_workspace() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let other = unique_workspace();
    let owner = ["p9-owner-", &uuid_like()].concat();
    let hub = Hub::new(
        pool.clone(),
        Lease::new(pool, owner, Duration::from_secs(20)),
        Duration::from_secs(1),
        32,
    );

    const N: usize = 8;
    let start = Arc::new(tokio::sync::Barrier::new(N));
    let mut tasks = Vec::with_capacity(N);
    for _ in 0..N {
        let hub = Arc::clone(&hub);
        let ws = ws.clone();
        let start = Arc::clone(&start);
        tasks.push(tokio::spawn(async move {
            start.wait().await;
            hub.get_room(&ws).await.expect("get_room")
        }));
    }

    let mut rooms = Vec::with_capacity(N);
    for task in tasks {
        rooms.push(task.await.expect("join"));
    }
    let first = Arc::clone(&rooms[0]);
    for room in &rooms {
        assert!(
            Arc::ptr_eq(&first, room),
            "P9: concurrent get_room must return the same Room"
        );
    }
    assert_eq!(
        hub.hydrate_attempts(),
        1,
        "P9: one hydrate per workspace_id, not one per waiter"
    );
    assert!(
        first.persist_task_is_some(),
        "P8: persist JoinHandle lives on the Room"
    );

    let other_room = hub.get_room(&other).await.expect("other workspace");
    assert!(
        !Arc::ptr_eq(&first, &other_room),
        "P9: flight key is workspace_id, not process-wide"
    );
    assert_eq!(hub.hydrate_attempts(), 2);

    hub.abort_persist_no_flush().await;
    assert!(
        !first.persist_task_is_some(),
        "P8: abort takes the handle off the Room"
    );
    assert!(!other_room.persist_task_is_some());
    hub.shutdown().await;
}

/// S7: N concurrent cold exports share one SQL hydrate+encode.
#[tokio::test]
async fn concurrent_cold_export_is_single_flight() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let other = unique_workspace();
    db::push_update(&pool, &ws, PAGE_DOC_ID, &spike_bin())
        .await
        .expect("seed");
    let hub = Hub::new(
        pool.clone(),
        Lease::new(
            pool,
            ["s7-owner-", &uuid_like()].concat(),
            Duration::from_secs(20),
        ),
        Duration::from_secs(1),
        32,
    );

    const N: usize = 8;
    let start = Arc::new(tokio::sync::Barrier::new(N));
    let mut tasks = Vec::with_capacity(N);
    for _ in 0..N {
        let hub = Arc::clone(&hub);
        let ws = ws.clone();
        let start = Arc::clone(&start);
        tasks.push(tokio::spawn(async move {
            start.wait().await;
            hub.live_export(&ws).await.expect("export")
        }));
    }
    let mut outs = Vec::with_capacity(N);
    for task in tasks {
        outs.push(task.await.expect("join"));
    }
    assert_eq!(
        hub.export_sql_attempts(),
        1,
        "S7: one cold export SQL per workspace_id, not one per waiter"
    );
    let first = &outs[0];
    for bin in &outs {
        assert_eq!(bin, first, "waiters must share the same export bytes");
    }
    let mut doc = Doc::default();
    apply_v1(&mut doc, first).expect("apply");
    assert!(map_has_v(&doc), "cold export must still hydrate the spike");

    let _ = hub.live_export(&other).await.expect("other wiki");
    assert_eq!(
        hub.export_sql_attempts(),
        2,
        "S7: flight key is workspace_id"
    );
    hub.shutdown().await;
}

#[tokio::test]
async fn get_room_after_shutdown_is_store_and_does_not_acquire() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let owner = ["l19-owner-", &uuid_like()].concat();
    let hub = Hub::new(
        pool.clone(),
        Lease::new(pool.clone(), owner.clone(), Duration::from_secs(20)),
        Duration::from_secs(1),
        32,
    );
    hub.shutdown().await;
    hub.shutdown().await;
    match hub.get_room(&ws).await {
        Err(GetRoomError::Store(e)) => {
            let msg = e.to_string();
            assert!(
                msg.contains("shutting down"),
                "Store must name the gate: {msg}"
            );
        }
        Ok(_) => panic!("get_room after shutdown must not open a room"),
        Err(GetRoomError::Held { .. }) => panic!("shutting down is Store, not Held"),
    }
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM workspace_lease WHERE owner = $1")
            .bind(&owner)
            .fetch_one(&pool)
            .await
            .expect("lease count");
    assert_eq!(n, 0, "shutdown gate must not try_acquire");
}

/// L20: two migrates on one pool must not fail the DDL lock race.
/// `tokio::join!` (not `spawn`): sqlx `raw_sql` on `&mut PgConnection` is not `'static`.
#[tokio::test]
async fn migrate_serializes_two_hubs() {
    let pool = connect_fresh().await;
    let (a, b) = tokio::join!(db::migrate(&pool), db::migrate(&pool));
    a.expect("migrate a");
    b.expect("migrate b");
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM pg_trigger
         WHERE NOT tgisinternal
           AND tgname IN ('crdt_update_dirty', 'crdt_snapshot_dirty')",
    )
    .fetch_one(&pool)
    .await
    .expect("trigger count");
    assert_eq!(n, 2, "concurrent migrate must leave both dirty triggers");
}

/// S10: cap the persist buffer; drop oldest; newest still flushes. Not on SQL error.
#[tokio::test]
async fn persist_cap_drops_oldest_keeps_newest() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let first = spike_kv("first", "a");
    let cap = first.len() * 3;
    let room = Room::with_budgets(ws.clone(), Doc::default(), 0, OUTBOUND_BYTES, cap);
    let (id, tx, _rx) = room.connect_client();
    room.attach(id, tx).await.expect("attach");

    let dead = db::connect(&PG.get().expect("PG started").database_url)
        .await
        .expect("reconnect");
    dead.close().await;

    let mut last = String::new();
    for i in 0..20u32 {
        let key = ["k", &i.to_string()].concat();
        last = key.clone();
        let frame = encode_doc_update(spike_kv(&key, "v")).expect("encode");
        room.handle_binary(id, &frame).await;
        let queued = room.persist_queued_bytes().await;
        assert!(
            queued <= cap + first.len(),
            "over-cap is only a single newest bin: queued={queued} cap={cap}"
        );
    }
    assert!(
        room.persist_queued_bytes().await <= cap + first.len(),
        "closed-pool applies must still respect the byte cap"
    );
    room.flush(&dead)
        .await
        .expect_err("flush against a closed pool must fail without dropping the cap remainder");
    assert!(
        room.persist_queued_bytes().await > 0,
        "L2: SQL error must keep bins"
    );

    room.flush(&pool).await.expect("flush newest after cap");
    let out = db::get_doc(&pool, &ws, PAGE_DOC_ID)
        .await
        .expect("getDoc after cap flush");
    let mut b = Doc::default();
    apply_v1(&mut b, &out).expect("apply");
    assert!(
        map_has(&b, &last, "v"),
        "newest update must hydrate after the cap dropped older bins"
    );
    assert!(
        !map_has(&b, "k0", "v"),
        "oldest bins past the cap must not be in SQL"
    );
}
