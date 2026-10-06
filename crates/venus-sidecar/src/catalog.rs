//! Catalog pin walk (y-octo). YAML + `page_identity` + `gitPath`. Not `fromDoc`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use y_octo::{Any, Doc, Map, Value};

use crate::hydrate::hydrate_v1;
use crate::{CATALOG_DOC_ID, PAGE_DOC_ID, PAGE_DOC_UUID};

const NODES_KEY: &str = "nodes";
const KIND_DOC: &str = "doc";
const KIND_FOLDER: &str = "folder";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPage {
    pub sql_uuid: String,
    pub doc_id: String,
    pub name: String,
    pub git_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFolder {
    pub id: String,
    pub name: String,
    pub git_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogWalk {
    pub pages: Vec<CatalogPage>,
    pub folders: Vec<CatalogFolder>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldPage {
    pub sql_uuid: String,
    pub doc_id: String,
    pub git_path: String,
    pub name: String,
}

struct RawNode {
    id: String,
    kind: String,
    name: String,
    parent_id: Option<String>,
    git_name: String,
    doc_id: Option<String>,
}

pub fn is_catalog_sql_id(doc_id: &str) -> bool {
    doc_id == CATALOG_DOC_ID
}

pub fn sidecar_doc_id(sql_id: &str) -> &str {
    if sql_id == PAGE_DOC_UUID {
        PAGE_DOC_ID
    } else {
        sql_id
    }
}

pub fn walk_pin(bytes: &[u8]) -> Result<CatalogWalk> {
    let doc = hydrate_v1(bytes).context("hydrate catalog pin")?;
    walk_doc(&doc)
}

pub fn walk_doc(doc: &Doc) -> Result<CatalogWalk> {
    let nodes = doc
        .get_map(NODES_KEY)
        .map_err(|e| anyhow!("catalog nodes map: {e}"))?;
    let mut raw = HashMap::new();
    for (id, value) in nodes.iter() {
        let Value::Map(ymap) = value else {
            continue;
        };
        if let Some(node) = read_node(&id, &ymap) {
            raw.insert(node.id.clone(), node);
        }
    }
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let mut pages = Vec::new();
    let mut folders = Vec::new();
    for node in raw.values() {
        if let Some(reason) = structure_repair(&raw, &node.id) {
            tracing::warn!(
                node_id = %node.id,
                reason,
                "catalog structure repair"
            );
        }
        let git_path = git_path_of(&raw, &node.id, &mut memo, &mut visiting);
        match node.kind.as_str() {
            KIND_DOC => {
                let doc_id = node
                    .doc_id
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| node.id.clone());
                let Some(sql_uuid) = sql_uuid_for_doc(&doc_id) else {
                    continue;
                };
                pages.push(CatalogPage {
                    sql_uuid,
                    doc_id: doc_id.clone(),
                    name: published_name(&node.name, &doc_id),
                    git_path,
                });
            }
            KIND_FOLDER => {
                folders.push(CatalogFolder {
                    id: node.id.clone(),
                    name: published_name(&node.name, &node.id),
                    git_path,
                });
            }
            _ => {}
        }
    }
    disambiguate_published_paths(&mut pages);
    pages.sort_by(|a, b| {
        a.git_path
            .cmp(&b.git_path)
            .then(a.sql_uuid.cmp(&b.sql_uuid))
    });
    folders.sort_by(|a, b| a.git_path.cmp(&b.git_path).then(a.id.cmp(&b.id)));
    Ok(CatalogWalk { pages, folders })
}

/// Published paths only. The catalog CRDT is unchanged.
///
/// Pages that share a case-folded `git_path` keep one winner (lowest `doc_id`).
/// The others are published as `name-<sql uuid prefix>.md`.
pub fn disambiguate_published_paths(pages: &mut [CatalogPage]) {
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, page) in pages.iter().enumerate() {
        groups
            .entry(case_fold_path(&page.git_path))
            .or_default()
            .push(index);
    }
    let mut taken: HashSet<String> = HashSet::new();
    let mut losers: Vec<usize> = Vec::new();
    for group in groups.values_mut() {
        group.sort_by(|&a, &b| pages[a].doc_id.cmp(&pages[b].doc_id));
        taken.insert(case_fold_path(&pages[group[0]].git_path));
        losers.extend(group.iter().skip(1).copied());
    }
    losers.sort_by(|&a, &b| pages[a].doc_id.cmp(&pages[b].doc_id));
    for index in losers {
        let path = allocate_published_path(&pages[index].git_path, &pages[index].sql_uuid, &taken);
        taken.insert(case_fold_path(&path));
        pages[index].git_path = path;
    }
}

fn case_fold_path(path: &str) -> String {
    path.to_lowercase()
}

fn allocate_published_path(path: &str, sql_uuid: &str, taken: &HashSet<String>) -> String {
    let short = sql_uuid.get(..8).unwrap_or(sql_uuid);
    for tag in [short, sql_uuid] {
        let candidate = suffix_git_path(path, tag);
        if !taken.contains(&case_fold_path(&candidate)) {
            return candidate;
        }
    }
    let mut n = 2u32;
    loop {
        let candidate = suffix_git_path(path, &format!("{sql_uuid}-{n}"));
        if !taken.contains(&case_fold_path(&candidate)) {
            return candidate;
        }
        n += 1;
    }
}

fn suffix_git_path(path: &str, tag: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".md") {
        let stem = &path[..path.len() - 3];
        format!("{stem}-{tag}.md")
    } else {
        format!("{path}-{tag}")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PagesFile {
    pages: BTreeMap<String, PageYaml>,
    folders: BTreeMap<String, FolderYaml>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PageYaml {
    uuid: String,
    #[serde(rename = "docId")]
    doc_id: String,
    name: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FolderYaml {
    id: String,
    name: String,
}

pub fn pages_yaml(walk: &CatalogWalk) -> String {
    let file = PagesFile {
        pages: walk
            .pages
            .iter()
            .map(|page| {
                (
                    page.git_path.clone(),
                    PageYaml {
                        uuid: page.sql_uuid.clone(),
                        doc_id: page.doc_id.clone(),
                        name: page.name.clone(),
                        tags: Vec::new(),
                    },
                )
            })
            .collect(),
        folders: walk
            .folders
            .iter()
            .map(|folder| {
                (
                    folder.git_path.clone(),
                    FolderYaml {
                        id: folder.id.clone(),
                        name: folder.name.clone(),
                    },
                )
            })
            .collect(),
    };
    serde_yaml::to_string(&file).unwrap_or_else(|err| {
        tracing::error!(error = %err, "pages.yaml encode failed");
        "pages: {}\nfolders: {}\n".to_string()
    })
}

/// Pages recorded in `pages.yaml`. An error means the file did not parse;
/// callers must not treat that as an empty catalog.
pub fn pages_from_yaml(yaml: &str) -> Result<Vec<CatalogPage>> {
    let file: PagesFile = serde_yaml::from_str(yaml).context("pages.yaml")?;
    Ok(file
        .pages
        .into_iter()
        .map(|(git_path, page)| CatalogPage {
            sql_uuid: page.uuid,
            doc_id: page.doc_id,
            name: page.name,
            git_path,
        })
        .collect())
}

pub fn page_identity_matches(old: &HashMap<String, OldPage>, pages: &[CatalogPage]) -> bool {
    if old.len() != pages.len() {
        return false;
    }
    pages.iter().all(|page| {
        old.get(&page.sql_uuid).is_some_and(|row| {
            row.doc_id == page.doc_id && row.git_path == page.git_path && row.name == page.name
        })
    })
}

/// `pages.yaml` path → docId. Same library as [`pages_yaml`].
pub fn path_to_doc_from_pages_yaml(yaml: &str) -> HashMap<String, String> {
    let file: PagesFile = match serde_yaml::from_str(yaml) {
        Ok(file) => file,
        Err(err) => {
            tracing::warn!(error = %err, "pages.yaml decode failed");
            return HashMap::new();
        }
    };
    file.pages
        .into_iter()
        .map(|(path, page)| (path, page.doc_id))
        .collect()
}

/// Drop ASCII and Unicode control characters from a catalog display name.
fn published_name(name: &str, node_id: &str) -> String {
    if !name.chars().any(char::is_control) {
        return name.to_string();
    }
    tracing::warn!(node_id = %node_id, "catalog name dropped control characters");
    name.chars().filter(|c| !c.is_control()).collect()
}

pub async fn load_old_pages(pool: &PgPool, workspace_id: &str) -> Result<HashMap<String, OldPage>> {
    load_old_pages_exec(pool, workspace_id).await
}

pub async fn load_old_pages_exec<'e, E>(
    executor: E,
    workspace_id: &str,
) -> Result<HashMap<String, OldPage>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT uuid::text, doc_id, git_path, name
         FROM page_identity
         WHERE workspace_id = $1::uuid",
    )
    .bind(workspace_id)
    .fetch_all(executor)
    .await
    .context("load page_identity")?;
    Ok(rows
        .into_iter()
        .map(|(sql_uuid, doc_id, git_path, name)| {
            (
                sql_uuid.clone(),
                OldPage {
                    sql_uuid,
                    doc_id,
                    git_path,
                    name,
                },
            )
        })
        .collect())
}

pub async fn replace_page_identity(
    pool: &PgPool,
    workspace_id: &str,
    walk: &CatalogWalk,
) -> Result<()> {
    let mut tx = pool.begin().await.context("page_identity begin")?;
    replace_page_identity_tx(&mut tx, workspace_id, &walk.pages).await?;
    tx.commit().await.context("page_identity commit")?;
    Ok(())
}

pub async fn replace_page_identity_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: &str,
    pages: &[CatalogPage],
) -> Result<()> {
    sqlx::query("DELETE FROM page_identity WHERE workspace_id = $1::uuid")
        .bind(workspace_id)
        .execute(&mut **tx)
        .await
        .context("delete page_identity")?;
    for page in pages {
        sqlx::query(
            "INSERT INTO page_identity (workspace_id, uuid, doc_id, name, git_path)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5)",
        )
        .bind(workspace_id)
        .bind(&page.sql_uuid)
        .bind(&page.doc_id)
        .bind(&page.name)
        .bind(&page.git_path)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("insert page_identity {}", page.sql_uuid))?;
    }
    Ok(())
}

