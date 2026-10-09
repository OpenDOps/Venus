//! ss-cluster step-enqueue.
//! `cargo test -p venus-graph --test enqueue` with the sidecar and profile `graph`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use sqlx::PgPool;
use surrealastic::{apply, ApplyStatus, Body, Cluster, Config, Entry};
use venus_graph::{
    claim_job, ensure_wiki, graph_database, lease_name, migrate_allocation, record_sha,
    set_node_url, write_job, Doc, GRAPH_JOBS_DDL, SURREAL_IMAGE,
};
use venus_sidecar::git::WikiConfig;
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{flush_claimed, Claim};
use venus_sidecar::PAGE_DOC_UUID;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn sidecar_creates_the_same_graph_jobs_table() {
    let side = include_str!("../../venus-sidecar/src/queue.rs");
    assert!(
        side.contains(GRAPH_JOBS_DDL.trim()),
        "sidecar graph_jobs DDL must match venus-graph"
    );
    let hub = include_str!("../../venus-hub/src/schema.sql");
    assert!(
        !hub.contains("graph_jobs"),
        "hub schema must not create graph_jobs"
    );
}

/// One `graph_jobs` row carries that `wiki_sha`. `last_flushed` matches.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn row_after_flush() {
    let _guard = LOCK.lock().await;
    let world = world();
    ready(world).await;
    let pool = cluster_pool(&world.pg).await;
    let ws = workspace();
    let wiki = tempfile::tempdir().expect("wiki");
    let config = WikiConfig::new(wiki.path());

    let first = flush_wiki(&pool, &ws, &config).await;
    assert!(first.committed, "Row after flush");
    let sha = first.sha.expect("sha");
    assert_row(&pool, &ws, &sha).await;

    let second = flush_wiki(&pool, &ws, &config).await;
    let sha = second.sha.expect("sha");
    assert_row(&pool, &ws, &sha).await;
}

/// Stop graph and both search nodes. Flush still commits. `graph_jobs` still upserts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cluster_down() {
    let _guard = LOCK.lock().await;
    let world = world();
    ready(world).await;
    let pool = cluster_pool(&world.pg).await;
    let ws = workspace();
    let wiki = tempfile::tempdir().expect("wiki");
    let config = WikiConfig::new(wiki.path());
    let _paused = pause_cluster(world);
    let outcome = flush_wiki(&pool, &ws, &config).await;
    assert!(outcome.committed, "Cluster down");
    let sha = outcome.sha.expect("sha");
    assert_row(&pool, &ws, &sha).await;
}

/// Two workers: one holds the writer lease. The other's writes are fenced.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_claim() {
    let _guard = LOCK.lock().await;
    let world = world();
    unpause_board(world);
    ready(world).await;
    let pool = cluster_pool(&world.pg).await;
    let ws = workspace();
    ensure_wiki(&world.pg, &ws, 2, 1).await.expect("wiki");
    let cluster = cluster(&world.pg).await;
    let layout = venus_graph::open_writer(&cluster, &ws, "worker-a")
        .await
        .expect("open")
        .expect("worker-a holds the lease");

    sqlx::query(
        "INSERT INTO graph_jobs (workspace_id, wiki_sha, dirty_doc_ids, state)
         VALUES ($1::uuid, 'sha-flush', $2::text[], 'pending')",
    )
    .bind(&ws)
    .bind(vec![PAGE_DOC_UUID.to_string()])
    .execute(&pool)
    .await
    .expect("pending row");

    let (ours, theirs) = tokio::join!(
        claim_job(&pool, &ws, "worker-a"),
        claim_job(&pool, &ws, "worker-b"),
    );
    let job = ours.expect("claim").expect("One claim");
    assert!(theirs.expect("other claim").is_none(), "One claim");
    assert_eq!(job.wiki_sha, "sha-flush");
    assert!(job.fence > 0, "One claim");

    let holder: String =
        sqlx::query_scalar("SELECT holder FROM repl_lease WHERE name = $1 AND lease_until > now()")
            .bind(lease_name(&ws))
            .fetch_one(&pool)
            .await
            .expect("lease");
    assert_eq!(holder, "worker-a", "One claim");

    let lsn = venus_graph::project(&layout, &job.workspace_id, &[sample()])
        .await
        .expect("write");
    let logged = surrealastic::raw(
        &cluster,
        "0",
        &world.graph_url,
        "graph",
        &graph_database(&ws),
        &format!("SELECT tag FROM _repl_log:{lsn}"),
    )
    .await
    .expect("log");
    assert!(logged.contains(&ws), "One claim {logged}");

    let err = write_job(&cluster, &pool, &ws, "worker-b", &[sample()])
        .await
        .expect_err("fenced");
    assert!(err.to_string().contains("fenced"), "One claim: {err:#}");
    let reply = apply(
        &cluster,
        "0",
        &world.graph_url,
        "graph",
        &graph_database(&ws),
        &Entry {
            lsn: 1,
            prev: 0,
            prev_fence: 0,
            fence: 0,
            tag: "worker-b".into(),
            at: "2026-10-08T00:00:00Z".into(),
            body: Body::new(),
        },
    )
    .await
    .expect("other write");
    assert_eq!(reply.status, ApplyStatus::Fenced, "One claim");

    let recorded = record_sha(&pool, &job, "worker-a").await.expect("record");
    assert_eq!(recorded, "sha-flush", "One claim");
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM graph_jobs WHERE workspace_id = $1::uuid")
            .bind(&ws)
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(left, 0, "One claim");
    drop(layout);
}

