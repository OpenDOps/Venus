//! After claim: REPEATABLE READ copy of dirty set S into the pin Map.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use sqlx::{PgPool, Postgres, Transaction};
use y_octo::Doc;

use crate::catalog::{self, walk_pin};
use crate::hydrate::{apply_v1, encode_v1, is_noop_update};
use crate::links::{self, convert_set};
use crate::pin::{PinEntry, PinMap};
use crate::CATALOG_DOC_ID;

/// Catalog v0 path for `doc:home`. Used when the catalog pin is missing.
pub const GIT_PATH: &str = "spec/home.md";

struct PageBins {
    doc_id: String,
    clock: i64,
    snap: Option<Vec<u8>>,
    trail: Vec<(i64, Vec<u8>)>,
}

struct CutBins {
    pages: Vec<PageBins>,
    blobs: Vec<(String, Vec<u8>)>,
}

/// Copy this wiki's dirty pages (and workspace blobs) into `pins`, then COMMIT.
/// Convert-set extras (inbound / outbound hits) are pinned in the **same** RR
/// txn when `wiki_dir` is set. Hydrate + encode into the Map after the txn
/// ends. Does not run `fromDoc`.
pub async fn cut_workspace(
    pool: &PgPool,
    workspace_id: &str,
    pins: &mut PinMap,
    wiki_dir: Option<&Path>,
) -> Result<()> {
    let bins = match load_cut_bins(pool, workspace_id, wiki_dir).await {
        Ok(bins) => bins,
        Err(e) if is_retryable_tx(&e) => {
            tracing::warn!(error = %e, workspace_id, "cut serialization/deadlock; retry once");
            load_cut_bins(pool, workspace_id, wiki_dir)
                .await
                .context("cut retry")?
        }
        Err(e) => return Err(e).context("cut"),
    };

    pins.clear();
    pins.git_path = Some(GIT_PATH.to_string());
    for (hash, bytes) in bins.blobs {
        pins.insert_blob(hash, bytes);
    }
    for page in bins.pages {
        if page.snap.is_none() && page.trail.is_empty() {
            continue;
        }
        let bytes = hydrate_page(page.snap.as_deref(), &page.trail)
            .with_context(|| format!("hydrate {}", page.doc_id))?;
        pins.insert(
            page.doc_id,
            PinEntry {
                bytes,
                clock: page.clock,
            },
        );
    }
    Ok(())
}

async fn load_cut_bins(
    pool: &PgPool,
    workspace_id: &str,
    wiki_dir: Option<&Path>,
) -> Result<CutBins, sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;

    let dirty: Vec<(String, i64)> = sqlx::query_as(
        "SELECT d.doc_id::text, d.clock
         FROM dirty d
         LEFT JOIN last_flushed lf
           ON lf.workspace_id = d.workspace_id AND lf.doc_id = d.doc_id
         WHERE d.workspace_id = $1::uuid
           AND (lf.clock IS NULL OR d.clock > lf.clock)",
    )
    .bind(workspace_id)
    .fetch_all(&mut *tx)
    .await?;

    let mut pages = Vec::with_capacity(dirty.len());
    let mut have_catalog = false;
    let mut have_page = false;
    for (doc_id, clock) in &dirty {
        if doc_id == CATALOG_DOC_ID {
            have_catalog = true;
        } else {
            have_page = true;
        }
        pages.push(load_page_bins(&mut tx, workspace_id, doc_id, *clock).await?);
    }
    if have_page && !have_catalog {
        if let Some(page) = load_catalog_if_present(&mut tx, workspace_id).await? {
            pages.push(page);
        }
    }
    pin_convert_set_extras(&mut tx, workspace_id, wiki_dir, &mut pages).await?;

    let blobs: Vec<(String, Vec<u8>)> =
        sqlx::query_as("SELECT hash, bytes FROM blob WHERE workspace_id = $1::uuid")
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(CutBins { pages, blobs })
}

async fn load_page_bins(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
    doc_id: &str,
    clock: i64,
) -> Result<PageBins, sqlx::Error> {
    let snap: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_optional(&mut **tx)
    .await?;
    let trail: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, bin FROM crdt_update
         WHERE workspace_id = $1::uuid AND doc_id = $2::uuid
         ORDER BY seq",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(PageBins {
        doc_id: doc_id.to_string(),
        clock,
        snap: snap.map(|r| r.0),
        trail,
    })
}

