//! ss-cluster step-schema.
//! Graph has no index: :8000 has `links_to` and no full-text index.
//! Search has no graph: :8001 and :8002 have the search indexes and no `links_to`.
//! Allocation: the fixture wiki lives in `layout_db`. A second migrate leaves it.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use venus_graph::{
    accept_graph_schema, accept_search_schema, copy_pairs, graph_surql, legacy_table_names,
    migrate_allocation, migrate_surreal, search_surql, wiki_layout, WORKSPACE_ID,
};

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
        "docker {} failed\n{}\n{}",
        full.join(" "),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn live_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn assert_host_port_free(port: u16) {
    let out = docker(&["ps", "--format", "{{.Names}}\t{{.Ports}}"]);
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}->");
    let holders: Vec<_> = text
        .lines()
        .filter(|line| line.contains(&needle) && !line.starts_with("venus-surreal-"))
        .collect();
    assert!(
        holders.is_empty(),
        "host port {port} is already published:\n{}",
        holders.join("\n")
    );
}

struct GraphProfile;

impl GraphProfile {
    fn start() -> Self {
        for port in [8000u16, 8001, 8002] {
            assert_host_port_free(port);
        }
        compose(&[
            "--profile",
            "graph",
            "up",
            "-d",
            "--wait",
            "--wait-timeout",
            "300",
            "--no-deps",
            "surreal-graph",
            "surreal-search-0",
            "surreal-search-1",
        ]);
        Self
    }
}

impl Drop for GraphProfile {
    fn drop(&mut self) {
        let _ = docker(&[
            "compose",
            "--profile",
            "graph",
            "rm",
            "-sfv",
            "surreal-graph",
            "surreal-search-0",
            "surreal-search-1",
        ]);
    }
}

fn sql(port: u16, body: &str) -> String {
    let url = format!("http://127.0.0.1:{port}/sql");
    let out = Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "20",
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

fn migrate_twice() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        for _ in 0..2 {
            migrate_surreal(
                "127.0.0.1:8000",
                &["127.0.0.1:8001", "127.0.0.1:8002"],
                "venus",
                "venus",
            )
            .await
            .expect("migrate surreal twice");
        }
    });
}

#[test]
fn graph_migrator_refuses_search_index() {
    let rejected = accept_graph_schema(
        "DEFINE INDEX heading_text ON heading FIELDS body SEARCH ANALYZER wiki BM25;",
    );
    assert_eq!(rejected, Err("graph schema refuses a SEARCH index"));
    let sql = graph_surql();
    assert!(sql.contains("DEFINE TABLE IF NOT EXISTS links_to "));
    assert!(sql.contains("search_sha"));
    assert!(sql.contains("search_error"));
    assert!(sql.contains("hkey"));
    accept_graph_schema(&sql).expect("shipped graph schema");
}

#[test]
fn search_migrator_omits_graph_edges() {
    assert_eq!(
        accept_search_schema("DEFINE TABLE links_to SCHEMAFULL;"),
        Err("search schema does not create links_to")
    );
    assert_eq!(
        accept_search_schema("DEFINE NAMESPACE graph;"),
        Err("search schema does not create namespace graph")
    );
    let sql = search_surql();
    assert!(sql.contains("heading_text"));
    assert!(sql.contains("mention_text"));
    assert!(!sql.contains("links_to"));
}

/// :8000 has `links_to` and no full-text index. A second migrate still applies.
#[test]
fn graph_has_no_index() {
    let _guard = live_lock();
    let _profile = GraphProfile::start();
    migrate_twice();
    let info = sql(
        8000,
        &format!("USE NS graph DB ⟨{WORKSPACE_ID}⟩; INFO FOR DB;"),
    );
    assert!(
        info.contains("links_to"),
        "graph database missing links_to: {info}"
    );
    assert!(
        !info.contains("heading_text") && !info.contains("SEARCH ANALYZER"),
        "graph database has a full-text index: {info}"
    );
    let page = sql(
        8000,
        &format!("USE NS graph DB ⟨{WORKSPACE_ID}⟩; INFO FOR TABLE page;"),
    );
    assert!(
        page.contains("search_sha") && page.contains("search_error"),
        "graph page missing search clocks: {page}"
    );
}

