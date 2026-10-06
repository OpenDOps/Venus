//! M3 step-flush: autoinit wiki/, pin clock sidecar, last_flushed, drop Map.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use git2::Repository;
use sqlx::PgPool;
use tokio::sync::OnceCell;
use venus_hub::db;
use venus_sidecar::catalog::{replace_page_identity, CatalogPage, CatalogWalk, Leaving};
use venus_sidecar::config::database_url_from_env;
use venus_sidecar::cut::{cut_workspace, GIT_PATH};
use venus_sidecar::git::{WikiConfig, LINKS_JSON_REL, PAGES_YAML_REL, SIDECAR_REL};
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{convert_pins, flush_claimed, Claim};
use venus_sidecar::{CATALOG_DOC_ID, PAGE_DOC_ID, PAGE_DOC_UUID};
use y_octo::Doc;

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

fn affine_home_pin(paragraph: &str) -> Vec<u8> {
    affine_page_pin("Venus", paragraph, None)
}

fn affine_page_pin(title: &str, paragraph: &str, embed_page_id: Option<&str>) -> Vec<u8> {
    let doc = Doc::default();
    let mut blocks = doc.get_or_create_map("blocks").expect("blocks");

    let mut page = doc.create_map().expect("page map");
    page.insert("sys:flavour".into(), "affine:page")
        .expect("page flavour");
    page.insert("sys:id".into(), "page").expect("page id");
    let mut page_children = doc.create_array().expect("page children");
    page_children.push("note").expect("page child");
    page.insert("sys:children".into(), page_children)
        .expect("page sys:children");
    let mut title_text = doc.create_text().expect("title");
    title_text.insert(0, title).expect("title text");
    page.insert("prop:title".into(), title_text)
        .expect("page title");
    blocks.insert("page".into(), page).expect("insert page");

    let mut note = doc.create_map().expect("note map");
    note.insert("sys:flavour".into(), "affine:note")
        .expect("note flavour");
    note.insert("sys:id".into(), "note").expect("note id");
    let mut note_children = doc.create_array().expect("note children");
    note_children.push("para").expect("note child");
    if embed_page_id.is_some() {
        note_children.push("embed").expect("embed child");
    }
    note.insert("sys:children".into(), note_children)
        .expect("note sys:children");
    blocks.insert("note".into(), note).expect("insert note");

    let mut para = doc.create_map().expect("para map");
    para.insert("sys:flavour".into(), "affine:paragraph")
        .expect("para flavour");
    para.insert("sys:id".into(), "para").expect("para id");
    para.insert("prop:type".into(), "text").expect("para type");
    let para_children = doc.create_array().expect("para children");
    para.insert("sys:children".into(), para_children)
        .expect("para sys:children");
    let mut text = doc.create_text().expect("para text");
    text.insert(0, paragraph).expect("para insert");
    para.insert("prop:text".into(), text).expect("prop:text");
    blocks.insert("para".into(), para).expect("insert para");

    if let Some(page_id) = embed_page_id {
        let mut embed = doc.create_map().expect("embed map");
        embed
            .insert("sys:flavour".into(), "affine:embed-linked-doc")
            .expect("embed flavour");
        embed.insert("sys:id".into(), "embed").expect("embed id");
        embed.insert("prop:pageId".into(), page_id).expect("pageId");
        let embed_children = doc.create_array().expect("embed children");
        embed
            .insert("sys:children".into(), embed_children)
            .expect("embed sys:children");
        blocks.insert("embed".into(), embed).expect("insert embed");
    }

    doc.encode_update_v1().expect("encode pin")
}