/// When HEAD `.venus/pages.yaml` and `page_identity` disagree, replace the
/// table from the yaml. A parse error leaves the table unchanged.
pub async fn reconcile_page_identity(pool: &PgPool, workspace_id: &str, yaml: &str) -> Result<()> {
    let pages = match pages_from_yaml(yaml) {
        Ok(pages) => pages,
        Err(err) => {
            tracing::warn!(error = %err, "pages.yaml in HEAD did not parse; leaving page_identity");
            return Ok(());
        }
    };
    let old = load_old_pages(pool, workspace_id).await?;
    if page_identity_matches(&old, &pages) {
        return Ok(());
    }
    tracing::warn!(
        workspace_id,
        head_pages = pages.len(),
        sql_pages = old.len(),
        "page_identity rebuilt from HEAD pages.yaml"
    );
    replace_page_identity(
        pool,
        workspace_id,
        &CatalogWalk {
            pages,
            folders: Vec::new(),
        },
    )
    .await
}

fn read_node(id: &str, ymap: &Map) -> Option<RawNode> {
    let kind = map_string(ymap, "kind")?;
    let name = map_string(ymap, "name").unwrap_or_default();
    let stored_id = map_string(ymap, "id").unwrap_or_else(|| id.to_string());
    let parent_id = match ymap.get("parentId") {
        None => None,
        Some(Value::Any(Any::Null)) | Some(Value::Any(Any::Undefined)) => None,
        Some(Value::Any(Any::String(s))) if s.is_empty() => None,
        Some(Value::Any(Any::String(s))) => Some(s),
        Some(other) => {
            let s = other.to_string();
            if s.is_empty() || s == "null" {
                None
            } else {
                Some(s)
            }
        }
    };
    let git_name = stored_git_name(ymap);
    let doc_id = map_string(ymap, "docId");
    Some(RawNode {
        id: stored_id,
        kind,
        name,
        parent_id,
        git_name,
        doc_id,
    })
}

