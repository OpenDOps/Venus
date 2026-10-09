//! ss-cluster step-project.
//! `cargo test -p venus-graph --test project` with profile `graph`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use surrealastic::{log_ids, shard_set_id, Cluster, Config, Health};
use venus_graph::{
    commit_set_id, copy_pairs, db_id, ensure_wiki, graph_database, hkey, lease_name,
    legacy_table_names, map_snapshot, migrate_allocation, project, serve, set_node_url,
    shard_database, wiki_layout, Doc, LAYOUT_EPOCH, SURREAL_IMAGE, WORKSPACE_ID,
};

const DOC0: &str = "doca";
const DOC1: &str = "docb";
const HKEY0: i64 = 5_494_150_247_110_801_094;
const HKEY1: i64 = 2_293_668_409_663_586_427;
const MENTION: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn hkey_vectors() {
    assert_eq!(hkey(DOC0), HKEY0);
    assert_eq!(hkey(DOC1), HKEY1);
    assert_eq!(hkey("page0"), 4_009_681_261_430_118_155);
    for id in [DOC0, DOC1, "page0", "b0001"] {
        assert!(hkey(id) >= 0, "{id}");
    }
    assert_ne!(hkey(DOC0) % 2, hkey(DOC1) % 2, "fixtures share a shard");
}

