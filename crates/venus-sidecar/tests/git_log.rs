//! M3 step-git-log: git2 log of spec/home.md; GET /git/log JSON.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use git2::{Repository, Signature};
use http_body_util::BodyExt;
use tower::ServiceExt;
use venus_sidecar::cut::GIT_PATH;
use venus_sidecar::git::{self, GitLogEntry, WikiConfig};
use venus_sidecar::http::router_with_wiki;

fn commit_rel(dir: &Path, rel: &str, body: &str, message: &str) -> String {
    git::ensure_repo(dir).expect("ensure_repo");
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(&path, body).expect("write");
    let repo = Repository::open(dir).expect("open");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new(rel)).expect("add");
    index.write().expect("index write");
    let tree_id = index.write_tree().expect("write_tree");
    let tree = repo.find_tree(tree_id).expect("tree");
    let sig = Signature::now("Venus", "venus@localhost").expect("sig");
    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit().expect("peel")),
        Err(_) => None,
    };
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .expect("commit");
    oid.to_string()
}

#[test]
fn log_empty_without_repo() {
    let tmp = tempfile::tempdir().expect("tmp");
    let wiki = WikiConfig::new(tmp.path());
    let log = git::log_path(&wiki, GIT_PATH).expect("log");
    assert!(log.is_empty(), "no .git → empty log, not an error");
}

#[test]
fn log_shows_autocomment_newest_first() {
    let tmp = tempfile::tempdir().expect("tmp");
    let wiki = WikiConfig::new(tmp.path());
    let sha1 = commit_rel(tmp.path(), GIT_PATH, "# Venus\n", "snapshot: Venus");
    let sha2 = commit_rel(
        tmp.path(),
        GIT_PATH,
        "# Venus\n\nhello\n",
        "snapshot: Venus",
    );
    let log = git::log_path(&wiki, GIT_PATH).expect("log");
    assert_eq!(log.len(), 2);
    assert_eq!(
        log[0],
        GitLogEntry {
            subject: "snapshot: Venus".into(),
            sha: sha2,
        }
    );
    assert_eq!(log[1].sha, sha1);
    assert_eq!(log[1].subject, "snapshot: Venus");
}

#[test]
fn log_skips_commits_that_do_not_touch_home() {
    let tmp = tempfile::tempdir().expect("tmp");
    let wiki = WikiConfig::new(tmp.path());
    let home = commit_rel(tmp.path(), GIT_PATH, "# Venus\n", "snapshot: Venus");
    let _other = commit_rel(tmp.path(), "README.md", "nope\n", "chore: readme");
    let log = git::log_path(&wiki, GIT_PATH).expect("log");
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].sha, home);
    assert_eq!(log[0].subject, "snapshot: Venus");
}

#[test]
fn log_rejects_non_catalog_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let wiki = WikiConfig::new(tmp.path());
    assert!(git::log_path(&wiki, "../Cargo.toml").is_err());
    assert!(git::log_path(&wiki, "spec/other.md").is_err());
    assert!(!git::is_catalog_log_path(".."));
    assert!(!git::is_catalog_log_path("/spec/home.md"));
    assert!(git::is_catalog_log_path(GIT_PATH));
}

#[tokio::test]
async fn http_git_log_json_and_default_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let sha = commit_rel(tmp.path(), GIT_PATH, "# Venus\n", "snapshot: Venus");
    let app = router_with_wiki(None, WikiConfig::new(tmp.path()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/git/log?path=spec/home.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let rows: Vec<GitLogEntry> = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].subject, "snapshot: Venus");
    assert_eq!(rows[0].sha, sha);

    let app = router_with_wiki(None, WikiConfig::new(tmp.path()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/git/log")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let rows: Vec<GitLogEntry> = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(rows[0].subject, "snapshot: Venus");
}

#[tokio::test]
async fn http_git_log_empty_without_repo() {
    let tmp = tempfile::tempdir().expect("tmp");
    let app = router_with_wiki(None, WikiConfig::new(tmp.path()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/git/log?path=spec/home.md")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim(), "[]");
}

#[tokio::test]
async fn http_git_log_rejects_escape_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let app = router_with_wiki(None, WikiConfig::new(tmp.path()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/git/log?path=../Cargo.toml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn git_log_is_not_yjs_undo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let git = fs::read_to_string(root.join("src/git.rs")).expect("git.rs");
    let http = fs::read_to_string(root.join("src/http.rs")).expect("http.rs");
    for src in [git.as_str(), http.as_str()] {
        assert!(!src.contains("store.history"));
        assert!(!src.contains("Y.UndoManager"));
        assert!(!src.contains("undoManager"));
    }
}
