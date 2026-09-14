//! Sidecar Postgres pool. Schema lives in hub `schema.sql` (migrate is hub-only).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(database_url)
        .await
        .context("connect postgres")
}
