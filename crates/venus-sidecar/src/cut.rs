//! After claim: REPEATABLE READ copy of dirty set S into the pin Map.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use sqlx::{PgPool, Postgres, Transaction};
use y_octo::Doc;

use crate::hydrate::{apply_v1, encode_v1, is_noop_update};
use crate::pin::{PinEntry, PinMap};

/// Catalog v0 path for `doc:home`. Constant in the same collect (no catalog CRDT).
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
/// Hydrate + encode into the Map after the txn ends. Does not run `fromDoc`.
pub async fn cut_workspace(pool: &PgPool, workspace_id: &str, pins: &mut PinMap) -> Result<()> {
    let bins = match load_cut_bins(pool, workspace_id).await {
        Ok(bins) => bins,
        Err(e) if is_retryable_tx(&e) => {
            tracing::warn!(error = %e, workspace_id, "cut serialization/deadlock; retry once");
            load_cut_bins(pool, workspace_id)
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

async fn load_cut_bins(pool: &PgPool, workspace_id: &str) -> Result<CutBins, sqlx::Error> {
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
    for (doc_id, clock) in dirty {
        let snap: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
        )
        .bind(workspace_id)
        .bind(&doc_id)
        .fetch_optional(&mut *tx)
        .await?;
        let trail: Vec<(i64, Vec<u8>)> = sqlx::query_as(
            "SELECT seq, bin FROM crdt_update
             WHERE workspace_id = $1::uuid AND doc_id = $2::uuid
             ORDER BY seq",
        )
        .bind(workspace_id)
        .bind(&doc_id)
        .fetch_all(&mut *tx)
        .await?;
        pages.push(PageBins {
            doc_id,
            clock,
            snap: snap.map(|r| r.0),
            trail,
        });
    }

    let blobs: Vec<(String, Vec<u8>)> =
        sqlx::query_as("SELECT hash, bytes FROM blob WHERE workspace_id = $1::uuid")
            .bind(workspace_id)
            .fetch_all(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(CutBins { pages, blobs })
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
