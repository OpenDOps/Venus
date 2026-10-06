//! Nested wiki repo: autoinit on first snapshot, then one autocomment commit.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{ErrorCode, IndexAddOption, Repository, RepositoryInitOptions, Signature};
use serde::{Deserialize, Serialize};

use crate::catalog::{CatalogWalk, OldPage};
use crate::convert::{Converted, SidecarBlock};
use crate::cut::GIT_PATH;
use crate::pin::PinMap;
use crate::{PAGE_DOC_ID, PAGE_DOC_UUID};

pub const DEFAULT_WIKI_DIR: &str = "wiki";
pub const DEFAULT_BRANCH: &str = "main";
pub const DEFAULT_AUTHOR_NAME: &str = "Venus";
pub const DEFAULT_AUTHOR_EMAIL: &str = "venus@localhost";
pub const SIDECAR_REL: &str = ".venus/ids/doc:home.json";
pub const PAGES_YAML_REL: &str = ".venus/pages.yaml";
/// Plain-text workspace uuid this directory publishes. Written on the first Flush.
pub const WORKSPACE_STAMP_REL: &str = ".venus/workspace";
pub use crate::links::LINKS_JSON_REL;

#[derive(Debug, Clone)]
pub struct WikiConfig {
    pub dir: PathBuf,
    pub author_name: String,
    pub author_email: String,
    /// Workspace allowed to publish into [`Self::dir`]. `None` is a test wiki
    /// that accepts whichever id the caller flushes. Product config always sets it.
    pub workspace_id: Option<String>,
}

impl WikiConfig {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            author_name: DEFAULT_AUTHOR_NAME.into(),
            author_email: DEFAULT_AUTHOR_EMAIL.into(),
            workspace_id: None,
        }
    }

    pub fn bound(dir: impl Into<PathBuf>, workspace_id: impl Into<String>) -> Self {
        let mut wiki = Self::new(dir);
        wiki.workspace_id = Some(workspace_id.into());
        wiki
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotWrite {
    pub sha: String,
    pub committed: bool,
}

#[derive(Debug, Serialize)]
struct DiskSidecar<'a> {
    #[serde(rename = "docId")]
    doc_id: &'a str,
    clock: i64,
    blocks: &'a [SidecarBlock],
}

