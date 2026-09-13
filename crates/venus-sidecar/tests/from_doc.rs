//! M3 step-rust-adapter: in-process fromDoc vs JS CLI oracle.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod common;

use venus_sidecar::from_doc;
use venus_sidecar::PAGE_DOC_ID;

use crate::common::{
    assert_identity, convert_cfg, first_byte_diff, load_fixture, LARGE_PARAGRAPH_COUNT,
    MIN_LARGE_MARKDOWN,
};

fn panic_convert(label: &str, what: &str, e: impl std::fmt::Display) -> ! {
    let mut msg = String::from(label);
    msg.push_str(": ");
    msg.push_str(what);
    msg.push_str(": ");
    msg.push_str(&e.to_string());
    std::panic::panic_any(msg)
}

const SEED_PIN: &[u8] = include_bytes!("fixtures/seed-home.yjs");
const SEED_GOLDEN: &str = include_str!("../../../apps/web/src/host/mdgate/goldens/seed.fromDoc.md");

const M2_GOLDENS: &[(&str, &str, &str)] = &[
    (
        "rt-paragraph.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-paragraph.md"),
        "rt-paragraph",
    ),
    (
        "rt-headings.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-headings.md"),
        "rt-headings",
    ),
    (
        "rt-list.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-list.md"),
        "rt-list",
    ),
    (
        "rt-code.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-code.md"),
        "rt-code",
    ),
    (
        "rt-link.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-link.md"),
        "rt-link",
    ),
    (
        "rt-marks.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-marks.md"),
        "rt-marks",
    ),
    (
        "rt-linked-doc.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-linked-doc.md"),
        "rt-linked-doc",
    ),
    (
        "loss-color.yjs",
        include_str!("../../../apps/web/src/host/mdgate/goldens/loss-color.md"),
        "loss-color",
    ),
];

#[test]
fn rust_from_doc_is_in_sidecar_not_a_live_store() {
    let src = include_str!("../src/from_doc/mod.rs");
    assert!(src.contains("hydrate_v1"));
    assert!(!src.contains("session.store"));
    assert!(!src.contains("incrementalFromDoc"));
    assert!(!src.contains("simple-git"));
}

#[test]
fn seed_rust_markdown_matches_m2_golden() {
    let rust = from_doc::from_pinned_bytes(SEED_PIN).expect("rust fromDoc");
    assert_eq!(
        rust.markdown, SEED_GOLDEN,
        "markdown mismatch: {}",
        first_byte_diff(&rust.markdown, SEED_GOLDEN)
    );
    assert_eq!(rust.sidecar.doc_id, PAGE_DOC_ID);
    assert!(rust.markdown.ends_with('\n'));
    assert!(!rust.markdown.contains("<!-- id:"));
    assert!(rust.sidecar.blocks.len() >= 2);
}

#[test]
fn m2_golden_pins_match_committed_markdown() {
    for (pin_name, golden, label) in M2_GOLDENS {
        let pin = load_fixture(pin_name);
        let rust = from_doc::from_pinned_bytes(&pin)
            .unwrap_or_else(|e| panic_convert(label, "rust fromDoc failed", e));
        assert_eq!(
            rust.markdown, *golden,
            "{label}: rust markdown vs M2 golden: {}",
            first_byte_diff(&rust.markdown, golden)
        );
        assert_eq!(rust.sidecar.doc_id, PAGE_DOC_ID);
        assert!(
            !rust.markdown.contains("<!-- id:"),
            "{label}: body must not embed <!-- id:… -->"
        );
    }
}

#[test]
fn opaque_image_without_blobs_matches_js_omission_form() {
    let pin = load_fixture("opaque-image.yjs");
    let rust = from_doc::from_pinned_bytes(&pin).expect("rust fromDoc of unresolved image pin");
    let paragraph_golden =
        include_str!("../../../apps/web/src/host/mdgate/goldens/rt-paragraph.md");
    assert_eq!(
        rust.markdown, paragraph_golden,
        "unresolved affine:image must not invent a nicer dump: {}",
        first_byte_diff(&rust.markdown, paragraph_golden)
    );
    assert!(!rust.markdown.contains("!["));
    assert!(!rust.markdown.contains("<!-- id:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn seed_and_m2_goldens_identical_to_js_cli() {
    let cfg = convert_cfg();
    let rust = from_doc::from_pinned_bytes(SEED_PIN).expect("rust seed");
    let js = venus_sidecar::convert::from_pinned_bytes(SEED_PIN, &cfg)
        .await
        .expect("JS CLI seed");
    assert_identity("seed-home", &rust, &js);
    assert_eq!(rust.markdown, SEED_GOLDEN);

    for (pin_name, golden, label) in M2_GOLDENS {
        let pin = load_fixture(pin_name);
        let rust = from_doc::from_pinned_bytes(&pin)
            .unwrap_or_else(|e| panic_convert(label, "rust fromDoc failed", e));
        let js = venus_sidecar::convert::from_pinned_bytes(&pin, &cfg)
            .await
            .unwrap_or_else(|e| panic_convert(label, "JS CLI failed", e));
        assert_identity(label, &rust, &js);
        assert_eq!(
            rust.markdown, *golden,
            "{label}: rust matched JS but not M2 golden: {}",
            first_byte_diff(&rust.markdown, golden)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn large_file_identical_to_js_cli() {
    let pin = load_fixture("large-home.yjs");
    let rust = from_doc::from_pinned_bytes(&pin).expect("rust large fromDoc");
    assert!(
        rust.markdown.len() >= MIN_LARGE_MARKDOWN,
        "large markdown must be ≥ 512 KiB, got {} bytes",
        rust.markdown.len()
    );
    let para_rows = rust
        .sidecar
        .blocks
        .iter()
        .filter(|b| b.id != rust.sidecar.blocks[0].id)
        .count();
    assert_eq!(
        para_rows, LARGE_PARAGRAPH_COUNT,
        "sidecar note rows must equal 2000 paragraphs (+ title wrapper); blocks.len()={}",
        rust.sidecar.blocks.len()
    );
    assert_eq!(
        rust.sidecar.blocks.len(),
        LARGE_PARAGRAPH_COUNT + 1,
        "title + 2000 paragraphs"
    );
    assert_eq!(&rust.markdown[rust.markdown.len() - 1..], "\n");
    assert!(!rust.markdown.contains("<!-- id:"));

    let js = venus_sidecar::convert::from_pinned_bytes(&pin, &convert_cfg())
        .await
        .expect("JS CLI large fromDoc");
    assert_identity("large-home", &rust, &js);
    assert_eq!(js.markdown.len(), rust.markdown.len());
}
