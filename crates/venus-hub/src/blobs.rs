//! Blob hash = BlockSuite `sha()`: SHA-256, base64url **with padding**.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use sha2::{Digest, Sha256};

pub fn blob_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    URL_SAFE.encode(digest)
}

pub fn sniff_content_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 8 && bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4e {
        return "image/png";
    }
    if bytes.len() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        return "image/jpeg";
    }
    if bytes.len() >= 6 && bytes[0] == 0x47 && bytes[1] == 0x49 && bytes[2] == 0x46 {
        return "image/gif";
    }
    if bytes.len() >= 12
        && bytes[0] == 0x52
        && bytes[8] == 0x57
        && bytes[9] == 0x45
        && bytes[10] == 0x42
        && bytes[11] == 0x50
    {
        return "image/webp";
    }
    "application/octet-stream"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_blob_matches_blocksuite_sha() {
        // api-map: POST body `hello-blob` → this id (M1 recon vs keck).
        assert_eq!(
            blob_hash(b"hello-blob"),
            "V6JWxl21rxj4oEx6Qt8mwwCd0BOB-6qux7Qy_DDUyNA="
        );
    }
}