/// First ATX H1, else seed title `Venus`. Message is `snapshot: <title>`.
pub fn snapshot_title(markdown: &str) -> &str {
    markdown
        .strip_prefix("# ")
        .and_then(|rest| rest.lines().next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Venus")
}

/// Converted pins → files → one commit (or skip if the tree matches HEAD).
/// Empty / no home page and no catalog walk: no init, no commit.
pub fn commit_pins(
    wiki: &WikiConfig,
    workspace_id: &str,
    pins: &PinMap,
    converted: &[(String, Converted)],
    walk: Option<&CatalogWalk>,
    old_pages: &std::collections::HashMap<String, OldPage>,
) -> Result<Option<SnapshotWrite>> {
    if pins.is_empty() {
        return Ok(None);
    }
    if let Some(bound) = wiki.workspace_id.as_deref() {
        anyhow::ensure!(
            bound.eq_ignore_ascii_case(workspace_id),
            "wiki {} is bound to {bound}, refusing workspace {workspace_id}",
            wiki.dir.display()
        );
    }
    ensure_repo(&wiki.dir)?;
    ensure_workspace_stamp(&wiki.dir, workspace_id)?;
    match walk {
        Some(walk) => commit_catalog_walk(wiki, pins, converted, walk, old_pages),
        None => commit_home_only(wiki, pins, converted),
    }
}

fn commit_home_only(
    wiki: &WikiConfig,
    pins: &PinMap,
    converted: &[(String, Converted)],
) -> Result<Option<SnapshotWrite>> {
    let mut home: Option<(&Converted, i64)> = None;
    for (doc_id, conv) in converted {
        if doc_id != PAGE_DOC_UUID && doc_id != PAGE_DOC_ID {
            continue;
        }
        let clock = pins
            .get(doc_id)
            .or_else(|| pins.get(PAGE_DOC_UUID))
            .or_else(|| pins.get(PAGE_DOC_ID))
            .map(|e| e.clock)
            .context("pin clock missing for home page")?;
        home = Some((conv, clock));
        break;
    }
    let Some((conv, clock)) = home else {
        return Ok(None);
    };
    let git_path = pins.git_path.as_deref().unwrap_or(GIT_PATH);
    ensure_repo(&wiki.dir)?;
    write_page(&wiki.dir, git_path, PAGE_DOC_ID, conv, clock)?;
    for (hash, bytes) in pins.blobs() {
        write_blob(&wiki.dir, hash, bytes)?;
    }
    let message = format!("snapshot: {}", snapshot_title(&conv.markdown));
    commit_tree(wiki, &message).map(Some)
}

fn commit_catalog_walk(
    wiki: &WikiConfig,
    pins: &PinMap,
    converted: &[(String, Converted)],
    walk: &CatalogWalk,
    old_pages: &std::collections::HashMap<String, OldPage>,
) -> Result<Option<SnapshotWrite>> {
    anyhow::ensure!(
        walk.pages.iter().any(|p| p.doc_id == PAGE_DOC_ID),
        "catalog pin has no home doc"
    );
    ensure_repo(&wiki.dir)?;
    let converted_by_id: std::collections::HashMap<&str, &Converted> = converted
        .iter()
        .map(|(id, conv)| (id.as_str(), conv))
        .collect();
    let live: std::collections::HashSet<&str> =
        walk.pages.iter().map(|p| p.sql_uuid.as_str()).collect();

    for page in &walk.pages {
        let conv = converted_by_id
            .get(page.sql_uuid.as_str())
            .or_else(|| converted_by_id.get(page.doc_id.as_str()));
        if let Some(conv) = conv {
            let clock = pins
                .get(&page.sql_uuid)
                .or_else(|| pins.get(&page.doc_id))
                .map(|e| e.clock)
                .context("pin clock missing for converted page")?;
            if let Some(old) = old_pages.get(&page.sql_uuid) {
                if old.git_path != page.git_path {
                    git_mv(&wiki.dir, &old.git_path, &page.git_path)?;
                }
            }
            write_page(&wiki.dir, &page.git_path, &page.doc_id, conv, clock)?;
            continue;
        }
        if let Some(old) = old_pages.get(&page.sql_uuid) {
            if old.git_path != page.git_path {
                git_mv(&wiki.dir, &old.git_path, &page.git_path)?;
            }
        }
    }

    for (sql_uuid, old) in old_pages {
        if live.contains(sql_uuid.as_str()) {
            continue;
        }
        git_rm_page(&wiki.dir, &old.git_path, &old.doc_id)?;
    }

    write_pages_yaml(&wiki.dir, walk)?;
    crate::links::persist_on_flush(&wiki.dir, walk, converted, old_pages)?;
    for (hash, bytes) in pins.blobs() {
        write_blob(&wiki.dir, hash, bytes)?;
    }

    let message = autocomment(converted);
    commit_tree(wiki, &message).map(Some)
}

fn autocomment(converted: &[(String, Converted)]) -> String {
    if converted.len() == 1 {
        format!("snapshot: {}", snapshot_title(&converted[0].1.markdown))
    } else {
        "snapshot:".to_string()
    }
}

pub fn sidecar_rel(doc_id: &str) -> String {
    format!(".venus/ids/{doc_id}.json")
}

fn git_mv(dir: &std::path::Path, old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    let src = dir.join(old);
    if !src.exists() {
        return Ok(());
    }
    let dst = dir.join(new);
    anyhow::ensure!(
        !dst.exists() || same_file(&src, &dst),
        "git mv would overwrite {new}"
    );
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::rename(&src, &dst).with_context(|| format!("git mv {old} -> {new}"))?;
    Ok(())
}

fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn git_rm_page(dir: &std::path::Path, git_path: &str, doc_id: &str) -> Result<()> {
    let md = dir.join(git_path);
    if md.exists() {
        fs::remove_file(&md).with_context(|| format!("git rm {git_path}"))?;
    }
    let sidecar = dir.join(sidecar_rel(doc_id));
    if sidecar.exists() {
        fs::remove_file(&sidecar).with_context(|| format!("git rm {}", sidecar.display()))?;
    }
    Ok(())
}

fn write_pages_yaml(dir: &std::path::Path, walk: &CatalogWalk) -> Result<()> {
    let path = dir.join(PAGES_YAML_REL);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&path, crate::catalog::pages_yaml(walk))
        .with_context(|| format!("write {}", path.display()))
}

fn write_page(
    dir: &std::path::Path,
    git_path: &str,
    doc_id: &str,
    conv: &Converted,
    clock: i64,
) -> Result<()> {
    let md_path = dir.join(git_path);
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&md_path, conv.markdown.as_bytes())
        .with_context(|| format!("write {}", md_path.display()))?;
    let sidecar = DiskSidecar {
        doc_id,
        clock,
        blocks: &conv.sidecar.blocks,
    };
    let mut json = serde_json::to_vec_pretty(&sidecar).context("sidecar json")?;
    json.push(b'\n');
    let sidecar_path = dir.join(sidecar_rel(doc_id));
    if let Some(parent) = sidecar_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&sidecar_path, json).with_context(|| format!("write {}", sidecar_path.display()))?;
    Ok(())
}

