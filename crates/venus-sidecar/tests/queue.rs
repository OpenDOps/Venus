//! M3 step-pin-queue: observer writes `jobs`; SKIP LOCKED claim; pin Map empty.
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
use venus_sidecar::config::database_url_from_env;
use venus_sidecar::http::router;
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{
    claim_one, flush_status, observe_once, record_flush_failure, MAX_FLUSH_ATTEMPTS,
};
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
    let mixed = t ^ (n << 48);
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

async fn seed_dirty_wiki(pool: &PgPool, workspace: &str) {
    sqlx::query(
        "INSERT INTO dirty (workspace_id, doc_id, clock)
         VALUES ($1::uuid, $2::uuid, 1)",
    )
    .bind(workspace)
    .bind(PAGE_DOC_UUID)
    .execute(pool)
    .await
    .expect("dirty");
    sqlx::query(
        "INSERT INTO dirty_wiki (workspace_id, first_dirty_at)
         VALUES ($1::uuid, now())",
    )
    .bind(workspace)
    .execute(pool)
    .await
    .expect("dirty_wiki");
}

async fn job_count(pool: &PgPool, workspace: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM jobs WHERE workspace_id = $1::uuid")
            .bind(workspace)
            .fetch_one(pool)
            .await
            .expect("jobs count");
    n
}

async fn insert_due_job(pool: &PgPool, workspace: &str) {
    seed_dirty_wiki(pool, workspace).await;
    sqlx::query(
        "INSERT INTO jobs (workspace_id, reason, not_before)
         VALUES ($1::uuid, 'idle', now())",
    )
    .bind(workspace)
    .execute(pool)
    .await
    .expect("jobs fixture");
}

#[test]
fn observer_does_not_get_export() {
    let queue = include_str!("../src/queue.rs");
    for needle in ["/export", "reqwest", "crdt_snapshot", "get_doc"] {
        assert!(
            !queue.contains(needle),
            "observer/claim must not {needle} (dirty_wiki → jobs, then SKIP LOCKED)"
        );
    }
}

/// Observer enqueues one job; a second tick is ON CONFLICT DO NOTHING.
#[tokio::test]
async fn observer_enqueues_one_job() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    seed_dirty_wiki(&pool, &ws).await;

    let other = unique_workspace();
    seed_dirty_wiki(&pool, &other).await;

    let n1 = observe_once(&pool, Duration::ZERO, &ws)
        .await
        .expect("observer 1");
    let n2 = observe_once(&pool, Duration::ZERO, &ws)
        .await
        .expect("observer 2");
    assert_eq!(n1, 1, "first tick inserts one idle job");
    assert_eq!(n2, 0, "second tick must ON CONFLICT DO NOTHING");
    assert_eq!(job_count(&pool, &ws).await, 1);
    assert_eq!(
        job_count(&pool, &other).await,
        0,
        "observer must not enqueue a workspace this wiki is not bound to"
    );

    let (reason, due): (String, bool) = sqlx::query_as(
        "SELECT reason, not_before <= now() FROM jobs WHERE workspace_id = $1::uuid",
    )
    .bind(&ws)
    .fetch_one(&pool)
    .await
    .expect("job row");
    assert_eq!(reason, "idle");
    assert!(due, "SNAPSHOT_IDLE_MS=0 → not_before <= now()");
}

/// One wiki, two consumers: SKIP LOCKED → one owner. Pin Map stays empty.
#[tokio::test(flavor = "multi_thread")]
async fn one_wiki_two_consumers_one_claim() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    insert_due_job(&pool, &ws).await;

    let (a, b) = tokio::join!(
        claim_one(&pool, "worker-a", &ws),
        claim_one(&pool, "worker-b", &ws)
    );
    let a = a.expect("claim a");
    let b = b.expect("claim b");
    let mine: Vec<&str> = [a.as_ref(), b.as_ref()]
        .into_iter()
        .flatten()
        .filter(|c| c.workspace_id == ws)
        .map(|c| c.owner.as_str())
        .collect();
    assert_eq!(
        mine.len(),
        1,
        "exactly one claim for this wiki; other SKIP LOCKED"
    );
    assert!(
        mine[0] == "worker-a" || mine[0] == "worker-b",
        "owner must be one of the two workers, got {:?}",
        mine
    );

    // 4.2: claim itself does not fill the Map. (4.3 worker_turn cuts after claim.)
    let pins = PinMap::new();
    let again = claim_one(&pool, "worker-c", &ws)
        .await
        .expect("third claim");
    assert!(
        again.as_ref().map(|c| c.workspace_id.as_str()) != Some(ws.as_str()),
        "inflight stays 1 while lease holds"
    );
    assert!(pins.is_empty(), "claim must not copy Yjs into the pin Map");
}