fn stored_git_name(ymap: &Map) -> String {
    if let Some(name) = map_string(ymap, "gitName").filter(|s| !s.is_empty()) {
        return name;
    }
    map_string(ymap, "gitPath")
        .and_then(|p| p.rsplit('/').next().map(str::to_string))
        .unwrap_or_default()
}

fn git_path_of(
    raw: &HashMap<String, RawNode>,
    id: &str,
    memo: &mut HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> String {
    if let Some(path) = memo.get(id) {
        return path.clone();
    }
    let Some(node) = raw.get(id) else {
        memo.insert(id.to_string(), String::new());
        return String::new();
    };
    if visiting.contains(id) {
        return node.git_name.clone();
    }
    visiting.insert(id.to_string());
    let path = match repaired_parent(raw, id) {
        Some(parent) => join_git_names(&git_path_of(raw, parent, memo, visiting), &node.git_name),
        None => node.git_name.clone(),
    };
    visiting.remove(id);
    memo.insert(id.to_string(), path.clone());
    path
}

/// Missing parent, or the greatest id on a parent cycle, publishes at the root.
fn structure_repair(raw: &HashMap<String, RawNode>, id: &str) -> Option<&'static str> {
    let node = raw.get(id)?;
    let parent = node.parent_id.as_deref()?;
    if !raw.contains_key(parent) {
        return Some("orphan");
    }
    if is_cycle_break(raw, id) {
        return Some("cycle");
    }
    None
}

