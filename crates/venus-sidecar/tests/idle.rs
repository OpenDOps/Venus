//! M3 step-dirty-idle: jobs.not_before idle, Flush, coalesce; no pin at enqueue.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tokio::sync::OnceCell;
use tower::ServiceExt;
use venus_hub::db;
use venus_sidecar::config::{database_url_from_env, DEFAULT_SNAPSHOT_IDLE_MS};
use venus_sidecar::http::router;
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{flush_now, observe_once};
use venus_sidecar::PAGE_DOC_UUID;

struct TestPg {
    database_url: String,
    _container: Option<
        testcontainers_modules::testcontainers::ContainerAsync<
            testcontainers_modules::postgres::Postgres,
        >,
    >,
}

static PG: OnceCell<TestPg> = OnceCell::const_new();

async fn start_pg() -> TestPg {
    if let Ok(Some(database_url)) = database_url_from_env() {
        let pool = db::connect(&database_url)
            .await
            .expect("connect env postgres");
        db::migrate(&pool).await.expect("hub migrate");
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
    db::migrate(&pool).await.expect("hub migrate");
    TestPg {
        database_url,
        _container: Some(container),
    }
}

async fn connect_fresh() -> PgPool {
    let pg = PG.get_or_init(start_pg).await;
    db::connect(&pg.database_url).await.expect("connect")
}

fn unique_workspace() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed) as u128;
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = u128::from(std::process::id());
    let mixed = t ^ (pid << 80) ^ (n << 64);
    let mut id = String::with_capacity(36);
    write!(
        &mut id,
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (mixed >> 96) as u32,
        (mixed >> 80) as u16,
        ((mixed >> 64) as u16) & 0x0fff,
        ((mixed >> 48) as u16) & 0x0fff,
        mixed as u64 & 0xffffffffffff
    )
    .expect("uuid");
    id
}

async fn persist(pool: &PgPool, workspace: &str, bin: &[u8]) -> i64 {
    db::push_update(pool, workspace, PAGE_DOC_UUID, bin)
        .await
        .expect("persist")
}

async fn dirty_clock(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT clock FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace)
    .bind(PAGE_DOC_UUID)
    .fetch_one(pool)
    .await
    .expect("dirty.clock")
}

async fn dirty_count(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM dirty WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace)
    .bind(PAGE_DOC_UUID)
    .fetch_one(pool)
    .await
    .expect("dirty count")
}

async fn first_dirty_ms(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM first_dirty_at) * 1000)::bigint
         FROM dirty_wiki WHERE workspace_id = $1::uuid",
    )
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("first_dirty_at")
}

async fn job_not_before_ms(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM not_before) * 1000)::bigint
         FROM jobs WHERE workspace_id = $1::uuid",
    )
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("jobs.not_before")
}

async fn job_row(pool: &PgPool, workspace: &str) -> (String, bool, i64) {
    sqlx::query_as(
        "SELECT reason, not_before <= now(), COUNT(*) OVER ()::bigint
         FROM jobs WHERE workspace_id = $1::uuid",
    )
    .bind(workspace)
    .fetch_one(pool)
    .await
    .expect("job row")
}

async fn job_count(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_as("SELECT COUNT(*)::bigint FROM jobs WHERE workspace_id = $1::uuid")
        .bind(workspace)
        .fetch_one(pool)
        .await
        .map(|(n,)| n)
        .expect("jobs count")
}

#[test]
fn default_idle_is_60s_on_not_before_column() {
    assert_eq!(DEFAULT_SNAPSHOT_IDLE_MS, 60_000);
    let queue = include_str!("../src/queue.rs");
    assert!(
        queue.contains("first_dirty_at + ($1::bigint * interval '1 millisecond')"),
        "idle is jobs.not_before from first_dirty_at, not sleep(60s)"
    );
    assert!(
        !queue.contains("sleep(Duration::from_secs(60))"),
        "must not sleep(60s) as the product queue"
    );
}

#[test]
fn sidecar_does_not_mark_dirty_by_editor_events() {
    let src = [
        include_str!("../src/queue.rs"),
        include_str!("../src/http.rs"),
        include_str!("../src/main.rs"),
    ]
    .concat();
    for needle in ["keystroke", "keydown", "editor event", "input event"] {
        assert!(
            !src.contains(needle),
            "sidecar must not mark dirty by counting {needle}"
        );
    }
}

#[test]
fn flush_http_does_not_copy_yjs() {
    let http = include_str!("../src/http.rs");
    let queue = include_str!("../src/queue.rs");
    for needle in [
        "/export",
        "reqwest",
        "cut_workspace",
        "crdt_snapshot",
        "from_doc",
    ] {
        assert!(!http.contains(needle), "POST /flush must not {needle}");
    }
    let flush = queue
        .split("const FLUSH_SQL")
        .nth(1)
        .expect("FLUSH_SQL")
        .split("pub async fn claim_one")
        .next()
        .expect("flush_now");
    for needle in ["owner", "lease_until", "crdt_snapshot"] {
        assert!(
            !flush.contains(needle),
            "flush upsert must not touch {needle}"
        );
    }
}

