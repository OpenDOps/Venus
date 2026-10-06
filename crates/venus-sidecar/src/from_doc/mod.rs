//! In-process Path B `fromDoc` over a y-octo-hydrated pin.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod markdown;
mod ranges;
mod tree;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use y_octo::{CrdtWrite, Doc, RawEncoder};

use crate::catalog::CatalogWalk;
use crate::convert::{Converted, Sidecar};
use crate::hydrate::hydrate_v1;
use crate::{DEFAULT_WORKSPACE_ID, PAGE_DOC_ID};

use tree::CatalogLinkCtx;

pub use markdown::missing_linked_doc;

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
    from_pinned_bytes_catalog_missing(bytes, workspace_id, sidecar_doc_id, walk, None)
}

pub fn from_pinned_bytes_catalog_missing(
    bytes: &[u8],
    workspace_id: &str,
    sidecar_doc_id: &str,
    walk: Option<&CatalogWalk>,
    missing: Option<&HashMap<String, String>>,
) -> Result<Converted> {
    let missing = missing.map(|names| Arc::new(names.clone()));
    from_pinned_bytes_catalog_shared(
        bytes,
        workspace_id,
        sidecar_doc_id,
        walk,
        None,
        missing.as_ref(),
    )
}

/// docId → (name, gitPath), built once per Flush.
pub fn catalog_page_index(walk: &CatalogWalk) -> HashMap<String, (String, String)> {
    walk.pages
        .iter()
        .map(|page| {
            (
                page.doc_id.clone(),
                (page.name.clone(), page.git_path.clone()),
            )
        })
        .collect()
}

/// `pages` is the Flush's one docId → (name, gitPath) map. `None` builds it
/// from `walk` for a single pin. A Flush passes the same `Arc` to every page.
pub fn from_pinned_bytes_catalog_shared(
    bytes: &[u8],
    workspace_id: &str,
    sidecar_doc_id: &str,
    walk: Option<&CatalogWalk>,
    pages: Option<&Arc<HashMap<String, (String, String)>>>,
    missing: Option<&Arc<HashMap<String, String>>>,
) -> Result<Converted> {
    let doc = hydrate_v1(bytes)?;
    let owned_pages;
    let owned_missing;
    let catalog = match walk {
        Some(walk) => {
            let pages = match pages {
                Some(pages) => pages,
                None => {
                    owned_pages = Arc::new(catalog_page_index(walk));
                    &owned_pages
                }
            };
            let missing = match missing {
                Some(missing) => missing,
                None => {
                    owned_missing = Arc::new(HashMap::new());
                    &owned_missing
                }
            };
            catalog_ctx(walk, sidecar_doc_id, pages, missing)
        }
        None => None,
    };
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

fn catalog_ctx(
    walk: &CatalogWalk,
    sidecar_doc_id: &str,
    pages: &Arc<HashMap<String, (String, String)>>,
    missing: &Arc<HashMap<String, String>>,
) -> Option<CatalogLinkCtx> {
    let source = walk
        .pages
        .iter()
        .find(|p| p.doc_id == sidecar_doc_id || p.sql_uuid == sidecar_doc_id)?;
    Some(CatalogLinkCtx {
        source_git_path: source.git_path.clone(),
        pages: Arc::clone(pages),
        missing: Arc::clone(missing),
    })
}

fn encode_clock(doc: &Doc) -> Result<String> {
    let sv = doc.get_state_vector();
    let mut encoder = RawEncoder::default();
    CrdtWrite::write(&sv, &mut encoder).map_err(|e| anyhow!("encode state vector: {e}"))?;
    Ok(STANDARD.encode(encoder.into_inner()))
}
