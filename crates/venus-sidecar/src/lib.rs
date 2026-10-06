//! Venus snapshotter: y-octo hydrate + Path B convert (Node CLI oracle + Rust fromDoc).
//! SPDX-License-Identifier: MIT OR Apache-2.0

pub mod catalog;
pub mod config;
pub mod convert;
pub mod cut;
pub mod db;
pub mod from_doc;
pub mod git;
pub mod http;
pub mod hydrate;
pub mod links;
pub mod pin;
pub mod queue;

/// M0 wiki. UUID v5 (DNS) of `venus-m0`. Same as `venus_hub::DEFAULT_WORKSPACE_ID`.
pub const DEFAULT_WORKSPACE_ID: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";

/// Hyphenated UUID (`8-4-4-4-12`). Same shape as the hub's workspace check.
pub fn workspace_id_ok(id: &str) -> bool {
    let b = id.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if *c != b'-' {
                    return false;
                }
            }
            _ => {
                if !c.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// BlockSuite page guid / convert sidecar `docId`.
pub const PAGE_DOC_ID: &str = "doc:home";

/// SQL `dirty.doc_id` for [`PAGE_DOC_ID`]. Same as `venus_hub::PAGE_DOC_ID`.
pub const PAGE_DOC_UUID: &str = "395cd07b-bdb1-5f54-ada8-e9a3fabb6a20";

/// SQL `doc_id` for the catalog Y.Doc. Same as `venus_hub::CATALOG_DOC_ID`.
pub const CATALOG_DOC_ID: &str = "4fe5c16e-4be3-5700-a456-ecc8e86cdf1a";
