//! Step 4 live checks. Profile `graph` search nodes and a scratch Postgres.
//! `cargo test -p surrealastic --test live --features fault`
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use surrealastic::{
    apply, dial_wss, explain_log, log_ids, raw, ApplyStatus, Body, Cluster, Config, Entry, Health,
    SetRow,
};
use tokio::task::JoinSet;
use tokio::time::timeout;

struct World {
    pg: String,
    pg_name: String,
}

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}

static PG_NAME: Mutex<String> = Mutex::new(String::new());

extern "C" fn remove_pg() {
    let Ok(name) = PG_NAME.lock() else { return };
    if name.is_empty() {
        return;
    }
    let _ = Command::new("docker").args(["rm", "-f", name.as_str()]).output();
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

fn world() -> &'static World {
    static WORLD: OnceLock<World> = OnceLock::new();
    WORLD.get_or_init(|| {
        sweep_scratch();
        for port in [8001_u16, 8002] {
            let listed = docker(&["ps", "--format", "{{.Names}}\t{{.Ports}}"]);
            let text = String::from_utf8_lossy(&listed.stdout);
            let needle = format!(":{port}->");
            let holders: Vec<_> = text
                .lines()
                .filter(|line| line.contains(&needle) && !line.contains("surreal-search"))
                .collect();
            assert!(holders.is_empty(), "port {port} is taken:\n{}", holders.join("\n"));
        }
        compose(&[
            "--profile", "graph", "up", "-d", "--wait", "--wait-timeout", "300", "--no-deps",
            "surreal-search-0", "surreal-search-1",
        ]);
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
                return World { pg, pg_name: name };
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!("postgres did not become ready");
    })
}