fn seed_home_pin() -> Vec<u8> {
    fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/seed-home.yjs"))
        .expect("seed-home.yjs")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

async fn persist_pin(pool: &PgPool, workspace: &str, bytes: &[u8]) -> i64 {
    persist_doc(pool, workspace, PAGE_DOC_UUID, bytes).await
}

async fn persist_doc(pool: &PgPool, workspace: &str, doc_id: &str, bytes: &[u8]) -> i64 {
    db::push_update(pool, workspace, doc_id, bytes)
        .await
        .expect("persist crdt_update")
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

async fn last_flushed(pool: &PgPool, workspace: &str) -> Option<(i64, String)> {
    sqlx::query_as(
        "SELECT clock, git_sha FROM last_flushed
         WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace)
    .bind(PAGE_DOC_UUID)
    .fetch_optional(pool)
    .await
    .expect("last_flushed")
}

async fn job_count(pool: &PgPool, workspace: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE workspace_id = $1::uuid")
        .bind(workspace)
        .fetch_one(pool)
        .await
        .expect("jobs count")
}

async fn lease_this_wiki(pool: &PgPool, workspace: &str) -> Claim {
    sqlx::query(
        "INSERT INTO jobs (workspace_id, reason, not_before)
         VALUES ($1::uuid, 'flush', now())
         ON CONFLICT (workspace_id) DO UPDATE
         SET reason = 'flush', not_before = now()",
    )
    .bind(workspace)
    .execute(pool)
    .await
    .expect("jobs row");
    let mut tx = pool.begin().await.expect("lease begin");
    let owned: Option<String> = sqlx::query_scalar(
        "UPDATE jobs
         SET owner = 'flush-worker', lease_until = now() + interval '2 minutes'
         WHERE workspace_id = $1::uuid
           AND not_before <= now()
           AND (lease_until IS NULL OR lease_until < now())
         RETURNING workspace_id::text",
    )
    .bind(workspace)
    .fetch_optional(&mut *tx)
    .await
    .expect("lease");
    tx.commit().await.expect("lease commit");
    assert_eq!(
        owned.as_deref(),
        Some(workspace),
        "lease this wiki (do not SKIP LOCKED another test's job)"
    );
    Claim {
        workspace_id: workspace.to_string(),
        owner: "flush-worker".into(),
    }
}

async fn flush_wiki(
    pool: &PgPool,
    workspace: &str,
    wiki: &WikiConfig,
    convert_sleep: Duration,
) -> venus_sidecar::queue::FlushOutcome {
    let claim = lease_this_wiki(pool, workspace).await;
    let mut pins = PinMap::new();
    flush_claimed(pool, claim, &mut pins, wiki, convert_sleep)
        .await
        .expect("flush_claimed")
}

fn commit_count(dir: &Path) -> usize {
    let repo = Repository::open(dir).expect("open wiki");
    let mut revwalk = repo.revwalk().expect("revwalk");
    revwalk.push_head().expect("push HEAD");
    revwalk.count()
}

fn head_subject(dir: &Path) -> String {
    let repo = Repository::open(dir).expect("open wiki");
    let commit = repo.head().expect("HEAD").peel_to_commit().expect("commit");
    commit
        .message()
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Autoinit + sidecar on disk + unique word in the file (scenarios 1, 2, 8).
#[tokio::test]
async fn autoinit_commit_sidecar_and_word() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &seed_home_pin()).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    assert!(
        !tmp.path().join(".git").exists(),
        "Flush must not require a pre-inited repo"
    );
    let wiki = WikiConfig::new(tmp.path());
    let outcome = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(outcome.committed, "first snapshot must commit");
    assert!(outcome.sha.is_some());
    assert!(tmp.path().join(".git").is_dir());

    let repo = Repository::open(tmp.path()).expect("repo");
    assert_eq!(
        repo.head().expect("HEAD").shorthand(),
        Some("main"),
        "default branch Actual is main"
    );
    assert_eq!(
        repo.remotes().expect("remotes").iter().flatten().count(),
        0,
        "no origin in M3"
    );
    assert_eq!(head_subject(tmp.path()), "snapshot: Venus");
    let md = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("spec/home.md");
    assert!(
        md.contains("Why Venus"),
        "seed page must land in git: {md:?}"
    );
    assert!(
        !md.contains("<!-- id:"),
        "markdown must not embed per-block id comments"
    );

    let sidecar_raw = fs::read_to_string(tmp.path().join(SIDECAR_REL)).expect("sidecar json");
    let sidecar: serde_json::Value = serde_json::from_str(&sidecar_raw).expect("json");
    assert_eq!(sidecar["docId"], PAGE_DOC_ID);
    let t = last_flushed(&pool, &ws)
        .await
        .expect("last_flushed after commit");
    assert_eq!(
        sidecar["clock"].as_i64(),
        Some(t.0),
        "disk clock is pin dirty.clock at cut"
    );
    assert!(sidecar["blocks"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false));
    for b in sidecar["blocks"].as_array().unwrap() {
        assert!(b.get("id").and_then(|v| v.as_str()).is_some());
        assert!(b.get("start").and_then(|v| v.as_u64()).is_some());
        assert!(b.get("end").and_then(|v| v.as_u64()).is_some());
    }
    assert_eq!(t.1, outcome.sha.unwrap());
    assert_eq!(job_count(&pool, &ws).await, 0, "jobs row released");

    let gi = fs::read_to_string(repo_root().join(".gitignore")).expect("gitignore");
    assert!(
        gi.lines()
            .any(|l| l.trim() == "/wiki" || l.trim() == "/wiki/"),
        "product git must ignore /wiki/"
    );
    let check = Command::new("git")
        .args(["check-ignore", "-v", "wiki"])
        .current_dir(repo_root())
        .output()
        .expect("git check-ignore");
    assert!(
        check.status.success(),
        "wiki must be ignored: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let tracked = Command::new("git")
        .args(["ls-files", "--", "wiki"])
        .current_dir(repo_root())
        .output()
        .expect("git ls-files wiki");
    assert!(
        String::from_utf8_lossy(&tracked.stdout).trim().is_empty(),
        "wiki/ must not be tracked on the Venus remote"
    );
}

/// Persist a unique string (script stand-in for typed word) and find it in the file.
#[tokio::test]
async fn unique_word_lands_in_markdown() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let word = format!("flush-word-{}", std::process::id());
    persist_pin(&pool, &ws, &affine_home_pin(&word)).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    flush_wiki(&pool, &ws, &WikiConfig::new(tmp.path()), Duration::ZERO).await;
    let md = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("md");
    assert!(md.contains(&word), "file must contain {word}: {md:?}");
}

/// Writes after pin clock T are not in this commit; dirty.clock > T.
#[tokio::test]
async fn writes_after_t_stay_dirty() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &affine_home_pin("alpha")).await;
    let t = dirty_clock(&pool, &ws).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    let claim = lease_this_wiki(&pool, &ws).await;
    let mut pins = PinMap::new();
    let persist = {
        let pool = pool.clone();
        let ws = ws.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            persist_pin(&pool, &ws, &affine_home_pin("beta")).await
        })
    };
    let outcome = flush_claimed(&pool, claim, &mut pins, &wiki, Duration::from_millis(250))
        .await
        .expect("flush with delayed convert");
    persist.await.expect("join persist");
    assert!(outcome.committed);
    let md = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("md");
    assert!(md.contains("alpha"), "pin text missing: {md:?}");
    assert!(
        !md.contains("beta"),
        "post-pin typing must not be in this commit: {md:?}"
    );
    let dirty = dirty_clock(&pool, &ws).await;
    assert!(dirty > t, "dirty.clock {dirty} must be > pin T {t}");
    let lf = last_flushed(&pool, &ws).await.expect("last_flushed");
    assert_eq!(lf.0, t);
}

