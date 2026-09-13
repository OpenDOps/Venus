//! In-process Path B `fromDoc` over a y-octo-hydrated pin.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod markdown;
mod ranges;
mod tree;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use y_octo::{CrdtWrite, Doc, RawEncoder};

use crate::convert::{Converted, Sidecar};
use crate::hydrate::hydrate_v1;
use crate::{DEFAULT_WORKSPACE_ID, PAGE_DOC_ID};

/// Convert pin bytes with the Rust adapter (no Node). Same `{ markdown, sidecar }` as the JS CLI.
pub fn from_pinned_bytes(bytes: &[u8]) -> Result<Converted> {
    from_pinned_bytes_in(bytes, DEFAULT_WORKSPACE_ID)
}

pub fn from_pinned_bytes_in(bytes: &[u8], workspace_id: &str) -> Result<Converted> {
    let doc = hydrate_v1(bytes)?;
    let tree = tree::BlockTree::load(&doc, workspace_id)?;
    let markdown = markdown::document_markdown(&tree)?;
    let title = ranges::place_page_title(&tree, &markdown)?;
    let items = ranges::ranged_items(&tree)?;
    let blocks = ranges::build_ranges(&markdown, &items, title)?;
    Ok(Converted {
        markdown,
        sidecar: Sidecar {
            doc_id: PAGE_DOC_ID.to_string(),
            clock: encode_clock(&doc)?,
            blocks,
        },
    })
}

fn encode_clock(doc: &Doc) -> Result<String> {
    let sv = doc.get_state_vector();
    let mut encoder = RawEncoder::default();
    CrdtWrite::write(&sv, &mut encoder).map_err(|e| anyhow!("encode state vector: {e}"))?;
    Ok(STANDARD.encode(encoder.into_inner()))
}
