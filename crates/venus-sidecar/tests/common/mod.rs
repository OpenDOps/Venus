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
    match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            let mut msg = String::from("missing pin fixture ");
            msg.push_str(&path.display().to_string());
            msg.push_str(" (");
            msg.push_str(&e.to_string());
            msg.push_str("); run apps/web/scripts/write-sidecar-fromdoc-pins.js");
            std::panic::panic_any(msg)
        }
    }
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
            let mut msg = String::from("byte ");
            msg.push_str(&i.to_string());
            msg.push_str(": rust=");
            msg.push(ab[i] as char);
            msg.push_str(" other=");
            msg.push(bb[i] as char);
            msg.push_str(" rust_ctx=");
            msg.push_str(&a[lo..hi.min(a.len())]);
            msg.push_str(" other_ctx=");
            msg.push_str(&b[lo..hi.min(b.len())]);
            return msg;
        }
    }
    let mut msg = String::from("prefix equal; rust_len=");
    msg.push_str(&a.len().to_string());
    msg.push_str(" other_len=");
    msg.push_str(&b.len().to_string());
    msg
}

pub fn first_range_diff(a: &[SidecarBlock], b: &[SidecarBlock]) -> String {
    if a.len() != b.len() {
        let mut msg = String::from("blocks.len rust=");
        msg.push_str(&a.len().to_string());
        msg.push_str(" other=");
        msg.push_str(&b.len().to_string());
        return msg;
    }
    for (i, (ra, rb)) in a.iter().zip(b.iter()).enumerate() {
        if ra.id != rb.id || ra.start != rb.start || ra.end != rb.end {
            let mut msg = String::from("block[");
            msg.push_str(&i.to_string());
            msg.push_str("]: rust=");
            msg.push_str(&ra.id);
            msg.push_str(" ");
            msg.push_str(&ra.start.to_string());
            msg.push_str("..");
            msg.push_str(&ra.end.to_string());
            msg.push_str(" other=");
            msg.push_str(&rb.id);
            msg.push_str(" ");
            msg.push_str(&rb.start.to_string());
            msg.push_str("..");
            msg.push_str(&rb.end.to_string());
            return msg;
        }
    }
    String::from("ranges equal")
}

/// Markdown bytes, sidecar `docId`, and `blocks[]` (`id`,`start`,`end`). Clock is not compared.
pub fn assert_identity(label: &str, rust: &Converted, js: &Converted) {
    assert_eq!(
        rust.markdown, js.markdown,
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
        rust.sidecar.blocks, js.sidecar.blocks,
        "{label}: sidecar ranges mismatch: {}",
        first_range_diff(&rust.sidecar.blocks, &js.sidecar.blocks)
    );
}
