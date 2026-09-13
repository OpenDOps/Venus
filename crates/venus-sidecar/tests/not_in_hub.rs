//! M3 step-worker: convert / git2 / sidecar are not hub product deps.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn hub_dir() -> PathBuf {
    repo_root().join("crates/venus-hub")
}

fn collect_rs(dir: &Path, out: &mut String) {
    for entry in fs::read_dir(dir).unwrap_or_else(|_| panic!("read {}", dir.display())) {
        let entry = entry.expect("dirent");
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push_str(&fs::read_to_string(&path).unwrap_or_default());
            out.push('\n');
        }
    }
}

#[test]
fn hub_cargo_toml_has_no_sidecar_git2_or_from_doc() {
    let cargo = fs::read_to_string(hub_dir().join("Cargo.toml")).expect("hub Cargo.toml");
    for needle in ["venus-sidecar", "git2", "from-doc", "MarkdownAdapter"] {
        assert!(
            !cargo.contains(needle),
            "crates/venus-hub/Cargo.toml must not contain {needle}"
        );
    }
}

#[test]
fn hub_sources_have_no_sidecar_git2_from_doc_or_markdown_adapter() {
    let mut src = String::new();
    collect_rs(&hub_dir().join("src"), &mut src);
    for needle in ["venus-sidecar", "git2", "from-doc", "MarkdownAdapter"] {
        assert!(
            !src.contains(needle),
            "crates/venus-hub/src must not contain {needle}"
        );
    }
}

#[test]
fn hub_dockerfile_entrypoint_is_not_sidecar() {
    let df = fs::read_to_string(repo_root().join("deploy/hub/Dockerfile")).expect("hub Dockerfile");
    assert!(
        df.contains("CMD [\"venus-hub\"]"),
        "hub image must run venus-hub"
    );
    assert!(
        !df.contains("CMD [\"venus-sidecar\"]"),
        "hub image must not COPY/run venus-sidecar as the entrypoint"
    );
}

#[test]
fn cargo_tree_hub_does_not_include_sidecar_or_git2() {
    let root = repo_root();
    for pkg in ["venus-sidecar", "git2"] {
        let out = Command::new("cargo")
            .args(["tree", "-p", "venus-hub", "-i", pkg])
            .current_dir(&root)
            .output()
            .expect("cargo tree");
        assert!(
            !out.status.success(),
            "cargo tree -p venus-hub -i {pkg} must fail (not a hub dep); stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
