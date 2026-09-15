//! Compile hub + RPC protos. PROTOC from protoc-bin-vendored (no apt protoc).
//! SPDX-License-Identifier: MIT OR Apache-2.0

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);
    let proto_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    println!("cargo:rerun-if-changed={}", proto_root.display());
    tonic_build::configure().compile_protos(
        &[
            proto_root.join("venus/hub/v1/hub.proto"),
            proto_root.join("venus/rpc/v1/error.proto"),
        ],
        &[proto_root],
    )?;
    Ok(())
}