/// Two wikis, two consumers: both jobs leased (no process-wide mutex).
#[tokio::test(flavor = "multi_thread")]
async fn two_wikis_two_consumers() {
    let pool = connect_fresh().await;
    let ws_a = unique_workspace();
    let ws_b = unique_workspace();
    insert_due_job(&pool, &ws_a).await;
    insert_due_job(&pool, &ws_b).await;

    let (a, b) = tokio::join!(claim_one(&pool, "w0", &ws_a), claim_one(&pool, "w1", &ws_b));
    let a = a.expect("claim a").expect("wiki A leased");
    let b = b.expect("claim b").expect("wiki B leased");
    let mut ids = [a.workspace_id, b.workspace_id];
    ids.sort();
    let mut want = [ws_a, ws_b];
    want.sort();
    assert_eq!(ids, want, "both wikis claimed in parallel");
    assert_ne!(a.owner, b.owner);
}

#[tokio::test]
async fn deterministic_failure_is_409_until_a_newer_clock() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    insert_due_job(&pool, &ws).await;
    record_flush_failure(
        &pool,
        &ws,
        "worker",
        &anyhow::anyhow!("catalog pin has no home doc"),
    )
    .await
    .expect("record");

    let claimed = claim_one(&pool, "worker", &ws).await.expect("claim");
    assert!(claimed.is_none(), "a failed job is not claimed");
    let status = flush_status(&pool, &ws).await.expect("status");
    assert!(status.failed);
    assert_eq!(status.attempts, 1);
    assert!(status
        .last_error
        .as_deref()
        .unwrap_or("")
        .contains("no home doc"));

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
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(std::str::from_utf8(&body).unwrap().contains("no home doc"));

    observe_once(&pool, Duration::ZERO, &ws)
        .await
        .expect("same clock");
    assert!(
        flush_status(&pool, &ws).await.expect("still failed").failed,
        "the same dirty clock must not reopen the job"
    );

    sqlx::query("UPDATE dirty SET clock = clock + 1 WHERE workspace_id = $1::uuid")
        .bind(&ws)
        .execute(&pool)
        .await
        .expect("bump clock");
    observe_once(&pool, Duration::ZERO, &ws)
        .await
        .expect("reopen");
    let reopened = flush_status(&pool, &ws).await.expect("reopened");
    assert!(!reopened.failed);
    assert_eq!(reopened.attempts, 0);
    assert!(reopened.last_error.is_none());
    assert!(claim_one(&pool, "worker", &ws)
        .await
        .expect("claim after reopen")
        .is_some());
}

#[tokio::test]
async fn transient_failure_backs_off_then_stops() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    insert_due_job(&pool, &ws).await;
    for attempt in 1..MAX_FLUSH_ATTEMPTS {
        record_flush_failure(&pool, &ws, "worker", &anyhow::anyhow!("connection refused"))
            .await
            .expect("record");
        let status = flush_status(&pool, &ws).await.expect("status");
        assert!(!status.failed, "attempt {attempt} still retries");
        assert_eq!(status.attempts, attempt);
        assert!(
            claim_one(&pool, "worker", &ws)
                .await
                .expect("claim")
                .is_none(),
            "backoff holds the job"
        );
    }
    record_flush_failure(&pool, &ws, "worker", &anyhow::anyhow!("connection refused"))
        .await
        .expect("final");
    let status = flush_status(&pool, &ws).await.expect("failed");
    assert!(status.failed);
    assert_eq!(status.attempts, MAX_FLUSH_ATTEMPTS);
}
