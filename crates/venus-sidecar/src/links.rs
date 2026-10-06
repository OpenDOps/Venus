//! Flush reverse index: `.venus/links.json` inbound/outbound catalog `docId`.
//! Derived from `<!-- venus:doc:… -->`. Not live SoT. Not a catalog node field.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::catalog::{sidecar_doc_id, CatalogWalk, OldPage};
use crate::convert::Converted;

pub const LINKS_JSON_REL: &str = ".venus/links.json";
pub const PAGES_YAML_REL: &str = ".venus/pages.yaml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkIndex {
    #[serde(default)]
    pub inbound: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub outbound: BTreeMap<String, Vec<String>>,
}

pub fn empty_index() -> LinkIndex {
    LinkIndex::default()
}

pub fn is_safe_page_id(page_id: &str) -> bool {
    !page_id.is_empty()
        && page_id.len() <= 128
        && page_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}

fn unique_sorted(ids: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = ids.into_iter().filter(|id| is_safe_page_id(id)).collect();
    out.sort();
    out.dedup();
    out
}

/// Targets named by `<!-- venus:doc:<id> -->` only. Ordinary markdown URLs
/// without that comment are ignored.
pub fn targets_in_markdown(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut search = 0;
    while let Some(rel) = markdown[search..].find("<!--") {
        let open = search + rel;
        let inner_start = open + 4;
        let Some(rel_close) = markdown[inner_start..].find("-->") else {
            break;
        };
        let inner = markdown[inner_start..inner_start + rel_close].trim();
        if let Some(rest) = inner.strip_prefix("venus:doc:") {
            let id = rest.trim();
            if is_safe_page_id(id) {
                found.push(id.to_string());
            }
        }
        search = inner_start + rel_close + 3;
    }
    unique_sorted(found)
}

/// O(1) map lookup. Does not read the wiki.
pub fn inbound<'a>(index: &'a LinkIndex, target_doc_id: &str) -> &'a [String] {
    index
        .inbound
        .get(target_doc_id)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// O(1) map lookup. Does not read the wiki.
pub fn outbound<'a>(index: &'a LinkIndex, source_doc_id: &str) -> &'a [String] {
    index
        .outbound
        .get(source_doc_id)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn remove_source_from_inbound(index: &mut LinkIndex, source_id: &str) {
    let mut empty = Vec::new();
    for (target, sources) in index.inbound.iter_mut() {
        sources.retain(|id| id != source_id);
        if sources.is_empty() {
            empty.push(target.clone());
        }
    }
    for target in empty {
        index.inbound.remove(&target);
    }
}

fn add_inbound(index: &mut LinkIndex, target_id: &str, source_id: &str) {
    let sources = index.inbound.entry(target_id.to_string()).or_default();
    if !sources.iter().any(|id| id == source_id) {
        sources.push(source_id.to_string());
        sources.sort();
        sources.dedup();
    }
}

/// Replace this source’s outbound row and rebuild inbound for those targets
/// (drop stale edges from this source).
pub fn upsert_outbound(index: &mut LinkIndex, source_id: &str, targets: &[String]) {
    if !is_safe_page_id(source_id) {
        return;
    }
    remove_source_from_inbound(index, source_id);
    let next = unique_sorted(targets.iter().filter(|t| t.as_str() != source_id).cloned());
    if next.is_empty() {
        index.outbound.remove(source_id);
    } else {
        for target in &next {
            add_inbound(index, target, source_id);
        }
        index.outbound.insert(source_id.to_string(), next);
    }
}

/// Drop an id from both maps (leaf delete / git rm).
pub fn drop_id(index: &mut LinkIndex, doc_id: &str) {
    index.outbound.remove(doc_id);
    index.inbound.remove(doc_id);
    remove_source_from_inbound(index, doc_id);
    let mut empty = Vec::new();
    for (source, targets) in index.outbound.iter_mut() {
        targets.retain(|id| id != doc_id);
        if targets.is_empty() {
            empty.push(source.clone());
        }
    }
    for source in empty {
        index.outbound.remove(&source);
    }
}

pub fn serialize(index: &LinkIndex) -> Result<Vec<u8>> {
    let mut v = serde_json::to_vec_pretty(index).context("serialize links.json")?;
    if !v.ends_with(b"\n") {
        v.push(b'\n');
    }
    Ok(v)
}

