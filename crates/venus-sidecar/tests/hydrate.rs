//! M3 step-worker: y-octo hydrate of seeded doc:home pin bytes.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use venus_sidecar::hydrate::{encode_v1, hydrate_v1, is_noop_update};

const SEED_PIN: &[u8] = include_bytes!("fixtures/seed-home.yjs");

#[test]
fn product_hydrate_is_y_octo_not_node_apply_update() {
    let src = include_str!("../src/hydrate.rs");
    assert!(
        src.contains("try_from_binary_v1") && src.contains("apply_update_from_binary_v1"),
        "product hydrate must call y-octo apply / try_from_binary_v1"
    );
    assert!(
        !src.contains("Y.applyUpdate"),
        "product hydrate must not call Node Y.applyUpdate"
    );
}

#[test]
fn hydrate_seed_pin_does_not_throw_and_reencode_is_nonempty() {
    assert!(
        SEED_PIN.len() > 2,
        "seed-home.yjs missing or empty; run apps/web/scripts/write-sidecar-seed-pin.js"
    );
    assert!(
        !is_noop_update(SEED_PIN),
        "seed pin must not be a noop update"
    );

    let doc = hydrate_v1(SEED_PIN).expect("y-octo hydrate of seeded doc:home pin");
    let encoded = encode_v1(&doc).expect("y-octo re-encode");
    assert!(
        encoded.len() > 2,
        "re-encode of hydrated seed pin must be >2 bytes, got {}",
        encoded.len()
    );
}
