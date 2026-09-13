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
use venus_hub::room::{GetRoomError, Hub, Room, OUTBOUND_BYTES, PERSIST_BYTES};
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

/// Dirty triggers are per table, not per workspace, so the tests that recreate
/// or assert them must not overlap with each other. Everything else survives a
/// stray snapshot trigger: `GREATEST` keeps the clock at `max_seq` either way.
/// Also held by the P5 flush test: `migrate` takes AccessExclusive on
/// `crdt_update`, which deadlocks against a cap-sized insert (L20).
static TRIGGER_STATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `(STATEMENT crdt_update_dirty, leftover crdt_snapshot dirty triggers)` —
/// same shape as the `db::migrate` probe.
async fn dirty_trigger_counts(pool: &PgPool) -> (i64, i64) {
    sqlx::query_as(
        "SELECT
           COUNT(*) FILTER (
             WHERE tgname = 'crdt_update_dirty' AND tgnewtable IS NOT NULL
           )::bigint,
           COUNT(*) FILTER (
             WHERE tgname IN (
               'crdt_snapshot_dirty',
               'crdt_snapshot_dirty_ins',
               'crdt_snapshot_dirty_upd'
             )
           )::bigint
         FROM pg_trigger
         WHERE NOT tgisinternal",
    )
    .fetch_one(pool)
    .await
    .expect("dirty trigger counts")
}

async fn dirty_rows(pool: &PgPool, workspace_id: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM dirty WHERE workspace_id = $1::uuid")
            .bind(workspace_id)
            .fetch_one(pool)
            .await
            .expect("dirty rows");
    n
}

async fn assert_no_jobs_row(pool: &PgPool) {
    let reg: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.jobs')::text")
        .fetch_one(pool)
        .await
        .expect("jobs regclass");
    if let Some(name) = reg {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*)::bigint FROM jobs")
            .fetch_one(pool)
            .await
            .expect("jobs count");
        assert_eq!(n, 0, "hub must not insert a {name} row");
    }
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
            work_mem: None,
        },
    )
    .await
    .expect("connect_with");
    assert_eq!(pool.options().get_max_connections(), 2);
    assert_eq!(pool.options().get_min_connections(), 0);
}

