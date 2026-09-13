//! Venus snapshotter: y-octo hydrate + Path B convert (Node CLI oracle + Rust fromDoc).
//! SPDX-License-Identifier: MIT OR Apache-2.0

pub mod config;
pub mod convert;
pub mod from_doc;
pub mod http;
pub mod hydrate;

/// M0 wiki. UUID v5 (DNS) of `venus-m0`. Same as `venus_hub::DEFAULT_WORKSPACE_ID`.
pub const DEFAULT_WORKSPACE_ID: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";

/// BlockSuite page guid / convert sidecar `docId`.
pub const PAGE_DOC_ID: &str = "doc:home";
