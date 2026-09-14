//! M3.0 step-ws: AFFiNE WebSocket apply, broadcast, persist. No keck.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::sync::OnceCell;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{connect_async, WebSocketStream};
use tower::ServiceExt;
use venus_hub::config::database_url_from_env;
use venus_hub::db;
use venus_hub::http::{router, AppState};
use venus_hub::lease::Lease;
use venus_hub::protocol::{apply_v1, decode_sync_messages, encode_doc_update, encode_v1};
use venus_hub::room::{GetRoomError, Hub};
use venus_hub::{PAGE_DOC_ID, SUBPROTOCOL};
use y_octo::{Doc, DocMessage, SyncMessage};

const YJS_SPIKE_KV_HEX: &str = "0101fc92c5bf0c002801057370696b65016b0177017600";
const YJS_BOLD_HELLO_HEX: &str = "010387bee4bb0300040101740568656c6c6f4687bee4bb030004626f6c6404747275658687bee4bb030404626f6c64046e756c6c00";

struct TestPg {
    database_url: String,
    _container: Option<
        testcontainers_modules::testcontainers::ContainerAsync<
            testcontainers_modules::postgres::Postgres,
        >,
    >,
}

static PG: OnceCell<TestPg> = OnceCell::const_new();

type Ws = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct LiveHub {
    addr: SocketAddr,
    hub: Arc<Hub>,
    app: axum::Router,
    server: JoinHandle<()>,
}

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn spike_bin() -> Vec<u8> {
    from_hex(YJS_SPIKE_KV_HEX)
}

fn map_has_v(doc: &Doc) -> bool {
    match doc.get_map("spike") {
        Ok(map) => format!("{:?}", map.get("k")).contains('v'),
        Err(_) => false,
    }
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

async fn lease_count(pool: &PgPool, owner: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM workspace_lease WHERE owner = $1")
            .bind(owner)
            .fetch_one(pool)
            .await
            .expect("lease count");
    n
}

async fn trail_count(pool: &PgPool, workspace: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace)
    .bind(PAGE_DOC_ID)
    .fetch_one(pool)
    .await
    .expect("trail count");
    n
}

async fn dirty_clock(pool: &PgPool, workspace: &str) -> Option<(i64, i64)> {
    sqlx::query_as(
        "SELECT clock, (SELECT COUNT(*)::bigint FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid)
         FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace)
    .bind(PAGE_DOC_ID)
    .fetch_optional(pool)
    .await
    .expect("dirty")
}

async fn dirty_wiki_rows(pool: &PgPool, workspace: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM dirty_wiki WHERE workspace_id = $1::uuid")
            .bind(workspace)
            .fetch_one(pool)
            .await
            .expect("dirty_wiki rows");
    n
}

async fn dirty_wiki_first_at(pool: &PgPool, workspace: &str) -> String {
    sqlx::query_scalar("SELECT first_dirty_at::text FROM dirty_wiki WHERE workspace_id = $1::uuid")
        .bind(workspace)
        .fetch_one(pool)
        .await
        .expect("dirty_wiki.first_dirty_at")
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

fn extra_spike_bin() -> Vec<u8> {
    let d = Doc::default();
    let mut map = d.get_or_create_map("spike").expect("map");
    map.insert("k2".to_string(), "v2").expect("insert");
    encode_v1(&d).expect("encode")
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

async fn connect_pool() -> PgPool {
    let pg = PG.get_or_init(start_pg).await;
    db::connect(&pg.database_url).await.expect("connect")
}

async fn spawn_hub(pool: PgPool, owner: String) -> LiveHub {
    spawn_hub_with_persist(pool, owner, Duration::from_secs(1)).await
}

async fn spawn_hub_with_persist(pool: PgPool, owner: String, persist: Duration) -> LiveHub {
    spawn_hub_with_state(pool, owner, persist, |_| {}).await
}

async fn spawn_hub_with_state(
    pool: PgPool,
    owner: String,
    persist: Duration,
    patch: impl FnOnce(&mut AppState),
) -> LiveHub {
    let lease = Lease::new(pool.clone(), owner, Duration::from_secs(20));
    let hub = Hub::new(pool, lease, persist, 32);
    let mut state = AppState::new(hub.clone());
    patch(&mut state);
    let app = router(state);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hub test port");
    let addr = listener.local_addr().expect("local addr");
    let served = app.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, served).await;
    });
    LiveHub {
        addr,
        hub,
        app,
        server,
    }
}

