//! M3 step-live-during-flush: persist and live CRDT keep moving during convert.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Write;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use sqlx::PgPool;
use tokio::sync::OnceCell;
use venus_hub::db;
use venus_sidecar::config::database_url_from_env;
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
         SET owner = 'live-worker', lease_until = now() + interval '2 minutes'
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
        owner: "live-worker".into(),
    }
}

/// Hub persist must return while convert is still sleeping (not queued behind git).
#[tokio::test]
async fn persist_not_paused_during_convert() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &affine_home_pin("alpha")).await;
    let t = dirty_clock(&pool, &ws).await;
    let tmp = tempfile::tempdir().expect("wiki dir");
    let wiki = WikiConfig::new(tmp.path());
    let claim = lease_this_wiki(&pool, &ws).await;
    let convert_sleep = Duration::from_millis(400);

    let flush_task = {
        let pool = pool.clone();
        let wiki = wiki.clone();
        tokio::spawn(async move {
            let mut pins = PinMap::new();
            flush_claimed(&pool, claim, &mut pins, &wiki, convert_sleep).await
        })
    };

    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(
        !flush_task.is_finished(),
        "convert sleep must still be in flight before persist"
    );
    let started = Instant::now();
    let seq_during = persist_pin(&pool, &ws, &affine_home_pin("during-flush")).await;
    let persist_took = started.elapsed();
    assert!(
        persist_took < Duration::from_millis(250),
        "persist took {persist_took:?}; must not wait on convert sleep {convert_sleep:?}"
    );
    assert!(
        seq_during > t,
        "crdt_update seq {seq_during} must land past pin T {t} during convert"
    );
    assert!(
        !flush_task.is_finished(),
        "persist returned while flush_claimed was still in convert sleep"
    );

    let outcome = flush_task.await.expect("join flush").expect("flush");
    assert!(outcome.committed);
    let dirty = dirty_clock(&pool, &ws).await;
    assert!(
        dirty > t,
        "dirty.clock {dirty} must move past pin T {t} during convert"
    );
}

#[test]
fn convert_sleep_is_after_cut_not_in_hub() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let queue = fs::read_to_string(root.join("src/queue.rs")).expect("queue.rs");
    let cut_at = queue
        .find("cut_workspace(pool, &claim.workspace_id, pins)")
        .expect("flush_claimed must cut");
    let sleep_at = queue
        .find("if !convert_sleep.is_zero()")
        .expect("SNAPSHOT_CONVERT_SLEEP_MS hook");
    assert!(
        sleep_at > cut_at,
        "convert sleep must be after cut COMMIT, not before the pin"
    );
    let hub_dir = root.join("../venus-hub/src");
    let mut hub_src = String::new();
    collect_rs(&hub_dir, &mut hub_src);
    assert!(
        !hub_src.contains("SNAPSHOT_CONVERT_SLEEP"),
        "convert delay is sidecar-only; hub must not pause apply/persist on Flush"
    );
}

fn collect_rs(dir: &std::path::Path, out: &mut String) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push_str(&fs::read_to_string(&path).unwrap_or_default());
        }
    }
}
