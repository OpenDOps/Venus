//! Nested wiki repo: autoinit on first snapshot, then one autocomment commit.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{ErrorCode, Repository, RepositoryInitOptions, Signature};
use serde::{Deserialize, Serialize};

use crate::catalog::{CatalogPage, CatalogWalk, OldPage};
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

/// Converted pins → one commit built from HEAD, then a forced checkout.
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
    check_workspace_stamp(&wiki.dir, workspace_id)?;
    match walk {
        Some(walk) => commit_catalog_walk(wiki, workspace_id, pins, converted, walk, old_pages),
        None => commit_home_only(wiki, workspace_id, pins, converted),
    }
}

fn commit_home_only(
    wiki: &WikiConfig,
    workspace_id: &str,
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
    let repo = Repository::open(&wiki.dir).context("open wiki for commit")?;
    let mut index = index_from_head(&repo)?;
    strip_index_prefix(&mut index, ".venus/tmp/")?;
    index_add_bytes(&mut index, git_path, conv.markdown.as_bytes())?;
    index_add_bytes(
        &mut index,
        &sidecar_rel(PAGE_DOC_ID),
        &sidecar_bytes(PAGE_DOC_ID, clock, &conv.sidecar.blocks)?,
    )?;
    add_blobs_and_stamp(&mut index, pins, workspace_id)?;
    let message = format!("snapshot: {}", snapshot_title(&conv.markdown));
    commit_built_index(wiki, &repo, &mut index, &message).map(Some)
}

