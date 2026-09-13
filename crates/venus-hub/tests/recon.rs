//! M3.0 step-recon-hub: y-octo apply + y-protocols without keck or Postgres.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use venus_hub::protocol::{apply_v1, decode_sync_messages, encode_doc_update, encode_v1};
use venus_hub::room::{Room, OUTBOUND_CAP, PERSIST_BYTES};
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
        "Chosen backend must name the export curl"
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

#[test]
fn hub_crate_does_not_link_keck() {
    let cargo = include_str!("../Cargo.toml");
    assert!(cargo.contains("y-octo"), "venus-hub must apply with y-octo");
    assert!(
        !cargo.contains("jwst") && !cargo.contains("octobase") && !cargo.contains("keck"),
        "venus-hub must not depend on keck / jwst-rpc / OctoBase crates"
    );
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