/// Second flush with no new persist does not add a commit; same .git.
#[tokio::test]
async fn idempotent_second_flush_skips_commit() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &affine_home_pin("same")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    let first = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(first.committed);
    let n1 = commit_count(tmp.path());
    assert_eq!(n1, 1);
    let git_dir = fs::canonicalize(tmp.path().join(".git")).expect("git dir");

    let second = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(
        !second.committed,
        "empty S / identical tree must skip commit"
    );
    assert_eq!(commit_count(tmp.path()), n1);
    assert_eq!(
        fs::canonicalize(tmp.path().join(".git")).expect("git dir 2"),
        git_dir,
        "must not init a second history"
    );
    assert_eq!(job_count(&pool, &ws).await, 0);
}

/// Pin Map is empty after a successful flush (not kept as lease T0).
#[tokio::test]
async fn drop_pin_after_commit() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &affine_home_pin("drop")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let claim = lease_this_wiki(&pool, &ws).await;
    let mut pins = PinMap::new();
    flush_claimed(
        &pool,
        claim,
        &mut pins,
        &WikiConfig::new(tmp.path()),
        Duration::ZERO,
    )
    .await
    .expect("flush");
    assert!(pins.is_empty(), "pin Map must be dropped after commit");
    assert!(
        pins.get(PAGE_DOC_UUID).is_none() && pins.get(PAGE_DOC_ID).is_none(),
        "no doc:home / PAGE_DOC_UUID entry after commit"
    );
    assert!(last_flushed(&pool, &ws).await.is_some());
}

#[test]
fn git_module_is_not_hub_export() {
    let src = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/git.rs"))
        .expect("git.rs");
    for needle in ["/export", "reqwest", "get_doc", ":3000/api/block"] {
        assert!(
            !src.contains(needle),
            "git snapshotter must not GET hub export ({needle})"
        );
    }
}

#[test]
fn queue_flush_is_not_export() {
    let src = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/queue.rs"))
        .expect("queue.rs");
    for needle in ["/export", "reqwest"] {
        assert!(
            !src.contains(needle),
            "flush path must not poll export ({needle})"
        );
    }
}