fn commit_catalog_walk(
    wiki: &WikiConfig,
    workspace_id: &str,
    pins: &PinMap,
    converted: &[(String, Converted)],
    walk: &CatalogWalk,
    old_pages: &HashMap<String, OldPage>,
) -> Result<Option<SnapshotWrite>> {
    anyhow::ensure!(
        walk.pages.iter().any(|p| p.doc_id == PAGE_DOC_ID),
        "catalog pin has no home doc"
    );
    let mut seen = HashSet::new();
    for page in &walk.pages {
        let key = case_fold_path(&page.git_path);
        anyhow::ensure!(seen.insert(key), "git mv would overwrite {}", page.git_path);
    }
    let converted_by_id: HashMap<&str, &Converted> = converted
        .iter()
        .map(|(id, conv)| (id.as_str(), conv))
        .collect();

    let repo = Repository::open(&wiki.dir).context("open wiki for commit")?;
    let head = head_commit(&repo)?;
    let tree = match &head {
        Some(commit) => Some(commit.tree().context("HEAD tree")?),
        None => None,
    };

    // Snapshot rename sources before any index remove so a swap still has both blobs.
    let mut snapshots: HashMap<String, Vec<u8>> = HashMap::new();
    for page in &walk.pages {
        if converted_for(&converted_by_id, page).is_some() {
            continue;
        }
        let Some(old) = old_pages.get(&page.sql_uuid) else {
            continue;
        };
        if old.git_path == page.git_path {
            continue;
        }
        let Some(tree) = tree.as_ref() else {
            continue;
        };
        if let Some(bytes) = read_tree_bytes_casefold(&repo, tree, &old.git_path)? {
            snapshots.insert(page.sql_uuid.clone(), bytes);
        }
    }

    let existing_links = match tree.as_ref() {
        Some(tree) => read_tree_bytes(&repo, tree, crate::links::LINKS_JSON_REL)?,
        None => None,
    };
    let (head_markdown, rebuild_paths) = if existing_links.is_none() {
        let mut markdown = Vec::new();
        if let Some(tree) = tree.as_ref() {
            collect_head_markdown(&repo, tree, "", &mut markdown)?;
        }
        let paths = old_pages
            .values()
            .map(|page| (page.git_path.clone(), page.doc_id.clone()))
            .collect();
        (markdown, paths)
    } else {
        (Vec::new(), HashMap::new())
    };
    let markdown_refs: Vec<(&str, &str)> = head_markdown
        .iter()
        .map(|(path, markdown)| (path.as_str(), markdown.as_str()))
        .collect();
    let links = crate::links::compose_flush_bytes(
        existing_links.as_deref(),
        &markdown_refs,
        &rebuild_paths,
        walk,
        converted,
        old_pages,
    )?;
    let pages_yaml = crate::catalog::pages_yaml(walk);

    let live: HashSet<&str> = walk
        .pages
        .iter()
        .map(|page| page.sql_uuid.as_str())
        .collect();
    let live_finals: HashSet<String> = walk
        .pages
        .iter()
        .map(|page| case_fold_path(&page.git_path))
        .collect();
    let mut will_write: HashSet<String> = HashSet::new();
    for page in &walk.pages {
        if converted_for(&converted_by_id, page).is_some() || snapshots.contains_key(&page.sql_uuid)
        {
            will_write.insert(case_fold_path(&page.git_path));
        }
    }

    let mut index = index_from_head(&repo)?;
    strip_index_prefix(&mut index, ".venus/tmp/")?;

    for (sql_uuid, old) in old_pages {
        if live.contains(sql_uuid.as_str()) {
            continue;
        }
        index_remove(&mut index, &sidecar_rel(&old.doc_id))?;
        let folded = case_fold_path(&old.git_path);
        let kept = live_finals.contains(&folded) && !will_write.contains(&folded);
        if !kept {
            index_remove(&mut index, &old.git_path)?;
        }
    }
    for page in &walk.pages {
        let Some(old) = old_pages.get(&page.sql_uuid) else {
            continue;
        };
        if old.git_path == page.git_path {
            continue;
        }
        let folded = case_fold_path(&old.git_path);
        let kept = live_finals.contains(&folded) && !will_write.contains(&folded);
        if !kept {
            index_remove(&mut index, &old.git_path)?;
        }
    }

    for page in &walk.pages {
        let Some(bytes) = snapshots.get(&page.sql_uuid) else {
            continue;
        };
        index_add_bytes(&mut index, &page.git_path, bytes)?;
    }
    for page in &walk.pages {
        let Some(conv) = converted_for(&converted_by_id, page) else {
            continue;
        };
        let clock = pins
            .get(&page.sql_uuid)
            .or_else(|| pins.get(&page.doc_id))
            .map(|entry| entry.clock)
            .context("pin clock missing for converted page")?;
        index_add_bytes(&mut index, &page.git_path, conv.markdown.as_bytes())?;
        index_add_bytes(
            &mut index,
            &sidecar_rel(&page.doc_id),
            &sidecar_bytes(&page.doc_id, clock, &conv.sidecar.blocks)?,
        )?;
    }
    index_add_bytes(&mut index, PAGES_YAML_REL, pages_yaml.as_bytes())?;
    index_add_bytes(&mut index, crate::links::LINKS_JSON_REL, &links)?;
    add_blobs_and_stamp(&mut index, pins, workspace_id)?;

    let message = autocomment(converted);
    commit_built_index(wiki, &repo, &mut index, &message).map(Some)
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

fn case_fold_path(path: &str) -> String {
    path.to_lowercase()
}

fn converted_for<'a>(
    converted: &HashMap<&str, &'a Converted>,
    page: &CatalogPage,
) -> Option<&'a Converted> {
    converted
        .get(page.sql_uuid.as_str())
        .copied()
        .or_else(|| converted.get(page.doc_id.as_str()).copied())
}

fn sidecar_bytes(
    doc_id: &str,
    clock: i64,
    blocks: &[crate::convert::SidecarBlock],
) -> Result<Vec<u8>> {
    let sidecar = DiskSidecar {
        doc_id,
        clock,
        blocks,
    };
    let mut json = serde_json::to_vec_pretty(&sidecar).context("sidecar json")?;
    json.push(b'\n');
    Ok(json)
}

