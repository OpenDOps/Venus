//! Nested wiki repo: autoinit on first snapshot, then one autocomment commit.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{ErrorCode, Repository, RepositoryInitOptions, Signature};
use serde::{Deserialize, Serialize};

use crate::convert::{Converted, SidecarBlock};
use crate::cut::GIT_PATH;
use crate::pin::PinMap;
use crate::{PAGE_DOC_ID, PAGE_DOC_UUID};

pub const DEFAULT_WIKI_DIR: &str = "wiki";
pub const DEFAULT_BRANCH: &str = "main";
pub const DEFAULT_AUTHOR_NAME: &str = "Venus";
pub const DEFAULT_AUTHOR_EMAIL: &str = "venus@localhost";
pub const SIDECAR_REL: &str = ".venus/ids/doc:home.json";

#[derive(Debug, Clone)]
pub struct WikiConfig {
    pub dir: PathBuf,
    pub author_name: String,
    pub author_email: String,
}

impl WikiConfig {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            author_name: DEFAULT_AUTHOR_NAME.into(),
            author_email: DEFAULT_AUTHOR_EMAIL.into(),
        }
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
/// Empty / no home page: no init, no commit.
pub fn commit_pins(
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
    write_home(&wiki.dir, git_path, conv, clock)?;
    for (hash, bytes) in pins.blobs() {
        write_blob(&wiki.dir, hash, bytes)?;
    }
    let message = format!("snapshot: {}", snapshot_title(&conv.markdown));
    commit_tree(wiki, git_path, &message).map(Some)
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

fn write_home(dir: &Path, git_path: &str, conv: &Converted, clock: i64) -> Result<()> {
    let md_path = dir.join(git_path);
    if let Some(parent) = md_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&md_path, conv.markdown.as_bytes())
        .with_context(|| format!("write {}", md_path.display()))?;
    let sidecar = DiskSidecar {
        doc_id: PAGE_DOC_ID,
        clock,
        blocks: &conv.sidecar.blocks,
    };
    let mut json = serde_json::to_vec_pretty(&sidecar).context("sidecar json")?;
    json.push(b'\n');
    let sidecar_path = dir.join(SIDECAR_REL);
    fs::write(&sidecar_path, json).with_context(|| format!("write {}", sidecar_path.display()))?;
    Ok(())
}

fn write_blob(dir: &Path, hash: &str, bytes: &[u8]) -> Result<()> {
    let path = dir.join("assets").join(hash);
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

fn commit_tree(wiki: &WikiConfig, git_path: &str, message: &str) -> Result<SnapshotWrite> {
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
        .add_path(Path::new(git_path))
        .with_context(|| format!("git add {git_path}"))?;
    index
        .add_path(Path::new(SIDECAR_REL))
        .context("git add sidecar")?;
    if let Ok(entries) = fs::read_dir(wiki.dir.join("assets")) {
        for entry in entries.flatten() {
            let ok = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            if !ok {
                continue;
            }
            let rel = Path::new("assets").join(entry.file_name());
            index
                .add_path(&rel)
                .with_context(|| format!("git add {}", rel.display()))?;
        }
    }
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