const CREATED_UUID: &str = "a1b2c3d4-e5f6-4780-abcd-ef1234567890";
const THIRD_UUID: &str = "c3d4e5f6-a7b8-4012-cdef-123456789012";
const FOURTH_UUID: &str = "d4e5f6a7-b8c9-4123-def0-234567890123";
const DESIGN_FOLDER: &str = "folder:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

struct CatalogDoc {
    doc: Doc,
}

impl CatalogDoc {
    fn seed(name: &str, git_name: &str, parent_id: &str) -> Self {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map("nodes").expect("nodes");
        put_catalog_node(
            &doc,
            &mut nodes,
            "folder:spec",
            "folder",
            "spec",
            None,
            "spec",
            None,
        );
        if parent_id != "folder:spec" {
            put_catalog_node(
                &doc,
                &mut nodes,
                DESIGN_FOLDER,
                "folder",
                "design",
                None,
                "design",
                None,
            );
        }
        put_catalog_node(
            &doc,
            &mut nodes,
            PAGE_DOC_ID,
            "doc",
            "home",
            Some("folder:spec"),
            "home.md",
            Some(PAGE_DOC_ID),
        );
        put_catalog_node(
            &doc,
            &mut nodes,
            CREATED_UUID,
            "doc",
            name,
            Some(parent_id),
            git_name,
            Some(CREATED_UUID),
        );
        Self { doc }
    }

    fn add_page(&self, id: &str, name: &str, git_name: &str, parent_id: &str) {
        let mut nodes = self.nodes();
        put_catalog_node(
            &self.doc,
            &mut nodes,
            id,
            "doc",
            name,
            Some(parent_id),
            git_name,
            Some(id),
        );
    }

    fn encode(&self) -> Vec<u8> {
        self.doc.encode_update_v1().expect("encode catalog")
    }

    fn nodes(&self) -> y_octo::Map {
        self.doc.get_or_create_map("nodes").expect("nodes")
    }

    fn node(&self, id: &str) -> y_octo::Map {
        match self.nodes().get(id) {
            Some(y_octo::Value::Map(m)) => m,
            other => panic!("catalog node {id}: {other:?}"),
        }
    }

    fn rename_created(&self, name: &str, git_name: &str) {
        let mut node = self.node(CREATED_UUID);
        node.insert("name".into(), name).expect("name");
        node.insert("gitName".into(), git_name).expect("gitName");
    }

    fn move_created_to_design(&self) {
        let mut nodes = self.nodes();
        if nodes.get(DESIGN_FOLDER).is_none() {
            put_catalog_node(
                &self.doc,
                &mut nodes,
                DESIGN_FOLDER,
                "folder",
                "design",
                None,
                "design",
                None,
            );
        }
        let mut node = self.node(CREATED_UUID);
        node.insert("parentId".into(), DESIGN_FOLDER)
            .expect("parentId");
    }

    fn delete_created(&self) {
        self.nodes().remove(CREATED_UUID);
    }
}

fn put_catalog_node(
    doc: &Doc,
    nodes: &mut y_octo::Map,
    id: &str,
    kind: &str,
    name: &str,
    parent_id: Option<&str>,
    git_name: &str,
    doc_id: Option<&str>,
) {
    let mut node = doc.create_map().expect("node map");
    node.insert("id".into(), id).expect("id");
    node.insert("kind".into(), kind).expect("kind");
    node.insert("name".into(), name).expect("name");
    if let Some(parent) = parent_id {
        node.insert("parentId".into(), parent).expect("parentId");
    }
    node.insert("order".into(), "a0").expect("order");
    node.insert("gitName".into(), git_name).expect("gitName");
    if let Some(doc_id) = doc_id {
        node.insert("docId".into(), doc_id).expect("docId");
    }
    nodes.insert(id.into(), node).expect("insert node");
}

fn catalog_seed_with_page(name: &str, git_name: &str, parent_id: &str) -> Vec<u8> {
    CatalogDoc::seed(name, git_name, parent_id).encode()
}

async fn identity_rows(pool: &PgPool, workspace: &str) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        "SELECT uuid::text, doc_id, name, git_path
         FROM page_identity
         WHERE workspace_id = $1::uuid AND deleted_at IS NULL
         ORDER BY git_path",
    )
    .bind(workspace)
    .fetch_all(pool)
    .await
    .expect("page_identity")
}