fn sweep_scratch() {
    let listed = docker(&["ps", "-aq", "--filter", "name=surrealastic-"]);
    for id in String::from_utf8_lossy(&listed.stdout).split_whitespace() {
        let _ = docker(&["rm", "-f", id]);
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn service_id(service: &str) -> String {
    let out = compose(&["ps", "-q", service]);
    out.lines().next().unwrap_or("").trim().to_string()
}

struct Paused(String);
impl Drop for Paused {
    fn drop(&mut self) {
        let _ = docker(&["unpause", &self.0]);
    }
}

struct Stopped(String);
impl Drop for Stopped {
    fn drop(&mut self) {
        let _ = docker(&["start", &self.0]);
    }
}

fn wait_port(port: u16) {
    let _ = world();
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

fn config_long() -> Config {
    let mut config = Config::from_env();
    config.lease = Duration::from_secs(60);
    config.renew = Duration::from_secs(20);
    config
}

fn node_pair() -> [(&'static str, &'static str, &'static str); 2] {
    [
        ("n8001", "http://127.0.0.1:8001", "z1"),
        ("n8002", "http://127.0.0.1:8002", "z2"),
    ]
}

async fn cluster_with(config: Config) -> Cluster {
    Cluster::connect(&world().pg, config).await.expect("cluster")
}

async fn scratch(cluster: &Cluster, ack: i32, copies: i32) -> (String, String) {
    let id = format!("s{}", uuid_ish());
    let database = format!("repl_t_{}", uuid_ish());
    let lease = format!("writer:{id}");
    for (node, url, zone) in node_pair().into_iter().take(copies as usize) {
        cluster
            .upsert_node(node, "search", url, zone, 1, "up")
            .await
            .unwrap();
    }
    cluster
        .upsert_set(&SetRow {
            set_id: id.clone(),
            pool: "search".into(),
            namespace: "search".into(),
            database: database.clone(),
            copies,
            ack,
            lease,
        })
        .await
        .unwrap();
    for (node, _, _) in node_pair().into_iter().take(copies as usize) {
        cluster.upsert_copy(&id, node, "in_sync").await.unwrap();
    }
    cluster.ensure(&id).await.unwrap();
    (id, database)
}

fn uuid_ish() -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{n:x}{:x}", std::process::id())
}

fn entry(lsn: i64, prev: i64, prev_fence: i64, fence: i64, body: Body) -> Entry {
    Entry {
        lsn,
        prev,
        prev_fence,
        fence,
        tag: "t".into(),
        at: "2026-10-07T00:00:00Z".into(),
        body,
    }
}

fn doc(n: i64) -> Body {
    Body::new().upsert("item:1", serde_json::json!({"n": n}))
}

async fn applied(cluster: &Cluster, node: &str, url: &str, database: &str) -> i64 {
    let text = raw(cluster, node, url, "search", database, "SELECT applied_lsn FROM ONLY _repl:state;")
        .await
        .unwrap();
    text.split("applied_lsn")
        .nth(1)
        .and_then(|rest| rest.chars().skip_while(|c| !c.is_ascii_digit() && *c != '-').take_while(|c| c.is_ascii_digit() || *c == '-').collect::<String>().parse().ok())
        .unwrap_or_else(|| panic!("applied_lsn in {text}"))
}

#[tokio::test]
async fn lease() {
    let _guard = lock();
    let cluster = cluster_with({
        let mut config = Config::from_env();
        config.lease = Duration::from_secs(30);
        config.renew = Duration::from_secs(10);
        config
    })
    .await;
    let (set_id, _) = scratch(&cluster, 1, 0).await;
    let a = cluster.claim(&set_id, "a").await.unwrap().expect("first claim");
    assert!(cluster.claim(&set_id, "b").await.unwrap().is_none(), "second claimant gets no row");
    assert_eq!(a.renew().await.unwrap(), Some(a.fence()));
    a.stop_renew();
    cluster.expire_lease(&format!("writer:{set_id}")).await.unwrap();
    let b = cluster.claim(&set_id, "b").await.unwrap().expect("claim after expiry");
    assert_eq!(b.fence(), a.fence() + 1);
}

#[tokio::test]
async fn renew_lost() {
    let _guard = lock();
    wait_port(8001);
    let cluster = cluster_with({
        let mut config = Config::from_env();
        config.lease = Duration::from_millis(800);
        config.renew = Duration::from_millis(200);
        config
    })
    .await;
    let (set_id, database) = scratch(&cluster, 1, 1).await;
    let writer = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    writer.write(&set_id, doc(1), "t").await.unwrap();
    let sends = writer.sends();
    let lsn = applied(&cluster, "n8001", "http://127.0.0.1:8001", &database).await;
    let _password = PgPassword(&world().pg_name);
    psql(&world().pg_name, "ALTER USER venus PASSWORD 'blocked'");
    terminate_others(&world().pg_name);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let err = writer.write(&set_id, doc(2), "t").await.unwrap_err();
    assert!(writer.sends() == sends, "sent after the lease died: {err}");
    assert_eq!(
        applied(&cluster, "n8001", "http://127.0.0.1:8001", &database).await,
        lsn
    );
}

struct PgPassword<'a>(&'a str);
impl Drop for PgPassword<'_> {
    fn drop(&mut self) {
        psql(self.0, "ALTER USER venus PASSWORD 'venus'");
    }
}

fn psql(container: &str, sql: &str) {
    let out = docker(&["exec", container, "psql", "-U", "venus", "-d", "venus", "-c", sql]);
    assert!(
        out.status.success(),
        "{sql}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn terminate_others(container: &str) {
    let _ = docker(&[
        "exec",
        container,
        "psql",
        "-U",
        "venus",
        "-d",
        "venus",
        "-c",
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()",
    ]);
}

#[tokio::test]
async fn fenced() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let cluster = cluster_with(config_long()).await;
    let (set_id, database) = scratch(&cluster, 2, 2).await;
    let a = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    a.write(&set_id, doc(1), "t").await.unwrap();
    a.stop_renew();
    cluster.expire_lease(&format!("writer:{set_id}")).await.unwrap();
    let b = cluster.claim(&set_id, "b").await.unwrap().unwrap();
    b.write(&set_id, doc(2), "t").await.unwrap();
    let err = a.write(&set_id, doc(3), "old").await.unwrap_err();
    assert!(err.to_string().contains("fenced"), "{err}");
    assert!(a.is_stopped());
    let sends = a.sends();
    assert!(a.write(&set_id, doc(4), "old").await.is_err());
    assert_eq!(a.sends(), sends, "stopped writer must not send");
    for (node, url, _) in node_pair() {
        let text = raw(&cluster, node, url, "search", &database, "SELECT fence, tag FROM _repl_log;").await.unwrap();
        assert!(!text.contains("old"), "{node} stored the fenced entry: {text}");
    }
}

#[tokio::test]
async fn gap_already_divergent_atomic() {
    let _guard = lock();
    wait_port(8001);
    let cluster = cluster_with(config_long()).await;
    let (_set, database) = scratch(&cluster, 1, 1).await;
    let url = "http://127.0.0.1:8001";
    let gap = apply(&cluster, "n8001", url, "search", &database, &entry(5, 4, 0, 1, doc(1)))
        .await
        .unwrap();
    assert_eq!(gap.status, ApplyStatus::Gap { at: 0 });
    assert_eq!(applied(&cluster, "n8001", url, &database).await, 0);

    let ok = apply(&cluster, "n8001", url, "search", &database, &entry(1, 0, 0, 1, doc(1)))
        .await
        .unwrap();
    assert_eq!(ok.status, ApplyStatus::Ok);
    let again = apply(&cluster, "n8001", url, "search", &database, &entry(1, 0, 0, 1, doc(1)))
        .await
        .unwrap();
    assert_eq!(again.status, ApplyStatus::Already);
    assert_eq!(log_ids(&cluster, "n8001", url, "search", &database, 1, 10).await.unwrap(), vec![1]);

    let ahead = apply(&cluster, "n8001", url, "search", &database, &entry(2, 0, 0, 1, doc(2)))
        .await
        .unwrap();
    assert_eq!(ahead.status, ApplyStatus::Divergent);
    let fence = apply(&cluster, "n8001", url, "search", &database, &entry(2, 1, 9, 1, doc(2)))
        .await
        .unwrap();
    assert_eq!(fence.status, ApplyStatus::Divergent);
    assert_eq!(applied(&cluster, "n8001", url, &database).await, 1);
    assert_eq!(log_ids(&cluster, "n8001", url, "search", &database, 1, 10).await.unwrap(), vec![1]);

    raw(
        &cluster,
        "n8001",
        url,
        "search",
        &database,
        "DEFINE TABLE item SCHEMAFULL; DEFINE FIELD n ON item TYPE int ASSERT $value > 0;",
    )
    .await
    .unwrap();
    let bad = Body::new()
        .upsert("item:9", serde_json::json!({"n": 1}))
        .upsert("item:10", serde_json::json!({"n": -1}));
    let err = apply(&cluster, "n8001", url, "search", &database, &entry(2, 1, 1, 1, bad))
        .await
        .unwrap_err();
    assert!(!err.to_string().contains("gap"), "{err}");
    let rows = raw(&cluster, "n8001", url, "search", &database, "SELECT * FROM item:9..=10;").await.unwrap();
    assert!(!rows.contains("item:9") && !rows.contains("item:10"), "rolled back: {rows}");
    assert_eq!(applied(&cluster, "n8001", url, &database).await, 1);
    assert_eq!(log_ids(&cluster, "n8001", url, "search", &database, 1, 10).await.unwrap(), vec![1]);
}

#[tokio::test]
async fn range_read() {
    let _guard = lock();
    wait_port(8001);
    let cluster = cluster_with(config_long()).await;
    let (set_id, database) = scratch(&cluster, 1, 1).await;
    let writer = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    for n in 1_i64..=101 {
        writer.write(&set_id, doc(n), "t").await.unwrap();
    }
    let url = "http://127.0.0.1:8001";
    let ids = log_ids(&cluster, "n8001", url, "search", &database, 98, 101).await.unwrap();
    assert_eq!(ids, vec![98, 99, 100, 101]);
    let plan = explain_log(&cluster, "n8001", url, "search", &database, 98, 101).await.unwrap();
    let lower = plan.to_ascii_lowercase();
    assert!(lower.contains("range"), "explain has no range read: {plan}");
    assert!(!lower.contains("iterate table"), "explain scanned the table: {plan}");
}

#[tokio::test]
async fn ack_policy() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let cluster = cluster_with(config_long()).await;
    let (fast, _) = scratch(&cluster, 1, 2).await;
    let writer = cluster.claim(&fast, "a").await.unwrap().unwrap();
    let id = service_id("surreal-search-1");
    let _paused = Paused(id.clone());
    let paused = docker(&["pause", &id]);
    assert!(paused.status.success(), "{}", String::from_utf8_lossy(&paused.stderr));
    let started = Instant::now();
    timeout(Duration::from_secs(5), writer.write(&fast, doc(1), "t"))
        .await
        .expect("ack 1 must return while :8002 is paused")
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    drop(_paused);
    wait_port(8002);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let (slow, _) = scratch(&cluster, 2, 2).await;
    let writer = cluster.claim(&slow, "a").await.unwrap().unwrap();
    let id = service_id("surreal-search-1");
    let paused = Paused(id.clone());
    let hold = docker(&["pause", &id]);
    assert!(hold.status.success(), "{}", String::from_utf8_lossy(&hold.stderr));
    let pending = writer.write(&slow, doc(2), "t");
    tokio::pin!(pending);
    tokio::select! {
        _ = &mut pending => panic!("ack 2 returned while :8002 was paused"),
        _ = tokio::time::sleep(Duration::from_millis(400)) => {}
    }
    drop(paused);
    timeout(Duration::from_secs(8), pending)
        .await
        .expect("ack 2 completes after :8002 resumes")
        .unwrap();
}