async fn stop_hub(live: LiveHub) {
    live.hub.shutdown().await;
    live.server.abort();
}

async fn crash_hub(live: LiveHub) {
    live.hub.abort_persist_no_flush().await;
    live.server.abort();
}

async fn post_protocol(app: axum::Router, workspace: &str) -> String {
    let uri = ["/collaboration/", workspace].concat();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("POST /collaboration");
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.expect("body").to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

async fn get_protocol(app: axum::Router, workspace: &str) -> (StatusCode, String) {
    let uri = ["/collaboration/", workspace].concat();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("GET /collaboration");
    let status = res.status();
    let bytes = res.into_body().collect().await.expect("body").to_bytes();
    (status, String::from_utf8(bytes.to_vec()).expect("utf8"))
}

async fn get_export(app: axum::Router, workspace: &str) -> Vec<u8> {
    let uri = ["/api/block/", workspace, "/export"].concat();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("GET export");
    assert_eq!(res.status(), StatusCode::OK);
    res.into_body()
        .collect()
        .await
        .expect("export body")
        .to_bytes()
        .to_vec()
}

async fn connect_affine(addr: SocketAddr, workspace: &str) -> Ws {
    let host = addr.to_string();
    let uri = ["ws://", host.as_str(), "/collaboration/", workspace].concat();
    let mut req = uri.into_client_request().expect("ws request");
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        SUBPROTOCOL.parse().expect("AFFiNE"),
    );
    let (ws, _) = connect_async(req)
        .await
        .expect("AFFiNE websocket (not keck)");
    ws
}

async fn connect_affine_http_status(addr: SocketAddr, workspace: &str) -> (u16, String) {
    let host = addr.to_string();
    let uri = ["ws://", host.as_str(), "/collaboration/", workspace].concat();
    let mut req = uri.into_client_request().expect("ws request");
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        SUBPROTOCOL.parse().expect("AFFiNE"),
    );
    let err = match connect_async(req).await {
        Ok(_) => panic!("expected HTTP rejection, not a websocket"),
        Err(e) => e,
    };
    let res = match err {
        WsError::Http(res) => res,
        other => panic!("expected HTTP rejection, got {other}"),
    };
    let status = res.status().as_u16();
    let body = res
        .body()
        .as_ref()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default();
    (status, body)
}

async fn drain_hello(ws: &mut Ws) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(800);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .expect("hello frames")
            .expect("ws open")
            .expect("ws frame");
        match msg {
            Message::Binary(bin) => {
                let msgs = decode_sync_messages(bin.as_ref()).messages;
                if let Some(SyncMessage::Doc(DocMessage::Step2(update))) = msgs.first() {
                    return update.clone();
                }
            }
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p)).await;
            }
            Message::Close(_) => panic!("closed during hello"),
            _ => {}
        }
    }
}

async fn send_update(ws: &mut Ws, yjs_update: Vec<u8>) {
    let frame = encode_doc_update(yjs_update).expect("encode Update");
    ws.send(Message::Binary(frame.into()))
        .await
        .expect("send Update");
}

