//! M3.0 step-recon-hub: y-octo apply + y-protocols without keck or Postgres.
//! M4 step-recon-catalog: gate M3, catalog Actuals, two docs on one Room / lease.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::OnceCell;
use venus_hub::config::database_url_from_env;
use venus_hub::db;
use venus_hub::lease::Lease;
use venus_hub::protocol::{apply_v1, decode_sync_messages, encode_doc_update, encode_v1};
use venus_hub::room::{Hub, Room, OUTBOUND_CAP, PERSIST_BYTES};
use venus_hub::CATALOG_DOC_ID;
use y_octo::{Any, Doc, DocMessage, SyncMessage, TextDeltaOp, TextInsert};

/// `yjs@13.6.32` `Y.encodeStateAsUpdate` after `doc.getMap('spike').set('k','v')` (23 bytes).
const YJS_SPIKE_KV_HEX: &str = "0101fc92c5bf0c002801057370696b65016b0177017600";

/// `yjs@13.6.32` `Y.Text('t')` insert `"hello"` then `format(0, 5, { bold: true })`.
const YJS_BOLD_HELLO_HEX: &str = "010387bee4bb0300040101740568656c6c6f4687bee4bb030004626f6c6404747275658687bee4bb030404626f6c64046e756c6c00";

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn map_has_v(doc: &Doc) -> bool {
    match doc.get_map("spike") {
        Ok(map) => format!("{:?}", map.get("k")).contains('v'),
        Err(_) => false,
    }
}

#[test]
fn api_map_chosen_backend_is_venus_hub() {
    let map = include_str!("../../../docs/design/api-map.md");
    let rest = match map.split_once("### Chosen backend (M3.0)") {
        Some((_, rest)) => rest,
        None => panic!("api-map.md missing Chosen backend (M3.0)"),
    };
    let chosen = rest
        .split("### Chosen backend (M1")
        .next()
        .expect("M3.0 chosen section");
    assert!(
        chosen.contains("crates/venus-hub") && chosen.contains("y-octo"),
        "Chosen backend must be the Venus hub + y-octo, not keck"
    );
    assert!(
        chosen.contains(venus_hub::DEFAULT_WORKSPACE_ID)
            && (chosen.contains("/collaboration/") || chosen.contains("collaboration/")),
        "Chosen backend must name the AFFiNE WS URL"
    );
    assert!(
        chosen.contains("/api/block/") && chosen.contains("/export"),
        "Chosen backend must name the export advertisement GET"
    );
    assert!(
        chosen.contains("venus.hub.v1.Hub/ExportDoc"),
        "Chosen backend must name gRPC ExportDoc"
    );
    assert!(
        chosen.contains("/api/blobs/") && chosen.contains(venus_hub::DEFAULT_WORKSPACE_ID),
        "Chosen backend must name blob paths"
    );
    assert!(
        chosen.contains("DATABASE_URL"),
        "Chosen backend must name DATABASE_URL"
    );
    assert!(
        chosen.contains("`hub`") || chosen.contains("postgres hub"),
        "Chosen backend must name Compose service hub"
    );
    assert!(
        !chosen.contains("keck SHA `276e0e94719a652483119c5fea16be13293ee21c`"),
        "M3.0 Chosen backend must not be the M1 keck SHA"
    );
}

/// M4 step-recon-catalog: M3 board steps 1–9 (and pin sub-steps) are `done`.
#[test]
fn m3_board_is_closed() {
    let board = include_str!("../../../docs/design/M3/M3.state.yaml");
    let pending = board
        .lines()
        .filter(|l| l.trim() == "state: pending")
        .count();
    assert_eq!(
        pending, 0,
        "M4 recon fails closed while M3.state.yaml still has pending steps"
    );
    for id in [
        "step-recon-snapshot",
        "step-worker",
        "step-rust-adapter",
        "step-pin",
        "step-pin-schema",
        "step-pin-queue",
        "step-pin-cut",
        "step-dirty-idle",
        "step-flush",
        "step-live-during-flush",
        "step-git-log",
        "step-verify",
    ] {
        assert_eq!(
            board_step_state(board, id),
            "done",
            "{id} must be done before M4 recon"
        );
    }
}

fn board_step_state<'a>(yaml: &'a str, id: &str) -> &'a str {
    let key = format!("{id}:");
    let mut lines = yaml.lines();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix(&key) else {
            continue;
        };
        if !rest.is_empty() && rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-') {
            continue;
        }
        for line in lines.by_ref() {
            let s = line.trim();
            if let Some(v) = s.strip_prefix("state:") {
                return v.trim();
            }
            if s.starts_with("step-") {
                break;
            }
        }
        panic!("{id}: no state: line");
    }
    panic!("M3.state.yaml missing {id}");
}