fn repaired_parent<'a>(raw: &'a HashMap<String, RawNode>, id: &str) -> Option<&'a str> {
    let node = raw.get(id)?;
    let parent = node.parent_id.as_deref()?;
    if !raw.contains_key(parent) || is_cycle_break(raw, id) {
        return None;
    }
    Some(parent)
}

fn is_cycle_break(raw: &HashMap<String, RawNode>, id: &str) -> bool {
    let Some(members) = cycle_members(raw, id) else {
        return false;
    };
    if !members.iter().any(|member| member == id) {
        return false;
    }
    members.iter().max().map(String::as_str) == Some(id)
}

fn cycle_members(raw: &HashMap<String, RawNode>, id: &str) -> Option<Vec<String>> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut chain = Vec::new();
    let mut cur = id.to_string();
    loop {
        if !raw.contains_key(&cur) {
            return None;
        }
        if let Some(start) = seen.get(&cur) {
            return Some(chain[*start..].to_vec());
        }
        seen.insert(cur.clone(), chain.len());
        chain.push(cur.clone());
        let parent = raw.get(&cur)?.parent_id.clone()?;
        if !raw.contains_key(&parent) {
            return None;
        }
        cur = parent;
    }
}

fn join_git_names(parent: &str, git_name: &str) -> String {
    let parent = parent.trim_end_matches('/');
    if parent.is_empty() {
        git_name.to_string()
    } else {
        format!("{parent}/{git_name}")
    }
}

fn sql_uuid_for_doc(doc_id: &str) -> Option<String> {
    if doc_id == PAGE_DOC_ID {
        return Some(PAGE_DOC_UUID.to_string());
    }
    if is_hyphenated_uuid(doc_id) {
        return Some(doc_id.to_string());
    }
    None
}