async fn recv_update(ws: &mut Ws) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(2000);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .expect("B should see Update without reload (~200ms ok)")
            .expect("ws open")
            .expect("ws frame");
        match msg {
            Message::Binary(bin) => {
                let msgs = decode_sync_messages(bin.as_ref()).messages;
                if let Some(SyncMessage::Doc(DocMessage::Update(u))) = msgs.first() {
                    return u.clone();
                }
            }
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p)).await;
            }
            Message::Close(_) => panic!("closed before Update"),
            _ => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ws_upgrade_closed_pool_is_not_503() {
    let pool = connect_pool().await;
    pool.close().await;
    let live = spawn_hub(pool, ["owner-l3-", &uuid_like()].concat()).await;
    let workspace = unique_workspace();

    match live.hub.get_room(&workspace).await {
        Err(GetRoomError::Store(_)) => {}
        Err(GetRoomError::Held {
            workspace_id,
            owner,
        }) => panic!("closed pool must be Store, not Held ({workspace_id}, owner={owner:?})"),
        Ok(_) => panic!("closed pool must not open a room"),
    }

    let (status, body) = connect_affine_http_status(live.addr, &workspace).await;
    assert_ne!(
        status, 503,
        "closed pool must not look like a lease conflict: {body}"
    );
    assert_eq!(status, 500);
    assert!(
        !body.contains("owned by another hub"),
        "500 body must not claim another hub owns the wiki: {body}"
    );

    stop_hub(live).await;
}

/// Unflushed RAM must export; `get_room` of another wiki must not wait on `encode_live`.
#[tokio::test(flavor = "multi_thread")]
async fn live_export_uses_ram_get_room_does_not_wait_on_rooms_lock() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let other = unique_workspace();
    let live = spawn_hub_with_persist(
        pool.clone(),
        ["owner-export-", &uuid_like()].concat(),
        Duration::from_secs(3600),
    )
    .await;

    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    live.hub.abort_persist_no_flush().await;
    send_update(&mut a, spike_bin()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let export = live.hub.live_export(&workspace);
    let other_room = live.hub.get_room(&other);
    let (bytes, other_room) = tokio::join!(export, other_room);
    let bytes = bytes.expect("live_export RAM");
    other_room.expect("get_room other wiki must not wait on export encode");

    let mut from_export = Doc::default();
    apply_v1(&mut from_export, &bytes).expect("apply export");
    assert!(
        map_has_v(&from_export),
        "GET export must encode live RAM before persist tick"
    );

    let http_bytes = get_export(live.app.clone(), &workspace).await;
    let mut from_http = Doc::default();
    apply_v1(&mut from_http, &http_bytes).expect("apply HTTP export");
    assert!(map_has_v(&from_http), "HTTP export must match live RAM");

    let sql_doc = db::hydrate_doc(&pool, &workspace, PAGE_DOC_ID)
        .await
        .expect("hydrate SQL");
    assert!(
        !map_has_v(&sql_doc),
        "persist interval is long; export must not have required SQL"
    );

    stop_hub(live).await;
}

/// L9 decision 1: plain GET is the same health JSON as POST, and does not take a lease.
#[tokio::test(flavor = "multi_thread")]
async fn get_collaboration_without_upgrade_is_health_json() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-l9-", &uuid_like()].concat();
    let live = spawn_hub(pool.clone(), owner.clone()).await;

    let post = post_protocol(live.app.clone(), &workspace).await;
    let (status, get) = get_protocol(live.app.clone(), &workspace).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "plain GET must not be axum's 400 upgrade rejection"
    );
    assert!(
        get.contains("AFFiNE"),
        "GET /collaboration without Upgrade must be health JSON: {get}"
    );
    assert_eq!(get, post, "GET health body must match POST");
    assert_eq!(
        lease_count(&pool, &owner).await,
        0,
        "health GET must not acquire workspace_lease"
    );

    let uri = ["/collaboration/", workspace.as_str()].concat();
    let bad = live
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .header("upgrade", "websocket")
                .body(Body::empty())
                .expect("malformed upgrade"),
        )
        .await
        .expect("GET with Upgrade only");
    assert_eq!(
        bad.status(),
        StatusCode::BAD_REQUEST,
        "Upgrade: websocket without Connection must still be rejected"
    );

    stop_hub(live).await;
}

