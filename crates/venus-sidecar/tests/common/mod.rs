//! Shared Path B test helpers (JS CLI oracle + identity diffs).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use venus_sidecar::convert::{ConvertConfig, Converted, SidecarBlock};

pub const MIN_LARGE_MARKDOWN: usize = 512 * 1024;
pub const LARGE_PARAGRAPH_COUNT: usize = 2000;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn load_fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing pin fixture {} ({e}); run apps/web/scripts/write-sidecar-fromdoc-pins.js",
            path.display()
        )
    })
}

pub fn convert_cfg() -> ConvertConfig {
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web");
    ConvertConfig {
        node: PathBuf::from("node"),
        cli: web.join("src/host/mdgate/from-pinned-cli.js"),
        cwd: web,
        blob_origin: None,
        timeout: Duration::from_secs(180),
    }
}

pub fn first_byte_diff(a: &str, b: &str) -> String {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let n = ab.len().min(bb.len());
    for i in 0..n {
        if ab[i] != bb[i] {
            let lo = i.saturating_sub(24);
            let hi = (i + 24).min(a.len()).min(b.len());
            return format!(
                "byte {i}: rust={:?} other={:?} rust_ctx={:?} other_ctx={:?}",
                ab[i] as char,
                bb[i] as char,
                &a[lo..hi.min(a.len())],
                &b[lo..hi.min(b.len())]
            );
        }
    }
    format!("prefix equal; rust_len={} other_len={}", a.len(), b.len())
}

pub fn first_range_diff(a: &[SidecarBlock], b: &[SidecarBlock]) -> String {
    if a.len() != b.len() {
        return format!("blocks.len rust={} other={}", a.len(), b.len());
    }
    for (i, (ra, rb)) in a.iter().zip(b.iter()).enumerate() {
        if ra.id != rb.id || ra.start != rb.start || ra.end != rb.end {
            return format!("block[{i}]: rust={ra:?} other={rb:?}");
        }
    }
    "ranges equal".into()
}

/// Markdown bytes, sidecar `docId`, and `blocks[]` (`id`,`start`,`end`). Clock is not compared.
pub fn assert_identity(label: &str, rust: &Converted, js: &Converted) {
    assert_eq!(
        rust.markdown,
        js.markdown,
        "{label}: markdown mismatch: {}",
        first_byte_diff(&rust.markdown, &js.markdown)
    );
    assert!(
        rust.markdown.ends_with('\n'),
        "{label}: markdown must end with EOF \\n"
    );
    assert!(
        !rust.markdown.contains("<!-- id:"),
        "{label}: body must not embed <!-- id:… -->"
    );
    assert_eq!(
        rust.sidecar.doc_id, js.sidecar.doc_id,
        "{label}: docId mismatch"
    );
    assert_eq!(
        rust.sidecar.blocks,
        js.sidecar.blocks,
        "{label}: sidecar ranges mismatch: {}",
        first_range_diff(&rust.sidecar.blocks, &js.sidecar.blocks)
    );
}