#[test]
fn api_map_m4_catalog_actuals() {
    let map = include_str!("../../../docs/design/api-map.md");
    let catalog = match map.split_once("## Names — catalog / tree / header") {
        Some((_, rest)) => rest.split("## Forbidden imports").next().expect("catalog"),
        None => panic!("api-map.md missing catalog names"),
    };
    assert!(
        !catalog.contains("*recon:") && !catalog.contains("| recon:"),
        "catalog Actuals must not be recon: placeholders"
    );
    assert!(
        catalog.contains("?doc=<sql uuid>")
            && catalog.contains("A1+B+C1")
            && catalog.contains("**400**")
            && catalog.contains("4fe5c16e-4be3-5700-a456-ecc8e86cdf1a")
            && catalog.contains("`venus:catalog`"),
        "M4 wire Actual must be A1 SQL uuid, omit=home, C1 400, catalog guid locked"
    );
    assert!(
        catalog.contains("**None.** Do not pre-seed a second page")
            && catalog.contains("{folder}/{uuid}.md"),
        "no second seed page; create is uuid.md"
    );
    assert!(
        catalog.contains("venus.hub.v1.Hub/ExportDoc")
            && catalog.contains("export_http_disabled")
            && catalog.contains("@headless-tree/react@1.7.0")
            && catalog.contains("data-testid=\"venus-tree\""),
        "gRPC ExportDoc; GET advertisement; tree pin 1.7.0 + venus-tree"
    );
    assert!(
        catalog.contains("`store.undo()`")
            && catalog.contains("canUndo$")
            && catalog.contains("canRedo$"),
        "header undo is store.undo / canUndo$ subscribe"
    );
    assert!(
        catalog.contains("On Flush, not on drop")
            && catalog.contains("page_identity")
            && catalog.contains("pages.yaml"),
        "git mv on Flush; page_identity + pages.yaml from catalog pin"
    );
    assert!(
        catalog.contains("typeof SharedWorker === 'function'")
            && catalog.contains("Not `navigator.serviceWorker`"),
        "SharedWorker detect is SharedWorker, not serviceWorker"
    );
    assert!(
        catalog.contains("One persist tick per Room"),
        "persist Actual is one tick per Room, not a task per doc"
    );
    let http = include_str!("../src/http.rs");
    assert!(
        http.contains("fn take_doc_id") && http.contains("attach_doc"),
        "hub HTTP must bind ?doc= via take_doc_id + attach_doc"
    );
    assert!(
        !http.contains("/collaboration/:workspace_id/:doc"),
        "wire A is a query, not a path suffix"
    );
}

/// M4 recon: two pages make S4/S5 live later. Do not “fix” apply here.
#[test]
fn from_doc_review_s4_s5_stay_open() {
    let review = include_str!("../../../docs/design/M2/fromDoc-review.md");
    assert!(
        review.contains("Revisit after M4")
            && review.contains("S4")
            && review.contains("S5")
            && review.contains("venus:doc:"),
        "fromDoc-review must still name S4/S5 and venus:doc: identity"
    );
    assert!(
        review.contains("Do **not** change apply")
            || review.contains("do not change apply")
            || review.contains("Do **not** change apply or `toDoc`"),
        "M4 recon must record: do not fix apply / toDoc in this step"
    );
    let map = include_str!("../../../docs/design/api-map.md");
    let catalog = map
        .split_once("## Names — catalog / tree / header")
        .expect("catalog")
        .1;
    assert!(
        catalog.contains("<!-- venus:doc:") && catalog.contains("Import: `venus:doc:` first"),
        "export Actual keeps venus:doc: first"
    );
}

#[test]
fn rpc_envelope_and_proto_exist() {
    let rpc = include_str!("../../../docs/design/rpc.md");
    assert!(
        rpc.contains("If `error` is present")
            && rpc.contains("advertisement")
            && rpc.contains("ExportDoc"),
        "rpc.md must define error / advertisement / gRPC export"
    );
    let hub_proto = include_str!("../../../proto/venus/hub/v1/hub.proto");
    assert!(
        hub_proto.contains("rpc ExportDoc") && hub_proto.contains("rpc ListDocs"),
        "hub.proto must declare ListDocs and ExportDoc"
    );
    let err_proto = include_str!("../../../proto/venus/rpc/v1/error.proto");
    assert!(
        err_proto.contains("message Error") && err_proto.contains("message Advertisement"),
        "error.proto must declare Error and Advertisement"
    );
}