#[tokio::test]
async fn one_copy_down() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let cluster = cluster_with(config_long()).await;
    let (set_id, database) = scratch(&cluster, 1, 2).await;
    let id = service_id("surreal-search-1");
    cluster.disconnect("n8002").await;
    let _stopped = Stopped(id.clone());
    docker(&["stop", "-t", "1", &id]);
    let writer = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    writer.write(&set_id, doc(7), "t").await.unwrap();
    let row = raw(&cluster, "n8001", "http://127.0.0.1:8001", "search", &database, "SELECT * FROM item;").await.unwrap();
    assert!(row.contains("7"), "{row}");
    let copies = cluster.copies(&set_id).await.unwrap();
    let slow = copies.iter().find(|c| c.node_id == "n8002").unwrap();
    assert_eq!(slow.state, "lagging");
    assert_eq!(cluster.health(&set_id).await.unwrap(), Health::Yellow);
}

#[tokio::test]
async fn one_in_flight() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let mut config = config_long();
    config.lag_entries = 10_000;
    let cluster = cluster_with(config).await;
    let (set_id, database) = scratch(&cluster, 2, 2).await;
    let writer = std::sync::Arc::new(cluster.claim(&set_id, "a").await.unwrap().unwrap());
    let mut tasks = JoinSet::new();
    for n in 1_i64..=200 {
        let writer = std::sync::Arc::clone(&writer);
        let set_id = set_id.clone();
        tasks.spawn(async move { writer.write(&set_id, doc(n), "t").await.unwrap() });
    }
    while tasks.join_next().await.is_some() {}
    for (node, _, _) in node_pair() {
        assert!(writer.peak_in_flight(node).await <= 1, "{node} had two entries in flight");
    }
    for (node, url, _) in node_pair() {
        let ids = log_ids(&cluster, node, url, "search", &database, 1, 200).await.unwrap();
        assert_eq!(ids, (1..=200).collect::<Vec<_>>(), "{node} applied out of order: {ids:?}");
    }
}

