//! Catalog pin walk (y-octo). YAML + `page_identity` + `gitPath`. Not `fromDoc`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Context, Result};
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
                    doc_id,
                    name: node.name.clone(),
                    git_path,
                });
            }
            KIND_FOLDER => {
                folders.push(CatalogFolder {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    git_path,
                });
            }
            _ => {}
        }
    }
    pages.sort_by(|a, b| {
        a.git_path
            .cmp(&b.git_path)
            .then(a.sql_uuid.cmp(&b.sql_uuid))
    });
    folders.sort_by(|a, b| a.git_path.cmp(&b.git_path).then(a.id.cmp(&b.id)));
    Ok(CatalogWalk { pages, folders })
}

pub fn pages_yaml(walk: &CatalogWalk) -> String {
    let mut out = String::from("pages:\n");
    if walk.pages.is_empty() {
        out.push_str("  {}\n");
    } else {
        for page in &walk.pages {
            out.push_str("  ");
            out.push_str(&yaml_key(&page.git_path));
            out.push_str(":\n");
            out.push_str("    uuid: ");
            out.push_str(&yaml_scalar(&page.sql_uuid));
            out.push('\n');
            out.push_str("    docId: ");
            out.push_str(&yaml_scalar(&page.doc_id));
            out.push('\n');
            out.push_str("    name: ");
            out.push_str(&yaml_scalar(&page.name));
            out.push('\n');
            out.push_str("    tags: []\n");
        }
    }
    out.push_str("folders:\n");
    if walk.folders.is_empty() {
        out.push_str("  {}\n");
    } else {
        for folder in &walk.folders {
            out.push_str("  ");
            out.push_str(&yaml_key(&folder.git_path));
            out.push_str(":\n");
            out.push_str("    id: ");
            out.push_str(&yaml_scalar(&folder.id));
            out.push('\n');
            out.push_str("    name: ");
            out.push_str(&yaml_scalar(&folder.name));
            out.push('\n');
        }
    }
    out
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
    sqlx::query("DELETE FROM page_identity WHERE workspace_id = $1::uuid")
        .bind(workspace_id)
        .execute(&mut *tx)
        .await
        .context("delete page_identity")?;
    for page in &walk.pages {
        sqlx::query(
            "INSERT INTO page_identity (workspace_id, uuid, doc_id, name, git_path)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5)",
        )
        .bind(workspace_id)
        .bind(&page.sql_uuid)
        .bind(&page.doc_id)
        .bind(&page.name)
        .bind(&page.git_path)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("insert page_identity {}", page.sql_uuid))?;
    }
    tx.commit().await.context("page_identity commit")?;
    Ok(())
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
    let path = match node.parent_id.as_deref() {
        Some(parent) => join_git_names(&git_path_of(raw, parent, memo, visiting), &node.git_name),
        None => node.git_name.clone(),
    };
    visiting.remove(id);
    memo.insert(id.to_string(), path.clone());
    path
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

fn yaml_key(s: &str) -> String {
    if needs_yaml_quotes(s) {
        yaml_quoted(s)
    } else {
        s.to_string()
    }
}

fn yaml_scalar(s: &str) -> String {
    if needs_yaml_quotes(s) {
        yaml_quoted(s)
    } else {
        s.to_string()
    }
}

fn needs_yaml_quotes(s: &str) -> bool {
    s.is_empty()
        || s.starts_with([' ', '\t'])
        || s.ends_with([' ', '\t'])
        || s.contains([
            ':', '#', '{', '}', '[', ']', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
        ])
}

fn yaml_quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
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
}