#[test]
fn hub_crate_does_not_link_keck() {
    let cargo = include_str!("../Cargo.toml");
    assert!(cargo.contains("y-octo"), "venus-hub must apply with y-octo");
    assert!(
        !cargo.contains("jwst") && !cargo.contains("octobase") && !cargo.contains("keck"),
        "venus-hub must not depend on keck / jwst-rpc / OctoBase crates"
    );
}

/// M3 step-pin-schema: hub sources and `venus_mark_dirty` must not write `jobs`.
#[test]
fn hub_sources_do_not_insert_into_jobs() {
    let schema = include_str!("../src/schema.sql");
    assert!(
        !sql_inserts_into_jobs(schema),
        "schema.sql / venus_mark_dirty must not INSERT INTO jobs"
    );
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut rust = String::new();
    collect_rs(&src, &mut rust);
    assert!(
        !sql_inserts_into_jobs(&rust),
        "crates/venus-hub/src must not INSERT INTO jobs"
    );
}

fn collect_rs(dir: &std::path::Path, out: &mut String) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|_| panic!("read {}", dir.display())) {
        let entry = entry.expect("dirent");
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            out.push('\n');
        }
    }
}

/// True if the blob contains an INSERT targeting `jobs` (not CREATE TABLE).
fn sql_inserts_into_jobs(sql: &str) -> bool {
    let stripped = strip_sql_line_comments(sql).to_ascii_lowercase();
    let mut rest = stripped.as_str();
    while let Some(i) = rest.find("insert") {
        let after = rest[i + 6..].trim_start();
        let Some(after_into) = after.strip_prefix("into") else {
            rest = &rest[i + 6..];
            continue;
        };
        let after_into = after_into.trim_start();
        let after_into = after_into
            .strip_prefix("public.")
            .unwrap_or(after_into)
            .trim_start();
        if after_into.starts_with("jobs") {
            let next = after_into.as_bytes().get(4).copied();
            if next
                .map(|b| !b.is_ascii_alphanumeric() && b != b'_')
                .unwrap_or(true)
            {
                return true;
            }
        }
        rest = &rest[i + 6..];
    }
    false
}