fn blank_entry(path: &str) -> git2::IndexEntry {
    git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: git2::Oid::zero(),
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

fn index_add_bytes(index: &mut git2::Index, path: &str, data: &[u8]) -> Result<()> {
    let entry = blank_entry(path);
    index
        .add_frombuffer(&entry, data)
        .with_context(|| format!("index add {path}"))
}

fn index_remove(index: &mut git2::Index, path: &str) -> Result<()> {
    match index.remove_path(Path::new(path)) {
        Ok(()) => Ok(()),
        Err(err) if err.code() == ErrorCode::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("index remove {path}")),
    }
}

fn index_paths_with_prefix(index: &git2::Index, prefix: &str) -> Vec<String> {
    index
        .iter()
        .filter_map(|entry| {
            let path = String::from_utf8(entry.path).ok()?;
            path.starts_with(prefix).then_some(path)
        })
        .collect()
}

fn strip_index_prefix(index: &mut git2::Index, prefix: &str) -> Result<()> {
    let paths = index_paths_with_prefix(index, prefix);
    for path in paths {
        index_remove(index, &path)?;
    }
    Ok(())
}

fn add_blobs_and_stamp(index: &mut git2::Index, pins: &PinMap, workspace_id: &str) -> Result<()> {
    for (hash, bytes) in pins.blobs() {
        index_add_bytes(index, &format!("assets/{hash}"), bytes)?;
    }
    index_add_bytes(
        index,
        WORKSPACE_STAMP_REL,
        format!("{workspace_id}\n").as_bytes(),
    )
}

fn index_from_head(repo: &Repository) -> Result<git2::Index> {
    let mut index = repo.index().context("index")?;
    match head_commit(repo)? {
        Some(commit) => {
            let tree = commit.tree().context("HEAD tree")?;
            index.read_tree(&tree).context("read HEAD tree")?;
        }
        None => index.clear().context("clear index")?,
    }
    Ok(index)
}

fn head_commit(repo: &Repository) -> Result<Option<git2::Commit<'_>>> {
    match repo.head() {
        Ok(head) => Ok(Some(head.peel_to_commit().context("peel HEAD")?)),
        Err(err) if err.code() == ErrorCode::UnbornBranch || err.code() == ErrorCode::NotFound => {
            Ok(None)
        }
        Err(err) => Err(err).context("HEAD"),
    }
}

/// `.venus/pages.yaml` from HEAD. Missing repo, unborn HEAD, or a missing blob is `None`.
pub fn head_pages_yaml(dir: &Path) -> Result<Option<String>> {
    with_head_tree(dir, None, |repo, tree| {
        let Some(bytes) = read_tree_bytes(repo, tree, PAGES_YAML_REL)? else {
            return Ok(None);
        };
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    })
}

/// `.venus/links.json` bytes from HEAD. Missing repo, unborn HEAD, or a missing
/// blob is `Ok(None)`. Opening the repository or reading the blob is an error.
pub fn head_links_blob(dir: &Path) -> Result<Option<Vec<u8>>> {
    with_head_tree(dir, None, |repo, tree| {
        read_tree_bytes(repo, tree, LINKS_JSON_REL)
    })
}

/// Markdown blobs in HEAD, path → text. Skips `.venus`. Missing repo or unborn
/// HEAD is an empty list. A read error is returned.
pub fn head_markdown_files(dir: &Path) -> Result<Vec<(String, String)>> {
    with_head_tree(dir, Vec::new(), |repo, tree| {
        let mut out = Vec::new();
        collect_head_markdown(repo, tree, "", &mut out)?;
        Ok(out)
    })
}

fn with_head_tree<T>(
    dir: &Path,
    absent: T,
    f: impl FnOnce(&Repository, &git2::Tree) -> Result<T>,
) -> Result<T> {
    if !dir.join(".git").exists() {
        return Ok(absent);
    }
    let repo = Repository::open(dir).with_context(|| format!("open {}", dir.display()))?;
    let Some(commit) = head_commit(&repo)? else {
        return Ok(absent);
    };
    let tree = commit.tree().context("HEAD tree")?;
    f(&repo, &tree)
}