/// S2: garbage `{workspace_id}` is 400 before lease or hydrate.
#[tokio::test(flavor = "multi_thread")]
async fn bad_workspace_id_is_400_and_does_not_acquire() {
    let pool = connect_pool().await;
    let owner = ["owner-s2-", &uuid_like()].concat();
    let live = spawn_hub(pool.clone(), owner.clone()).await;

    let (status, get) = get_protocol(live.app.clone(), "bad%20id").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        get.contains("invalid workspace_id"),
        "health GET must name the shape error: {get}"
    );
    assert_eq!(lease_count(&pool, &owner).await, 0);

    let (status, body) = connect_affine_http_status(live.addr, "bad%20id").await;
    assert_eq!(status, 400, "upgrade must not reach get_room: {body}");
    assert!(
        body.contains("invalid workspace_id"),
        "upgrade 400 body: {body}"
    );
    assert_eq!(
        lease_count(&pool, &owner).await,
        0,
        "invalid id must not acquire workspace_lease"
    );

    let long = "a".repeat(65);
    let (status, _) = get_protocol(live.app.clone(), &long).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    stop_hub(live).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_to_b_over_affine_ws() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub(pool, ["owner-ab-", &uuid_like()].concat()).await;

    let body = post_protocol(live.app.clone(), &workspace).await;
    assert!(
        body.contains("AFFiNE"),
        "POST /collaboration must be the Venus hub, not keck: {body}"
    );

    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    let mut b = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut b).await;

    send_update(&mut a, spike_bin()).await;
    let update = recv_update(&mut b).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &update).expect("B applies A's update");
    assert!(map_has_v(&doc), "B must see spike.k=v without reload");

    stop_hub(live).await;
}

/// B connects after A's write; Step2 (not live fan-out) must already have spike.k=v.
#[tokio::test(flavor = "multi_thread")]
async fn late_joiner_step2_has_spike_after_a_writes() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub(pool, ["owner-late-", &uuid_like()].concat()).await;

    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    send_update(&mut a, spike_bin()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut b = connect_affine(live.addr, &workspace).await;
    let step2 = drain_hello(&mut b).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &step2).expect("late joiner applies Step2");
    assert!(
        map_has_v(&doc),
        "B after A's write must hydrate spike.k=v from Step2 without waiting for an Update"
    );

    stop_hub(live).await;
}

/// Two Hub owners, same Postgres: second WS is 503 Held. Not a second live Doc.
#[tokio::test(flavor = "multi_thread")]
async fn second_hub_ws_is_503_while_lease_held() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live_a = spawn_hub(pool.clone(), ["owner-a-", &uuid_like()].concat()).await;
    let mut a = connect_affine(live_a.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;

    let live_b = spawn_hub(pool, ["owner-b-", &uuid_like()].concat()).await;
    match live_b.hub.get_room(&workspace).await {
        Err(GetRoomError::Held { workspace_id, .. }) => {
            assert_eq!(workspace_id, workspace);
        }
        Err(GetRoomError::Store(e)) => panic!("live lease must be Held, not Store: {e:#}"),
        Ok(_) => panic!("second hub must not open a RAM doc for the same wiki"),
    }

    let (status, body) = connect_affine_http_status(live_b.addr, &workspace).await;
    assert_eq!(
        status, 503,
        "second owner WS must be 503, got {status}: {body}"
    );
    assert!(
        body.contains("owned by another hub"),
        "503 body must name a lease conflict: {body}"
    );

    stop_hub(live_b).await;
    stop_hub(live_a).await;
}