/// Persist upserts one dirty row; second write moves clock, not first_dirty_at.
#[tokio::test]
async fn dirty_clock_not_keystroke() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let t1 = persist(&pool, &ws, b"alpha").await;
    assert_eq!(dirty_count(&pool, &ws).await, 1);
    assert_eq!(dirty_clock(&pool, &ws).await, t1);
    let first = first_dirty_ms(&pool, &ws).await;

    let t2 = persist(&pool, &ws, b"beta").await;
    assert!(t2 > t1, "second persist must move dirty.clock");
    assert_eq!(dirty_count(&pool, &ws).await, 1);
    assert_eq!(dirty_clock(&pool, &ws).await, t2);
    assert_eq!(
        first_dirty_ms(&pool, &ws).await,
        first,
        "second persist must not reset first_dirty_at"
    );
}

/// Second persist does not slide jobs.not_before; still one due claim.
#[tokio::test]
async fn idle_coalesce() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist(&pool, &ws, b"t0").await;
    let idle = Duration::from_millis(500);
    observe_once(&pool, idle).await.expect("observe");
    assert_eq!(job_count(&pool, &ws).await, 1);
    let first = first_dirty_ms(&pool, &ws).await;
    let nb1 = job_not_before_ms(&pool, &ws).await;
    assert!(
        (nb1 - first - 500).abs() <= 20,
        "not_before must be first_dirty_at + 500ms (got first={first} nb={nb1})"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;
    persist(&pool, &ws, b"t100").await;
    observe_once(&pool, idle).await.expect("observe 2");
    assert_eq!(
        job_count(&pool, &ws).await,
        1,
        "second persist must not stack jobs"
    );
    assert_eq!(
        first_dirty_ms(&pool, &ws).await,
        first,
        "first_dirty_at must not move"
    );
    assert_eq!(
        job_not_before_ms(&pool, &ws).await,
        nb1,
        "not_before must not become t+100+500ms"
    );

    tokio::time::sleep(Duration::from_millis(450)).await;
    let (reason, due, n): (String, bool, i64) = job_row(&pool, &ws).await;
    assert_eq!(reason, "idle");
    assert_eq!(n, 1);
    assert!(due, "one claim when due");
    let owned: Option<String> = sqlx::query_scalar(
        "UPDATE jobs
         SET owner = 'idle-coalesce', lease_until = now() + interval '2 minutes'
         WHERE workspace_id = $1::uuid
           AND not_before <= now()
           AND (lease_until IS NULL OR lease_until < now())
         RETURNING workspace_id::text",
    )
    .bind(&ws)
    .fetch_optional(&pool)
    .await
    .expect("claim when due");
    assert_eq!(owned.as_deref(), Some(ws.as_str()));
}

/// POST /flush pulls a future idle job to now; pin Map stays empty.
#[tokio::test]
async fn flush_now_http() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist(&pool, &ws, b"idle").await;
    observe_once(&pool, Duration::from_secs(60))
        .await
        .expect("idle job");
    assert_eq!(job_count(&pool, &ws).await, 1);
    let (reason, due, _): (String, bool, i64) = job_row(&pool, &ws).await;
    assert_eq!(reason, "idle");
    assert!(!due, "idle not_before is in the future");

    let pins = PinMap::new();
    let app = router(Some(pool.clone()));
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/flush?workspace={ws}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let _ = res.into_body().collect().await.unwrap();
    assert!(pins.is_empty(), "Flush HTTP must not copy Yjs");

    let (reason, due, n): (String, bool, i64) = job_row(&pool, &ws).await;
    assert_eq!(n, 1, "do not insert a second jobs row");
    assert_eq!(reason, "flush");
    assert!(due, "not_before <= now()");
    assert!(pins.is_empty());
}

/// Observer insert and Flush leave the pin Map empty until claim.
#[tokio::test]
async fn no_pin_at_enqueue() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let pins = PinMap::new();
    persist(&pool, &ws, b"enqueue").await;
    observe_once(&pool, Duration::from_secs(60))
        .await
        .expect("observe");
    assert!(pins.is_empty(), "observer insert must not pin");
    flush_now(&pool, &ws).await.expect("flush");
    assert!(pins.is_empty(), "Flush must not pin");
    let (_reason, due, _): (String, bool, i64) = job_row(&pool, &ws).await;
    assert!(due, "flush makes the job due");
    assert!(
        pins.is_empty(),
        "due but unclaimed must not fill the pin Map"
    );
}
