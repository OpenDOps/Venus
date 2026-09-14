//! Venus snapshotter: y-octo hydrate + Path B convert (Node CLI oracle + Rust fromDoc).
//! SPDX-License-Identifier: MIT OR Apache-2.0

pub mod config;
pub mod convert;
pub mod cut;
pub mod db;
pub mod from_doc;
pub mod git;
pub mod http;
pub mod hydrate;
pub mod pin;
pub mod queue;

/// M0 wiki. UUID v5 (DNS) of `venus-m0`. Same as `venus_hub::DEFAULT_WORKSPACE_ID`.
pub const DEFAULT_WORKSPACE_ID: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";

/// BlockSuite page guid / convert sidecar `docId`.
pub const PAGE_DOC_ID: &str = "doc:home";

/// SQL `dirty.doc_id` for [`PAGE_DOC_ID`]. Same as `venus_hub::PAGE_DOC_ID`.
pub const PAGE_DOC_UUID: &str = "395cd07b-bdb1-5f54-ada8-e9a3fabb6a20";