/// L4: a stolen lease must shed RAM. The old hub must not INSERT after that.
#[tokio::test(flavor = "multi_thread")]
async fn heartbeat_miss_sheds_room_no_further_insert() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner_a = ["owner-l4-a-", &uuid_like()].concat();
    let thief = ["owner-l4-b-", &uuid_like()].concat();

    let live_a = spawn_hub(pool.clone(), owner_a.clone()).await;
    let room = live_a.hub.get_room(&workspace).await.expect("A acquires");
    send_update_to_room(&room, spike_bin()).await;
    room.flush(&pool).await.expect("flush spike");
    let n_before = trail_count(&pool, &workspace).await;
    assert!(n_before >= 1, "spike must be in SQL before the steal");

    sqlx::query(
        "UPDATE workspace_lease
         SET owner = $1, lease_until = now() + interval '1 hour'
         WHERE workspace_id = $2::uuid",
    )
    .bind(&thief)
    .bind(&workspace)
    .execute(&pool)
    .await
    .expect("steal lease");

    live_a.hub.heartbeat().await;
    assert!(room.stopped(), "stolen lease must stop the RAM room");
    assert!(
        !room.persist_task_is_some(),
        "shed must take the persist task"
    );

    match live_a.hub.get_room(&workspace).await {
        Err(GetRoomError::Held { workspace_id, .. }) => {
            assert_eq!(workspace_id, workspace);
        }
        Err(GetRoomError::Store(e)) => panic!("stolen lease must be Held, not Store: {e:#}"),
        Ok(_) => panic!("old hub must not keep or re-open the room after a steal"),
    }

    send_update_to_room(&room, extra_spike_bin()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        trail_count(&pool, &workspace).await,
        n_before,
        "old hub must not INSERT after shed"
    );

    let hub_b = Hub::new(
        pool.clone(),
        Lease::new(pool.clone(), thief, Duration::from_secs(20)),
        Duration::from_secs(1),
        32,
    );
    hub_b
        .get_room(&workspace)
        .await
        .expect("thief owns SQL and must hydrate");
    hub_b.shutdown().await;
    stop_hub(live_a).await;
}

/// P6: no clients, empty persist buffer, idle ≥ lease TTL → shed + drop_one.
#[tokio::test(flavor = "multi_thread")]
async fn idle_room_is_shed_and_lease_dropped() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-p6-", &uuid_like()].concat();
    let hub = Hub::new(
        pool.clone(),
        Lease::new(pool.clone(), owner.clone(), Duration::from_millis(80)),
        Duration::from_secs(1),
        32,
    );
    let room = hub.get_room(&workspace).await.expect("open idle room");
    let opened = hub.hydrate_attempts();
    tokio::time::sleep(Duration::from_millis(120)).await;
    hub.heartbeat().await;
    assert!(room.stopped(), "idle TTL must shed the room");
    assert_eq!(
        lease_count(&pool, &owner).await,
        0,
        "idle shed must drop the lease so the next get_room can acquire"
    );

    hub.get_room(&workspace)
        .await
        .expect("next WS hydrates and acquires again");
    assert_eq!(
        hub.hydrate_attempts(),
        opened + 1,
        "idle shed must drop RAM so the next get_room hydrates"
    );
    hub.shutdown().await;
}

async fn send_update_to_room(room: &venus_hub::room::Room, yjs_update: Vec<u8>) {
    let frame = encode_doc_update(yjs_update).expect("encode Update");
    room.handle_binary(1, &frame).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn format_mark_over_ws_does_not_crash() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub(pool, ["owner-mark-", &uuid_like()].concat()).await;

    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    let mut b = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut b).await;

    send_update(&mut a, from_hex(YJS_BOLD_HELLO_HEX)).await;
    let update = recv_update(&mut b).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &update).expect("B applies Format/bold without panic");

    let body = post_protocol(live.app.clone(), &workspace).await;
    assert!(
        body.contains("AFFiNE"),
        "hub must still serve after Format apply: {body}"
    );

    stop_hub(live).await;
}