fn strip_sql_line_comments(sql: &str) -> String {
    sql.lines()
        .map(|line| match line.find("--") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn apply_browser_yjs_map_key() {
    let bin = from_hex(YJS_SPIKE_KV_HEX);
    eprintln!("yjs spike.k=v update v1: {} bytes", bin.len());
    let mut doc = Doc::default();
    apply_v1(&mut doc, &bin).expect("y-octo apply of yjs@13.6.32 map update");
    assert!(map_has_v(&doc), "expected spike.k=v from browser Yjs bytes");
}

#[test]
fn apply_browser_yjs_bold_does_not_panic() {
    let bin = from_hex(YJS_BOLD_HELLO_HEX);
    eprintln!("yjs bold hello update v1: {} bytes", bin.len());
    let mut doc = Doc::default();
    apply_v1(&mut doc, &bin).expect("y-octo apply of yjs@13.6.32 Format/bold");
}

#[test]
fn y_octo_format_delta_does_not_panic() {
    let a = Doc::default();
    let mut text = a.get_or_create_text("t").expect("text");
    let mut attrs = y_octo::TextAttributes::new();
    attrs.insert("bold".to_string(), Any::True);
    text.apply_delta(&[TextDeltaOp::Insert {
        insert: TextInsert::Text("hello".into()),
        format: Some(attrs),
    }])
    .expect("format delta");
    let bin = encode_v1(&a).unwrap();
    eprintln!("y-octo bold hello update v1: {} bytes", bin.len());
    let mut b = Doc::default();
    apply_v1(&mut b, &bin).expect("apply format");
}

#[tokio::test]
async fn two_clients_step2_and_update_without_keck() {
    let room = Room::new(venus_hub::DEFAULT_WORKSPACE_ID.into(), Doc::default());

    let (a_id, a_tx, _a_rx) = room.connect_client();
    let _hello_a = room.attach(a_id, a_tx).await.expect("attach A");

    let a = Doc::default();
    {
        let mut map = a.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v").expect("insert");
    }
    let bin = encode_v1(&a).unwrap();
    eprintln!("hub-side spike.k=v update v1: {} bytes", bin.len());
    let frame = encode_doc_update(bin).unwrap();
    room.handle_binary(a_id, &frame).await;

    let (b_id, b_tx, mut b_rx) = room.connect_client();
    let hello_b = room.attach(b_id, b_tx).await.expect("attach B");
    let step2_bin = hello_b
        .iter()
        .find(|f| match decode_sync_messages(f).messages.first() {
            Some(SyncMessage::Doc(DocMessage::Step2(_))) => true,
            _ => false,
        })
        .expect("hello includes Step2");
    let step2 = decode_sync_messages(step2_bin).messages;
    let update = match &step2[0] {
        SyncMessage::Doc(DocMessage::Step2(update)) => update,
        other => panic!("expected Step2, got {other:?}"),
    };
    let mut b = Doc::default();
    apply_v1(&mut b, update).expect("B applies Step2");
    assert!(map_has_v(&b), "B Step2 must contain spike.k=v without keck");

    let a2 = Doc::default();
    {
        let mut map = a2.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v2").expect("insert");
    }
    let bin2 = encode_v1(&a2).unwrap();
    room.handle_binary(a_id, &encode_doc_update(bin2).unwrap())
        .await;
    let out = b_rx.recv().await.expect("B receives Update");
    let msgs = decode_sync_messages(&out).messages;
    assert!(
        matches!(&msgs[0], SyncMessage::Doc(DocMessage::Update(_))),
        "expected Update fanout, got {msgs:?}"
    );

    room.detach(b_id).await;
    let a3 = Doc::default();
    {
        let mut map = a3.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "v3").expect("insert");
    }
    room.handle_binary(a_id, &encode_doc_update(encode_v1(&a3).unwrap()).unwrap())
        .await;
    let late = tokio::time::timeout(std::time::Duration::from_millis(80), b_rx.recv()).await;
    match late {
        Err(_) | Ok(None) => {}
        Ok(Some(frame)) => panic!("detached client must not receive fanout, got {frame:?}"),
    }
}

/// S1: a client that does not read is detached on Full; apply does not wait.
#[tokio::test]
async fn lagged_client_is_detached_without_blocking_apply() {
    let room = Room::new("s1-lag".into(), Doc::default());
    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach(a_id, a_tx).await.expect("attach A");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach(b_id, b_tx).await.expect("attach B");

    let frame = encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap();
    for _ in 0..(OUTBOUND_CAP + 1) {
        room.handle_binary(a_id, &frame).await;
    }

    let mut queued = 0usize;
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(20), b_rx.recv()).await {
            Ok(Some(_)) => queued += 1,
            Ok(None) => break,
            Err(_) => panic!("lagged client must close after Full detach, not hang"),
        }
    }
    assert_eq!(
        queued, OUTBOUND_CAP,
        "the Full frame is dropped; queued frames stay until recv"
    );

    let (c_id, c_tx, mut c_rx) = room.connect_client();
    room.attach(c_id, c_tx).await.expect("attach C");
    room.handle_binary(a_id, &frame).await;
    let out = tokio::time::timeout(std::time::Duration::from_millis(200), c_rx.recv())
        .await
        .expect("apply must not block on a lagged peer")
        .expect("C receives Update");
    match decode_sync_messages(&out).messages.first() {
        Some(SyncMessage::Doc(DocMessage::Update(_))) => {}
        other => panic!("expected Update fanout, got {other:?}"),
    }
}

/// S6: detach when queued bytes cross the budget, not after 256 frames.
#[tokio::test]
async fn lagged_client_detaches_on_byte_budget() {
    let frame = encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap();
    let n = frame.len();
    let room = Room::with_budgets("s6-lag".into(), Doc::default(), 0, n * 2, PERSIST_BYTES);
    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach(a_id, a_tx).await.expect("attach A");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach(b_id, b_tx).await.expect("attach B");

    for _ in 0..3 {
        room.handle_binary(a_id, &frame).await;
    }

    let mut queued = 0usize;
    loop {
        match tokio::time::timeout(std::time::Duration::from_millis(20), b_rx.recv()).await {
            Ok(Some(_)) => queued += 1,
            Ok(None) => break,
            Err(_) => panic!("lagged client must close after byte-budget detach, not hang"),
        }
    }
    assert_eq!(
        queued, 2,
        "budget is 2×frame; the third send must detach, not wait for {OUTBOUND_CAP} slots"
    );

    let (c_id, c_tx, mut c_rx) = room.connect_client();
    room.attach(c_id, c_tx).await.expect("attach C");
    room.handle_binary(a_id, &frame).await;
    let out = tokio::time::timeout(std::time::Duration::from_millis(200), c_rx.recv())
        .await
        .expect("apply must not block on a lagged peer")
        .expect("C receives Update");
    match decode_sync_messages(&out).messages.first() {
        Some(SyncMessage::Doc(DocMessage::Update(_))) => {}
        other => panic!("expected Update fanout, got {other:?}"),
    }
}