async fn tombstoned_uuids(pool: &PgPool, workspace: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT uuid::text FROM page_identity
         WHERE workspace_id = $1::uuid AND deleted_at IS NOT NULL
         ORDER BY uuid",
    )
    .bind(workspace)
    .fetch_all(pool)
    .await
    .expect("tombstones")
}

fn head_renames(dir: &Path) -> Vec<(String, String)> {
    let repo = Repository::open(dir).expect("open");
    let commit = repo.head().expect("HEAD").peel_to_commit().expect("commit");
    let tree = commit.tree().expect("tree");
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let mut diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .expect("diff");
    let mut find = git2::DiffFindOptions::new();
    find.renames(true);
    diff.find_similar(Some(&mut find)).expect("find_similar");
    let mut out = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if delta.status() == git2::Delta::Renamed {
                let old = delta.old_file().path().map(|p| p.display().to_string());
                let new = delta.new_file().path().map(|p| p.display().to_string());
                if let (Some(o), Some(n)) = (old, new) {
                    out.push((o, n));
                }
            }
            true
        },
        None,
        None,
        None,
    )
    .expect("foreach");
    out
}

fn file_blob(dir: &Path, rel: &str) -> Vec<u8> {
    fs::read(dir.join(rel)).unwrap_or_default()
}

#[tokio::test]
async fn two_files_yaml_and_identity() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &seed_home_pin()).await;
    persist_doc(
        &pool,
        &ws,
        CATALOG_DOC_ID,
        &catalog_seed_with_page(CREATED_UUID, &format!("{CREATED_UUID}.md"), "folder:spec"),
    )
    .await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let outcome = flush_wiki(&pool, &ws, &WikiConfig::new(tmp.path()), Duration::ZERO).await;
    assert!(outcome.committed);
    assert!(tmp.path().join(GIT_PATH).is_file(), "home.md");
    let created_md = format!("spec/{CREATED_UUID}.md");
    assert!(
        tmp.path().join(&created_md).is_file(),
        "created page must not be skipped: {created_md}"
    );
    assert!(tmp.path().join(SIDECAR_REL).is_file());
    assert!(tmp
        .path()
        .join(format!(".venus/ids/{CREATED_UUID}.json"))
        .is_file());
    let yaml = fs::read_to_string(tmp.path().join(PAGES_YAML_REL)).expect("pages.yaml");
    assert!(
        tmp.path().join(LINKS_JSON_REL).is_file(),
        "catalog Flush writes .venus/links.json"
    );
    assert!(yaml.contains("spec/home.md:"));
    assert!(yaml.contains(&format!("spec/{CREATED_UUID}.md:")));
    assert!(yaml.contains("folder:spec"));
    assert!(yaml.contains(&format!("uuid: {CREATED_UUID}")));
    let rows = identity_rows(&pool, &ws).await;
    assert!(
        rows.iter()
            .any(|r| r.0 == PAGE_DOC_UUID && r.1 == PAGE_DOC_ID && r.3 == "spec/home.md"),
        "home identity: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|r| r.0 == CREATED_UUID && r.1 == CREATED_UUID && r.3 == created_md),
        "created identity: {rows:?}"
    );
    assert!(last_flushed(&pool, &ws).await.is_some());
    let catalog_lf: Option<(i64, String)> = sqlx::query_as(
        "SELECT clock, git_sha FROM last_flushed
         WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(&ws)
    .bind(CATALOG_DOC_ID)
    .fetch_optional(&pool)
    .await
    .expect("catalog last_flushed");
    assert!(catalog_lf.is_some(), "catalog clock must be last_flushed");
}

#[tokio::test]
async fn rename_then_flush_git_mv_same_uuid() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed(CREATED_UUID, &format!("{CREATED_UUID}.md"), "folder:spec");
    let seed = catalog.encode();
    catalog.rename_created("protocol", "protocol.md");
    let renamed = catalog.encode();
    drop(catalog);
    persist_pin(&pool, &ws, &seed_home_pin()).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    let created_md = format!("spec/{CREATED_UUID}.md");
    let before = file_blob(tmp.path(), &created_md);
    assert!(!before.is_empty());

    persist_doc(&pool, &ws, CATALOG_DOC_ID, &renamed).await;
    let second = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(second.committed);
    assert!(tmp.path().join("spec/protocol.md").is_file());
    assert!(!tmp.path().join(&created_md).exists());
    let after = file_blob(tmp.path(), "spec/protocol.md");
    assert_eq!(
        before, after,
        "catalog-only rename must not fromDoc the page body"
    );
    let yaml = fs::read_to_string(tmp.path().join(PAGES_YAML_REL)).expect("yaml");
    assert!(yaml.contains("spec/protocol.md:"));
    assert!(!yaml.contains(&format!("spec/{CREATED_UUID}.md:")));
    assert!(yaml.contains(&format!("uuid: {CREATED_UUID}")));
    let rows = identity_rows(&pool, &ws).await;
    let created = rows.iter().find(|r| r.0 == CREATED_UUID).expect("row");
    assert_eq!(created.2, "protocol");
    assert_eq!(created.3, "spec/protocol.md");
    assert_eq!(head_subject(tmp.path()), "snapshot:");
}