#[tokio::test]
async fn migrate_and_new_wiki() {
    let pg = Pg::start();
    seed_legacy(&pg.url).await;
    migrate_allocation(&pg.url).await.expect("migrate");
    let layout = wiki_layout(&pg.url, WORKSPACE_ID).await.expect("layout");
    assert_eq!(layout.shard_count, 2, "Migrate");
    assert_eq!(layout.replica_count, 1, "Migrate");
    assert_eq!(layout.commit_set, commit_set_id(WORKSPACE_ID));
    assert_eq!(layout.shard_ack, 1);
    assert_eq!(layout.search_sets, 2);
    assert_eq!(layout.commit_copies, 1);
    assert!(
        legacy_table_names(&pg.url)
            .await
            .expect("legacy")
            .is_empty(),
        "Migrate"
    );
    let copies = copy_pairs(&pg.url, WORKSPACE_ID).await.expect("copies");
    let commit_nodes: Vec<_> = copies
        .iter()
        .filter(|(set, _)| set == &layout.commit_set)
        .map(|(_, node)| node.clone())
        .collect();
    assert_eq!(commit_nodes, vec!["0".to_string()]);
    for shard in 0..2 {
        let set = shard_set_id(&db_id(WORKSPACE_ID), LAYOUT_EPOCH, shard);
        let mut nodes: Vec<_> = copies
            .iter()
            .filter(|(id, _)| id == &set)
            .map(|(_, node)| node.clone())
            .collect();
        nodes.sort();
        assert_eq!(nodes, vec!["1".to_string(), "2".to_string()], "{set}");
    }
    let snap = map_snapshot(&pg.url).await.expect("snapshot");
    migrate_allocation(&pg.url).await.expect("migrate again");
    assert_eq!(
        map_snapshot(&pg.url).await.expect("snapshot"),
        snap,
        "Migrate"
    );

    let second = "22222222-2222-4222-8222-222222222222";
    ensure_wiki(&pg.url, second, 1, 1).await.expect("new wiki");
    let wiki = wiki_layout(&pg.url, second).await.expect("new wiki");
    assert_eq!(wiki.shard_count, 1, "New wiki");
    assert_eq!(wiki.search_sets, 1, "New wiki");
    assert_eq!(wiki.replica_count, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_live() {
    let world = world();
    let ws = format!("fix{}", std::process::id());
    let _ready = cluster_pool(&world.pg).await;
    migrate_allocation(&world.pg).await.expect("migrate");
    set_node_url(&world.pg, "0", &world.graph_url)
        .await
        .expect("graph url");
    ensure_wiki(&world.pg, &ws, 2, 1).await.expect("wiki");
    let cluster = cluster(&world.pg).await;
    let layout = venus_graph::open_writer(&cluster, &ws, "venus-graph")
        .await
        .expect("open")
        .expect("lease");

    schema_hook(&world, &ws).await;
    serve_holds(&world, &cluster).await;

    {
        let _paused = pause(&["surreal-search-1"]);
        let commit = project(
            &layout,
            "job-down",
            &[sample(DOC0, "sha-1"), sample(DOC1, "sha-1")],
        )
        .await
        .expect("Search node down");
        assert!(commit > 0, "Search node down");
        assert!(
            sql(
                28731,
                &format!(
                    "USE NS search DB {}; SELECT title FROM page:{DOC0};",
                    shard_database(&ws, LAYOUT_EPOCH, (hkey(DOC0) % 2) as i32)
                )
            )
            .contains("Title doca"),
            "Search node down"
        );
        assert!(
            sql(
                28731,
                &format!(
                    "USE NS search DB {}; SELECT title FROM page:{DOC1};",
                    shard_database(&ws, LAYOUT_EPOCH, (hkey(DOC1) % 2) as i32)
                )
            )
            .contains("Title docb"),
            "Search node down"
        );
        // Ack 1 returns before the paused copy answers. The probe or the send
        // timeout marks it lagging after the write.
        for shard in 0..2 {
            let set = shard_set_id(&db_id(&ws), LAYOUT_EPOCH, shard);
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let finger = cluster.fingerprint(&set).await.expect("fingerprint");
                if finger.iter().any(|row| row.starts_with("2:lagging:")) {
                    break;
                }
                assert!(Instant::now() < deadline, "Search node down {finger:?}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert_eq!(
                cluster.health(&set).await.expect("health"),
                Health::Yellow,
                "Search node down"
            );
        }
        let clocks = sql(
            world.graph_port,
            &format!(
                "USE NS graph DB {}; SELECT search_error, search_sha FROM page:{DOC0};",
                graph_database(&ws)
            ),
        );
        assert!(clocks.contains("null"), "Search node down {clocks}");
    }
    wait_caught_up(&layout, &cluster, &ws).await;
    both_copies(&cluster, &layout, &ws).await;

    project(&layout, "job-sha", &[sample(DOC0, "sha-2")])
        .await
        .expect("Indexed sha");
    indexed_sha(&world, &ws).await;

    let item = sql(
        world.graph_port,
        &format!(
            "USE NS graph DB {}; SELECT item, rids FROM _layout_item:{HKEY0};",
            graph_database(&ws)
        ),
    );
    assert!(item.contains(DOC0), "Item {item}");
    assert!(item.contains(&format!("page:{DOC0}")), "Item {item}");
    let mut gone = sample(DOC1, "sha-1");
    gone.remove = true;
    project(&layout, "job-remove", &[gone]).await.expect("Item");
    let item_gone = sql(
        world.graph_port,
        &format!(
            "USE NS graph DB {}; SELECT item FROM _layout_item:{HKEY1};",
            graph_database(&ws)
        ),
    );
    assert!(!item_gone.contains(DOC1), "Item {item_gone}");
    let search_gone = sql(
        28731,
        &format!(
            "USE NS search DB {}; SELECT title FROM page:{DOC1};",
            shard_database(&ws, LAYOUT_EPOCH, (hkey(DOC1) % 2) as i32)
        ),
    );
    assert!(!search_gone.contains("Title docb"), "Item {search_gone}");

    entry_limit(&world, &cluster).await;

    let graph_before = graph_log(&cluster, &world.graph_url, &ws).await;
    {
        let _paused = pause(&["surreal-search-0", "surreal-search-1"]);
        let commit = project(&layout, "job-retry", &[sample("hold", "sha-hold")])
            .await
            .expect("All copies down");
        assert!(commit > 0, "All copies down");
        let heading = sql(
            world.graph_port,
            &format!(
                "USE NS graph DB {}; SELECT body FROM heading:holdh;",
                graph_database(&ws)
            ),
        );
        assert!(
            heading.contains("body of hold"),
            "All copies down {heading}"
        );
        let lag = layout.lag().await.expect("lag");
        assert!(
            lag.len() == 2 && lag.iter().all(|row| row.behind > 0),
            "Search lag {lag:?}"
        );
        for shard in 0..2 {
            let set = shard_set_id(&db_id(&ws), LAYOUT_EPOCH, shard);
            assert_eq!(
                cluster.health(&set).await.expect("health"),
                Health::Red,
                "All copies down"
            );
        }
        let clocks = sql(
            world.graph_port,
            &format!(
                "USE NS graph DB {}; SELECT search_error FROM page:hold;",
                graph_database(&ws)
            ),
        );
        assert!(clocks.contains("null"), "All copies down {clocks}");
    }
    let graph_mid = graph_log(&cluster, &world.graph_url, &ws).await;
    assert_eq!(graph_mid.len(), graph_before.len() + 1, "Retry");
    wait_caught_up(&layout, &cluster, &ws).await;
    let lag = layout.lag().await.expect("lag");
    assert!(lag.iter().all(|row| row.behind == 0), "Search lag {lag:?}");
    assert_eq!(
        graph_log(&cluster, &world.graph_url, &ws).await,
        graph_mid,
        "Retry"
    );
    let held = sql(
        28731,
        &format!(
            "USE NS search DB {}; SELECT title FROM page:hold;",
            shard_database(&ws, LAYOUT_EPOCH, (hkey("hold") % 2) as i32)
        ),
    );
    assert!(held.contains("Title hold"), "Retry {held}");

    let before_miss = search_log(&cluster, &ws, 0).await;
    {
        let _paused = pause_graph(&world.graph_container);
        let err = project(&layout, "job-miss", &[sample("miss", "sha-miss")]).await;
        assert!(err.is_err(), "Graph not acked");
    }
    assert_eq!(
        search_log(&cluster, &ws, 0).await,
        before_miss,
        "Graph not acked"
    );
    let missed = sql(
        28731,
        &format!(
            "USE NS search DB {}; SELECT title FROM page:miss;",
            shard_database(&ws, LAYOUT_EPOCH, (hkey("miss") % 2) as i32)
        ),
    );
    assert!(!missed.contains("Title miss"), "Graph not acked {missed}");
    let _layout = layout;
}

async fn schema_hook(world: &World, ws: &str) {
    let page = sql(
        world.graph_port,
        &format!(
            "USE NS graph DB {}; INFO FOR TABLE page;",
            graph_database(ws)
        ),
    );
    assert!(page.contains("hkey"), "Schema hook {page}");
    let db = sql(
        world.graph_port,
        &format!("USE NS graph DB {}; INFO FOR DB;", graph_database(ws)),
    );
    assert!(db.contains("links_to"), "Schema hook {db}");
    let log = sql(
        world.graph_port,
        &format!(
            "USE NS graph DB {}; SELECT * FROM _repl_log;",
            graph_database(ws)
        ),
    );
    assert!(!log.contains("_repl_log:"), "Schema hook {log}");
    for port in [28731_u16, 28732] {
        for shard in 0..2 {
            let info = sql(
                port,
                &format!(
                    "USE NS search DB {}; INFO FOR TABLE heading;",
                    shard_database(ws, LAYOUT_EPOCH, shard)
                ),
            );
            assert!(info.contains("heading_text"), "Schema hook :{port} {info}");
            assert!(!info.contains("links_to"), "Schema hook :{port} {info}");
        }
    }
}

async fn serve_holds(world: &World, cluster: &Cluster) {
    let ws = format!("srv{}", std::process::id());
    let first = serve(&world.pg, &[ws.clone()], "serve-a", config())
        .await
        .expect("Serve");
    assert!(first[0].is_some(), "Serve");
    let pool = cluster_pool(&world.pg).await;
    let holder: String = sqlx::query_scalar("SELECT holder FROM repl_lease WHERE name = $1")
        .bind(lease_name(&ws))
        .fetch_one(&pool)
        .await
        .expect("lease");
    assert_eq!(holder, "serve-a", "Serve");
    let before = graph_log(cluster, &world.graph_url, &ws).await;
    let second = serve(&world.pg, &[ws.clone()], "serve-b", config())
        .await
        .expect("Serve");
    assert!(second[0].is_none(), "Serve");
    assert_eq!(
        graph_log(cluster, &world.graph_url, &ws).await,
        before,
        "Serve"
    );
    let holder: String = sqlx::query_scalar("SELECT holder FROM repl_lease WHERE name = $1")
        .bind(lease_name(&ws))
        .fetch_one(&pool)
        .await
        .expect("lease");
    assert_eq!(holder, "serve-a", "Serve");
    drop(first);
}

async fn both_copies(cluster: &Cluster, layout: &surrealastic::Layout, ws: &str) {
    for (doc, shard) in [(DOC0, hkey(DOC0) % 2), (DOC1, hkey(DOC1) % 2)] {
        let db = shard_database(ws, LAYOUT_EPOCH, shard as i32);
        for port in [28731_u16, 28732] {
            let text = sql(
                port,
                &format!("USE NS search DB {db}; SELECT title FROM page:{doc};"),
            );
            assert!(
                text.contains(&format!("Title {doc}")),
                "Both copies :{port} {text}"
            );
        }
        let db_name = shard_database(ws, LAYOUT_EPOCH, shard as i32);
        let left = sql(
            28731,
            &format!(
            "USE NS search DB {db_name}; SELECT applied_lsn, applied_fence FROM ONLY _repl:state;"
        ),
        );
        let right = sql(
            28732,
            &format!(
            "USE NS search DB {db_name}; SELECT applied_lsn, applied_fence FROM ONLY _repl:state;"
        ),
        );
        assert_eq!(
            number(&left, "applied_lsn"),
            number(&right, "applied_lsn"),
            "Both copies"
        );
        assert_eq!(
            number(&left, "applied_fence"),
            number(&right, "applied_fence"),
            "Both copies"
        );
    }
    let lag = layout.lag().await.expect("lag");
    assert!(lag.iter().all(|row| row.behind == 0), "Both copies {lag:?}");
    for shard in 0..2 {
        let set = shard_set_id(&db_id(ws), LAYOUT_EPOCH, shard);
        assert_eq!(
            cluster.health(&set).await.expect("health"),
            Health::Green,
            "Both copies"
        );
    }
    assert_eq!(
        cluster.health(&commit_set_id(ws)).await.expect("health"),
        Health::Green,
        "Both copies"
    );
}

async fn indexed_sha(world: &World, ws: &str) {
    let shard = (hkey(DOC0) % 2) as i32;
    let db = shard_database(ws, LAYOUT_EPOCH, shard);
    let graph = sql(
        world.graph_port,
        &format!(
            "USE NS graph DB {}; SELECT indexed_sha FROM page:{DOC0};",
            graph_database(ws)
        ),
    );
    assert!(graph.contains("sha-2"), "Indexed sha {graph}");
    for port in [28731_u16, 28732] {
        for table in ["page", "heading", "mention"] {
            let id = match table {
                "page" => format!("page:{DOC0}"),
                "heading" => format!("heading:{DOC0}h"),
                _ => format!("mention:{DOC0}m"),
            };
            let text = sql(
                port,
                &format!("USE NS search DB {db}; SELECT indexed_sha FROM {id};"),
            );
            assert!(text.contains("sha-2"), "Indexed sha :{port} {table} {text}");
        }
    }
}

async fn entry_limit(world: &World, cluster: &Cluster) {
    let ws = format!("bulk{}", std::process::id());
    ensure_wiki(&world.pg, &ws, 2, 1).await.expect("bulk wiki");
    let layout = venus_graph::open_writer(cluster, &ws, "venus-bulk")
        .await
        .expect("bulk open")
        .expect("bulk lease");
    let mut docs = Vec::new();
    let mut n = 0_u32;
    while docs.len() < 300 {
        let id = format!("b{n:04}");
        n += 1;
        if hkey(&id) % 2 == 0 {
            docs.push(small(&id));
        }
    }
    project(&layout, "job-bulk", &docs)
        .await
        .expect("Entry limit");
    let ids = search_log(cluster, &ws, 0).await;
    assert_eq!(ids.len(), 2, "Entry limit {ids:?}");
    let mut seen = 0_usize;
    for id in ids {
        let body = sql(
            28731,
            &format!(
                "USE NS search DB {}; SELECT body FROM _repl_log:{id};",
                shard_database(&ws, LAYOUT_EPOCH, 0)
            ),
        );
        let pages = body.matches("UPSERT page:").count();
        assert!(pages <= 256, "Entry limit {pages}");
        assert!(
            body.len() < 4 * 1024 * 1024,
            "Entry limit {} bytes",
            body.len()
        );
        seen += pages;
    }
    assert_eq!(seen, 300, "Entry limit");
    drop(layout);
}

async fn wait_caught_up(layout: &surrealastic::Layout, cluster: &Cluster, ws: &str) {
    for _ in 0..80 {
        for shard in 0..2 {
            let set = shard_set_id(&db_id(ws), LAYOUT_EPOCH, shard);
            let _ = layout.catch_up(&set).await;
        }
        let lag = layout.lag().await.unwrap_or_default();
        let mut synced = true;
        for shard in 0..2 {
            let set = shard_set_id(&db_id(ws), LAYOUT_EPOCH, shard);
            let rows = cluster.fingerprint(&set).await.unwrap_or_default();
            if rows.is_empty() || rows.iter().any(|row| !row.contains(":in_sync:")) {
                synced = false;
            }
        }
        let ready = !lag.is_empty() && lag.iter().all(|row| row.behind == 0) && synced;
        if ready && !lag.is_empty() {
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
    panic!("copies did not catch up");
}

async fn graph_log(cluster: &Cluster, graph_url: &str, ws: &str) -> Vec<i64> {
    log_ids(
        cluster,
        "0",
        graph_url,
        "graph",
        &graph_database(ws),
        1,
        10_000,
    )
    .await
    .unwrap_or_default()
}

async fn search_log(cluster: &Cluster, ws: &str, shard: i32) -> Vec<i64> {
    log_ids(
        cluster,
        "1",
        "http://127.0.0.1:28731",
        "search",
        &shard_database(ws, LAYOUT_EPOCH, shard),
        1,
        10_000,
    )
    .await
    .unwrap_or_default()
}

fn sample(doc_id: &str, sha: &str) -> Doc {
    Doc {
        doc_id: doc_id.to_string(),
        git_path: format!("docs/{doc_id}.md"),
        title: format!("Title {doc_id}"),
        indexed_sha: sha.to_string(),
        heading: "Overview".to_string(),
        body: format!("body of {doc_id}"),
        mention: MENTION.to_string(),
        remove: false,
    }
}

fn small(doc_id: &str) -> Doc {
    Doc {
        doc_id: doc_id.to_string(),
        git_path: format!("docs/{doc_id}.md"),
        title: "t".to_string(),
        indexed_sha: "s".to_string(),
        heading: "H".to_string(),
        body: "b".to_string(),
        mention: "u".to_string(),
        remove: false,
    }
}

fn number(text: &str, name: &str) -> i64 {
    let pat = format!("\"{name}\"");
    let rest = text
        .split(&pat)
        .nth(1)
        .unwrap_or_else(|| panic!("{name} missing in {text}"));
    let digits: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '-')
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{name} in {text}"))
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

async fn cluster_pool(pg: &str) -> sqlx::PgPool {
    for _ in 0..40 {
        if let Ok(pool) = sqlx::PgPool::connect(pg).await {
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

async fn seed_legacy(url: &str) {
    let pool = cluster_pool(url).await;
    for statement in [
        "CREATE TABLE search_cluster (
            workspace_id UUID PRIMARY KEY,
            shard_count INT NOT NULL,
            replica_count INT NOT NULL
        )",
        "CREATE TABLE search_node (node_id TEXT PRIMARY KEY, url TEXT NOT NULL)",
        "CREATE TABLE search_allocation (
            workspace_id UUID NOT NULL,
            shard INT NOT NULL,
            role TEXT NOT NULL,
            node_id TEXT NOT NULL,
            state TEXT NOT NULL,
            synced_sha TEXT,
            PRIMARY KEY (workspace_id, shard, role, node_id)
        )",
    ] {
        sqlx::raw_sql(statement)
            .execute(&pool)
            .await
            .expect("legacy ddl");
    }
    sqlx::query(
        "INSERT INTO search_cluster (workspace_id, shard_count, replica_count)
         VALUES ($1::uuid, 2, 1)",
    )
    .bind(WORKSPACE_ID)
    .execute(&pool)
    .await
    .expect("legacy row");
}

fn published_port(stdout: &[u8]) -> u16 {
    String::from_utf8_lossy(stdout)
        .lines()
        .find_map(|line| line.trim().rsplit(':').next()?.parse().ok())
        .unwrap_or_else(|| panic!("port map {}", String::from_utf8_lossy(stdout)))
}

fn sql(port: u16, body: &str) -> String {
    let url = format!("http://127.0.0.1:{port}/sql");
    let out = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "30",
            "-u",
            "venus:venus",
            "-H",
            "Accept: application/json",
            "-H",
            "Content-Type: text/plain",
            "--data-binary",
            body,
            &url,
        ])
        .output()
        .unwrap_or_else(|err| panic!("curl {url}: {err}"));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "sql :{port} failed: {}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    text
}

struct World {
    pg: String,
    graph_url: String,
    graph_port: u16,
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
        // Sibling tests in this process start `venus-project-*-{pid}` containers concurrently.
        let own = format!("-{}", std::process::id());
        let listed = docker(&[
            "ps",
            "-a",
            "--filter",
            "name=venus-project-",
            "--format",
            "{{.Names}}",
        ]);
        for name in String::from_utf8_lossy(&listed.stdout).split_whitespace() {
            if !name.ends_with(&own) {
                let _ = docker(&["rm", "-f", name]);
            }
        }
        graph_up(&["surreal-search-0", "surreal-search-1"]);
        for port in [28731_u16, 28732] {
            wait_port(port);
        }
        let (graph_url, graph_port, graph_container) = ensure_graph();
        let name = format!("venus-project-pg-{}", std::process::id());
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
        let pg = format!("postgres://venus:venus@127.0.0.1:{host_port}/venus?sslmode=disable");
        wait_pg(&name);
        *PG_NAME.lock().unwrap_or_else(|err| err.into_inner()) = name;
        unsafe {
            atexit(remove_pg);
        }
        World {
            pg,
            graph_url,
            graph_port,
            graph_container,
        }
    })
}

fn ensure_graph() -> (String, u16, String) {
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
        return (
            "http://127.0.0.1:28730".into(),
            28730,
            service_id("surreal-graph"),
        );
    }
    let name = format!("venus-project-graph-{}", std::process::id());
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
    (format!("http://127.0.0.1:{host_port}"), host_port, name)
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

fn pause(services: &[&str]) -> Paused {
    let ids: Vec<String> = services.iter().map(|service| service_id(service)).collect();
    for id in &ids {
        let out = docker(&["pause", id]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Paused(ids)
}

fn pause_graph(container: &str) -> Paused {
    let out = docker(&["pause", container]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Paused(vec![container.to_string()])
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
    let graph = format!("venus-project-graph-{}", std::process::id());
    let _ = Command::new("docker")
        .args(["rm", "-f", graph.as_str()])
        .output();
}

struct Pg {
    name: String,
    url: String,
}

impl Pg {
    fn start() -> Self {
        let name = format!("venus-project-map-{}", std::process::id());
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
        Self {
            name,
            url: format!("postgres://venus:venus@127.0.0.1:{host_port}/venus?sslmode=disable"),
        }
    }
}

impl Drop for Pg {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.name]);
    }
}