pub fn parse(bytes: &[u8]) -> Option<LinkIndex> {
    let raw: LinkIndex = serde_json::from_slice(bytes).ok()?;
    let mut index = empty_index();
    for (source, targets) in raw.outbound {
        if !is_safe_page_id(&source) {
            continue;
        }
        upsert_outbound(&mut index, &source, &targets);
    }
    Some(index)
}

pub fn load(dir: &Path) -> Option<LinkIndex> {
    let bytes = fs::read(dir.join(LINKS_JSON_REL)).ok()?;
    parse(&bytes)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        s.to_string()
    }
}

/// `pages.yaml` path → docId (`pages:` only).
pub fn path_to_doc_id_from_pages_yaml(yaml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut in_pages = false;
    let mut path: Option<String> = None;
    for line in yaml.lines() {
        let trimmed = line.trim_end();
        if trimmed == "pages:" {
            in_pages = true;
            path = None;
            continue;
        }
        if !line.starts_with(' ') && trimmed.ends_with(':') {
            in_pages = trimmed == "pages:";
            path = None;
            continue;
        }
        if !in_pages {
            continue;
        }
        if let Some(rest) = line.strip_prefix("  ") {
            if !rest.starts_with(' ') && rest.trim_end().ends_with(':') {
                let key = rest.trim_end().trim_end_matches(':');
                path = Some(unquote(key));
                continue;
            }
        }
        if let Some(rest) = line.trim_start().strip_prefix("docId:") {
            if let Some(p) = &path {
                let id = unquote(rest);
                if is_safe_page_id(&id) {
                    map.insert(p.clone(), id);
                }
            }
        }
    }
    map
}

fn rel_posix(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

fn collect_md(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if dir == root && e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| format!("read_dir {}", dir.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("read_dir {}", dir.display()))?;
        let name = entry.file_name();
        if name == ".git" || name == ".venus" {
            continue;
        }
        let path = entry.path();
        let ft = entry
            .file_type()
            .with_context(|| format!("file_type {}", path.display()))?;
        if ft.is_dir() {
            collect_md(&path, root, out)?;
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.ends_with(".md") {
            continue;
        }
        let Some(rel) = rel_posix(root, &path) else {
            continue;
        };
        let markdown =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        out.push((rel, markdown));
    }
    Ok(())
}

fn load_path_map(
    dir: &Path,
    path_to_doc: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    if !path_to_doc.is_empty() {
        return Ok(path_to_doc.clone());
    }
    let yaml_path = dir.join(PAGES_YAML_REL);
    let yaml = match fs::read_to_string(&yaml_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", yaml_path.display())),
    };
    Ok(path_to_doc_id_from_pages_yaml(&yaml))
}

/// Rebuild from working-tree markdown + path→docId. Does not read pin bytes.
pub fn rebuild_from_wiki(dir: &Path, path_to_doc: &HashMap<String, String>) -> Result<LinkIndex> {
    let map = load_path_map(dir, path_to_doc)?;
    let mut files = Vec::new();
    collect_md(dir, dir, &mut files)?;
    let mut index = empty_index();
    for (rel, markdown) in files {
        let Some(id) = map.get(&rel) else {
            continue;
        };
        let targets = targets_in_markdown(&markdown);
        upsert_outbound(&mut index, id, &targets);
    }
    Ok(index)
}

pub fn posix_dirname(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((d, _)) => d,
        None => "",
    }
}

/// POSIX relative from the exporting file to the target file.
pub fn posix_relative(from_file: &str, to_file: &str) -> String {
    let from_dir = posix_dirname(from_file);
    let from: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    let to: Vec<&str> = to_file.split('/').filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    while i < from.len() && i < to.len() && from[i] == to[i] {
        i += 1;
    }
    let mut rel: Vec<&str> = (0..from.len().saturating_sub(i)).map(|_| "..").collect();
    rel.extend_from_slice(&to[i..]);
    if rel.is_empty() {
        to.last().copied().unwrap_or(to_file).to_string()
    } else {
        rel.join("/")
    }
}