#[tokio::test]
async fn move_then_flush_renames_not_copy() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed("protocol", "protocol.md", "folder:spec");
    let seed = catalog.encode();
    catalog.move_created_to_design();
    let moved = catalog.encode();
    drop(catalog);
    persist_pin(&pool, &ws, &seed_home_pin()).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &moved).await;
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(tmp.path().join("design/protocol.md").is_file());
    assert!(!tmp.path().join("spec/protocol.md").exists());
    let yaml = fs::read_to_string(tmp.path().join(PAGES_YAML_REL)).expect("yaml");
    assert!(yaml.contains("design/protocol.md:"));
    assert!(yaml.contains("design:"));
    assert!(yaml.contains(DESIGN_FOLDER));
    let rows = identity_rows(&pool, &ws).await;
    assert_eq!(
        rows.iter()
            .find(|r| r.0 == CREATED_UUID)
            .map(|r| r.3.as_str()),
        Some("design/protocol.md")
    );
    let renames = head_renames(tmp.path());
    assert!(
        renames
            .iter()
            .any(|(o, n)| o == "spec/protocol.md" && n == "design/protocol.md"),
        "expected git rename, got {renames:?}"
    );
    let clone = tempfile::tempdir().expect("clone");
    let dest = clone.path().join("copy");
    let status = Command::new("git")
        .args([
            "clone",
            "--",
            tmp.path().to_str().expect("wiki utf8"),
            dest.to_str().expect("clone utf8"),
        ])
        .status()
        .expect("git clone");
    assert!(status.success(), "clone without hub");
    assert!(dest.join("design/protocol.md").is_file());
    assert!(!dest.join("spec/protocol.md").exists());
    assert!(dest.join(PAGES_YAML_REL).is_file());
}

#[tokio::test]
async fn delete_then_flush_git_rm() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed(CREATED_UUID, &format!("{CREATED_UUID}.md"), "folder:spec");
    let seed = catalog.encode();
    catalog.delete_created();
    let deleted = catalog.encode();
    drop(catalog);
    persist_pin(&pool, &ws, &seed_home_pin()).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &deleted).await;
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(!tmp.path().join(format!("spec/{CREATED_UUID}.md")).exists());
    assert!(!tmp
        .path()
        .join(format!(".venus/ids/{CREATED_UUID}.json"))
        .exists());
    assert!(tmp.path().join(GIT_PATH).is_file());
    let yaml = fs::read_to_string(tmp.path().join(PAGES_YAML_REL)).expect("yaml");
    assert!(!yaml.contains(CREATED_UUID));
    let rows = identity_rows(&pool, &ws).await;
    assert!(rows.iter().all(|r| r.0 != CREATED_UUID), "{rows:?}");
    assert!(rows.iter().any(|r| r.0 == PAGE_DOC_UUID));
    assert_eq!(
        tombstoned_uuids(&pool, &ws).await,
        vec![CREATED_UUID.to_string()]
    );
    let deleted = db::page_deleted(&pool, &ws, CREATED_UUID)
        .await
        .expect("probe");
    assert!(deleted, "hub sees the Flush tombstone");
}