/// Pin catalog bytes whenever a page is dirty so `gitPath` matches this cut.
async fn load_catalog_if_present(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
) -> Result<Option<PageBins>, sqlx::Error> {
    let clock: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(d.clock, lf.clock)
         FROM (SELECT $1::uuid AS workspace_id) w
         LEFT JOIN dirty d
           ON d.workspace_id = w.workspace_id AND d.doc_id = $2::uuid
         LEFT JOIN last_flushed lf
           ON lf.workspace_id = w.workspace_id AND lf.doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(CATALOG_DOC_ID)
    .fetch_one(&mut **tx)
    .await?;
    let page = load_page_bins(tx, workspace_id, CATALOG_DOC_ID, clock.unwrap_or(0)).await?;
    if page.snap.is_none() && page.trail.is_empty() {
        return Ok(None);
    }
    Ok(Some(page))
}

/// Extra convert-set `doc_id`s join this RR pin Map before COMMIT.
async fn pin_convert_set_extras(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
    wiki_dir: Option<&Path>,
    pages: &mut Vec<PageBins>,
) -> Result<(), sqlx::Error> {
    let Some(wiki_dir) = wiki_dir else {
        return Ok(());
    };
    let Some(idx) = pages.iter().position(|p| p.doc_id == CATALOG_DOC_ID) else {
        return Ok(());
    };
    let cat = &pages[idx];
    let Ok(bytes) = hydrate_page(cat.snap.as_deref(), &cat.trail) else {
        return Ok(());
    };
    let Ok(walk) = walk_pin(&bytes) else {
        return Ok(());
    };
    let old = match catalog::load_old_pages_exec(&mut **tx, workspace_id).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "cut skipped convert-set extras; page_identity");
            return Ok(());
        }
    };
    let path_to_doc: HashMap<String, String> = walk
        .pages
        .iter()
        .map(|p| (p.git_path.clone(), p.doc_id.clone()))
        .collect();
    let index = match links::load(wiki_dir) {
        Some(idx) => idx,
        None => links::rebuild_from_wiki(wiki_dir, &path_to_doc).unwrap_or_default(),
    };
    let dirty_sql: HashSet<String> = pages
        .iter()
        .filter(|p| p.doc_id != CATALOG_DOC_ID)
        .map(|p| p.doc_id.clone())
        .collect();
    let need = convert_set(&walk, &old, &dirty_sql, &index);
    let pinned: HashSet<String> = pages.iter().map(|p| p.doc_id.clone()).collect();
    let extras: Vec<String> = need
        .into_iter()
        .filter(|id| !pinned.contains(id) && id != CATALOG_DOC_ID)
        .collect();
    for sql in extras {
        let clock = load_page_clock(tx, workspace_id, &sql).await?;
        let page = load_page_bins(tx, workspace_id, &sql, clock).await?;
        if page.snap.is_none() && page.trail.is_empty() {
            continue;
        }
        pages.push(page);
    }
    Ok(())
}

async fn load_page_clock(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
    doc_id: &str,
) -> Result<i64, sqlx::Error> {
    let clock: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(d.clock, lf.clock)
         FROM (SELECT $1::uuid AS workspace_id) w
         LEFT JOIN dirty d
           ON d.workspace_id = w.workspace_id AND d.doc_id = $2::uuid
         LEFT JOIN last_flushed lf
           ON lf.workspace_id = w.workspace_id AND lf.doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(clock.unwrap_or(0))
}

fn hydrate_page(snap: Option<&[u8]>, trail: &[(i64, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut doc = match snap {
        Some(bin) if !is_noop_update(bin) => apply_snapshot(bin),
        _ => Doc::default(),
    };
    for (_seq, bin) in trail {
        if is_noop_update(bin) {
            continue;
        }
        if let Err(e) = apply_v1(&mut doc, bin) {
            tracing::warn!(error = %e, bytes = bin.len(), "cut skipped a trail bin");
        }
    }
    encode_v1(&doc)
}

fn apply_snapshot(bin: &[u8]) -> Doc {
    let mut doc = Doc::default();
    match apply_v1(&mut doc, bin) {
        Ok(()) => doc,
        Err(e) => {
            tracing::error!(
                error = %e,
                bytes = bin.len(),
                "cut snapshot failed to apply; trail only"
            );
            Doc::default()
        }
    }
}

fn is_retryable_tx(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => matches!(db.code().as_deref(), Some("40001" | "40P01")),
        _ => false,
    }
}