fn sample() -> Doc {
    Doc {
        doc_id: "doca".into(),
        git_path: "spec/doca.md".into(),
        title: "Title doca".into(),
        indexed_sha: "sha-1".into(),
        heading: "Overview".into(),
        body: "body of doca".into(),
        mention: "11111111-1111-4111-8111-111111111111".into(),
        remove: false,
    }
}

async fn assert_row(pool: &PgPool, ws: &str, sha: &str) {
    let rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM graph_jobs WHERE workspace_id = $1::uuid")
            .bind(ws)
            .fetch_one(pool)
            .await
            .expect("graph_jobs count");
    assert_eq!(rows, 1, "one graph_jobs row");
    let (wiki_sha, dirty): (String, Vec<String>) = sqlx::query_as(
        "SELECT wiki_sha, dirty_doc_ids FROM graph_jobs WHERE workspace_id = $1::uuid",
    )
    .bind(ws)
    .fetch_one(pool)
    .await
    .expect("graph_jobs row");
    assert_eq!(wiki_sha, sha, "wiki_sha");
    assert!(
        dirty.iter().any(|id| id == PAGE_DOC_UUID),
        "dirty doc ids {dirty:?}"
    );
    let flushed: Vec<String> =
        sqlx::query_scalar("SELECT git_sha FROM last_flushed WHERE workspace_id = $1::uuid")
            .bind(ws)
            .fetch_all(pool)
            .await
            .expect("last_flushed");
    assert!(!flushed.is_empty(), "last_flushed");
    assert!(
        flushed.iter().all(|git| git == sha),
        "last_flushed matches {flushed:?} {sha}"
    );
}

async fn flush_wiki(
    pool: &PgPool,
    ws: &str,
    wiki: &WikiConfig,
) -> venus_sidecar::queue::FlushOutcome {
    let bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../venus-sidecar/tests/fixtures/seed-home.yjs"),
    )
    .expect("seed-home.yjs");
    venus_hub::db::push_update(pool, ws, PAGE_DOC_UUID, &bytes)
        .await
        .expect("pin");
    sqlx::query("DELETE FROM jobs WHERE workspace_id = $1::uuid")
        .bind(ws)
        .execute(pool)
        .await
        .expect("clear job");
    sqlx::query(
        "INSERT INTO jobs (workspace_id, reason, not_before)
         VALUES ($1::uuid, 'flush', now())",
    )
    .bind(ws)
    .execute(pool)
    .await
    .expect("jobs row");
    let mut tx = pool.begin().await.expect("lease begin");
    let owned: Option<String> = sqlx::query_scalar(
        "UPDATE jobs
         SET owner = 'flush-worker', lease_until = now() + interval '2 minutes'
         WHERE workspace_id = $1::uuid
         RETURNING workspace_id::text",
    )
    .bind(ws)
    .fetch_optional(&mut *tx)
    .await
    .expect("lease");
    tx.commit().await.expect("lease commit");
    assert_eq!(owned.as_deref(), Some(ws));
    let mut pins = PinMap::new();
    flush_claimed(
        pool,
        Claim {
            workspace_id: ws.to_string(),
            owner: "flush-worker".into(),
        },
        &mut pins,
        wiki,
        Duration::ZERO,
    )
    .await
    .expect("flush")
}

