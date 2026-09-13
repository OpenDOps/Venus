//! One live owner per `workspace_id` (wiki sticky). Not cookie / IP / `doc_id`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

/// `try_acquire` failed. `Held` is the only case HTTP maps to 503.
#[derive(Debug)]
pub enum LeaseError {
    Held {
        workspace_id: String,
        owner: Option<String>,
    },
    Store(sqlx::Error),
}

impl fmt::Display for LeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaseError::Held {
                workspace_id,
                owner: Some(other),
            } => write!(f, "workspace {workspace_id} owned by {other}"),
            LeaseError::Held {
                workspace_id,
                owner: None,
            } => write!(f, "workspace {workspace_id} owned by another hub"),
            LeaseError::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LeaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LeaseError::Store(e) => Some(e),
            LeaseError::Held { .. } => None,
        }
    }
}

#[derive(Clone)]
pub struct Lease {
    pool: PgPool,
    owner: String,
    ttl: Duration,
}

impl Lease {
    pub fn new(pool: PgPool, owner: impl Into<String>, ttl: Duration) -> Self {
        Self {
            pool,
            owner: owner.into(),
            ttl,
        }
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Insert or steal an expired lease. `Held` if another live owner holds it.
    pub async fn try_acquire(&self, workspace_id: &str) -> Result<(), LeaseError> {
        let secs = self.ttl.as_secs() as i64;
        let row: Option<(String,)> = sqlx::query_as(
            "INSERT INTO workspace_lease (workspace_id, owner, lease_until)
             VALUES ($1::uuid, $2, now() + make_interval(secs => $3))
             ON CONFLICT (workspace_id) DO UPDATE
               SET owner = EXCLUDED.owner,
                   lease_until = EXCLUDED.lease_until
               WHERE workspace_lease.owner = EXCLUDED.owner
                  OR workspace_lease.lease_until < now()
             RETURNING owner",
        )
        .bind(workspace_id)
        .bind(&self.owner)
        .bind(secs)
        .fetch_optional(&self.pool)
        .await
        .map_err(LeaseError::Store)?;

        match row {
            Some((owner,)) if owner == self.owner => Ok(()),
            Some((other,)) => Err(LeaseError::Held {
                workspace_id: workspace_id.to_string(),
                owner: Some(other),
            }),
            None => Err(LeaseError::Held {
                workspace_id: workspace_id.to_string(),
                owner: None,
            }),
        }
    }

    pub async fn heartbeat(&self, workspace_id: &str) -> Result<bool> {
        let secs = self.ttl.as_secs() as i64;
        let res = sqlx::query(
            "UPDATE workspace_lease
             SET lease_until = now() + make_interval(secs => $3)
             WHERE workspace_id = $1::uuid AND owner = $2",
        )
        .bind(workspace_id)
        .bind(&self.owner)
        .bind(secs)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// One `UPDATE … ANY($2)` for the whole tick (P12). A statement error is
    /// logged once and returns no misses, so no id is mistaken for stolen (L10).
    /// Returns ids whose row was missing or stolen for a later shed (L4).
    pub async fn heartbeat_many(&self, workspace_ids: &[String]) -> Vec<String> {
        if workspace_ids.is_empty() {
            return Vec::new();
        }
        let secs = self.ttl.as_secs() as i64;
        let refreshed = sqlx::query_scalar::<_, String>(
            "UPDATE workspace_lease
             SET lease_until = now() + make_interval(secs => $3)
             WHERE owner = $1 AND workspace_id = ANY($2::uuid[])
             RETURNING workspace_id::text",
        )
        .bind(&self.owner)
        .bind(workspace_ids)
        .bind(secs)
        .fetch_all(&self.pool)
        .await;

        match refreshed {
            Ok(rows) => {
                let refreshed: HashSet<String> = rows.into_iter().collect();
                let mut missed = Vec::new();
                for id in workspace_ids {
                    if refreshed.contains(id) {
                        continue;
                    }
                    tracing::warn!(
                        workspace_id = %id,
                        "lease heartbeat missed (stolen or missing)"
                    );
                    missed.push(id.clone());
                }
                missed
            }
            Err(e) => {
                tracing::warn!(error = %e, "lease heartbeat");
                Vec::new()
            }
        }
    }

    pub async fn drop_one(&self, workspace_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM workspace_lease WHERE workspace_id = $1::uuid AND owner = $2")
            .bind(workspace_id)
            .bind(&self.owner)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn drop_all(&self) -> Result<()> {
        sqlx::query("DELETE FROM workspace_lease WHERE owner = $1")
            .bind(&self.owner)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
