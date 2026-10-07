//! Step 5 live checks. Profile `graph` and a scratch Postgres.
//! `cargo test -p surrealastic --test layout_live`
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use surrealastic::{
    log_ids, raw, remove_database, shard_set_id, Body, BoxFut, Cluster, Config, Item, Layout,
    LayoutRow, Owner, SetRow,
};

const NS: &str = "layout";

struct World {
    pg: String,
}

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}

static PG_NAME: Mutex<String> = Mutex::new(String::new());
static GRAPH: OnceLock<String> = OnceLock::new();

extern "C" fn remove_pg() {
    let Ok(name) = PG_NAME.lock() else { return };
    if name.is_empty() {
        return;
    }
    let _ = Command::new("docker").args(["rm", "-f", name.as_str()]).output();
    let graph = format!("surrealastic-graph-{}", std::process::id());
    let _ = Command::new("docker").args(["rm", "-f", graph.as_str()]).output();
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

fn surreal_on(port: u16) -> bool {
    let out = Command::new("curl")
        .args(["-sI", "--max-time", "2", &format!("http://127.0.0.1:{port}/health")])
        .output();
    let Ok(out) = out else { return false };
    if !out.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    text.contains("200") && !text.contains("nginx")
}

/// Compose publishes the graph on :8000. When that port is already taken, the
/// commit set runs in a scratch SurrealDB container on a free port.
fn ensure_graph() -> String {
    let up = docker(&[
        "compose", "--profile", "graph", "up", "-d", "--wait", "--wait-timeout", "120",
        "--no-deps", "surreal-graph",
    ]);
    if up.status.success() && surreal_on(8000) {
        return "http://127.0.0.1:8000".to_string();
    }
    let name = format!("surrealastic-graph-{}", std::process::id());
    let _ = docker(&["rm", "-f", &name]);
    let started = docker(&[
        "run", "-d", "--name", &name,
        "-p", "127.0.0.1::8000",
        "-u", "0:0",
        "-e", "SURREAL_USER=venus",
        "-e", "SURREAL_PASS=venus",
        "surrealdb/surrealdb:v2.7.0",
        "start", "rocksdb:/data/graph.db",
    ]);
    assert!(
        started.status.success(),
        "graph container: {}",
        String::from_utf8_lossy(&started.stderr)
    );
    let mapped = docker(&["port", &name, "8000"]);
    let host_port = String::from_utf8_lossy(&mapped.stdout)
        .trim()
        .rsplit(':')
        .next()
        .unwrap()
        .parse::<u16>()
        .unwrap();
    wait_port(host_port);
    format!("http://127.0.0.1:{host_port}")
}

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        let listed = docker(&["ps", "-aq", "--filter", "name=surrealastic-"]);
        for id in String::from_utf8_lossy(&listed.stdout).split_whitespace() {
            let _ = docker(&["rm", "-f", id]);
        }
        compose(&[
            "--profile", "graph", "up", "-d", "--wait", "--wait-timeout", "300", "--no-deps",
            "surreal-search-0", "surreal-search-1",
        ]);
        for port in [8001_u16, 8002] {
            wait_port(port);
        }
        let graph = ensure_graph();
        let _ = GRAPH.set(graph);
        let name = format!("surrealastic-pg-{}", std::process::id());
        let _ = docker(&["rm", "-f", &name]);
        let started = docker(&[
            "run", "-d", "--name", &name,
            "-e", "POSTGRES_USER=venus",
            "-e", "POSTGRES_PASSWORD=venus",
            "-e", "POSTGRES_DB=venus",
            "-p", "127.0.0.1::5432",
            "postgres:16",
        ]);
        assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
        let mapped = docker(&["port", &name, "5432"]);
        let host_port = String::from_utf8_lossy(&mapped.stdout)
            .trim()
            .rsplit(':')
            .next()
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let pg = format!("postgres://venus:venus@127.0.0.1:{host_port}/venus?sslmode=disable");
        *PG_NAME.lock().unwrap_or_else(|err| err.into_inner()) = name.clone();
        unsafe {
            atexit(remove_pg);
        }
        for _ in 0..60 {
            let ready = docker(&["exec", &name, "psql", "-U", "venus", "-d", "venus", "-c", "SELECT 1"]);
            if ready.status.success() {
                return World { pg };
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!("postgres did not become ready");
    })
}