fn workspace() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("{:08x}-1111-4111-a111-{:012x}", std::process::id(), n)
}

async fn ready(world: &World) {
    let pool = cluster_pool(&world.pg).await;
    venus_hub::db::migrate(&pool).await.expect("hub migrate");
    migrate_allocation(&world.pg).await.expect("allocation");
    set_node_url(&world.pg, "0", &world.graph_url)
        .await
        .expect("graph url");
}

fn config() -> Config {
    let mut config = Config::from_env();
    config.lease = Duration::from_secs(120);
    config.renew = Duration::from_secs(30);
    config.probe = Duration::from_millis(200);
    config
}

async fn cluster(pg: &str) -> Cluster {
    Cluster::connect(pg, config()).await.expect("cluster")
}

async fn cluster_pool(pg: &str) -> PgPool {
    for _ in 0..40 {
        if let Ok(pool) = PgPool::connect(pg).await {
            if sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(&pool)
                .await
                .is_ok()
            {
                return pool;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("postgres pool {pg}");
}

struct World {
    pg: String,
    graph_url: String,
    graph_container: String,
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        for name in [
            "venus-surreal-graph",
            "venus-surreal-search-0",
            "venus-surreal-search-1",
        ] {
            let listed = docker(&["ps", "-aq", "--filter", &format!("name={name}")]);
            for id in String::from_utf8_lossy(&listed.stdout).split_whitespace() {
                let _ = docker(&["unpause", id]);
                wait_healthy(id);
            }
        }
        graph_up(&["surreal-search-0", "surreal-search-1"]);
        for port in [28731_u16, 28732] {
            wait_port(port);
        }
        let (graph_url, graph_container) = ensure_graph();
        let name = format!("venus-enqueue-pg-{}", std::process::id());
        let _ = docker(&["rm", "-f", &name]);
        let started = docker(&[
            "run",
            "-d",
            "--name",
            &name,
            "-e",
            "POSTGRES_USER=venus",
            "-e",
            "POSTGRES_PASSWORD=venus",
            "-e",
            "POSTGRES_DB=venus",
            "-p",
            "127.0.0.1::5432",
            "postgres:16",
        ]);
        assert!(
            started.status.success(),
            "{}",
            String::from_utf8_lossy(&started.stderr)
        );
        let mapped = docker(&["port", &name, "5432"]);
        let host_port = published_port(&mapped.stdout);
        wait_pg(&name);
        *PG_NAME.lock().unwrap_or_else(|err| err.into_inner()) = name;
        unsafe {
            atexit(remove_pg);
        }
        World {
            pg: format!("postgres://venus:venus@127.0.0.1:{host_port}/venus?sslmode=disable"),
            graph_url,
            graph_container,
        }
    })
}

fn ensure_graph() -> (String, String) {
    wait_healthy("venus-surreal-graph-1");
    let up = docker(&[
        "compose",
        "--profile",
        "graph",
        "up",
        "-d",
        "--wait",
        "--wait-timeout",
        "60",
        "--no-deps",
        "surreal-graph",
    ]);
    if up.status.success() && surreal_on(28730) {
        return ("http://127.0.0.1:28730".into(), service_id("surreal-graph"));
    }
    let name = format!("venus-enqueue-graph-{}", std::process::id());
    let _ = docker(&["rm", "-f", &name]);
    let started = docker(&[
        "run",
        "-d",
        "--name",
        &name,
        "-p",
        "127.0.0.1::8000",
        "-u",
        "0:0",
        "-e",
        "SURREAL_USER=venus",
        "-e",
        "SURREAL_PASS=venus",
        SURREAL_IMAGE,
        "start",
        "rocksdb:/data/graph.db",
    ]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let mapped = docker(&["port", &name, "8000"]);
    let host_port = published_port(&mapped.stdout);
    wait_port(host_port);
    (format!("http://127.0.0.1:{host_port}"), name)
}

fn surreal_on(port: u16) -> bool {
    let out = Command::new("curl")
        .args([
            "-sI",
            "--max-time",
            "2",
            &format!("http://127.0.0.1:{port}/health"),
        ])
        .output();
    let Ok(out) = out else { return false };
    if !out.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    text.contains("200") && !text.contains("nginx")
}

fn wait_port(port: u16) {
    for _ in 0..240 {
        if surreal_on(port) {
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
    panic!(":{port} did not become healthy");
}

fn wait_pg(name: &str) {
    for _ in 0..60 {
        let ready = docker(&["exec", name, "pg_isready", "-U", "venus", "-d", "venus"]);
        if ready.status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
    panic!("postgres did not become ready");
}

struct Paused(Vec<String>);
impl Drop for Paused {
    fn drop(&mut self) {
        for id in &self.0 {
            let _ = docker(&["unpause", id]);
        }
    }
}

fn pause_cluster(world: &World) -> Paused {
    let mut ids = vec![world.graph_container.clone()];
    for service in ["surreal-search-0", "surreal-search-1"] {
        let id = service_id(service);
        assert!(!id.is_empty(), "missing {service}");
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    for id in &ids {
        let _ = docker(&["unpause", id]);
        let out = docker(&["pause", id]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Paused(ids)
}

fn unpause_board(world: &World) {
    let _ = docker(&["unpause", &world.graph_container]);
    for service in ["surreal-search-0", "surreal-search-1", "surreal-graph"] {
        let id = service_id(service);
        if !id.is_empty() {
            let _ = docker(&["unpause", &id]);
        }
    }
}

/// `compose up --wait` fails at once on a container Docker still reports
/// `unhealthy` after an unpause, or whose name a removal from another test
/// binary still holds. Both clear within seconds.
fn graph_up(services: &[&str]) {
    let mut args = vec![
        "compose",
        "--profile",
        "graph",
        "up",
        "-d",
        "--wait",
        "--wait-timeout",
        "300",
        "--no-deps",
    ];
    args.extend_from_slice(services);
    for attempt in 1..=5 {
        for service in services {
            wait_healthy(&format!("venus-{service}-1"));
        }
        let out = docker(&args);
        if out.status.success() {
            return;
        }
        assert!(
            attempt < 5,
            "docker {}\n{}\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        thread::sleep(Duration::from_secs(2));
    }
}

/// Docker reports a container `unhealthy` while paused and until its next probe
/// after unpause. `compose up --wait` fails on that instead of waiting.
fn wait_healthy(id: &str) {
    for _ in 0..60 {
        let out = docker(&[
            "inspect",
            "-f",
            "{{.State.Running}} {{if .State.Health}}{{.State.Health.Status}}{{end}}",
            id,
        ]);
        let text = String::from_utf8_lossy(&out.stdout);
        if text.trim() != "true unhealthy" {
            return;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn service_id(service: &str) -> String {
    let out = compose(&["ps", "-q", service]);
    out.lines().next().unwrap_or("").trim().to_string()
}

fn published_port(stdout: &[u8]) -> u16 {
    String::from_utf8_lossy(stdout)
        .lines()
        .find_map(|line| line.trim().rsplit(':').next()?.parse().ok())
        .unwrap_or_else(|| panic!("port map {}", String::from_utf8_lossy(stdout)))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|err| panic!("docker {}: {err}", args.join(" ")))
}

fn compose(args: &[&str]) -> String {
    let mut full = vec!["compose"];
    full.extend_from_slice(args);
    let out = docker(&full);
    assert!(
        out.status.success(),
        "docker {}\n{}\n{}",
        full.join(" "),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

static PG_NAME: Mutex<String> = Mutex::new(String::new());

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}

extern "C" fn remove_pg() {
    let Ok(name) = PG_NAME.lock() else { return };
    if !name.is_empty() {
        let _ = Command::new("docker")
            .args(["rm", "-f", name.as_str()])
            .output();
    }
    let graph = format!("venus-enqueue-graph-{}", std::process::id());
    let _ = Command::new("docker")
        .args(["rm", "-f", graph.as_str()])
        .output();
}