#[tokio::test]
async fn page_identity_swaps_paths_and_revives_a_tombstone() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let page = |uuid: &str, path: &str| CatalogPage {
        sql_uuid: uuid.into(),
        doc_id: uuid.into(),
        name: uuid[..8].into(),
        git_path: path.into(),
    };
    const A: &str = "aaaaaaaa-0000-4000-8000-000000000001";
    const B: &str = "bbbbbbbb-0000-4000-8000-000000000002";
    let write = |pages: Vec<CatalogPage>, leaving: Leaving| {
        let pool = pool.clone();
        let ws = ws.clone();
        async move {
            let walk = CatalogWalk {
                pages,
                folders: Vec::new(),
            };
            replace_page_identity(&pool, &ws, &walk, leaving)
                .await
                .expect("replace page_identity");
        }
    };

    write(
        vec![page(A, "spec/x.md"), page(B, "spec/y.md")],
        Leaving::Tombstone,
    )
    .await;
    write(
        vec![page(A, "spec/y.md"), page(B, "spec/x.md")],
        Leaving::Tombstone,
    )
    .await;
    let rows = identity_rows(&pool, &ws).await;
    assert_eq!(
        rows.iter()
            .map(|r| (r.0.as_str(), r.3.as_str()))
            .collect::<Vec<_>>(),
        vec![(B, "spec/x.md"), (A, "spec/y.md")]
    );

    write(vec![page(A, "spec/y.md")], Leaving::Tombstone).await;
    assert_eq!(tombstoned_uuids(&pool, &ws).await, vec![B.to_string()]);
    write(
        vec![page(A, "spec/y.md"), page(B, "spec/y2.md")],
        Leaving::Tombstone,
    )
    .await;
    assert!(
        tombstoned_uuids(&pool, &ws).await.is_empty(),
        "B is live again"
    );
    assert_eq!(identity_rows(&pool, &ws).await.len(), 2);

    write(vec![page(A, "spec/y.md")], Leaving::Drop).await;
    assert!(
        tombstoned_uuids(&pool, &ws).await.is_empty(),
        "yaml rebuild never tombstones"
    );
    assert_eq!(identity_rows(&pool, &ws).await.len(), 1);
}

#[tokio::test]
async fn convert_pins_skips_catalog() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_doc(
        &pool,
        &ws,
        CATALOG_DOC_ID,
        &catalog_seed_with_page(CREATED_UUID, &format!("{CREATED_UUID}.md"), "folder:spec"),
    )
    .await;
    persist_pin(&pool, &ws, &affine_home_pin("alpha")).await;
    let mut pins = PinMap::new();
    let claim = lease_this_wiki(&pool, &ws).await;
    cut_workspace(&pool, &claim.workspace_id, &mut pins, None)
        .await
        .expect("cut");
    assert!(
        pins.get(CATALOG_DOC_ID).is_some(),
        "catalog must be in the cut"
    );
    let converted = convert_pins(&ws, &pins).expect("convert");
    assert!(
        converted.iter().all(|(id, _)| id != CATALOG_DOC_ID),
        "must not fromDoc catalog: {:?}",
        converted.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    assert!(converted.iter().any(|(id, _)| id == PAGE_DOC_UUID));
}

#[test]
fn catalog_walk_is_rust_not_from_doc() {
    let catalog =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/catalog.rs"))
            .expect("catalog.rs");
    let git = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/git.rs"))
        .expect("git.rs");
    assert!(!catalog.contains("from_doc::"));
    assert!(!catalog.contains("MarkdownAdapter"));
    assert!(catalog.contains("y_octo"));
    assert!(!git.contains("from_doc"));
    // A rename is the old path removed and the new path added on an index
    // read from HEAD; git history pairs them (`head_renames` tests).
    assert!(git.contains("fn index_from_head"));
    assert!(git.contains("index_remove(&mut index, &old.git_path)"));
}

fn links_json(dir: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(dir.join(LINKS_JSON_REL)).expect("links.json");
    serde_json::from_str(&raw).expect("links json")
}