/// Write `.venus/workspace` on the first Flush. A later Flush for a different
/// id refuses, so two workspaces cannot share one wiki directory.
pub fn ensure_workspace_stamp(dir: &Path, workspace_id: &str) -> Result<()> {
    let path = dir.join(WORKSPACE_STAMP_REL);
    if path.exists() {
        let existing =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let existing = existing.trim();
        anyhow::ensure!(
            existing.eq_ignore_ascii_case(workspace_id),
            "wiki is bound to {existing}, refusing workspace {workspace_id}"
        );
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&path, format!("{workspace_id}\n"))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// `git2` init (branch `main`) if `.git` is missing. No origin. Idempotent.
pub fn ensure_repo(dir: &Path) -> Result<Repository> {
    fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
    fs::create_dir_all(dir.join("spec")).context("mkdir spec")?;
    fs::create_dir_all(dir.join(".venus/ids")).context("mkdir .venus/ids")?;
    fs::create_dir_all(dir.join("assets")).context("mkdir assets")?;
    if dir.join(".git").exists() {
        return Repository::open(dir).with_context(|| format!("open {}", dir.display()));
    }
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head(DEFAULT_BRANCH);
    Repository::init_opts(dir, &opts).with_context(|| format!("git2 init {}", dir.display()))
}

fn write_blob(dir: &Path, hash: &str, bytes: &[u8]) -> Result<()> {
    let path = dir.join("assets").join(hash);
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

fn commit_tree(wiki: &WikiConfig, message: &str) -> Result<SnapshotWrite> {
    let repo = Repository::open(&wiki.dir).context("open wiki for commit")?;
    anyhow::ensure!(
        repo.remotes()
            .context("remotes")?
            .iter()
            .flatten()
            .next()
            .is_none(),
        "wiki repo must have no origin in M3"
    );
    let mut index = repo.index().context("index")?;
    index
        .add_all(["."], IndexAddOption::DEFAULT, None)
        .context("git add -A")?;
    index.update_all(["."], None).context("git add -u")?;
    index.write().context("index write")?;
    let tree_id = index.write_tree().context("write_tree")?;
    let tree = repo.find_tree(tree_id).context("find tree")?;
    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit().context("peel HEAD")?),
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => None,
        Err(e) => return Err(e).context("HEAD"),
    };
    if let Some(ref parent) = parent {
        if parent.tree_id() == tree_id {
            return Ok(SnapshotWrite {
                sha: parent.id().to_string(),
                committed: false,
            });
        }
    }
    let sig = Signature::now(&wiki.author_name, &wiki.author_email).context("git signature")?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .context("git commit")?;
    Ok(SnapshotWrite {
        sha: oid.to_string(),
        committed: true,
    })
}

/// One `git log` row for a catalog path. Subjects, not Yjs undo labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub subject: String,
    pub sha: String,
}

/// M3 log is catalog v0 `spec/home.md` only. Reject `..` / other paths.
pub fn is_catalog_log_path(path: &str) -> bool {
    path == GIT_PATH
}

/// Commits that change `path`, newest first. Missing / empty repo → `[]`.
pub fn log_path(wiki: &WikiConfig, path: &str) -> Result<Vec<GitLogEntry>> {
    anyhow::ensure!(is_catalog_log_path(path), "git log path must be {GIT_PATH}");
    if !wiki.dir.join(".git").exists() {
        return Ok(Vec::new());
    }
    let repo = Repository::open(&wiki.dir)
        .with_context(|| format!("open {} for log", wiki.dir.display()))?;
    match repo.head() {
        Ok(_) => {}
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => return Err(e).context("HEAD for log"),
    }
    let mut walk = repo.revwalk().context("revwalk")?;
    walk.push_head().context("push HEAD")?;
    let mut out = Vec::new();
    for oid in walk {
        let oid = oid.context("revwalk oid")?;
        let commit = repo.find_commit(oid).context("find commit")?;
        if !commit_touches_path(&commit, path)? {
            continue;
        }
        let subject = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        out.push(GitLogEntry {
            subject,
            sha: oid.to_string(),
        });
    }
    Ok(out)
}

fn tree_blob_oid(tree: &git2::Tree, path: &str) -> Option<git2::Oid> {
    tree.get_path(Path::new(path)).ok().map(|e| e.id())
}

#[cfg(test)]
mod stamp_tests {
    use super::*;

    #[test]
    fn stamp_written_once_and_refuses_other_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = "11111111-1111-4111-a111-111111111111";
        let b = "22222222-2222-4222-a222-222222222222";
        ensure_workspace_stamp(tmp.path(), a).expect("first stamp");
        let text = fs::read_to_string(tmp.path().join(WORKSPACE_STAMP_REL)).expect("read stamp");
        assert_eq!(text.trim(), a);
        ensure_workspace_stamp(tmp.path(), a).expect("same workspace");
        let err = ensure_workspace_stamp(tmp.path(), b)
            .expect_err("other workspace")
            .to_string();
        assert!(err.contains(a), "{err}");
        assert!(err.contains("refusing"), "{err}");
    }
}

fn commit_touches_path(commit: &git2::Commit, path: &str) -> Result<bool> {
    let tree = commit.tree().context("commit tree")?;
    let current = tree_blob_oid(&tree, path);
    if commit.parent_count() == 0 {
        return Ok(current.is_some());
    }
    let parent = commit.parent(0).context("parent")?;
    let parent_tree = parent.tree().context("parent tree")?;
    let prev = tree_blob_oid(&parent_tree, path);
    Ok(current != prev)
}