fn read_tree_bytes(repo: &Repository, tree: &git2::Tree, path: &str) -> Result<Option<Vec<u8>>> {
    let Some(oid) = tree_blob_oid(tree, path) else {
        return Ok(None);
    };
    let blob = repo
        .find_blob(oid)
        .with_context(|| format!("blob {path}"))?;
    Ok(Some(blob.content().to_vec()))
}

fn read_tree_bytes_casefold(
    repo: &Repository,
    tree: &git2::Tree,
    path: &str,
) -> Result<Option<Vec<u8>>> {
    if let Some(bytes) = read_tree_bytes(repo, tree, path)? {
        return Ok(Some(bytes));
    }
    let want = case_fold_path(path);
    let Some(oid) = find_oid_casefold(repo, tree, "", &want)? else {
        return Ok(None);
    };
    let blob = repo
        .find_blob(oid)
        .with_context(|| format!("blob {path}"))?;
    Ok(Some(blob.content().to_vec()))
}

fn find_oid_casefold(
    repo: &Repository,
    tree: &git2::Tree,
    prefix: &str,
    want: &str,
) -> Result<Option<git2::Oid>> {
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let sub = repo.find_tree(entry.id()).context("subtree")?;
                if let Some(oid) = find_oid_casefold(repo, &sub, &path, want)? {
                    return Ok(Some(oid));
                }
            }
            Some(git2::ObjectType::Blob) if case_fold_path(&path) == want => {
                return Ok(Some(entry.id()));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn collect_head_markdown(
    repo: &Repository,
    tree: &git2::Tree,
    prefix: &str,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                if path == ".venus" || path.starts_with(".venus/") {
                    continue;
                }
                let sub = repo.find_tree(entry.id()).context("subtree")?;
                collect_head_markdown(repo, &sub, &path, out)?;
            }
            Some(git2::ObjectType::Blob) if name.ends_with(".md") => {
                let blob = repo.find_blob(entry.id()).context("markdown blob")?;
                out.push((path, String::from_utf8_lossy(blob.content()).into_owned()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn commit_built_index(
    wiki: &WikiConfig,
    repo: &Repository,
    index: &mut git2::Index,
    message: &str,
) -> Result<SnapshotWrite> {
    anyhow::ensure!(
        repo.remotes()
            .context("remotes")?
            .iter()
            .flatten()
            .next()
            .is_none(),
        "wiki repo must have no origin in M3"
    );
    let tree_id = index.write_tree().context("write_tree")?;
    let tree = repo.find_tree(tree_id).context("find tree")?;
    let parent = head_commit(repo)?;
    if let Some(ref parent) = parent {
        if parent.tree_id() == tree_id {
            checkout_head_force(repo)?;
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
    checkout_head_force(repo)?;
    Ok(SnapshotWrite {
        sha: oid.to_string(),
        committed: true,
    })
}

fn checkout_head_force(repo: &Repository) -> Result<()> {
    // The in-memory index used to build the tree already dropped removed
    // paths. Reload the on-disk index so checkout's baseline still lists them
    // and the working tree loses those files.
    {
        let mut index = repo.index().context("index")?;
        index.read(true).context("reload index from disk")?;
    }
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.force();
    repo.checkout_head(Some(&mut opts))
        .context("checkout HEAD")?;
    // Checkout skips a workdir file that already equals HEAD and leaves it out
    // of the index, so `git status` would show it deleted and untracked.
    let tree = repo
        .head()
        .context("HEAD")?
        .peel_to_tree()
        .context("HEAD tree")?;
    let mut index = repo.index().context("index")?;
    index.read_tree(&tree).context("index from HEAD")?;
    index.write().context("write index")
}

/// Refuse a wiki whose `.venus/workspace` names another workspace, so two
/// workspaces cannot share one directory. The first Flush commits the stamp
/// (`add_blobs_and_stamp`) and checkout writes the file.
pub fn check_workspace_stamp(dir: &Path, workspace_id: &str) -> Result<()> {
    let path = dir.join(WORKSPACE_STAMP_REL);
    if !path.exists() {
        return Ok(());
    }
    let existing = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let existing = existing.trim();
    anyhow::ensure!(
        existing.eq_ignore_ascii_case(workspace_id),
        "wiki is bound to {existing}, refusing workspace {workspace_id}"
    );
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

/// Default `GET /git/log` cap. The walk stops once this many commits touch the path.
pub const GIT_LOG_DEFAULT_LIMIT: usize = 50;

/// OID stored in a loose ref. `None` when HEAD is unborn or the ref is packed.
fn loose_head_oid(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let oid = if let Some(rel) = head.strip_prefix("ref: ") {
        fs::read_to_string(git_dir.join(rel.trim())).ok()?
    } else {
        head.to_string()
    };
    let oid = oid.trim();
    if oid.is_empty() {
        None
    } else {
        Some(oid.to_string())
    }
}

/// HEAD commit id, if the repo has one. Missing repo or unborn HEAD is `None`.
/// A loose ref is read from disk so the log cache can key on HEAD without walking.
pub fn head_oid(wiki: &WikiConfig) -> Result<Option<String>> {
    let git_dir = wiki.dir.join(".git");
    if !git_dir.exists() {
        return Ok(None);
    }
    if let Some(oid) = loose_head_oid(&git_dir) {
        return Ok(Some(oid));
    }
    let repo = Repository::open(&wiki.dir)
        .with_context(|| format!("open {} for log", wiki.dir.display()))?;
    let oid = match repo.head() {
        Ok(head) => head.target().map(|oid| oid.to_string()),
        Err(e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => None,
        Err(e) => return Err(e).context("HEAD for log"),
    };
    Ok(oid)
}

/// Commits that change `path`, newest first, at most `limit`. Missing / empty repo → `[]`.
pub fn log_path(wiki: &WikiConfig, path: &str, limit: usize) -> Result<Vec<GitLogEntry>> {
    anyhow::ensure!(is_catalog_log_path(path), "git log path must be {GIT_PATH}");
    if limit == 0 {
        return Ok(Vec::new());
    }
    if head_oid(wiki)?.is_none() {
        return Ok(Vec::new());
    }
    let repo = Repository::open(&wiki.dir)
        .with_context(|| format!("open {} for log", wiki.dir.display()))?;
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
        if out.len() == limit {
            break;
        }
    }
    Ok(out)
}

fn tree_blob_oid(tree: &git2::Tree, path: &str) -> Option<git2::Oid> {
    tree.get_path(Path::new(path)).ok().map(|e| e.id())
}

#[cfg(test)]
mod apply_tests {
    use std::collections::HashMap;

    use super::*;
    use crate::catalog::{disambiguate_published_paths, CatalogFolder, CatalogPage};
    use crate::convert::{Converted, Sidecar};
    use crate::pin::{PinEntry, PinMap};

    const WS: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";
    const LOW: &str = "11111111-1111-4111-8111-111111111111";
    const HIGH: &str = "22222222-2222-4222-8222-222222222222";

    fn page(sql: &str, name: &str, git_path: &str) -> CatalogPage {
        CatalogPage {
            sql_uuid: sql.into(),
            doc_id: sql.into(),
            name: name.into(),
            git_path: git_path.into(),
        }
    }

    fn home() -> CatalogPage {
        CatalogPage {
            sql_uuid: PAGE_DOC_UUID.into(),
            doc_id: PAGE_DOC_ID.into(),
            name: "home".into(),
            git_path: "spec/home.md".into(),
        }
    }

    fn walk(mut pages: Vec<CatalogPage>) -> CatalogWalk {
        pages.insert(0, home());
        CatalogWalk {
            pages,
            folders: vec![CatalogFolder {
                id: "folder:spec".into(),
                name: "spec".into(),
                git_path: "spec".into(),
            }],
        }
    }

    fn identity(walk: &CatalogWalk) -> HashMap<String, OldPage> {
        walk.pages
            .iter()
            .map(|page| {
                (
                    page.sql_uuid.clone(),
                    OldPage {
                        sql_uuid: page.sql_uuid.clone(),
                        doc_id: page.doc_id.clone(),
                        git_path: page.git_path.clone(),
                        name: page.name.clone(),
                    },
                )
            })
            .collect()
    }

    fn markdown(doc_id: &str, body: &str) -> Converted {
        Converted {
            markdown: format!("# {body}\n\n{body}\n"),
            sidecar: Sidecar {
                doc_id: doc_id.into(),
                clock: "1".into(),
                blocks: vec![],
            },
        }
    }

    fn flush(
        dir: &std::path::Path,
        walk: &CatalogWalk,
        old: &HashMap<String, OldPage>,
        bodies: &[(&str, &str)],
    ) -> Result<()> {
        let mut pins = PinMap::new();
        let mut converted = Vec::new();
        for page in &walk.pages {
            let body = bodies
                .iter()
                .find(|(id, _)| *id == page.sql_uuid || *id == page.doc_id)
                .map(|(_, body)| *body)
                .unwrap_or("home");
            let id = page.sql_uuid.clone();
            pins.insert(
                id.clone(),
                PinEntry {
                    bytes: vec![1],
                    clock: 1,
                },
            );
            converted.push((id, markdown(&page.doc_id, body)));
        }
        let wiki = WikiConfig::new(dir);
        commit_pins(&wiki, WS, &pins, &converted, Some(walk), old)?;
        Ok(())
    }

    fn read_md(dir: &std::path::Path, rel: &str) -> String {
        fs::read_to_string(dir.join(rel)).unwrap_or_default()
    }

    fn status_lines(dir: &std::path::Path) -> Vec<String> {
        let repo = Repository::open(dir).expect("open");
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut opts)).expect("statuses");
        statuses
            .iter()
            .map(|s| format!("{:?} {}", s.status(), s.path().unwrap_or("")))
            .collect()
    }

    #[test]
    fn flush_leaves_git_status_clean_and_heals_a_missing_index_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![page(LOW, "notes", "spec/notes.md")]);
        flush(tmp.path(), &first, &HashMap::new(), &[(LOW, "PAGE-P")]).expect("first");
        assert_eq!(status_lines(tmp.path()), Vec::<String>::new());
        let stamp = fs::read_to_string(tmp.path().join(WORKSPACE_STAMP_REL)).expect("stamp");
        assert_eq!(stamp.trim(), WS);

        // A wiki whose stamp is committed and on disk but not in the index.
        let repo = Repository::open(tmp.path()).expect("open");
        let mut index = repo.index().expect("index");
        index
            .remove_path(Path::new(WORKSPACE_STAMP_REL))
            .expect("remove");
        index.write().expect("write");
        let broken = status_lines(tmp.path());
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].contains("INDEX_DELETED") && broken[0].contains("WT_NEW"));

        flush(tmp.path(), &first, &identity(&first), &[(LOW, "PAGE-Q")]).expect("second");
        assert_eq!(status_lines(tmp.path()), Vec::<String>::new());
        assert!(read_md(tmp.path(), "spec/notes.md").contains("PAGE-Q"));
    }

    #[test]
    fn delete_then_reuse_name_flushes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![
            page(LOW, "notes", "spec/notes.md"),
            page(HIGH, "q", "spec/q.md"),
        ]);
        flush(
            tmp.path(),
            &first,
            &HashMap::new(),
            &[(LOW, "PAGE-P"), (HIGH, "PAGE-Q")],
        )
        .expect("first flush");
        let second = walk(vec![page(HIGH, "notes", "spec/notes.md")]);
        flush(tmp.path(), &second, &identity(&first), &[(HIGH, "PAGE-Q")]).expect("reuse flush");
        assert!(read_md(tmp.path(), "spec/notes.md").contains("PAGE-Q"));
        assert!(!tmp.path().join("spec/q.md").exists());
        assert!(!read_md(tmp.path(), "spec/notes.md").contains("PAGE-P"));
    }

    #[test]
    fn swap_names_flushes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![
            page(LOW, "a", "spec/a.md"),
            page(HIGH, "b", "spec/b.md"),
        ]);
        flush(
            tmp.path(),
            &first,
            &HashMap::new(),
            &[(LOW, "BODY-A"), (HIGH, "BODY-B")],
        )
        .expect("first flush");
        let second = walk(vec![
            page(LOW, "b", "spec/b.md"),
            page(HIGH, "a", "spec/a.md"),
        ]);
        flush(
            tmp.path(),
            &second,
            &identity(&first),
            &[(LOW, "BODY-A2"), (HIGH, "BODY-B2")],
        )
        .expect("swap flush");
        assert!(read_md(tmp.path(), "spec/a.md").contains("BODY-B2"));
        assert!(read_md(tmp.path(), "spec/b.md").contains("BODY-A2"));
    }

    #[test]
    fn same_name_publishes_each_page_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut pages = vec![
            home(),
            page(LOW, "notes", "spec/notes.md"),
            page(HIGH, "notes", "spec/Notes.md"),
        ];
        disambiguate_published_paths(&mut pages);
        let published = CatalogWalk {
            pages,
            folders: vec![CatalogFolder {
                id: "folder:spec".into(),
                name: "spec".into(),
                git_path: "spec".into(),
            }],
        };
        flush(
            tmp.path(),
            &published,
            &HashMap::new(),
            &[(LOW, "LOW-BODY"), (HIGH, "HIGH-BODY")],
        )
        .expect("collision flush");
        assert!(read_md(tmp.path(), "spec/notes.md").contains("LOW-BODY"));
        assert!(read_md(tmp.path(), "spec/Notes-22222222.md").contains("HIGH-BODY"));
        let md_count = fs::read_dir(tmp.path().join("spec"))
            .expect("spec")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .count();
        assert_eq!(md_count, 3, "home, winner, and the suffixed page");
    }

    #[test]
    fn stray_and_dirty_workdir_are_not_committed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![page(LOW, "notes", "spec/notes.md")]);
        flush(tmp.path(), &first, &HashMap::new(), &[(LOW, "PAGE-P")]).expect("first");
        fs::write(tmp.path().join("stray.txt"), b"stray").expect("stray");
        fs::write(tmp.path().join("spec/notes.md"), b"DIRTY").expect("dirty");
        flush(tmp.path(), &first, &identity(&first), &[(LOW, "PAGE-Q")]).expect("second");
        let notes = read_md(tmp.path(), "spec/notes.md");
        assert!(notes.contains("PAGE-Q"), "{notes}");
        assert!(!notes.contains("DIRTY"));
        assert_eq!(fs::read(tmp.path().join("stray.txt")).unwrap(), b"stray");
        let repo = Repository::open(tmp.path()).unwrap();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = commit.tree().unwrap();
        assert!(tree.get_path(std::path::Path::new("stray.txt")).is_err());
        let oid = tree
            .get_path(std::path::Path::new("spec/notes.md"))
            .unwrap()
            .id();
        let blob = repo.find_blob(oid).unwrap();
        let published = String::from_utf8_lossy(blob.content());
        assert!(published.contains("PAGE-Q"), "{published}");
        assert!(!published.contains("DIRTY"));
    }

    #[test]
    fn matching_tree_checks_out_a_deleted_workdir_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pages = walk(vec![page(LOW, "notes", "spec/notes.md")]);
        flush(tmp.path(), &pages, &HashMap::new(), &[(LOW, "PAGE-P")]).expect("first");
        fs::remove_file(tmp.path().join("spec/notes.md")).expect("delete workdir");
        let mut pins = PinMap::new();
        let mut converted = Vec::new();
        for page in &pages.pages {
            let body = if page.sql_uuid == LOW {
                "PAGE-P"
            } else {
                "home"
            };
            pins.insert(
                page.sql_uuid.clone(),
                PinEntry {
                    bytes: vec![1],
                    clock: 1,
                },
            );
            converted.push((page.sql_uuid.clone(), markdown(&page.doc_id, body)));
        }
        let wrote = commit_pins(
            &WikiConfig::new(tmp.path()),
            WS,
            &pins,
            &converted,
            Some(&pages),
            &identity(&pages),
        )
        .expect("second")
        .expect("snapshot");
        assert!(!wrote.committed);
        assert!(read_md(tmp.path(), "spec/notes.md").contains("PAGE-P"));
    }

    #[test]
    fn unconverted_rename_copies_the_head_blob() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![page(LOW, "a", "spec/a.md")]);
        flush(tmp.path(), &first, &HashMap::new(), &[(LOW, "BODY-A")]).expect("first");
        let second = walk(vec![page(LOW, "c", "spec/c.md")]);
        let mut pins = PinMap::new();
        pins.insert(
            PAGE_DOC_UUID.into(),
            PinEntry {
                bytes: vec![1],
                clock: 1,
            },
        );
        let converted = vec![(PAGE_DOC_UUID.to_string(), markdown(PAGE_DOC_ID, "home"))];
        commit_pins(
            &WikiConfig::new(tmp.path()),
            WS,
            &pins,
            &converted,
            Some(&second),
            &identity(&first),
        )
        .expect("rename");
        assert!(read_md(tmp.path(), "spec/c.md").contains("BODY-A"));
        assert!(!tmp.path().join("spec/a.md").exists());
    }

    #[test]
    fn duplicate_published_path_does_not_change_the_workdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = walk(vec![page(LOW, "notes", "spec/notes.md")]);
        flush(tmp.path(), &first, &HashMap::new(), &[(LOW, "PAGE-P")]).expect("first");
        let second = walk(vec![
            page(LOW, "notes", "spec/notes.md"),
            page(HIGH, "notes", "spec/Notes.md"),
        ]);
        let err = flush(tmp.path(), &second, &identity(&first), &[(LOW, "PAGE-Q")])
            .expect_err("collision");
        assert!(err.to_string().contains("would overwrite"), "{err}");
        let notes = read_md(tmp.path(), "spec/notes.md");
        assert!(notes.contains("PAGE-P"), "{notes}");
        assert!(!notes.contains("PAGE-Q"));
    }

    #[test]
    fn commit_does_not_stage_the_workdir() {
        let src = include_str!("git.rs");
        assert!(!src.contains(&["add", "_all"].concat()));
        assert!(!src.contains(&["git add", " -A"].concat()));
        assert!(src.contains("checkout_head"));
        assert!(src.contains("read_tree"));
    }
}

#[cfg(test)]
mod stamp_tests {
    use super::*;

    #[test]
    fn stamp_check_refuses_other_workspace_and_writes_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let a = "11111111-1111-4111-a111-111111111111";
        let b = "22222222-2222-4222-a222-222222222222";
        check_workspace_stamp(tmp.path(), a).expect("unstamped wiki");
        assert!(!tmp.path().join(WORKSPACE_STAMP_REL).exists());
        fs::create_dir_all(tmp.path().join(".venus")).expect("mkdir");
        fs::write(tmp.path().join(WORKSPACE_STAMP_REL), format!("{a}\n")).expect("stamp");
        check_workspace_stamp(tmp.path(), a).expect("same workspace");
        let err = check_workspace_stamp(tmp.path(), b)
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
