//! y-protocols/sync framing via y-octo (`MSG_SYNC=0`, awareness=1, auth=2, query=3).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use y_octo::{
    read_sync_message, write_sync_message, CrdtRead, CrdtWrite, Doc, DocMessage, RawDecoder,
    RawEncoder, StateVector, SyncMessage,
};

pub fn encode_sync(msg: &SyncMessage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    write_sync_message(&mut buf, msg).context("write_sync_message")?;
    Ok(buf)
}

/// Parsed prefix of a y-protocols frame. `remaining > 0` means the tail was
/// dropped (decode error or a non-advancing read). Callers must log that.
#[derive(Debug)]
pub struct DecodedSync {
    pub messages: Vec<SyncMessage>,
    /// Byte offset of the unparsed tail (`input.len() - remaining`).
    pub offset: usize,
    pub remaining: usize,
}

pub fn decode_sync_messages(input: &[u8]) -> DecodedSync {
    let total = input.len();
    let mut rest = input;
    let mut messages = Vec::new();
    loop {
        if rest.is_empty() {
            return DecodedSync {
                messages,
                offset: total,
                remaining: 0,
            };
        }
        match read_sync_message(rest) {
            Ok((tail, msg)) => {
                if tail.len() == rest.len() {
                    return DecodedSync {
                        messages,
                        offset: total - rest.len(),
                        remaining: rest.len(),
                    };
                }
                messages.push(msg);
                rest = tail;
            }
            Err(_) => {
                return DecodedSync {
                    messages,
                    offset: total - rest.len(),
                    remaining: rest.len(),
                };
            }
        }
    }
}

pub fn encode_doc_step1(sv: Vec<u8>) -> Result<Vec<u8>> {
    encode_sync(&SyncMessage::Doc(DocMessage::Step1(sv)))
}

pub fn encode_doc_step2(update: Vec<u8>) -> Result<Vec<u8>> {
    encode_sync(&SyncMessage::Doc(DocMessage::Step2(update)))
}

pub fn encode_doc_update(update: Vec<u8>) -> Result<Vec<u8>> {
    encode_sync(&SyncMessage::Doc(DocMessage::Update(update)))
}

pub fn encode_auth_ok() -> Result<Vec<u8>> {
    encode_sync(&SyncMessage::Auth(None))
}

pub fn encode_awareness_empty() -> Result<Vec<u8>> {
    encode_sync(&SyncMessage::Awareness(Default::default()))
}

pub fn empty_state_vector() -> Vec<u8> {
    // Yjs SV v1: varuint count of client entries. Zero clients → a single 0.
    Vec::from([0u8])
}

pub fn encode_state_vector(doc: &Doc) -> Result<Vec<u8>> {
    let sv = doc.get_state_vector();
    let mut encoder = RawEncoder::default();
    CrdtWrite::write(&sv, &mut encoder).map_err(|e| anyhow::anyhow!("encode state vector: {e}"))?;
    let bytes: Vec<u8> = encoder.into_inner();
    Ok(bytes)
}

/// Empty Yjs update v1 is typically two zero bytes. Skip persist/broadcast.
pub fn is_noop_update(bin: &[u8]) -> bool {
    bin.is_empty() || bin.iter().all(|&b| b == 0)
}

pub fn apply_v1(doc: &mut Doc, bin: &[u8]) -> Result<()> {
    doc.apply_update_from_binary_v1(bin)
        .map_err(|e| anyhow::anyhow!("y-octo apply: {e}"))
}

pub fn encode_v1(doc: &Doc) -> Result<Vec<u8>> {
    doc.encode_update_v1()
        .map_err(|e| anyhow::anyhow!("y-octo encode: {e}"))
}

/// Diff against a client Step1 state vector. Falls back to a full encode
/// if the bytes are not a state vector (M3.0: full encode is allowed).
pub fn encode_step2_for(doc: &Doc, client_sv: &[u8]) -> Result<Vec<u8>> {
    if let Some(sv) = decode_state_vector(client_sv) {
        match doc.encode_state_as_update_v1(&sv) {
            Ok(bin) => return Ok(bin),
            Err(e) => tracing::warn!(error = %e, "diff encode failed; sending full update"),
        }
    }
    encode_v1(doc)
}

fn decode_state_vector(bytes: &[u8]) -> Option<StateVector> {
    let mut decoder = RawDecoder::new(bytes);
    StateVector::read(&mut decoder).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_doc_state_vector_is_zero() {
        let doc = Doc::default();
        assert_eq!(encode_state_vector(&doc).unwrap(), empty_state_vector());
    }

    #[test]
    fn roundtrip_step2() {
        let body = encode_doc_step2(vec![1, 2, 3]).unwrap();
        let decoded = decode_sync_messages(&body);
        assert_eq!(decoded.remaining, 0);
        assert_eq!(decoded.messages.len(), 1);
        match &decoded.messages[0] {
            SyncMessage::Doc(DocMessage::Step2(u)) => assert_eq!(u, &vec![1, 2, 3]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn decode_reports_trailing_bytes() {
        let first = encode_doc_update(vec![1, 2, 3]).unwrap();
        let second = encode_doc_step1(vec![0]).unwrap();
        let mut batch = first.clone();
        batch.extend_from_slice(&second);
        let clean = decode_sync_messages(&batch);
        assert_eq!(clean.messages.len(), 2);
        assert_eq!(clean.remaining, 0);

        batch.extend_from_slice(&[0xff, 0xfe, 0xfd]);
        let decoded = decode_sync_messages(&batch);
        assert_eq!(decoded.messages.len(), 2);
        assert_eq!(decoded.remaining, 3);
        assert_eq!(decoded.offset, first.len() + second.len());
    }

    #[test]
    fn two_docs_map_key() {
        let a = Doc::default();
        {
            let mut map = a.get_or_create_map("spike").expect("map");
            map.insert("k".to_string(), "v").expect("insert");
        }
        let bin = encode_v1(&a).unwrap();
        let mut b = Doc::default();
        apply_v1(&mut b, &bin).unwrap();
        let map = b.get_map("spike").expect("get_map");
        let got = map.get("k");
        assert!(
            format!("{got:?}").contains('v'),
            "expected spike.k=v, got {got:?}"
        );
    }

    #[test]
    fn format_mark_does_not_panic() {
        use y_octo::{Any, TextDeltaOp, TextInsert};

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
        let mut b = Doc::default();
        apply_v1(&mut b, &bin).expect("apply format");
    }
}