fn service_id(service: &str) -> String {
    let out = compose(&["ps", "-q", service]);
    out.lines().next().unwrap_or("").trim().to_string()
}

struct Paused(Vec<String>);
impl Drop for Paused {
    fn drop(&mut self) {
        for id in &self.0 {
            let _ = docker(&["unpause", id]);
        }
    }
}

fn pause(services: &[&str]) -> Paused {
    let ids: Vec<String> = services.iter().map(|service| service_id(service)).collect();
    for id in &ids {
        let out = docker(&["pause", id]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    Paused(ids)
}

fn wait_port(port: u16) {
    for _ in 0..60 {
        let out = Command::new("curl")
            .args(["-sf", "--max-time", "2", &format!("http://127.0.0.1:{port}/health")])
            .output();
        if out.map(|o| o.status.success()).unwrap_or(false) {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!(":{port} did not become healthy");
}

fn config() -> Config {
    let mut config = Config::from_env();
    config.lease = Duration::from_secs(120);
    config.renew = Duration::from_secs(30);
    config
}

async fn cluster() -> Cluster {
    Cluster::connect(&world().pg, config()).await.expect("cluster")
}

async fn gate() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    LOCK.lock().await
}

fn stamp() -> String {
    static N: AtomicUsize = AtomicUsize::new(1);
    format!("{}{:x}", std::process::id(), N.fetch_add(1, Ordering::Relaxed))
}

struct Hooks {
    rebuilds: AtomicUsize,
    lost: AtomicUsize,
    schema: Vec<String>,
}

impl Owner for Hooks {
    fn schema<'a>(&'a self, _set: &'a str) -> BoxFut<'a, anyhow::Result<Vec<String>>> {
        let schema = self.schema.clone();
        Box::pin(async move { Ok(schema) })
    }
    fn rebuild<'a>(&'a self, _set: &'a str) -> BoxFut<'a, anyhow::Result<()>> {
        self.rebuilds.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
    fn lost<'a>(&'a self, _set: &'a str, _tags: &'a [String]) -> BoxFut<'a, anyhow::Result<()>> {
        self.lost.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

struct Scratch {
    commit_db: String,
    shards: Vec<ShardDb>,
}

struct ShardDb {
    set_id: String,
    database: String,
    nodes: Vec<&'static str>,
}

fn node_url(node: &str) -> &'static str {
    match node {
        "g8000" => GRAPH.get().expect("graph endpoint").as_str(),
        "n8001" => "http://127.0.0.1:8001",
        "n8002" => "http://127.0.0.1:8002",
        other => panic!("unknown node {other}"),
    }
}

async fn scratch(
    cluster: &Cluster,
    homes: &[Vec<&'static str>],
    next: Option<i32>,
    owner: Option<Arc<dyn Owner>>,
) -> (Layout, Scratch) {
    for (node, url, pool, zone) in [
        ("g8000", node_url("g8000"), "graph", "z0"),
        ("n8001", "http://127.0.0.1:8001", "search", "z1"),
        ("n8002", "http://127.0.0.1:8002", "search", "z2"),
    ] {
        cluster.upsert_node(node, pool, url, zone, 1, "up").await.unwrap();
    }
    let id = stamp();
    let db_id = format!("db{id}");
    let commit_set = format!("{db_id}:commit");
    let commit_db = format!("c{id}");
    let lease = format!("writer:{db_id}");
    cluster
        .upsert_set(&SetRow {
            set_id: commit_set.clone(),
            pool: "graph".into(),
            namespace: NS.into(),
            database: commit_db.clone(),
            copies: 1,
            ack: 1,
            lease: lease.clone(),
        })
        .await
        .unwrap();
    cluster.upsert_copy(&commit_set, "g8000", "in_sync").await.unwrap();
    let mut shards = Vec::new();
    let span = homes.len() as i32;
    let shard_count = if next.is_some() { 2 } else { span };
    for shard in 0..span {
        let set_id = shard_set_id(&db_id, 1, shard);
        let database = format!("s{id}{shard}");
        let nodes = homes[shard as usize].clone();
        cluster
            .upsert_set(&SetRow {
                set_id: set_id.clone(),
                pool: "search".into(),
                namespace: NS.into(),
                database: database.clone(),
                copies: nodes.len() as i32,
                ack: 1,
                lease: lease.clone(),
            })
            .await
            .unwrap();
        for node in &nodes {
            cluster.upsert_copy(&set_id, node, "in_sync").await.unwrap();
        }
        shards.push(ShardDb { set_id, database, nodes });
    }
    cluster
        .upsert_layout(&LayoutRow {
            db_id: db_id.clone(),
            commit_set,
            epoch: 1,
            shard_count,
            shard_count_next: next,
            replica_count: 1,
            shard_ack: 1,
        })
        .await
        .unwrap();
    let layout = Layout::open(cluster.clone(), &db_id, "holder", owner).await.expect("layout");
    (
        layout,
        Scratch { commit_db, shards },
    )
}

async fn finish(cluster: &Cluster, scratch: &Scratch) {
    let _ = remove_database(cluster, "g8000", node_url("g8000"), NS, &scratch.commit_db).await;
    for shard in &scratch.shards {
        for node in &shard.nodes {
            let _ = remove_database(cluster, node, node_url(node), NS, &shard.database).await;
        }
    }
}

fn row_item(key: i64, name: &str) -> Item {
    Item {
        key,
        item: name.into(),
        tag: "job".into(),
        rids: vec![format!("row:{key}")],
        body: Body::new().upsert(&format!("row:{key}"), serde_json::json!({ "n": key })),
        delete: false,
    }
}

async fn q(cluster: &Cluster, node: &str, database: &str, sql: &str) -> String {
    raw(cluster, node, node_url(node), NS, database, sql)
        .await
        .unwrap_or_else(|err| panic!("{sql}: {err}"))
}

async fn logs(cluster: &Cluster, node: &str, database: &str) -> Vec<i64> {
    log_ids(cluster, node, node_url(node), NS, database, 1, 500)
        .await
        .unwrap()
}

fn commit_is_number(commit: i64) {
    assert!(commit > 0, "{commit}");
    assert!(
        !commit.to_string().contains("shard"),
        "write result names a shard: {commit}"
    );
}

async fn until<F, Fut>(label: &str, mut probe: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        if probe().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("timed out waiting for {label}");
}

#[tokio::test]
async fn commit_returns_and_queued_apply() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["g8000"], vec!["n8001", "n8002"]],
        None,
        None,
    )
    .await;
    let paused = pause(&["surreal-search-0", "surreal-search-1"]);
    let commit = layout
        .write(Body::new(), vec![row_item(0, "a"), row_item(1, "b")])
        .await
        .expect("commit acks");
    commit_is_number(commit);
    let on_commit = q(&cluster, "g8000", &scratch.commit_db, "SELECT * FROM _layout_item;").await;
    assert!(on_commit.contains("a") && on_commit.contains("b"), "{on_commit}");
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await.len(), 1);
    let shard0 = q(&cluster, "g8000", &scratch.shards[0].database, "SELECT * FROM row;").await;
    assert!(shard0.contains("row:0") || shard0.contains('0'), "{shard0}");
    let lag = layout.lag().await.unwrap();
    assert_eq!(lag.iter().find(|row| row.shard == 0).unwrap().behind, 0);
    assert!(lag.iter().find(|row| row.shard == 1).unwrap().behind > 0, "{lag:?}");
    drop(paused);
    wait_port(8001);
    wait_port(8002);
    until("queued item on shard 1", || {
        let cluster = cluster.clone();
        let database = scratch.shards[1].database.clone();
        async move {
            for node in ["n8001", "n8002"] {
                let text = raw(&cluster, node, node_url(node), NS, &database, "SELECT * FROM row;")
                    .await
                    .unwrap_or_default();
                if text.contains("row:1") || text.contains("n: 1") || text.contains("\"n\":1") {
                    return true;
                }
            }
            false
        }
    })
    .await;
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await.len(), 1);
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn queue_order() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["g8000"], vec!["n8001", "n8002"]],
        None,
        None,
    )
    .await;
    let paused = pause(&["surreal-search-0", "surreal-search-1"]);
    let mut third = 0;
    for n in 1..=3 {
        let mut item = row_item(1, "seq");
        item.body = Body::new().upsert("row:1", serde_json::json!({ "n": n }));
        item.rids = vec!["row:1".into()];
        third = layout.write(Body::new(), vec![item]).await.expect("commit");
        commit_is_number(third);
    }
    drop(paused);
    wait_port(8001);
    wait_port(8002);
    let database = scratch.shards[1].database.clone();
    until("three commits on shard 1", || {
        let cluster = cluster.clone();
        let database = database.clone();
        async move { logs(&cluster, "n8001", &database).await.len() >= 3 }
    })
    .await;
    let text = q(&cluster, "n8001", &scratch.shards[1].database, "SELECT * FROM row:1;").await;
    assert!(text.contains('3'), "{text}");
    let cursor = q(
        &cluster,
        "n8001",
        &scratch.shards[1].database,
        "SELECT * FROM _layout:cursor;",
    )
    .await;
    assert!(cursor.contains(&third.to_string()), "{cursor} third {third}");
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await.len(), 3);
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn one_copy_down() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["n8001", "n8002"], vec!["n8001"]],
        None,
        None,
    )
    .await;
    let paused = pause(&["surreal-search-1"]);
    let commit = layout.write(Body::new(), vec![row_item(0, "solo")]).await.expect("commit");
    commit_is_number(commit);
    let on_a = q(&cluster, "n8001", &scratch.shards[0].database, "SELECT * FROM row;").await;
    assert!(on_a.contains("solo") || on_a.contains("row:0"), "{on_a}");
    let copies = cluster.copies(&scratch.shards[0].set_id).await.unwrap();
    let down = copies.iter().find(|copy| copy.node_id == "n8002").unwrap();
    assert_eq!(down.state, "lagging");
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await.len(), 1);
    drop(paused);
    wait_port(8002);
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn item_table_collision_and_delete() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(&cluster, &[vec!["n8001"], vec!["n8002"]], None, None).await;
    let commit = layout
        .write(Body::new(), vec![row_item(4, "kept")])
        .await
        .unwrap();
    commit_is_number(commit);
    let row = q(&cluster, "g8000", &scratch.commit_db, "SELECT * FROM _layout_item:4;").await;
    for field in ["kept", "job", "row:4", &commit.to_string()] {
        assert!(row.contains(field), "{field} missing in {row}");
    }
    let before = logs(&cluster, "g8000", &scratch.commit_db).await;
    let err = layout
        .write(
            Body::new().statement("THROW \"nope\";").unwrap(),
            vec![row_item(8, "absent")],
        )
        .await
        .expect_err("throw");
    assert!(!err.to_string().contains("shard"), "{err}");
    let missed = q(&cluster, "g8000", &scratch.commit_db, "SELECT * FROM _layout_item:8;").await;
    assert!(!missed.contains("absent"), "{missed}");
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await, before);

    let err = layout
        .write(Body::new(), vec![row_item(4, "other")])
        .await
        .expect_err("collision");
    assert!(err.to_string().contains("key collision"), "{err}");
    assert!(!err.to_string().contains("shard"), "{err}");
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await, before);
    let still = q(&cluster, "n8001", &scratch.shards[0].database, "SELECT * FROM row;").await;
    assert!(still.contains("kept") || still.contains("row:4"), "{still}");

    let mut gone = row_item(4, "kept");
    gone.delete = true;
    let deleted = layout.write(Body::new(), vec![gone]).await.unwrap();
    commit_is_number(deleted);
    let item = q(&cluster, "g8000", &scratch.commit_db, "SELECT * FROM _layout_item:4;").await;
    assert!(!item.contains("kept"), "{item}");
    let shard = q(&cluster, "n8001", &scratch.shards[0].database, "SELECT * FROM row:4;").await;
    assert!(!shard.contains("kept"), "{shard}");
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn cursor_and_fresh_read() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["g8000"], vec!["n8001", "n8002"]],
        None,
        None,
    )
    .await;
    let first = layout
        .write(Body::new(), vec![row_item(0, "c0"), row_item(1, "c1")])
        .await
        .unwrap();
    let cursor = q(
        &cluster,
        "g8000",
        &scratch.shards[0].database,
        "SELECT * FROM _layout:cursor;",
    )
    .await;
    assert!(cursor.contains(&first.to_string()), "{cursor}");
    let lag = layout.lag().await.unwrap();
    assert!(lag.iter().all(|row| row.behind == 0), "{lag:?}");

    let paused = pause(&["surreal-search-0", "surreal-search-1"]);
    let last = layout
        .write(Body::new(), vec![row_item(2, "c2"), row_item(3, "c3")])
        .await
        .unwrap();
    let lag = layout.lag().await.unwrap();
    assert!(lag.iter().find(|row| row.shard == 1).unwrap().behind > 0, "{lag:?}");
    let read = layout.read("SELECT * FROM row;", Some(last)).await.unwrap();
    assert!(
        read.partial.iter().any(|part| part.shard == 1 && part.reason == "behind"),
        "{read:?}"
    );
    drop(paused);
    wait_port(8001);
    wait_port(8002);
    until("shard 1 caught up", || {
        let layout = &layout;
        async move {
            layout
                .lag()
                .await
                .map(|rows| rows.iter().all(|row| row.behind == 0))
                .unwrap_or(false)
        }
    })
    .await;
    let read = layout.read("SELECT * FROM row;", Some(last)).await.unwrap();
    assert!(read.partial.is_empty(), "{read:?}");
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn schema_reaches_a_copy_that_was_down() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let hooks = Arc::new(Hooks {
        rebuilds: AtomicUsize::new(0),
        lost: AtomicUsize::new(0),
        schema: vec![
            "DEFINE TABLE IF NOT EXISTS probe SCHEMAFULL;".into(),
            "DEFINE FIELD IF NOT EXISTS n ON probe TYPE int;".into(),
        ],
    });
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["n8001", "n8002"], vec!["n8001"]],
        None,
        Some(hooks.clone()),
    )
    .await;
    for node in ["n8001", "n8002"] {
        let info = q(&cluster, node, &scratch.shards[0].database, "INFO FOR TABLE probe;").await;
        assert!(info.to_ascii_lowercase().contains('n'), "{node} {info}");
        assert!(logs(&cluster, node, &scratch.shards[0].database).await.is_empty());
    }
    let paused = pause(&["surreal-search-1"]);
    let lsn = layout
        .define_on(
            &scratch.shards[0].set_id,
            &["DEFINE FIELD IF NOT EXISTS extra ON probe TYPE string;".into()],
        )
        .await
        .unwrap();
    assert_eq!(logs(&cluster, "n8001", &scratch.shards[0].database).await, vec![lsn]);
    drop(paused);
    wait_port(8002);
    layout.catch_up(&scratch.shards[0].set_id).await.unwrap();
    let info = q(&cluster, "n8002", &scratch.shards[0].database, "INFO FOR TABLE probe;").await;
    assert!(info.contains("extra"), "{info}");
    assert_eq!(logs(&cluster, "n8002", &scratch.shards[0].database).await.len(), 1);
    assert_eq!(hooks.rebuilds.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.lost.load(Ordering::SeqCst), 0);
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn empty_shard_refills_from_the_commit_set() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let hooks = Arc::new(Hooks {
        rebuilds: AtomicUsize::new(0),
        lost: AtomicUsize::new(0),
        schema: Vec::new(),
    });
    let (layout, scratch) = scratch(
        &cluster,
        &[vec!["n8001", "n8002"], vec!["n8001", "n8002"]],
        None,
        Some(hooks.clone()),
    )
    .await;
    layout
        .write(Body::new(), vec![row_item(0, "even"), row_item(1, "odd")])
        .await
        .unwrap();
    let sibling_before = logs(&cluster, "n8001", &scratch.shards[1].database).await;
    for node in ["n8001", "n8002"] {
        remove_database(&cluster, node, node_url(node), NS, &scratch.shards[0].database)
            .await
            .unwrap();
    }
    layout.refill(0).await.expect("refill");
    let restored = q(&cluster, "n8001", &scratch.shards[0].database, "SELECT * FROM row;").await;
    assert!(restored.contains("even") || restored.contains("row:0"), "{restored}");
    assert!(!restored.contains("odd"), "{restored}");
    let sibling_after = logs(&cluster, "n8001", &scratch.shards[1].database).await;
    assert_eq!(sibling_before, sibling_after);
    assert_eq!(hooks.rebuilds.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.lost.load(Ordering::SeqCst), 0);
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn dual_layout() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(
        &cluster,
        &[
            vec!["n8001"],
            vec!["n8001"],
            vec!["n8002"],
            vec!["n8002"],
        ],
        Some(4),
        None,
    )
    .await;
    let commit = layout.write(Body::new(), vec![row_item(2, "both")]).await.unwrap();
    commit_is_number(commit);
    let a = q(&cluster, "n8001", &scratch.shards[0].database, "SELECT * FROM row;").await;
    let b = q(&cluster, "n8002", &scratch.shards[2].database, "SELECT * FROM row;").await;
    assert!(a.contains("both") || a.contains("row:2"), "{a}");
    assert!(b.contains("both") || b.contains("row:2"), "{b}");
    finish(&cluster, &scratch).await;
}

#[tokio::test]
async fn entry_limit_live() {
    let _guard = gate().await;
    let cluster = cluster().await;
    let (layout, scratch) = scratch(&cluster, &[vec!["n8001"], vec!["n8002"]], None, None).await;
    let items: Vec<Item> = (0..300).map(|n| row_item(n * 2, &format!("i{n}"))).collect();
    let commit = layout.write(Body::new(), items).await.expect("commit");
    commit_is_number(commit);
    assert_eq!(logs(&cluster, "g8000", &scratch.commit_db).await.len(), 1);
    let ids = logs(&cluster, "n8001", &scratch.shards[0].database).await;
    assert_eq!(ids.len(), 2, "{ids:?}");
    for id in ids {
        let text = q(
            &cluster,
            "n8001",
            &scratch.shards[0].database,
            &format!("SELECT * FROM _repl_log:{id};"),
        )
        .await;
        let upserts = text.matches("UPSERT row:").count();
        assert!(upserts <= 256, "{id} has {upserts}");
        assert!(text.len() < 4 * 1024 * 1024, "{id} is {} bytes", text.len());
    }
    finish(&cluster, &scratch).await;
}