/// Convert set SQL uuids: body-dirty ∪ inbound[path- or name-changed] ∪
/// dirname-changed with outbound. Catalog is never converted.
pub fn convert_set(
    walk: &CatalogWalk,
    old_pages: &HashMap<String, OldPage>,
    body_dirty_sql: &HashSet<String>,
    index: &LinkIndex,
) -> HashSet<String> {
    let mut need: HashSet<String> = body_dirty_sql
        .iter()
        .filter(|id| *id != crate::CATALOG_DOC_ID)
        .cloned()
        .collect();
    let doc_to_sql: HashMap<&str, &str> = walk
        .pages
        .iter()
        .map(|p| (p.doc_id.as_str(), p.sql_uuid.as_str()))
        .collect();
    for page in &walk.pages {
        let old = old_pages.get(&page.sql_uuid);
        let path_changed = old.map(|o| o.git_path != page.git_path).unwrap_or(false);
        let name_changed = old.map(|o| o.name != page.name).unwrap_or(false);
        let dirname_changed = old
            .map(|o| posix_dirname(&o.git_path) != posix_dirname(&page.git_path))
            .unwrap_or(false);
        if path_changed || name_changed {
            for src in inbound(index, &page.doc_id) {
                if let Some(sql) = doc_to_sql.get(src.as_str()) {
                    need.insert((*sql).to_string());
                }
            }
        }
        if dirname_changed && !outbound(index, &page.doc_id).is_empty() {
            need.insert(page.sql_uuid.clone());
        }
    }
    need
}

pub fn write(dir: &Path, index: &LinkIndex) -> Result<()> {
    let path = dir.join(LINKS_JSON_REL);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    fs::write(&path, serialize(index)?).with_context(|| format!("write {}", path.display()))
}

/// Load existing JSON, or rebuild if missing/corrupt. Then upsert converted
/// sources and drop deleted ids. Lookup after this is map get, not a rescan.
pub fn persist_links_json(
    dir: &Path,
    path_to_doc: &HashMap<String, String>,
    converted_sources: &[(&str, &str)],
    deleted_doc_ids: &[&str],
) -> Result<()> {
    let mut index = match load(dir) {
        Some(idx) => idx,
        None => rebuild_from_wiki(dir, path_to_doc)?,
    };
    for (source_id, markdown) in converted_sources {
        let targets = targets_in_markdown(markdown);
        upsert_outbound(&mut index, source_id, &targets);
    }
    for id in deleted_doc_ids {
        drop_id(&mut index, id);
    }
    write(dir, &index)
}