/// L15: a truncated batch still applies the prefix; the socket is not closed.
#[tokio::test]
async fn truncated_frame_applies_prefix_without_closing() {
    let room = Room::new("l15".into(), Doc::default());
    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach(a_id, a_tx).await.expect("attach A");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach(b_id, b_tx).await.expect("attach B");

    let mut frame = encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap();
    let prefix_len = frame.len();
    frame.extend_from_slice(&[0xff, 0xfe, 0xfd]);
    let decoded = decode_sync_messages(&frame);
    assert_eq!(decoded.messages.len(), 1);
    assert_eq!(decoded.offset, prefix_len);
    assert_eq!(decoded.remaining, 3);

    room.handle_binary(a_id, &frame).await;
    let out = b_rx.recv().await.expect("B receives prefix Update");
    match decode_sync_messages(&out).messages.first() {
        Some(SyncMessage::Doc(DocMessage::Update(_))) => {}
        other => panic!("expected Update fanout, got {other:?}"),
    }

    let again = encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap();
    room.handle_binary(a_id, &again).await;
    let second = tokio::time::timeout(std::time::Duration::from_millis(200), b_rx.recv())
        .await
        .expect("A stays attached after a truncated frame")
        .expect("B receives the next Update");
    match decode_sync_messages(&second).messages.first() {
        Some(SyncMessage::Doc(DocMessage::Update(_))) => {}
        other => panic!("expected Update fanout, got {other:?}"),
    }
}

/// P7: fan-out clones `Bytes`, so two recipients share one frame allocation.
#[tokio::test]
async fn fanout_recipients_share_frame_bytes() {
    let room = Room::new("p7".into(), Doc::default());
    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach(a_id, a_tx).await.expect("attach A");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach(b_id, b_tx).await.expect("attach B");
    let (c_id, c_tx, mut c_rx) = room.connect_client();
    room.attach(c_id, c_tx).await.expect("attach C");

    let frame = encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap();
    room.handle_binary(a_id, &frame).await;

    let b = b_rx.recv().await.expect("B receives");
    let c = c_rx.recv().await.expect("C receives");
    assert_eq!(b, c, "B and C must get the same Update bytes");
    assert_eq!(
        b.as_ptr(),
        c.as_ptr(),
        "Outbound Bytes clone must share storage, not copy the frame"
    );
}

/// M4 recon spike: one Room, two docs, no keck. Home bytes stay; B on the
/// second `doc_id` sees A's map key.
#[tokio::test]
async fn two_docs_one_room_without_keck() {
    let room = Room::new(venus_hub::DEFAULT_WORKSPACE_ID.into(), Doc::default());

    let (home_id, home_tx, mut home_rx) = room.connect_client();
    room.attach(home_id, home_tx).await.expect("home");
    let home = Doc::default();
    {
        let mut map = home.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "home").expect("insert");
    }
    room.handle_binary(
        home_id,
        &encode_doc_update(encode_v1(&home).unwrap()).unwrap(),
    )
    .await;

    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach_doc(a_id, a_tx, CATALOG_DOC_ID)
        .await
        .expect("A on catalog uuid");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach_doc(b_id, b_tx, CATALOG_DOC_ID)
        .await
        .expect("B on catalog uuid");

    room.handle_binary(
        a_id,
        &encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap(),
    )
    .await;
    let out = tokio::time::timeout(Duration::from_millis(200), b_rx.recv())
        .await
        .expect("B receives second-doc Update")
        .expect("Update");
    let msgs = decode_sync_messages(&out).messages;
    let update = match &msgs[0] {
        SyncMessage::Doc(DocMessage::Update(bin)) => bin,
        other => panic!("expected Update, got {other:?}"),
    };
    let mut b = Doc::default();
    apply_v1(&mut b, update).expect("B applies");
    assert!(map_has_v(&b), "B on the second doc_id must see spike.k=v");

    let leaked = tokio::time::timeout(Duration::from_millis(80), home_rx.recv()).await;
    match leaked {
        Err(_) | Ok(None) => {}
        Ok(Some(frame)) => panic!("home socket must not receive second-doc fanout, got {frame:?}"),
    }

    let mut home_live = Doc::default();
    apply_v1(
        &mut home_live,
        &room.encode_live().await.expect("home live"),
    )
    .expect("apply home");
    assert!(
        map_has(&home_live, "k", "home"),
        "home must keep prior bytes"
    );
    assert!(
        !map_has_v(&home_live),
        "second doc must not apply into PAGE_DOC_ID"
    );
    let mut other_live = Doc::default();
    apply_v1(
        &mut other_live,
        &room
            .encode_live_doc(CATALOG_DOC_ID)
            .await
            .expect("other live"),
    )
    .expect("apply other");
    assert!(map_has_v(&other_live), "catalog uuid holds the spike");
}

