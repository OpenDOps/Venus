//! In-process Path B `fromDoc` over a y-octo-hydrated pin.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod markdown;
mod ranges;
mod tree;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use y_octo::{CrdtWrite, Doc, RawEncoder};

use crate::catalog::CatalogWalk;
use crate::convert::{Converted, Sidecar};
use crate::hydrate::hydrate_v1;
use crate::{DEFAULT_WORKSPACE_ID, PAGE_DOC_ID};

use tree::CatalogLinkCtx;

/// Convert pin bytes with the Rust adapter (no Node). Same `{ markdown, sidecar }` as the JS CLI.
pub fn from_pinned_bytes(bytes: &[u8]) -> Result<Converted> {
    from_pinned_bytes_in(bytes, DEFAULT_WORKSPACE_ID)
}

pub fn from_pinned_bytes_in(bytes: &[u8], workspace_id: &str) -> Result<Converted> {
    from_pinned_bytes_as(bytes, workspace_id, PAGE_DOC_ID)
}

pub fn from_pinned_bytes_as(
    bytes: &[u8],
    workspace_id: &str,
    sidecar_doc_id: &str,
) -> Result<Converted> {
    from_pinned_bytes_catalog(bytes, workspace_id, sidecar_doc_id, None)
}

pub fn from_pinned_bytes_catalog(
    bytes: &[u8],
    workspace_id: &str,
    sidecar_doc_id: &str,
    walk: Option<&CatalogWalk>,
) -> Result<Converted> {
    let doc = hydrate_v1(bytes)?;
    let catalog = walk.and_then(|w| catalog_ctx(w, sidecar_doc_id));
    let tree = tree::BlockTree::load_with(&doc, workspace_id, catalog)?;
    let markdown = markdown::document_markdown(&tree)?;
    let title = ranges::place_page_title(&tree, &markdown)?;
    let items = ranges::ranged_items(&tree)?;
    let blocks = ranges::build_ranges(&markdown, &items, title)?;
    Ok(Converted {
        markdown,
        sidecar: Sidecar {
            doc_id: sidecar_doc_id.to_string(),
            clock: encode_clock(&doc)?,
            blocks,
        },
    })
}

fn catalog_ctx(walk: &CatalogWalk, sidecar_doc_id: &str) -> Option<CatalogLinkCtx> {
    let source = walk
        .pages
        .iter()
        .find(|p| p.doc_id == sidecar_doc_id || p.sql_uuid == sidecar_doc_id)?;
    Some(CatalogLinkCtx {
        source_git_path: source.git_path.clone(),
        pages: walk
            .pages
            .iter()
            .map(|p| (p.doc_id.clone(), (p.name.clone(), p.git_path.clone())))
            .collect(),
    })
}

fn encode_clock(doc: &Doc) -> Result<String> {
    let sv = doc.get_state_vector();
    let mut encoder = RawEncoder::default();
    CrdtWrite::write(&sv, &mut encoder).map_err(|e| anyhow!("encode state vector: {e}"))?;
    Ok(STANDARD.encode(encoder.into_inner()))
}