/// Step 8: persist upserts one `dirty` row; a second write moves `clock`.
/// Product ids are M0 workspace UUID + `PAGE_DOC_ID`; this test uses a unique
/// workspace and the same SQL `doc_id`. Hub must not insert `jobs`.
#[tokio::test(flavor = "multi_thread")]
async fn persist_after_ws_upserts_dirty_clock_not_jobs() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub(pool.clone(), ["owner-dirty-", &uuid_like()].concat()).await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    send_update(&mut a, spike_bin()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (clock1, n1) = dirty_clock(&pool, &workspace)
        .await
        .expect("dirty row after first persist");
    assert_eq!(n1, 1, "one dirty row per (workspace_id, PAGE_DOC_ID)");
    assert!(clock1 >= 1, "clock must be the flushed seq, got {clock1}");
    assert_eq!(
        dirty_wiki_rows(&pool, &workspace).await,
        1,
        "persist must upsert one dirty_wiki row"
    );
    let first = dirty_wiki_first_at(&pool, &workspace).await;
    assert_no_jobs_row(&pool).await;

    send_update(&mut a, extra_spike_bin()).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let (clock2, n2) = dirty_clock(&pool, &workspace)
        .await
        .expect("dirty row after second persist");
    assert_eq!(n2, 1, "second write must upsert, not insert a second row");
    assert!(
        clock2 > clock1,
        "second persist must move dirty.clock ({clock1} → {clock2})"
    );
    assert_eq!(
        dirty_wiki_rows(&pool, &workspace).await,
        1,
        "second persist must not insert a second dirty_wiki row"
    );
    assert_eq!(
        dirty_wiki_first_at(&pool, &workspace).await,
        first,
        "ON CONFLICT DO NOTHING must not reset first_dirty_at"
    );
    assert_no_jobs_row(&pool).await;

    stop_hub(live).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn persist_after_ws_survives_hub_restart() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-p1-", &uuid_like()].concat();

    let live = spawn_hub(pool.clone(), owner.clone()).await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    send_update(&mut a, spike_bin()).await;

    tokio::time::sleep(Duration::from_secs(2)).await;
    stop_hub(live).await;
    assert_eq!(
        lease_count(&pool, &owner).await,
        0,
        "SIGTERM drain must drop workspace_lease for this owner"
    );

    let live = spawn_hub(pool, ["owner-p2-", &uuid_like()].concat()).await;
    let mut c = connect_affine(live.addr, &workspace).await;
    let step2 = drain_hello(&mut c).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &step2).expect("hydrate after hub restart");
    assert!(
        map_has_v(&doc),
        "new client must hydrate spike.k=v from Postgres, not RAM"
    );

    stop_hub(live).await;
}

/// L5: SIGTERM before the persist tick must still flush and drop the lease.
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_flushes_without_tick_and_drops_lease() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-drain-", &uuid_like()].concat();

    let live = spawn_hub_with_persist(pool.clone(), owner.clone(), Duration::from_secs(3600)).await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_update(&mut a, spike_bin()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    live.hub.shutdown().await;
    assert_eq!(
        lease_count(&pool, &owner).await,
        0,
        "drain must DELETE workspace_lease before a new owner hydrates"
    );

    let sql_doc = db::hydrate_doc(&pool, &workspace, PAGE_DOC_ID)
        .await
        .expect("hydrate after drain");
    assert!(
        map_has_v(&sql_doc),
        "shutdown must flush unflushed RAM without waiting for the persist tick"
    );

    let hub2 = Hub::new(
        pool.clone(),
        Lease::new(pool.clone(), owner.clone(), Duration::from_secs(20)),
        Duration::from_secs(1),
        32,
    );
    let room = hub2
        .get_room(&workspace)
        .await
        .expect("a new hub after drain must hydrate SQL");
    let bytes = room.encode_live().await.expect("encode after re-hydrate");
    let mut from_room = Doc::default();
    apply_v1(&mut from_room, &bytes).expect("apply re-hydrate");
    assert!(
        map_has_v(&from_room),
        "cleared rooms map must not keep an empty drained Doc"
    );
    hub2.shutdown().await;

    stop_hub(live).await;
}

