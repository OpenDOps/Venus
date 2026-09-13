//! Pin hydrate: y-octo apply of Yjs update v1. Not the Node CRDT apply path.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, Result};
use y_octo::Doc;

/// Empty Yjs update v1 is typically two zero bytes.
pub fn is_noop_update(bin: &[u8]) -> bool {
    bin.is_empty() || bin.iter().all(|&b| b == 0)
}

/// Hydrate pin bytes with y-octo (`try_from_binary_v1` → `apply_update_from_binary_v1`).
pub fn hydrate_v1(bytes: &[u8]) -> Result<Doc> {
    Doc::try_from_binary_v1(bytes).map_err(|e| anyhow!("y-octo hydrate: {e}"))
}

pub fn encode_v1(doc: &Doc) -> Result<Vec<u8>> {
    doc.encode_update_v1()
        .map_err(|e| anyhow!("y-octo encode: {e}"))
}

/// Apply v1 onto an existing doc (same engine as [`hydrate_v1`]).
pub fn apply_v1(doc: &mut Doc, bin: &[u8]) -> Result<()> {
    doc.apply_update_from_binary_v1(bin)
        .map_err(|e| anyhow!("y-octo apply: {e}"))
}
