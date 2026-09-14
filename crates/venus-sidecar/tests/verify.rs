//! M3 step-verify: git clone of a Flush wiki is readable markdown (no hub).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::OnceCell;
use venus_hub::db;
use venus_sidecar::config::database_url_from_env;
use venus_sidecar::cut::GIT_PATH;
use venus_sidecar::git::WikiConfig;
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::{flush_claimed, Claim};
use venus_sidecar::PAGE_DOC_UUID;
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

async fn persist_pin(pool: &PgPool, workspace: &str, bytes: &[u8]) -> i64 {
    db::push_update(pool, workspace, PAGE_DOC_UUID, bytes)
        .await
        .expect("persist crdt_update")
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
         SET owner = 'verify-worker', lease_until = now() + interval '2 minutes'
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
        owner: "verify-worker".into(),
    }
}

async fn flush_wiki(pool: &PgPool, workspace: &str, wiki: &WikiConfig) {
    let claim = lease_this_wiki(pool, workspace).await;
    let mut pins = PinMap::new();
    flush_claimed(pool, claim, &mut pins, wiki, Duration::ZERO)
        .await
        .expect("flush_claimed");
}

fn git_clone(src: &Path, dest: &Path) {
    let out = Command::new("git")
        .args([
            "clone",
            "--",
            src.to_str().expect("src utf8"),
            dest.to_str().expect("dest utf8"),
        ])
        .output()
        .expect("git clone");
    assert!(
        out.status.success(),
        "git clone failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn assert_readable_markdown(md: &str) {
    assert!(
        md.starts_with('#'),
        "clone must be markdown source, not Yjs: {md:?}"
    );
    assert!(
        !md.contains('\0'),
        "clone markdown must not contain NUL (Yjs/binary)"
    );
    let yjs = seed_home_pin();
    assert_ne!(
        md.as_bytes(),
        yjs.as_slice(),
        "clone spec/home.md must not be the pin .yjs bytes"
    );
}

/// Flush seed + unique word, `git clone` elsewhere, read `spec/home.md` with no hub.
#[tokio::test]
async fn clone_elsewhere_is_readable_markdown() {
    let pool = connect_fresh().await;

    let seed_ws = unique_workspace();
    persist_pin(&pool, &seed_ws, &seed_home_pin()).await;
    let seed_wiki = tempfile::tempdir().expect("seed wiki");
    flush_wiki(&pool, &seed_ws, &WikiConfig::new(seed_wiki.path())).await;
    let seed_clone = tempfile::tempdir().expect("seed clone");
    let seed_dest = seed_clone.path().join("wiki");
    git_clone(seed_wiki.path(), &seed_dest);
    let seed_md = fs::read_to_string(seed_dest.join(GIT_PATH)).expect("cloned spec/home.md");
    assert_readable_markdown(&seed_md);
    assert!(
        seed_md.contains("Why Venus"),
        "clone must contain seed H1: {seed_md:?}"
    );
    assert!(
        seed_md.contains("Empty host"),
        "clone must contain seed H2: {seed_md:?}"
    );

    let word = format!("verify-word-{}", std::process::id());
    let word_ws = unique_workspace();
    persist_pin(&pool, &word_ws, &affine_home_pin(&word)).await;
    let word_wiki = tempfile::tempdir().expect("word wiki");
    flush_wiki(&pool, &word_ws, &WikiConfig::new(word_wiki.path())).await;
    let word_clone = tempfile::tempdir().expect("word clone");
    let word_dest = word_clone.path().join("wiki");
    git_clone(word_wiki.path(), &word_dest);
    let word_md = fs::read_to_string(word_dest.join(GIT_PATH)).expect("cloned unique md");
    assert_readable_markdown(&word_md);
    assert!(
        word_md.contains(&word),
        "clone must contain unique word {word}: {word_md:?}"
    );
}
