//! Shard key for a document. The layout turns this into a shard.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use sha2::{Digest, Sha256};

/// `(uint64 big-endian of sha256(doc_id as UTF-8)[0..8]) >> 1`.
/// Always non-negative. The writer does not turn this into a shard.
pub fn hkey(doc_id: &str) -> i64 {
    let digest = Sha256::digest(doc_id.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(bytes) >> 1) as i64
}
