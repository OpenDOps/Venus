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
use venus_sidecar::config::database_url_from_env;
use venus_sidecar::cut::GIT_PATH;
use venus_sidecar::git::{WikiConfig, SIDECAR_REL};
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{flush_claimed, Claim};
use venus_sidecar::{PAGE_DOC_ID, PAGE_DOC_UUID};
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
    let mut title = doc.create_text().expect("title");
    title.insert(0, "Venus").expect("title text");
    page.insert("prop:title".into(), title).expect("page title");
    blocks.insert("page".into(), page).expect("insert page");

    let mut note = doc.create_map().expect("note map");
    note.insert("sys:flavour".into(), "affine:note")
        .expect("note flavour");
    note.insert("sys:id".into(), "note").expect("note id");
    let mut note_children = doc.create_array().expect("note children");
    note_children.push("para").expect("note child");
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
    db::push_update(pool, workspace, PAGE_DOC_UUID, bytes)
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