#[tokio::test]
async fn lag_max() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let mut config = config_long();
    config.lag_entries = 8;
    config.lag = Duration::from_secs(30);
    let cluster = cluster_with(config).await;
    let (set_id, database) = scratch(&cluster, 1, 2).await;
    let writer = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    let id = service_id("surreal-search-1");
    let _paused = Paused(id.clone());
    let paused = docker(&["pause", &id]);
    assert!(paused.status.success(), "{}", String::from_utf8_lossy(&paused.stderr));
    let started = Instant::now();
    for n in 1_i64..=100 {
        writer.write(&set_id, doc(n), "t").await.unwrap();
    }
    assert!(started.elapsed() < Duration::from_secs(15), "writes waited on the paused copy");
    assert_eq!(writer.lagged_at("n8002").await, Some(8));
    let copies = cluster.copies(&set_id).await.unwrap();
    assert_eq!(copies.iter().find(|c| c.node_id == "n8002").unwrap().state, "lagging");
    assert_eq!(applied(&cluster, "n8001", "http://127.0.0.1:8001", &database).await, 100);
}

#[tokio::test]
async fn quiet_map() {
    let _guard = lock();
    wait_port(8001);
    wait_port(8002);
    let mut config = config_long();
    config.lag_entries = 10_000;
    config.lag = Duration::from_secs(60);
    let cluster = cluster_with(config).await;
    let (set_id, _) = scratch(&cluster, 1, 2).await;
    let writer = cluster.claim(&set_id, "a").await.unwrap().unwrap();
    let before = cluster.fingerprint(&set_id).await.unwrap();
    for n in 1_i64..=100 {
        writer.write(&set_id, doc(n), "t").await.unwrap();
    }
    let after = cluster.fingerprint(&set_id).await.unwrap();
    assert_eq!(before, after, "in-sync writes must not touch repl_copy");
}

