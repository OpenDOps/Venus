//! Venus collab hub: y-octo apply, AFFiNE WebSocket, Postgres persist.
//! SPDX-License-Identifier: MIT OR Apache-2.0

pub mod blobs;
pub mod config;
pub mod db;
pub mod http;
pub mod lease;
pub mod protocol;
pub mod room;
pub mod rpc;

/// M0 wiki. UUID v5 (DNS) of `venus-m0`. URL `/collaboration/:workspace_id`.
pub const DEFAULT_WORKSPACE_ID: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";

/// SQL `doc_id` for the one page. UUID v5 (DNS) of `doc:home`.
/// BlockSuite `createDoc` stays `doc:home` (Yjs guid); the hub does not
/// store that string.
pub const PAGE_DOC_ID: &str = "395cd07b-bdb1-5f54-ada8-e9a3fabb6a20";

/// SQL `doc_id` for the catalog Y.Doc. UUID v5 (DNS) of `venus:catalog`.
/// Hub stores this uuid only. Client guid stays `venus:catalog`.
pub const CATALOG_DOC_ID: &str = "4fe5c16e-4be3-5700-a456-ecc8e86cdf1a";

pub const SUBPROTOCOL: &str = "AFFiNE";
