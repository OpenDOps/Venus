//! One layout write per job. The graph body and one search item per document.
//! The layout chooses the shard. This module does not.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use anyhow::Context;
use serde_json::json;
use surrealastic::{Body, BoxFut, Cluster, Config, Item, Layout, Owner};

use crate::hkey::hkey;
use crate::schema::{graph_surql, search_surql};
use crate::{db_id, ensure_wiki, migrate_allocation, wiki_layout, DEV_REPLICA_COUNT};

/// A document in one job. `remove` deletes its graph rows and its search item.
#[derive(Debug, Clone)]
pub struct Doc {
    pub doc_id: String,
    pub git_path: String,
    pub title: String,
    pub indexed_sha: String,
    pub heading: String,
    pub body: String,
    pub mention: String,
    pub remove: bool,
}

/// Schema for the commit set and for each shard set. `rebuild` and `lost` run
/// only when the set is the commit set. Step 9 and step 12 call them.
pub struct GraphOwner;

impl Owner for GraphOwner {
    fn schema<'a>(&'a self, set: &'a str) -> BoxFut<'a, anyhow::Result<Vec<String>>> {
        Box::pin(async move {
            if set.starts_with("graph:") {
                Ok(vec![graph_surql()])
            } else if set.starts_with("search:") {
                Ok(vec![search_surql()])
            } else {
                anyhow::bail!("schema for an unknown set");
            }
        })
    }

    fn rebuild<'a>(&'a self, set: &'a str) -> BoxFut<'a, anyhow::Result<()>> {
        Box::pin(async move {
            if !set.starts_with("graph:") {
                anyhow::bail!("rebuild runs on the commit set");
            }
            Ok(())
        })
    }

    fn lost<'a>(&'a self, set: &'a str, _tags: &'a [String]) -> BoxFut<'a, anyhow::Result<()>> {
        Box::pin(async move {
            if !set.starts_with("graph:") {
                anyhow::bail!("lost runs on the commit set");
            }
            Ok(())
        })
    }
}

/// Claim `writer:{ws}` and apply schema through the hook. `None` when another
/// holder already has the lease: this call does not write.
pub async fn open_writer(
    cluster: &Cluster,
    ws: &str,
    holder: &str,
) -> anyhow::Result<Option<Layout>> {
    match Layout::open(
        cluster.clone(),
        &db_id(ws),
        holder,
        Some(Arc::new(GraphOwner)),
    )
    .await
    {
        Ok(layout) => Ok(Some(layout)),
        Err(err) if lease_held(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Migrate, then hold the layout for each workspace. A workspace with no row
/// yet is created with `shard_count = 1`. A held lease yields `None` and no write.
pub async fn serve(
    database_url: &str,
    workspaces: &[String],
    holder: &str,
    config: Config,
) -> anyhow::Result<Vec<Option<Layout>>> {
    migrate_allocation(database_url).await?;
    for ws in workspaces {
        if wiki_layout(database_url, ws).await.is_err() {
            ensure_wiki(
                database_url,
                ws,
                1,
                i32::try_from(DEV_REPLICA_COUNT).expect("replica_count"),
            )
            .await?;
        }
    }
    let cluster = Cluster::connect(database_url, config)
        .await
        .context("serve cluster")?;
    let mut held = Vec::new();
    for ws in workspaces {
        held.push(open_writer(&cluster, ws, holder).await?);
    }
    Ok(held)
}

/// Replace each document's graph rows and record one search item. One call,
/// one graph commit. The returned lsn is that commit. A removed document is a
/// delete item. `search_sha` and `search_error` are left unset.
pub async fn project(layout: &Layout, job_id: &str, docs: &[Doc]) -> anyhow::Result<i64> {
    let mut body = Body::new();
    let mut items = Vec::with_capacity(docs.len());
    for doc in docs {
        if doc.doc_id.is_empty() || !doc.doc_id.chars().all(|c| c.is_ascii_alphanumeric()) {
            anyhow::bail!("doc id must be letters and digits");
        }
        let key = hkey(&doc.doc_id);
        let page = rid("page", &doc.doc_id, "");
        let heading = rid("heading", &doc.doc_id, "h");
        let mention = rid("mention", &doc.doc_id, "m");
        let contains = rid("contains", &doc.doc_id, "c");
        let mentions = rid("mentions", &doc.doc_id, "e");
        body = body
            .delete(&contains)
            .delete(&mentions)
            .delete(&mention)
            .delete(&heading)
            .delete(&page);
        if doc.remove {
            items.push(Item {
                key,
                item: doc.doc_id.clone(),
                tag: job_id.to_string(),
                rids: vec![page, heading, mention],
                body: Body::new(),
                delete: true,
            });
            continue;
        }
        let digest = body_hash(&doc.body);
        body = body
            .upsert(
                &page,
                json!({
                    "git_path": doc.git_path,
                    "title": doc.title,
                    "indexed_sha": doc.indexed_sha,
                    "hkey": key,
                    "pass": 1,
                    "incremental_count": 0,
                }),
            )
            .upsert(
                &heading,
                json!({
                    "doc_id": doc.doc_id,
                    "block_id": "h1",
                    "level": 1,
                    "text": doc.heading,
                    "body": doc.body,
                    "body_hash": digest,
                    "indexed_sha": doc.indexed_sha,
                }),
            )
            .upsert(
                &mention,
                json!({
                    "doc_id": doc.doc_id,
                    "block_id": "h1",
                    "kind": "uuid",
                    "text": doc.mention,
                    "norm": doc.mention,
                    "start": 0,
                    "end": doc.mention.len() as i64,
                    "pass": 1,
                    "indexed_sha": doc.indexed_sha,
                }),
            )
            .insert_relation(
                &contains,
                &page,
                &heading,
                json!({
                    "source": "outline",
                    "from_doc": doc.doc_id,
                    "indexed_sha": doc.indexed_sha,
                }),
            )
            .insert_relation(
                &mentions,
                &heading,
                &mention,
                json!({
                    "source": "extract",
                    "from_doc": doc.doc_id,
                    "indexed_sha": doc.indexed_sha,
                }),
            );
        let search = Body::new()
            .delete(&page)
            .delete(&heading)
            .delete(&mention)
            .upsert(
                &page,
                json!({
                    "git_path": doc.git_path,
                    "title": doc.title,
                    "indexed_sha": doc.indexed_sha,
                }),
            )
            .upsert(
                &heading,
                json!({
                    "doc_id": doc.doc_id,
                    "block_id": "h1",
                    "level": 1,
                    "text": doc.heading,
                    "body": doc.body,
                    "indexed_sha": doc.indexed_sha,
                }),
            )
            .upsert(
                &mention,
                json!({
                    "doc_id": doc.doc_id,
                    "block_id": "h1",
                    "kind": "uuid",
                    "text": doc.mention,
                    "norm": doc.mention,
                    "indexed_sha": doc.indexed_sha,
                }),
            );
        items.push(Item {
            key,
            item: doc.doc_id.clone(),
            tag: job_id.to_string(),
            rids: vec![page, heading, mention],
            body: search,
            delete: false,
        });
    }
    layout
        .write(body, items)
        .await
        .with_context(|| format!("project {job_id}"))
}

fn rid(table: &str, doc_id: &str, suffix: &str) -> String {
    format!("{table}:{doc_id}{suffix}")
}

fn body_hash(body: &str) -> String {
    use sha2::{Digest, Sha256};
    hex_encode(&Sha256::digest(body.as_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn lease_held(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("lease is held"))
}