fn is_hyphenated_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b[8] == b'-'
        && b[13] == b'-'
        && b[18] == b'-'
        && b[23] == b'-'
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn map_string(map: &Map, key: &str) -> Option<String> {
    match map.get(key)? {
        Value::Any(Any::String(s)) => Some(s),
        Value::Text(t) => Some(t.to_string()),
        other => {
            let s = other.to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use y_octo::Doc;

    fn put_node(
        doc: &Doc,
        nodes: &mut Map,
        id: &str,
        kind: &str,
        name: &str,
        parent_id: Option<&str>,
        git_name: &str,
        doc_id: Option<&str>,
    ) {
        let mut node = doc.create_map().expect("node map");
        node.insert("id".into(), id).expect("id");
        node.insert("kind".into(), kind).expect("kind");
        node.insert("name".into(), name).expect("name");
        if let Some(parent) = parent_id {
            node.insert("parentId".into(), parent).expect("parentId");
        }
        node.insert("order".into(), "a0").expect("order");
        node.insert("gitName".into(), git_name).expect("gitName");
        if let Some(doc_id) = doc_id {
            node.insert("docId".into(), doc_id).expect("docId");
        }
        nodes.insert(id.into(), node).expect("insert node");
    }

    #[test]
    fn walk_seed_and_created_page_git_paths() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        put_node(
            &doc,
            &mut nodes,
            "folder:spec",
            KIND_FOLDER,
            "spec",
            None,
            "spec",
            None,
        );
        put_node(
            &doc,
            &mut nodes,
            PAGE_DOC_ID,
            KIND_DOC,
            "home",
            Some("folder:spec"),
            "home.md",
            Some(PAGE_DOC_ID),
        );
        let uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        put_node(
            &doc,
            &mut nodes,
            uuid,
            KIND_DOC,
            uuid,
            Some("folder:spec"),
            &format!("{uuid}.md"),
            Some(uuid),
        );
        let walk = walk_doc(&doc).expect("walk");
        assert!(walk
            .folders
            .iter()
            .any(|f| f.id == "folder:spec" && f.git_path == "spec"));
        let home = walk
            .pages
            .iter()
            .find(|p| p.doc_id == PAGE_DOC_ID)
            .expect("home");
        assert_eq!(home.sql_uuid, PAGE_DOC_UUID);
        assert_eq!(home.git_path, "spec/home.md");
        let created = walk.pages.iter().find(|p| p.doc_id == uuid).expect("page");
        assert_eq!(created.sql_uuid, uuid);
        assert_eq!(created.git_path, format!("spec/{uuid}.md"));
        let yaml = pages_yaml(&walk);
        assert!(yaml.contains("spec/home.md:"));
        assert!(yaml.contains(&format!("spec/{uuid}.md:")));
        assert!(yaml.contains("folder:spec"));
        assert!(!yaml.contains("fromDoc"));
    }

    #[test]
    fn folder_rename_updates_descendant_git_path() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        put_node(
            &doc,
            &mut nodes,
            "folder:spec",
            KIND_FOLDER,
            "SPEC",
            None,
            "SPEC",
            None,
        );
        put_node(
            &doc,
            &mut nodes,
            PAGE_DOC_ID,
            KIND_DOC,
            "home",
            Some("folder:spec"),
            "home.md",
            Some(PAGE_DOC_ID),
        );
        let walk = walk_doc(&doc).expect("walk");
        assert_eq!(walk.folders[0].git_path, "SPEC");
        assert_eq!(walk.pages[0].git_path, "SPEC/home.md");
    }

    #[test]
    fn deleted_page_is_absent_from_walk() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        put_node(
            &doc,
            &mut nodes,
            "folder:spec",
            KIND_FOLDER,
            "spec",
            None,
            "spec",
            None,
        );
        put_node(
            &doc,
            &mut nodes,
            PAGE_DOC_ID,
            KIND_DOC,
            "home",
            Some("folder:spec"),
            "home.md",
            Some(PAGE_DOC_ID),
        );
        let uuid = "a1b2c3d4-e5f6-4780-abcd-ef1234567890";
        put_node(
            &doc,
            &mut nodes,
            uuid,
            KIND_DOC,
            uuid,
            Some("folder:spec"),
            &format!("{uuid}.md"),
            Some(uuid),
        );
        nodes.remove(uuid);
        let walk = walk_doc(&doc).expect("walk");
        assert!(walk.pages.iter().all(|p| p.doc_id != uuid));
        assert!(walk.pages.iter().any(|p| p.doc_id == PAGE_DOC_ID));
        assert!(!pages_yaml(&walk).contains(uuid));
    }

    #[test]
    fn duplicate_and_case_variant_paths_publish_once() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        put_node(
            &doc,
            &mut nodes,
            "folder:spec",
            KIND_FOLDER,
            "spec",
            None,
            "spec",
            None,
        );
        let low = "11111111-1111-4111-8111-111111111111";
        let high = "22222222-2222-4222-8222-222222222222";
        put_node(
            &doc,
            &mut nodes,
            low,
            KIND_DOC,
            "notes",
            Some("folder:spec"),
            "notes.md",
            Some(low),
        );
        put_node(
            &doc,
            &mut nodes,
            high,
            KIND_DOC,
            "notes",
            Some("folder:spec"),
            "Notes.md",
            Some(high),
        );
        let walk = walk_doc(&doc).expect("walk");
        let low_page = walk.pages.iter().find(|p| p.doc_id == low).expect("low");
        let high_page = walk.pages.iter().find(|p| p.doc_id == high).expect("high");
        assert_eq!(low_page.git_path, "spec/notes.md");
        assert_eq!(high_page.git_path, "spec/Notes-22222222.md");
        let yaml = pages_yaml(&walk);
        assert!(yaml.contains("spec/notes.md:"));
        assert!(yaml.contains("spec/Notes-22222222.md:"));
        assert_eq!(yaml.matches("spec/notes.md:").count(), 1);
    }

    #[test]
    fn orphan_publishes_at_root_and_cycle_breaks_at_greatest_id() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        let low = "folder:00000000-0000-4000-8000-000000000001";
        let high = "folder:00000000-0000-4000-8000-000000000002";
        put_node(
            &doc,
            &mut nodes,
            low,
            KIND_FOLDER,
            "a",
            Some(high),
            "a",
            None,
        );
        put_node(
            &doc,
            &mut nodes,
            high,
            KIND_FOLDER,
            "b",
            Some(low),
            "b",
            None,
        );
        let page = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
        put_node(
            &doc,
            &mut nodes,
            page,
            KIND_DOC,
            "notes",
            Some("folder:missing"),
            "notes.md",
            Some(page),
        );
        let walk = walk_doc(&doc).expect("walk");
        let low_folder = walk.folders.iter().find(|f| f.id == low).expect("low");
        let high_folder = walk.folders.iter().find(|f| f.id == high).expect("high");
        assert_eq!(high_folder.git_path, "b");
        assert_eq!(low_folder.git_path, "b/a");
        let notes = walk.pages.iter().find(|p| p.doc_id == page).expect("notes");
        assert_eq!(notes.git_path, "notes.md");
    }

    #[test]
    fn pages_from_yaml_matches_the_writer_and_a_disagreement() {
        let walk = CatalogWalk {
            pages: vec![CatalogPage {
                sql_uuid: PAGE_DOC_UUID.into(),
                doc_id: PAGE_DOC_ID.into(),
                name: "true".into(),
                git_path: "spec/home.md".into(),
            }],
            folders: vec![],
        };
        let pages = pages_from_yaml(&pages_yaml(&walk)).expect("yaml");
        let old = pages
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
            .collect();
        assert!(page_identity_matches(&old, &pages));
        assert_eq!(pages[0].name, "true");
        let mut renamed = pages.clone();
        renamed[0].git_path = "spec/Start.md".into();
        assert!(!page_identity_matches(&old, &renamed));
        assert!(pages_from_yaml("pages: [").is_err());
    }

    #[test]
    fn pages_yaml_round_trips_bool_looking_names_and_drops_controls() {
        let doc = Doc::default();
        let mut nodes = doc.get_or_create_map(NODES_KEY).expect("nodes");
        put_node(
            &doc,
            &mut nodes,
            "folder:spec",
            KIND_FOLDER,
            "spec",
            None,
            "spec",
            None,
        );
        put_node(
            &doc,
            &mut nodes,
            PAGE_DOC_ID,
            KIND_DOC,
            "true",
            Some("folder:spec"),
            "home.md",
            Some(PAGE_DOC_ID),
        );
        let broken = "a1b2c3d4-e5f6-4780-abcd-ef1234567890";
        put_node(
            &doc,
            &mut nodes,
            broken,
            KIND_DOC,
            "a\nb",
            Some("folder:spec"),
            &format!("{broken}.md"),
            Some(broken),
        );
        let walk = walk_doc(&doc).expect("walk");
        let home = walk
            .pages
            .iter()
            .find(|p| p.doc_id == PAGE_DOC_ID)
            .expect("home");
        assert_eq!(home.name, "true");
        let injected = walk
            .pages
            .iter()
            .find(|p| p.doc_id == broken)
            .expect("page");
        assert_eq!(injected.name, "ab");
        let yaml = pages_yaml(&walk);
        assert!(!yaml.contains("a\nb"), "{yaml}");
        let map = path_to_doc_from_pages_yaml(&yaml);
        assert_eq!(
            map.get("spec/home.md").map(String::as_str),
            Some(PAGE_DOC_ID)
        );
        assert_eq!(
            map.get(&format!("spec/{broken}.md")).map(String::as_str),
            Some(broken)
        );
        let again = path_to_doc_from_pages_yaml(
            r#"
pages:
  "spec/a:b.md":
    uuid: "395cd07b-bdb1-5f54-ada8-e9a3fabb6a20"
    docId: "doc:home"
    name: "yes"
    tags: []
folders: {}
"#,
        );
        assert_eq!(
            again.get("spec/a:b.md").map(String::as_str),
            Some(PAGE_DOC_ID)
        );
    }
}