pub fn persist_on_flush(
    dir: &Path,
    walk: &CatalogWalk,
    converted: &[(String, Converted)],
    old_pages: &HashMap<String, OldPage>,
) -> Result<()> {
    let path_to_doc: HashMap<String, String> = walk
        .pages
        .iter()
        .map(|p| (p.git_path.clone(), p.doc_id.clone()))
        .collect();
    let live: HashSet<&str> = walk.pages.iter().map(|p| p.sql_uuid.as_str()).collect();
    let sql_to_doc: HashMap<&str, &str> = walk
        .pages
        .iter()
        .map(|p| (p.sql_uuid.as_str(), p.doc_id.as_str()))
        .collect();
    let live_doc: HashSet<&str> = walk.pages.iter().map(|p| p.doc_id.as_str()).collect();

    let converted_owned: Vec<(String, String)> = converted
        .iter()
        .map(|(id, conv)| {
            let doc_id = sql_to_doc
                .get(id.as_str())
                .copied()
                .or_else(|| live_doc.contains(id.as_str()).then_some(id.as_str()))
                .unwrap_or_else(|| sidecar_doc_id(id))
                .to_string();
            (doc_id, conv.markdown.clone())
        })
        .collect();
    let converted_refs: Vec<(&str, &str)> = converted_owned
        .iter()
        .map(|(id, md)| (id.as_str(), md.as_str()))
        .collect();
    let deleted: Vec<&str> = old_pages
        .iter()
        .filter(|(sql, _)| !live.contains(sql.as_str()))
        .map(|(_, old)| old.doc_id.as_str())
        .collect();
    persist_links_json(dir, &path_to_doc, &converted_refs, &deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: &str = "a1b2c3d4-e5f6-4780-abcd-ef1234567890";
    const HOME: &str = "doc:home";

    #[test]
    fn targets_ignore_urls_without_comment() {
        let md = format!("See [x](spec/{TARGET}.md) and [plain](https://example.com).\n");
        assert!(targets_in_markdown(&md).is_empty());
        let with = format!("{md}<!-- venus:doc:{TARGET} -->\n");
        assert_eq!(targets_in_markdown(&with), vec![TARGET.to_string()]);
    }

    #[test]
    fn upsert_skips_self_links() {
        let mut index = empty_index();
        upsert_outbound(
            &mut index,
            HOME,
            &[HOME.to_string(), TARGET.to_string(), TARGET.to_string()],
        );
        assert_eq!(outbound(&index, HOME), &[TARGET.to_string()][..]);
        assert_eq!(inbound(&index, TARGET), &[HOME.to_string()][..]);
        assert!(index.inbound.get(HOME).is_none());
    }

    #[test]
    fn convert_set_inbound_not_unrelated_or_git_mv_only() {
        use crate::catalog::CatalogPage;
        use crate::PAGE_DOC_UUID;

        let home = CatalogPage {
            sql_uuid: PAGE_DOC_UUID.into(),
            doc_id: HOME.into(),
            name: "home".into(),
            git_path: "spec/home.md".into(),
        };
        let uuid = CatalogPage {
            sql_uuid: TARGET.into(),
            doc_id: TARGET.into(),
            name: "protocol".into(),
            git_path: "design/protocol.md".into(),
        };
        let third = CatalogPage {
            sql_uuid: "c3d4e5f6-a7b8-4012-cdef-123456789012".into(),
            doc_id: "c3d4e5f6-a7b8-4012-cdef-123456789012".into(),
            name: "third".into(),
            git_path: "spec/third.md".into(),
        };
        let walk = CatalogWalk {
            pages: vec![home, uuid, third.clone()],
            folders: vec![],
        };
        let mut old = HashMap::new();
        old.insert(
            TARGET.to_string(),
            OldPage {
                sql_uuid: TARGET.into(),
                doc_id: TARGET.into(),
                git_path: "spec/protocol.md".into(),
                name: "protocol".into(),
            },
        );
        old.insert(
            PAGE_DOC_UUID.to_string(),
            OldPage {
                sql_uuid: PAGE_DOC_UUID.into(),
                doc_id: HOME.into(),
                git_path: "spec/home.md".into(),
                name: "home".into(),
            },
        );
        old.insert(
            third.sql_uuid.clone(),
            OldPage {
                sql_uuid: third.sql_uuid.clone(),
                doc_id: third.doc_id.clone(),
                git_path: third.git_path.clone(),
                name: third.name.clone(),
            },
        );
        let mut index = empty_index();
        upsert_outbound(&mut index, HOME, &[TARGET.to_string()]);
        let need = convert_set(&walk, &old, &HashSet::new(), &index);
        assert!(need.contains(PAGE_DOC_UUID), "home is inbound: {need:?}");
        assert!(
            !need.contains(TARGET),
            "uuid has no outbound: git mv only: {need:?}"
        );
        assert!(
            !need.contains(&third.sql_uuid),
            "third is unrelated: {need:?}"
        );
    }

    #[test]
    fn convert_set_outbound_when_dirname_changes() {
        use crate::catalog::CatalogPage;
        let uuid = CatalogPage {
            sql_uuid: TARGET.into(),
            doc_id: TARGET.into(),
            name: "protocol".into(),
            git_path: "design/protocol.md".into(),
        };
        let fourth = "d4e5f6a7-b8c9-4123-def0-234567890123";
        let walk = CatalogWalk {
            pages: vec![uuid],
            folders: vec![],
        };
        let mut old = HashMap::new();
        old.insert(
            TARGET.to_string(),
            OldPage {
                sql_uuid: TARGET.into(),
                doc_id: TARGET.into(),
                git_path: "spec/protocol.md".into(),
                name: "protocol".into(),
            },
        );
        let mut index = empty_index();
        upsert_outbound(&mut index, TARGET, &[fourth.to_string()]);
        let need = convert_set(&walk, &old, &HashSet::new(), &index);
        assert!(need.contains(TARGET), "dirname+outbound: {need:?}");
        assert!(!need.contains(fourth));
    }

    #[test]
    fn posix_relative_same_dir_and_nested() {
        assert_eq!(
            posix_relative("spec/home.md", "spec/protocol.md"),
            "protocol.md"
        );
        assert_eq!(
            posix_relative("spec/home.md", "design/protocol.md"),
            "../design/protocol.md"
        );
    }
}