/// :8001 and :8002 have the search indexes and no `links_to` or namespace `graph`.
#[test]
fn search_has_no_graph() {
    let _guard = live_lock();
    let _profile = GraphProfile::start();
    migrate_twice();
    for port in [8001u16, 8002] {
        let info = sql(
            port,
            &format!("USE NS search DB ⟨{WORKSPACE_ID}⟩; INFO FOR DB;"),
        );
        assert!(
            !info.contains("links_to"),
            ":{port} created links_to: {info}"
        );
        let heading = sql(
            port,
            &format!("USE NS search DB ⟨{WORKSPACE_ID}⟩; INFO FOR TABLE heading;"),
        );
        let mention = sql(
            port,
            &format!("USE NS search DB ⟨{WORKSPACE_ID}⟩; INFO FOR TABLE mention;"),
        );
        assert!(
            heading.contains("heading_text") && mention.contains("mention_text"),
            ":{port} missing search indexes: heading={heading} mention={mention}"
        );
        let root = sql(port, "INFO FOR ROOT;");
        assert!(
            !root.contains("graph:"),
            ":{port} created namespace graph: {root}"
        );
    }
}

struct Pg {
    name: String,
    url: String,
}

impl Pg {
    fn start() -> Self {
        let name = "venus-graph-schema-pg".to_string();
        let _ = docker(&["rm", "-f", &name]);
        let out = docker(&[
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
            out.status.success(),
            "postgres: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mapped = docker(&["port", &name, "5432"]);
        let mapped = String::from_utf8_lossy(&mapped.stdout);
        let host_port = mapped
            .trim()
            .rsplit(':')
            .next()
            .expect("port")
            .parse::<u16>()
            .expect("port number");
        let url = format!("postgres://venus:venus@127.0.0.1:{host_port}/venus?sslmode=disable");
        for _ in 0..60 {
            let ready = docker(&[
                "exec", &name, "psql", "-U", "venus", "-d", "venus", "-c", "SELECT 1",
            ]);
            if ready.status.success() {
                return Self { name, url };
            }
            thread::sleep(Duration::from_millis(500));
        }
        panic!("postgres did not become ready");
    }
}

impl Drop for Pg {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.name]);
    }
}

/// Fixture wiki in `layout_db`. A second migrate leaves that row in place.
#[test]
fn allocation() {
    let pg = Pg::start();
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let (first, second, legacy, copies) = rt.block_on(async {
        migrate_allocation(&pg.url).await.expect("seed");
        let first = wiki_layout(&pg.url, WORKSPACE_ID).await.expect("layout");
        migrate_allocation(&pg.url).await.expect("seed again");
        let second = wiki_layout(&pg.url, WORKSPACE_ID).await.expect("layout");
        let legacy = legacy_table_names(&pg.url).await.expect("legacy");
        let copies = copy_pairs(&pg.url, WORKSPACE_ID).await.expect("copies");
        (first, second, legacy, copies)
    });
    assert_eq!(first, second, "a second migrate changes the layout");
    assert_eq!(first.shard_count, 2);
    assert_eq!(first.replica_count, 1);
    assert_eq!(first.shard_ack, 1);
    assert_eq!(first.search_sets, 2);
    assert_eq!(first.commit_copies, 1);
    assert!(legacy.is_empty(), "step 3 tables remain: {legacy:?}");
    let commit = format!("graph:{WORKSPACE_ID}");
    let commit_nodes: Vec<_> = copies
        .iter()
        .filter(|(set, _)| set == &commit)
        .map(|(_, node)| node.as_str())
        .collect();
    assert_eq!(commit_nodes, vec!["0"]);
    for shard in 0..2 {
        let set = format!("search:{WORKSPACE_ID}:1:{shard}");
        let mut nodes: Vec<_> = copies
            .iter()
            .filter(|(id, _)| id == &set)
            .map(|(_, node)| node.as_str())
            .collect();
        nodes.sort_unstable();
        assert_eq!(nodes, vec!["1", "2"], "{set}");
    }
}