#[tokio::test]
async fn tls() {
    let _guard = lock();
    let dir = std::env::temp_dir().join(format!("surrealastic-tls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let made = Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", key.to_str().unwrap(),
            "-out", cert.to_str().unwrap(),
            "-days", "1",
            "-subj", "/CN=127.0.0.1",
            "-addext", "subjectAltName=IP:127.0.0.1,DNS:localhost",
            "-addext", "basicConstraints=CA:FALSE",
            "-addext", "keyUsage=digitalSignature,keyEncipherment",
            "-addext", "extendedKeyUsage=serverAuth",
        ])
        .output()
        .expect("openssl");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let name = format!("surrealastic-tls-{}", std::process::id());
    let _ = docker(&["rm", "-f", &name]);
    let started = docker(&[
        "run", "-d", "--name", &name,
        "-p", "127.0.0.1:8019:8000",
        "-v", &format!("{}:/tls:ro", dir.display()),
        "-e", "SURREAL_USER=venus",
        "-e", "SURREAL_PASS=venus",
        "surrealdb/surrealdb:v2.7.0",
        "start", "--web-crt", "/tls/cert.pem", "--web-key", "/tls/key.pem",
        "--bind", "0.0.0.0:8000", "--user", "venus", "--pass", "venus",
        "rocksdb:/tmp/tls.db",
    ]);
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let cleanup = name.clone();
    let _drop = scopeguard_stop(cleanup);
    for _ in 0..40 {
        let ready = docker(&["exec", &name, "/surreal", "is-ready", "--endpoint", "http://127.0.0.1:8000"]);
        if ready.status.success() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let pem = std::fs::read_to_string(&cert).unwrap();
    dial_wss("127.0.0.1:8019", Some(&pem), "venus", "venus")
        .await
        .expect("wss with the test CA");
    let refused = dial_wss("127.0.0.1:8019", None, "venus", "venus").await;
    assert!(refused.is_err(), "wss without the test CA must be refused");
}

struct RemoveContainer(String);
impl Drop for RemoveContainer {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.0]);
    }
}

fn scopeguard_stop(name: String) -> RemoveContainer {
    RemoveContainer(name)
}
