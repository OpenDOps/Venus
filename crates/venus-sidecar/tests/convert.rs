//! M3 step-worker: Path B convert of the seed pin via from-pinned-cli.js.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use venus_sidecar::convert::{from_pinned_bytes, ConvertConfig};
use venus_sidecar::PAGE_DOC_ID;

const SEED_PIN: &[u8] = include_bytes!("fixtures/seed-home.yjs");

fn convert_cfg() -> ConvertConfig {
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web");
    ConvertConfig {
        node: PathBuf::from("node"),
        cli: web.join("src/host/mdgate/from-pinned-cli.js"),
        cwd: web,
        blob_origin: None,
        timeout: Duration::from_secs(120),
    }
}

#[test]
fn product_convert_is_path_b_cli_not_incremental() {
    let src = include_str!("../src/convert.rs");
    assert!(
        src.contains("from-pinned-cli"),
        "product convert must spawn from-pinned-cli.js"
    );
    assert!(
        !src.contains("incrementalFromDoc") && !src.contains("splice.js"),
        "product convert must not call incrementalFromDoc or import splice.js"
    );
    let cli = include_str!("../../../apps/web/src/host/mdgate/from-pinned-cli.js");
    assert!(cli.contains("fromPinnedBytes"));
    assert!(cli.contains("ssrLoadModule"));
    assert!(!cli.contains("incrementalFromDoc"));
    assert!(!cli.contains("simple-git"));
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_pin_path_b_contains_why_venus_and_empty_host() {
    let converted = from_pinned_bytes(SEED_PIN, &convert_cfg())
        .await
        .expect("Path B convert of seed pin");

    assert!(
        converted.markdown.contains("Why Venus"),
        "markdown missing Why Venus:\n{}",
        converted.markdown
    );
    assert!(
        converted.markdown.contains("Empty host"),
        "markdown missing Empty host:\n{}",
        converted.markdown
    );
    assert!(
        !converted.markdown.contains("<!-- id:"),
        "body must not embed <!-- id:… --> comments:\n{}",
        converted.markdown
    );
    assert_eq!(converted.sidecar.doc_id, PAGE_DOC_ID);
    assert!(
        !converted.sidecar.clock.is_empty(),
        "sidecar clock (pin state vector) must be present"
    );
    assert!(
        converted.sidecar.blocks.len() >= 2,
        "sidecar must have block ids (page + note ranges), got {}",
        converted.sidecar.blocks.len()
    );
    for block in &converted.sidecar.blocks {
        assert!(!block.id.is_empty(), "sidecar block id must be non-empty");
        assert!(
            block.end >= block.start,
            "range {}..{}",
            block.start,
            block.end
        );
    }
}