/// P5: every pooled session gets `work_mem`, and a cap-sized flush stops
/// spilling to a temp file. Both pools set it explicitly, so the comparison
/// holds whatever the server default is (`DATABASE_URL` may point anywhere).
#[tokio::test]
async fn work_mem_is_set_per_session_and_stops_the_cap_flush_spilling() {
    // Not about triggers: a cap-sized flush is slow enough to still be holding
    // `crdt_update` when a migrate test asks for AccessExclusive (L20).
    let _serial = TRIGGER_STATE.lock().await;
    let url = PG.get_or_init(start_pg).await.database_url.clone();
    let pool_for = |work_mem: &str| {
        let (url, work_mem) = (url.clone(), work_mem.to_string());
        async move {
            db::connect_with(
                &url,
                &db::PoolSettings {
                    // One, and never held across the flush: `temp_bytes` can
                    // only force *its own* backend's pending stats, so the
                    // reads and the flush have to share a connection.
                    max_connections: 1,
                    min_connections: 0,
                    acquire_timeout: Duration::from_secs(10),
                    work_mem: Some(work_mem),
                },
            )
            .await
            .expect("connect_with work_mem")
        }
    };
    // An 8 MiB batch of incompressible bins: the shape that spills.
    let mut rng = Rng::new(0x9e37_79b9_7f4a_7c15);
    let bins: Vec<Vec<u8>> = (0..PERSIST_BYTES / (64 * 1024))
        .map(|_| rng.bytes(64 * 1024))
        .collect();
    let views: Vec<&[u8]> = bins.iter().map(|b| b.as_slice()).collect();

    let mut spilled = Vec::new();
    for work_mem in ["4MB", "64MB"] {
        let pool = pool_for(work_mem).await;
        let ws = unique_workspace();
        let before = {
            let mut conn = pool.acquire().await.expect("acquire");
            let shown: String = sqlx::query_scalar("SHOW work_mem")
                .fetch_one(&mut *conn)
                .await
                .expect("SHOW work_mem");
            assert_eq!(
                shown, work_mem,
                "after_connect must set work_mem on every pooled session"
            );
            temp_bytes(&mut conn).await
        };

        db::flush_updates(&pool, &ws, PAGE_DOC_ID, &views)
            .await
            .expect("cap-sized flush");

        let mut conn = pool.acquire().await.expect("re-acquire the same backend");
        spilled.push((temp_bytes(&mut conn).await - before).max(0));
        sqlx::query("DELETE FROM crdt_update WHERE workspace_id = $1::uuid")
            .bind(&ws)
            .execute(&mut *conn)
            .await
            .expect("drop the measurement rows");
    }

    let (at_default, tuned) = (spilled[0], spilled[1]);
    assert!(
        at_default > 4 * 1024 * 1024,
        "P5: an 8 MiB flush must spill at work_mem = 4MB, saw {at_default} temp bytes"
    );
    assert!(
        tuned < 1024 * 1024,
        "P5: 64MB must keep the same flush in RAM, saw {tuned} temp bytes \
         (at 4MB it was {at_default})"
    );
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

    let stored: (Vec<u8>,) = sqlx::query_as(
        "SELECT bin FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
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
    sqlx::query(
        "INSERT INTO crdt_update (workspace_id, doc_id, bin) VALUES ($1::uuid, $2::uuid, $3)",
    )
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

    let (snap,): (Vec<u8>,) = sqlx::query_as(
        "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
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

/// D1 / P1: after `now_max` matches, a concurrent `crdt_update` must leave
/// `dirty.clock` at the newer seq. The snapshot replace has no trigger, and
/// `GREATEST` covers an out-of-order flush statement.
#[tokio::test]
async fn compact_snapshot_must_not_rewind_dirty_clock() {
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
    let newer = Arc::new(AtomicU64::new(0));
    let newer_seq = newer.clone();
    let out = db::compact_after_now_max(&pool, &ws, PAGE_DOC_ID, THRESHOLD, move |max| {
        let pool = inject_pool;
        let ws = inject_ws;
        let newer_seq = newer_seq;
        async move {
            let seq = db::push_update(&pool, &ws, PAGE_DOC_ID, &extra)
                .await
                .expect("insert after now_max");
            assert!(
                seq > max,
                "injected seq {seq} must be newer than compact max_seq {max}"
            );
            newer_seq.store(seq as u64, Ordering::SeqCst);
        }
    })
    .await
    .expect("compact after now_max");
    assert!(out.merged, "now_max matched; compact must write");
    assert_eq!(out.trail_len, 1, "injected row is seq > max_seq");

    let injected = newer.load(Ordering::SeqCst) as i64;
    let (clock,): (i64,) = sqlx::query_as(
        "SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("dirty after compact");
    assert_eq!(
        clock, injected,
        "D1: snapshot clock must not rewind dirty.clock below the newer flush seq"
    );
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

/// xorshift64*, so the P1 shapes can be incompressible without a dev-dependency
/// and still byte-identical run to run.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len + 8);
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_le_bytes());
        }
        out.truncate(len);
        out
    }
}

/// Same statement shape as `flush_updates`, against any table, on one
/// connection so the two P1 measurements share a backend and a plan cache.
async fn time_batch_insert(
    conn: &mut sqlx::PgConnection,
    table: &str,
    workspace_id: &str,
    bins: &[&[u8]],
) -> Duration {
    let sql = format!(
        "INSERT INTO {table} (workspace_id, doc_id, bin)
         SELECT $1::uuid, $2::uuid, bin FROM unnest($3::bytea[]) WITH ORDINALITY AS t(bin, ord)
         ORDER BY ord
         RETURNING seq"
    );
    let start = std::time::Instant::now();
    let seqs: Vec<i64> = sqlx::query_scalar(&sql)
        .bind(workspace_id)
        .bind(PAGE_DOC_ID)
        .bind(bins)
        .fetch_all(&mut *conn)
        .await
        .expect("timed batch insert");
    let elapsed = start.elapsed();
    assert_eq!(seqs.len(), bins.len(), "one row per bin in {table}");
    elapsed
}

/// Best of `runs` for the triggered table and the baseline, **interleaved** —
/// Docker-on-laptop drifts far more than the trigger costs, so measuring all of
/// A then all of B attributes the drift to the trigger. Min of each.
async fn ab_batch_insert(
    conn: &mut sqlx::PgConnection,
    workspace_id: &str,
    bins: &[&[u8]],
    runs: usize,
) -> (Duration, Duration) {
    // Warm-ups are not counted: first call pays plan + TOAST path setup.
    time_batch_insert(conn, "crdt_update", workspace_id, bins).await;
    time_batch_insert(conn, "p1_no_trigger", workspace_id, bins).await;
    let (mut trigger, mut baseline) = (Duration::MAX, Duration::MAX);
    for _ in 0..runs {
        trigger = trigger.min(time_batch_insert(conn, "crdt_update", workspace_id, bins).await);
        baseline = baseline.min(time_batch_insert(conn, "p1_no_trigger", workspace_id, bins).await);
    }
    (trigger, baseline)
}

/// `temp_bytes` for this database, forced out of the backend's pending stats so
/// it reflects the statement that just ran (PG15+ shared-memory stats are
/// rate-limited otherwise).
async fn temp_bytes(conn: &mut sqlx::PgConnection) -> i64 {
    sqlx::raw_sql("SELECT pg_stat_force_next_flush()")
        .execute(&mut *conn)
        .await
        .expect("flush pending stats");
    sqlx::query_scalar("SELECT temp_bytes FROM pg_stat_database WHERE datname = current_database()")
        .fetch_one(&mut *conn)
        .await
        .expect("temp_bytes")
}

/// One P1 shape: interleaved A/B timing, then one extra triggered statement
/// bracketed by `temp_bytes` to see whether `ins` spills to a temp file.
async fn measure_p1_shape(
    conn: &mut sqlx::PgConnection,
    label: &str,
    bin_len: usize,
    count: usize,
    random: bool,
    runs: usize,
) {
    let mut rng = Rng::new(0x5eed_1234_9876_abcd);
    let bins: Vec<Vec<u8>> = (0..count)
        .map(|i| {
            if random {
                rng.bytes(bin_len)
            } else {
                let fill = (i % 251) as u8;
                let mut bin = Vec::with_capacity(bin_len);
                bin.resize(bin_len, fill);
                bin
            }
        })
        .collect();
    let views: Vec<&[u8]> = bins.iter().map(|b| b.as_slice()).collect();
    // Fresh workspace per shape so `stored` describes only these rows.
    let ws = unique_workspace();

    let (with_trigger, without) = ab_batch_insert(conn, &ws, &views, runs).await;
    let overhead = with_trigger.saturating_sub(without);
    let pct = overhead.as_secs_f64() / without.as_secs_f64() * 100.0;

    let before = temp_bytes(conn).await;
    time_batch_insert(conn, "crdt_update", &ws, &views).await;
    let spilled = (temp_bytes(conn).await - before).max(0);

    let (stored,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(avg(pg_column_size(bin)), 0)::bigint
         FROM crdt_update WHERE workspace_id = $1::uuid",
    )
    .bind(&ws)
    .fetch_one(&mut *conn)
    .await
    .expect("stored size of the measured bins");

    println!(
        "  {label:<22} {count:>4} x {bin_len:>6} B = {:>5} KiB | trigger {:>8.2?} | \
         base {:>8.2?} | dirty {:>8.2?} ({pct:>3.0}%) | stored {stored:>6} B | \
         temp {:>6} KiB",
        count * bin_len / 1024,
        with_trigger,
        without,
        overhead,
        spilled / 1024,
    );

    for table in ["crdt_update", "p1_no_trigger"] {
        sqlx::query(&format!(
            "DELETE FROM {table} WHERE workspace_id = $1::uuid"
        ))
        .bind(&ws)
        .execute(&mut *conn)
        .await
        .expect("drop the measurement rows");
    }
}

/// P1 (open half): `crdt_update_dirty` needs `REFERENCING NEW TABLE AS ins` to
/// know which page moved, so Postgres copies every `bin` in the flush. This is
/// a **measurement, not a gate** — the numbers decide whether moving the
/// `dirty` upsert into `flush_updates` is worth losing "any writer marks
/// dirty" (LiveSnapshot acceptance #9 prefers the trigger).
///
/// ```text
/// cargo test -p venus-hub --test store p1_flush_transition_table_cost -- --ignored --nocapture
/// ```
///
/// Compares the shipped statement against the same payload going into a
/// trigger-less temp table of the same shape. The cap shape is the ceiling
/// (`PERSIST_BYTES` in one flush); the typing shape is the everyday case.
#[tokio::test]
#[ignore = "measurement; run with --ignored --nocapture"]
async fn p1_flush_transition_table_cost() {
    const RUNS: usize = 7;
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let mut conn = pool.acquire().await.expect("dedicated connection");

    sqlx::raw_sql(
        "CREATE TEMP TABLE p1_no_trigger (
             workspace_id UUID NOT NULL,
             doc_id UUID NOT NULL,
             seq BIGSERIAL NOT NULL,
             bin BYTEA NOT NULL,
             created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
             PRIMARY KEY (workspace_id, doc_id, seq)
         )",
    )
    .execute(&mut *conn)
    .await
    .expect("trigger-less baseline table");

    let cap_64k = PERSIST_BYTES / (64 * 1024);
    let cap_2k = PERSIST_BYTES / (2 * 1024);

    println!("P1 flush cost: bins are opaque to the INSERT; min of {RUNS} interleaved A/B runs");
    println!("  stored = avg pg_column_size(bin), on-disk after compression");
    println!("  temp   = pg_stat_database.temp_bytes for one triggered statement");
    // A run of one byte is what pglz squashes, so it measures the best case.
    // Real updates are y-octo binary: compress badly, and typing deltas sit
    // under the 2 KiB TOAST threshold, which is the shape that copies verbatim.
    measure_p1_shape(
        &mut conn,
        "cap, runs of 1 byte",
        64 * 1024,
        cap_64k,
        false,
        RUNS,
    )
    .await;
    measure_p1_shape(
        &mut conn,
        "cap, incompressible",
        64 * 1024,
        cap_64k,
        true,
        RUNS,
    )
    .await;
    measure_p1_shape(
        &mut conn,
        "cap, sub-TOAST bins",
        2 * 1024,
        cap_2k,
        true,
        RUNS,
    )
    .await;
    // Same row count as the shape above, 1/128th of the bytes: if the cost
    // holds, `ins` is priced per row, not per byte.
    measure_p1_shape(&mut conn, "same rows, 16 B bins", 16, cap_2k, true, RUNS).await;
    measure_p1_shape(&mut conn, "typing (~1s batch)", 128, 64, true, RUNS).await;

    // A transition table is a tuplestore sized by `work_mem`, so a cap-sized
    // `ins` can spill to a temp file at the 4 MB default. If that is the cost,
    // a session `work_mem` keeps both the trigger and the copy in RAM — no
    // seam change, no acceptance #9 re-accept.
    println!("  work_mem sweep, cap shapes only:");
    for work_mem in ["4MB", "16MB", "64MB"] {
        sqlx::raw_sql(&format!("SET work_mem = '{work_mem}'"))
            .execute(&mut *conn)
            .await
            .expect("set work_mem");
        measure_p1_shape(
            &mut conn,
            &format!("{work_mem}, incompressible"),
            64 * 1024,
            cap_64k,
            true,
            RUNS,
        )
        .await;
        measure_p1_shape(
            &mut conn,
            &format!("{work_mem}, sub-TOAST"),
            2 * 1024,
            cap_2k,
            true,
            RUNS,
        )
        .await;
    }
    sqlx::raw_sql("RESET work_mem")
        .execute(&mut *conn)
        .await
        .expect("reset work_mem");

    sqlx::raw_sql("DROP TABLE p1_no_trigger")
        .execute(&mut *conn)
        .await
        .expect("drop baseline table");
    drop(conn);

    // The cap path is never exercised by the other tests; prove it is correct
    // at that size, not only fast.
    let bins: Vec<Vec<u8>> = (0..PERSIST_BYTES / (64 * 1024))
        .map(|i| vec![(i % 251) as u8; 64 * 1024])
        .collect();
    let views: Vec<&[u8]> = bins.iter().map(|b| b.as_slice()).collect();
    let seqs = db::flush_updates(&pool, &ws, PAGE_DOC_ID, &views)
        .await
        .expect("flush at the persist cap");
    let (clock,): (i64,) = sqlx::query_as(
        "SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("dirty after a cap-sized flush");
    assert_eq!(
        clock,
        *seqs.last().expect("seqs"),
        "a cap-sized flush must still upsert max(seq) once"
    );

    sqlx::query("DELETE FROM crdt_update WHERE workspace_id = $1::uuid")
        .bind(&ws)
        .execute(&pool)
        .await
        .expect("drop the measurement rows");
}

/// P4: one statement for the whole batch. `seq` must follow array order.
/// P13: dirty trigger is STATEMENT; `dirty.clock` is `max(seq)` (last seq).
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

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("dirty count");
    assert_eq!(n, 1, "upsert, not a second row");
    let (clock,): (i64,) = sqlx::query_as(
        "SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("dirty row");
    assert_eq!(
        clock,
        *seqs.last().expect("seqs"),
        "STATEMENT trigger upserts max(seq); last seq wins"
    );

    let (transition,): (Option<String>,) = sqlx::query_as(
        "SELECT tgnewtable FROM pg_trigger
         WHERE NOT tgisinternal AND tgname = 'crdt_update_dirty'",
    )
    .fetch_one(&pool)
    .await
    .expect("trigger transition");
    assert_eq!(
        transition.as_deref(),
        Some("ins"),
        "P13: crdt_update_dirty must be FOR EACH STATEMENT with NEW TABLE"
    );
    assert_no_jobs_row(&pool).await;
}

/// Step 8: missing `dirty` must not roll back persist (EXCEPTION in the trigger).
/// D2: the handler RAISE WARNINGs; it must not RAISE EXCEPTION.
/// Rename is transactional so parallel tests still see `dirty` until we commit.
#[tokio::test]
async fn flush_lands_when_dirty_table_is_dropped() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("ALTER TABLE dirty RENAME TO dirty_hidden_step8")
        .execute(&mut *tx)
        .await
        .expect("hide dirty");
    let ins: Result<(i64,), sqlx::Error> = sqlx::query_as(
        "INSERT INTO crdt_update (workspace_id, doc_id, bin)
         VALUES ($1::uuid, $2::uuid, $3) RETURNING seq",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .bind(&bin)
    .fetch_one(&mut *tx)
    .await;
    sqlx::query("ALTER TABLE dirty_hidden_step8 RENAME TO dirty")
        .execute(&mut *tx)
        .await
        .expect("restore dirty");
    tx.commit().await.expect("commit persist without dirty");
    ins.expect("crdt_update must land when dirty is missing");

    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("trail after missing dirty");
    assert_eq!(n, 1, "persist must keep the update when dirty is gone");
    assert_no_jobs_row(&pool).await;
}

/// P1: writing `crdt_snapshot` is not an edit — compact only rewrites trail
/// rows that `crdt_update` already marked. A snapshot INSERT/UPDATE alone must
/// leave `dirty` empty; the next `crdt_update` still marks it.
#[tokio::test]
async fn snapshot_write_alone_does_not_mark_dirty() {
    let _serial = TRIGGER_STATE.lock().await;
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let bin = spike_bin();

    sqlx::query(
        "INSERT INTO crdt_snapshot (workspace_id, doc_id, bin, clock)
         VALUES ($1::uuid, $2::uuid, $3, 7)",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .bind(&bin)
    .execute(&pool)
    .await
    .expect("snapshot insert");
    assert_eq!(
        dirty_rows(&pool, &ws).await,
        0,
        "P1: snapshot INSERT must not write dirty"
    );

    sqlx::query(
        "UPDATE crdt_snapshot SET bin = $3, clock = 9
         WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .bind(&bin)
    .execute(&pool)
    .await
    .expect("snapshot update");
    assert_eq!(
        dirty_rows(&pool, &ws).await,
        0,
        "P1: snapshot UPDATE must not write dirty"
    );

    let seq = db::push_update(&pool, &ws, PAGE_DOC_ID, &bin)
        .await
        .expect("pushUpdate");
    let (clock,): (i64,) = sqlx::query_as(
        "SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("dirty after crdt_update");
    assert_eq!(clock, seq, "crdt_update is the only dirty mark");
}

/// D4: trigger must not follow `"$user", public` (a later `venus_hub` schema
/// must not intercept the upsert).
#[tokio::test]
async fn venus_mark_dirty_pins_search_path_to_public() {
    let pool = connect_fresh().await;
    let cfgs: Vec<String> = sqlx::query_scalar(
        "SELECT unnest(proconfig) FROM pg_proc WHERE proname = 'venus_mark_dirty'",
    )
    .fetch_all(&pool)
    .await
    .expect("proconfig");
    assert!(
        cfgs.iter()
            .any(|c| c.starts_with("search_path=") && c.contains("public")),
        "D4: venus_mark_dirty must SET search_path = public, got {cfgs:?}"
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

    let (snap_clock, dirty_clock): (i64, i64) = sqlx::query_as(
        "SELECT s.clock, d.clock FROM crdt_snapshot s
         JOIN dirty d ON s.workspace_id = d.workspace_id AND s.doc_id = d.doc_id
         WHERE s.workspace_id = $1::uuid AND s.doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(PAGE_DOC_ID)
    .fetch_one(&pool)
    .await
    .expect("snapshot dirty");
    assert_eq!(
        dirty_clock, snap_clock,
        "P1: the flush already marked this clock, so compact needs no trigger \
         of its own (crdt_snapshot.clock is that same max_seq)"
    );
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
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM blob WHERE workspace_id::text = $1")
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
    let _serial = TRIGGER_STATE.lock().await;
    let pool = connect_fresh().await;
    let (a, b) = tokio::join!(db::migrate(&pool), db::migrate(&pool));
    a.expect("migrate a");
    b.expect("migrate b");
    let (statement_update, leftover_snapshot) = dirty_trigger_counts(&pool).await;
    assert_eq!(
        statement_update, 1,
        "concurrent migrate must leave the STATEMENT crdt_update dirty trigger"
    );
    assert_eq!(
        leftover_snapshot, 0,
        "P1: concurrent migrate must not recreate snapshot dirty triggers"
    );
}

/// P1: a volume built before the snapshot triggers were dropped must lose them
/// on the next boot (the probe cannot skip the batch while they exist).
#[tokio::test]
async fn migrate_drops_leftover_snapshot_dirty_triggers() {
    let _serial = TRIGGER_STATE.lock().await;
    let pool = connect_fresh().await;
    sqlx::raw_sql(
        "CREATE TRIGGER crdt_snapshot_dirty_upd
             AFTER UPDATE ON crdt_snapshot
             REFERENCING NEW TABLE AS ins
             FOR EACH STATEMENT
             EXECUTE FUNCTION venus_mark_dirty()",
    )
    .execute(&pool)
    .await
    .expect("recreate the pre-P1 trigger");

    db::migrate(&pool).await.expect("migrate");

    let (statement_update, leftover_snapshot) = dirty_trigger_counts(&pool).await;
    assert_eq!(statement_update, 1, "crdt_update_dirty must survive");
    assert_eq!(
        leftover_snapshot, 0,
        "migrate must drop the leftover snapshot dirty trigger"
    );
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
