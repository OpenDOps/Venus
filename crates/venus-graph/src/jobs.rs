//! `graph_jobs`: one row per wiki, claimed while this process holds `writer:{ws}`.
//! The sidecar inserts the row. This process claims it. Hub does neither.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use sqlx::PgPool;
use surrealastic::Cluster;

use crate::lease_name;
use crate::project::{open_writer, project, Doc};

/// Same columns the sidecar creates on flush. Hub `schema.sql` does not.
pub const GRAPH_JOBS_DDL: &str = "
CREATE TABLE IF NOT EXISTS graph_jobs (
    workspace_id UUID PRIMARY KEY,
    wiki_sha TEXT NOT NULL,
    dirty_doc_ids TEXT[] NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'failed')),
    owner TEXT,
    claimed_at TIMESTAMPTZ,
    fence BIGINT,
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT
)";

/// A pending row claimed by the holder of `writer:{workspace_id}`.
#[derive(Debug, Clone)]
pub struct GraphJob {
    pub workspace_id: String,
    pub wiki_sha: String,
    pub dirty_doc_ids: Vec<String>,
    pub fence: i64,
}

pub async fn ensure_graph_jobs(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(GRAPH_JOBS_DDL)
        .execute(pool)
        .await
        .context("graph_jobs table")?;
    Ok(())
}

/// `SKIP LOCKED` claim of the pending row. `None` when this holder does not
/// have `writer:{ws}`, or another claim already has the row.
pub async fn claim_job(
    pool: &PgPool,
    workspace_id: &str,
    holder: &str,
) -> anyhow::Result<Option<GraphJob>> {
    ensure_graph_jobs(pool).await?;
    let job = sqlx::query_as::<_, (String, String, Vec<String>, i64)>(
        "WITH lease AS (
           SELECT fence FROM repl_lease
           WHERE name = $1 AND holder = $2 AND lease_until > now()
         ),
         picked AS (
           SELECT g.workspace_id
           FROM graph_jobs g
           WHERE g.workspace_id = $3::uuid
             AND g.state = 'pending'
             AND (g.owner IS NULL OR g.owner = $2)
             AND EXISTS (SELECT 1 FROM lease)
           FOR UPDATE SKIP LOCKED
           LIMIT 1
         )
         UPDATE graph_jobs g
         SET owner = $2,
             claimed_at = now(),
             fence = (SELECT fence FROM lease),
             attempts = g.attempts + 1
         FROM picked
         WHERE g.workspace_id = picked.workspace_id
         RETURNING g.workspace_id::text AS workspace_id,
                   g.wiki_sha,
                   g.dirty_doc_ids,
                   COALESCE(g.fence, 0) AS fence",
    )
    .bind(lease_name(workspace_id))
    .bind(holder)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .context("claim graph_jobs")?;
    Ok(
        job.map(|(workspace_id, wiki_sha, dirty_doc_ids, fence)| GraphJob {
            workspace_id,
            wiki_sha,
            dirty_doc_ids,
            fence,
        }),
    )
}

/// No fixture documents: remember `wiki_sha` and finish the claim.
/// A newer flush that changed the sha leaves the row pending for the next claim.
pub async fn record_sha(pool: &PgPool, job: &GraphJob, holder: &str) -> anyhow::Result<String> {
    let deleted: Option<String> = sqlx::query_scalar(
        "DELETE FROM graph_jobs
         WHERE workspace_id = $1::uuid AND owner = $2 AND wiki_sha = $3
         RETURNING wiki_sha",
    )
    .bind(&job.workspace_id)
    .bind(holder)
    .bind(&job.wiki_sha)
    .fetch_optional(pool)
    .await
    .context("record graph_jobs sha")?;
    if deleted.is_none() {
        sqlx::query(
            "UPDATE graph_jobs
             SET owner = NULL, claimed_at = NULL, fence = NULL
             WHERE workspace_id = $1::uuid AND owner = $2",
        )
        .bind(&job.workspace_id)
        .bind(holder)
        .execute(pool)
        .await
        .context("release coalesced graph_jobs")?;
    }
    Ok(job.wiki_sha.clone())
}

/// Claim the pending row and run one `layout.write`. The job id is the entry
/// `tag`. The layout's fence is the writer lease. A holder who does not have
/// the lease is fenced and does not claim the row.
///
/// An empty `docs` records the sha and does not write. Markdown is not read.
pub async fn write_job(
    cluster: &Cluster,
    pool: &PgPool,
    workspace_id: &str,
    holder: &str,
    docs: &[Doc],
) -> anyhow::Result<i64> {
    let layout = open_writer(cluster, workspace_id, holder).await?;
    let Some(layout) = layout else {
        anyhow::bail!("fenced");
    };
    let Some(job) = claim_job(pool, workspace_id, holder).await? else {
        anyhow::bail!("no pending graph job");
    };
    if docs.is_empty() {
        record_sha(pool, &job, holder).await?;
        return Ok(0);
    }
    match project(&layout, &job.workspace_id, docs).await {
        Ok(lsn) => {
            record_sha(pool, &job, holder).await?;
            Ok(lsn)
        }
        Err(err) => {
            mark_failed(pool, &job, holder, &err.to_string()).await?;
            Err(err)
        }
    }
}

async fn mark_failed(
    pool: &PgPool,
    job: &GraphJob,
    holder: &str,
    error: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE graph_jobs
         SET state = 'failed',
             last_error = $3,
             owner = NULL,
             claimed_at = NULL,
             fence = NULL
         WHERE workspace_id = $1::uuid AND owner = $2",
    )
    .bind(&job.workspace_id)
    .bind(holder)
    .bind(error)
    .execute(pool)
    .await
    .context("graph_jobs failed")?;
    Ok(())
}