fn map_has(doc: &Doc, key: &str, needle: &str) -> bool {
    match doc.get_map("spike") {
        Ok(map) => format!("{:?}", map.get(key)).contains(needle),
        Err(_) => false,
    }
}

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
    if let Ok(database_url) = database_url_from_env() {
        let pool = db::connect(&database_url)
            .await
            .expect("connect env postgres");
        db::migrate(&pool).await.expect("migrate");
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
    db::migrate(&pool).await.expect("migrate");
    TestPg {
        database_url,
        _container: Some(container),
    }
}

async fn connect_pool() -> PgPool {
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
    let mixed = t ^ (n << 48);
    let mut id = String::with_capacity(36);
    let _ = std::fmt::Write::write_fmt(
        &mut id,
        format_args!(
            "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
            (mixed >> 96) as u32,
            (mixed >> 80) as u16,
            (mixed >> 64) as u16 & 0x0fff,
            (mixed >> 48) as u16 & 0x0fff,
            mixed as u64 & 0x0000_ffff_ffff_ffff
        ),
    );
    id
}

async fn lease_count(pool: &PgPool, owner: &str) -> i64 {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM workspace_lease WHERE owner = $1")
            .bind(owner)
            .fetch_one(pool)
            .await
            .expect("lease count");
    n
}

/// M4 recon: two `doc_id`s on the M0 wiki owner. Not a second hub process.
#[tokio::test]
async fn two_docs_one_wiki_lease() {
    let pool = connect_pool().await;
    let workspace = unique_workspace();
    let owner = ["owner-m4-recon-", &workspace[0..8]].concat();
    let lease = Lease::new(pool.clone(), owner.clone(), Duration::from_secs(20));
    let hub = Hub::new(pool.clone(), lease, Duration::from_secs(3600), 32);
    let room = hub.get_room(&workspace).await.expect("one wiki room");

    let (home_id, home_tx, _home_rx) = room.connect_client();
    room.attach(home_id, home_tx).await.expect("home");
    let home = Doc::default();
    {
        let mut map = home.get_or_create_map("spike").expect("map");
        map.insert("k".to_string(), "home").expect("insert");
    }
    room.handle_binary(
        home_id,
        &encode_doc_update(encode_v1(&home).unwrap()).unwrap(),
    )
    .await;

    let (a_id, a_tx, _a_rx) = room.connect_client();
    room.attach_doc(a_id, a_tx, CATALOG_DOC_ID)
        .await
        .expect("A");
    let (b_id, b_tx, mut b_rx) = room.connect_client();
    room.attach_doc(b_id, b_tx, CATALOG_DOC_ID)
        .await
        .expect("B");
    room.handle_binary(
        a_id,
        &encode_doc_update(from_hex(YJS_SPIKE_KV_HEX)).unwrap(),
    )
    .await;
    let _ = tokio::time::timeout(Duration::from_millis(200), b_rx.recv())
        .await
        .expect("B fanout")
        .expect("Update");

    let mut home_live = Doc::default();
    apply_v1(
        &mut home_live,
        &room.encode_live().await.expect("home live"),
    )
    .expect("apply home");
    assert!(
        map_has(&home_live, "k", "home"),
        "home still has prior bytes after the second doc writes"
    );
    let mut other_live = Doc::default();
    apply_v1(
        &mut other_live,
        &room
            .encode_live_doc(CATALOG_DOC_ID)
            .await
            .expect("other live"),
    )
    .expect("apply other");
    assert!(map_has_v(&other_live), "second doc_id round-trips");
    assert_eq!(
        lease_count(&pool, hub.lease.owner()).await,
        1,
        "opening a second doc_id must not mint a second workspace_lease"
    );
    assert_eq!(room.workspace_id, workspace);
    hub.shutdown().await;
}