/// Hard crash (no SIGTERM flush): SQL misses the last update; the client's local
/// doc resends it. Not Redis. Persist interval is long so the tick cannot sneak in.
#[tokio::test(flavor = "multi_thread")]
async fn crash_unflushed_client_resend_restores() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-crash-", &uuid_like()].concat();

    let live = spawn_hub_with_persist(pool.clone(), owner.clone(), Duration::from_secs(3600)).await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send_update(&mut a, spike_bin()).await;

    crash_hub(live).await;

    let live = spawn_hub(pool.clone(), owner).await;
    let mut c = connect_affine(live.addr, &workspace).await;
    let step2 = drain_hello(&mut c).await;
    let mut from_sql = Doc::default();
    apply_v1(&mut from_sql, &step2).expect("apply Step2 after crash");
    assert!(
        !map_has_v(&from_sql),
        "crash without flush must not have persisted spike.k=v (would hide client resend)"
    );

    let mut d = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut d).await;
    send_update(&mut c, spike_bin()).await;
    let update = recv_update(&mut d).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &update).expect("spectator applies resent Update");
    assert!(
        map_has_v(&doc),
        "client local doc must restore unflushed spike.k=v after hub crash"
    );

    stop_hub(live).await;
}

/// S3: a binary larger than `ws_max_message` must close the socket.
#[tokio::test(flavor = "multi_thread")]
async fn ws_oversized_binary_is_closed() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub_with_state(
        pool,
        ["owner-s3-size-", &uuid_like()].concat(),
        Duration::from_secs(3600),
        |st| {
            st.ws_max_message = 1024;
        },
    )
    .await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    a.send(Message::Binary(vec![0u8; 2048].into()))
        .await
        .expect("client may send; server must reject");

    let mut closed = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_millis(500), a.next()).await {
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => {
                closed = true;
                break;
            }
            Ok(Some(Ok(Message::Ping(p)))) => {
                let _ = a.send(Message::Pong(p)).await;
            }
            Ok(Some(Ok(_))) => {}
            Err(_) => break,
        }
    }
    assert!(closed, "server must close after an oversized WS frame");
    stop_hub(live).await;
}

/// S3: no Pong after a server ping detaches the socket.
#[tokio::test(flavor = "multi_thread")]
async fn ws_missed_pong_closes_socket() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub_with_state(
        pool,
        ["owner-s3-pong-", &uuid_like()].concat(),
        Duration::from_secs(3600),
        |st| {
            st.ws_ping = Duration::from_millis(50);
            st.ws_pong = Duration::from_millis(80);
        },
    )
    .await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut closed = false;
    for _ in 0..8 {
        match tokio::time::timeout(Duration::from_millis(400), a.next()).await {
            Ok(None) | Ok(Some(Err(_))) | Ok(Some(Ok(Message::Close(_)))) => {
                closed = true;
                break;
            }
            Ok(Some(Ok(Message::Ping(_)))) => {}
            Ok(Some(Ok(_))) => {}
            Err(_) => break,
        }
    }
    assert!(closed, "missed pong must close the socket");

    let mut b = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut b).await;
    stop_hub(live).await;
}

/// L18: `ws_ping = 0` must not panic `interval_at`; the socket still applies.
#[tokio::test(flavor = "multi_thread")]
async fn ws_zero_ping_does_not_panic() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let live = spawn_hub_with_state(
        pool,
        ["owner-l18-ping-", &uuid_like()].concat(),
        Duration::from_secs(3600),
        |st| {
            st.ws_ping = Duration::ZERO;
        },
    )
    .await;
    let mut a = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut a).await;
    let mut b = connect_affine(live.addr, &workspace).await;
    let _ = drain_hello(&mut b).await;
    send_update(&mut a, from_hex(YJS_SPIKE_KV_HEX)).await;
    let update = recv_update(&mut b).await;
    let mut doc = Doc::default();
    apply_v1(&mut doc, &update).expect("B applies with ping disabled");
    assert!(map_has_v(&doc), "B must see the Update with ping disabled");
    stop_hub(live).await;
}
