//! M3 step-pin-cut: MVCC SELECT of S into the pin Map; convert after COMMIT.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::OnceCell;
use venus_hub::db;
use venus_sidecar::config::database_url_from_env;
use venus_sidecar::cut::{cut_workspace, GIT_PATH};
use venus_sidecar::from_doc;
use venus_sidecar::pin::PinMap;
use venus_sidecar::queue::convert_pins;
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

/// Minimal affine page whose fromDoc markdown contains `paragraph`.
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

async fn lease_this_wiki(pool: &PgPool, workspace: &str) {
    sqlx::query(
        "INSERT INTO jobs (workspace_id, reason, not_before)
         VALUES ($1::uuid, 'idle', now())
         ON CONFLICT (workspace_id) DO NOTHING",
    )
    .bind(workspace)
    .execute(pool)
    .await
    .expect("jobs row");
    let mut tx = pool.begin().await.expect("lease begin");
    let owned: Option<String> = sqlx::query_scalar(
        "UPDATE jobs
         SET owner = 'cut-worker', lease_until = now() + interval '2 minutes'
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
        "claim this wiki then cut (do not SKIP LOCKED another test's job)"
    );
}

async fn claim_and_cut(pool: &PgPool, workspace: &str, pins: &mut PinMap) {
    lease_this_wiki(pool, workspace).await;
    cut_workspace(pool, workspace, pins).await.expect("cut");
}

#[test]
fn affine_home_pin_from_doc_has_alpha_not_beta() {
    let pin = affine_home_pin("alpha");
    let converted = from_doc::from_pinned_bytes(&pin).expect("fromDoc affine pin");
    assert!(
        converted.markdown.contains("alpha"),
        "expected alpha in {}",
        converted.markdown
    );
    assert!(
        !converted.markdown.contains("beta"),
        "unexpected beta in {}",
        converted.markdown
    );
}

#[test]
fn cut_is_not_get_export() {
    let cut = include_str!("../src/cut.rs");
    let pin = include_str!("../src/pin.rs");
    for (name, src) in [("cut.rs", cut), ("pin.rs", pin)] {
        for needle in ["/export", "reqwest", "get_doc", "FOR SHARE", "FOR UPDATE"] {
            assert!(
                !src.contains(needle),
                "{name} must not {needle} (MVCC plain SELECT, then fromDoc the Map)"
            );
        }
    }
    let queue = include_str!("../src/queue.rs");
    for needle in ["/export", "reqwest", "get_doc"] {
        assert!(!queue.contains(needle), "queue.rs must not {needle}");
    }
}

/// Persist `alpha` at clock T, cut S, persist `beta`, convert the Map → alpha only.
#[tokio::test]
async fn mvcc_pin_not_live_ram() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    let alpha = affine_home_pin("alpha");
    let beta = affine_home_pin("beta");
    let t = persist_pin(&pool, &ws, &alpha).await;
    assert_eq!(dirty_clock(&pool, &ws).await, t);

    let mut pins = PinMap::new();
    claim_and_cut(&pool, &ws, &mut pins).await;
    let pin = pins
        .get(PAGE_DOC_UUID)
        .expect("cut copied the dirty page")
        .clone();
    assert_eq!(pin.clock, t, "pin clock is dirty.clock at cut");
    assert_eq!(pins.git_path.as_deref(), Some(GIT_PATH));
    assert!(
        !pin.bytes.is_empty() && !pin.bytes.starts_with(b"#"),
        "pin bytes must be CRDT, not markdown"
    );

    let t2 = persist_pin(&pool, &ws, &beta).await;
    assert!(t2 > t, "later persist moves dirty.clock");
    assert_eq!(dirty_clock(&pool, &ws).await, t2);

    let converted = convert_pins(&ws, &pins).expect("fromDoc the Map");
    assert_eq!(converted.len(), 1);
    let markdown = &converted[0].1.markdown;
    assert_ne!(
        pin.bytes.as_slice(),
        markdown.as_bytes(),
        "pin bytes must not be the markdown string"
    );
    assert!(
        markdown.contains("alpha"),
        "Map convert must keep cut text alpha; got {markdown:?}"
    );
    assert!(
        !markdown.contains("beta"),
        "Map convert must not see persist after COMMIT; got {markdown:?}"
    );
}

/// Cut COMMIT releases the RR snapshot; persist still inserts during convert.
#[tokio::test(flavor = "multi_thread")]
async fn cut_released_before_from_doc() {
    let pool = connect_fresh().await;
    let ws = unique_workspace();
    persist_pin(&pool, &ws, &affine_home_pin("alpha")).await;

    let mut pins = PinMap::new();
    claim_and_cut(&pool, &ws, &mut pins).await;
    let pin = pins.get(PAGE_DOC_UUID).expect("pinned").clone();
    assert_eq!(pins.git_path.as_deref(), Some(GIT_PATH));

    let bytes = pin.bytes;
    let convert_ws = ws.clone();
    let convert = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        from_doc::from_pinned_bytes_in(&bytes, &convert_ws)
    });
    let persist = {
        let pool = pool.clone();
        let ws = ws.clone();
        let beta = affine_home_pin("beta");
        async move { persist_pin(&pool, &ws, &beta).await }
    };

    let (converted, seq) = tokio::join!(convert, persist);
    let converted = converted.expect("convert join").expect("fromDoc Map");
    assert!(
        converted.markdown.contains("alpha"),
        "convert used the pin, not live SQL: {}",
        converted.markdown
    );
    assert!(
        !converted.markdown.contains("beta"),
        "convert must not wait on a DB snapshot: {}",
        converted.markdown
    );
    assert!(seq > pin.clock, "crdt_update INSERT during convert");
}