#[tokio::test]
async fn link_move_converts_inbound_not_unrelated() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed("protocol", "protocol.md", "folder:spec");
    catalog.add_page(THIRD_UUID, "third", "third.md", "folder:spec");
    let seed = catalog.encode();
    catalog.move_created_to_design();
    let moved = catalog.encode();
    drop(catalog);
    persist_pin(
        &pool,
        &ws,
        &affine_page_pin("Venus", "hello-home", Some(CREATED_UUID)),
    )
    .await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    persist_doc(&pool, &ws, THIRD_UUID, &affine_home_pin("third-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    let first = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(first.committed);
    let home = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("home.md");
    assert!(home.starts_with("# Venus\n"), "{home}");
    assert!(
        home.contains("[protocol](protocol.md)"),
        "catalog form, not workspace URL: {home}"
    );
    assert!(home.contains(&format!("<!-- venus:doc:{CREATED_UUID} -->")));
    assert!(!home.contains("./workspace/"));
    assert!(!home.contains("[untitled]"));
    let links = links_json(tmp.path());
    let inbound = links["inbound"][CREATED_UUID]
        .as_array()
        .expect("inbound uuid");
    assert!(
        inbound.iter().any(|v| v.as_str() == Some(PAGE_DOC_ID)),
        "inbound: {links}"
    );
    let uuid_blob = file_blob(tmp.path(), "spec/protocol.md");
    let third_blob = file_blob(tmp.path(), "spec/third.md");
    assert!(!uuid_blob.is_empty());
    assert!(!third_blob.is_empty());

    persist_doc(&pool, &ws, CATALOG_DOC_ID, &moved).await;
    let second = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(second.committed);
    assert!(
        second.converted_ids.iter().any(|id| id == PAGE_DOC_UUID),
        "home inbound must fromDoc: {:?}",
        second.converted_ids
    );
    assert!(
        !second.converted_ids.iter().any(|id| id == CREATED_UUID),
        "uuid has no outbound: git mv only: {:?}",
        second.converted_ids
    );
    assert!(
        !second.converted_ids.iter().any(|id| id == THIRD_UUID),
        "third must not convert: {:?}",
        second.converted_ids
    );
    let home2 = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("home after move");
    assert!(
        home2.contains("[protocol](../design/protocol.md)"),
        "{home2}"
    );
    assert!(home2.contains(&format!("<!-- venus:doc:{CREATED_UUID} -->")));
    assert!(!home2.contains("spec/protocol.md"));
    assert_eq!(file_blob(tmp.path(), "design/protocol.md"), uuid_blob);
    assert_eq!(file_blob(tmp.path(), "spec/third.md"), third_blob);
    let links2 = links_json(tmp.path());
    let inbound2 = links2["inbound"][CREATED_UUID]
        .as_array()
        .expect("inbound after move");
    assert!(inbound2.iter().any(|v| v.as_str() == Some(PAGE_DOC_ID)));
}

#[tokio::test]
async fn outbound_dirname_converts_source_not_target() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed("protocol", "protocol.md", "folder:spec");
    catalog.add_page(FOURTH_UUID, "fourth", "fourth.md", "folder:spec");
    let seed = catalog.encode();
    catalog.move_created_to_design();
    let moved = catalog.encode();
    drop(catalog);
    persist_pin(&pool, &ws, &affine_home_pin("home-plain")).await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(
        &pool,
        &ws,
        CREATED_UUID,
        &affine_page_pin("Venus", "created-body", Some(FOURTH_UUID)),
    )
    .await;
    persist_doc(&pool, &ws, FOURTH_UUID, &affine_home_pin("fourth-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    let before = fs::read_to_string(tmp.path().join("spec/protocol.md")).expect("protocol");
    assert!(before.contains("[fourth](fourth.md)"), "{before}");
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &moved).await;
    let second = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(
        second.converted_ids.iter().any(|id| id == CREATED_UUID),
        "dirname+outbound must fromDoc uuid: {:?}",
        second.converted_ids
    );
    assert!(
        !second.converted_ids.iter().any(|id| id == FOURTH_UUID),
        "fourth is not inbound: {:?}",
        second.converted_ids
    );
    let after = fs::read_to_string(tmp.path().join("design/protocol.md")).expect("moved");
    assert!(after.contains("[fourth](../spec/fourth.md)"), "{after}");
    assert!(after.contains(&format!("<!-- venus:doc:{FOURTH_UUID} -->")));
}

#[tokio::test]
async fn name_change_converts_inbound_link_text() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let catalog = CatalogDoc::seed("protocol", "protocol.md", "folder:spec");
    let seed = catalog.encode();
    catalog.rename_created("lease", "lease.md");
    let renamed = catalog.encode();
    drop(catalog);
    persist_pin(
        &pool,
        &ws,
        &affine_page_pin("Venus", "hello-home", Some(CREATED_UUID)),
    )
    .await;
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &seed).await;
    persist_doc(&pool, &ws, CREATED_UUID, &affine_home_pin("created-body")).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    let home = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("home");
    assert!(home.contains("[protocol](protocol.md)"), "{home}");
    persist_doc(&pool, &ws, CATALOG_DOC_ID, &renamed).await;
    let second = flush_wiki(&pool, &ws, &wiki, Duration::ZERO).await;
    assert!(
        second.converted_ids.iter().any(|id| id == PAGE_DOC_UUID),
        "name change inbound: {:?}",
        second.converted_ids
    );
    let home2 = fs::read_to_string(tmp.path().join(GIT_PATH)).expect("home2");
    assert!(home2.contains("[lease](lease.md)"), "{home2}");
    assert!(!home2.contains("[protocol]"));
}
