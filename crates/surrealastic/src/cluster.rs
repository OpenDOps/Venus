//! Process-local connections and the map. Postgres is not on the per-entry path.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::link::Conn;
use crate::place::Health;
use crate::store::{self, CopyRow, SetRow};
use crate::writer::Writer;

#[derive(Clone)]
pub struct Cluster {
    inner: Arc<Inner>,
}

struct Inner {
    pool: PgPool,
    config: Config,
    conns: Mutex<HashMap<String, Arc<Conn>>>,
}

impl Cluster {
    pub async fn connect(database_url: &str, config: Config) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .context("connect postgres")?;
        store::migrate(&pool).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                pool,
                config,
                conns: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    pub async fn conn(&self, node_id: &str, url: &str) -> anyhow::Result<Arc<Conn>> {
        crate::link::cached(&self.inner.conns, node_id, url, &self.inner.config).await
    }

    pub async fn disconnect(&self, node_id: &str) {
        self.inner.conns.lock().await.remove(node_id);
    }

    pub async fn upsert_node(
        &self,
        node_id: &str,
        pool: &str,
        url: &str,
        zone: &str,
        weight: i32,
        state: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO repl_node (node_id, pool, url, zone, weight, state)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (node_id) DO UPDATE
               SET pool = EXCLUDED.pool, url = EXCLUDED.url, zone = EXCLUDED.zone,
                   weight = EXCLUDED.weight, state = EXCLUDED.state, state_at = now()",
        )
        .bind(node_id)
        .bind(pool)
        .bind(url)
        .bind(zone)
        .bind(weight)
        .bind(state)
        .execute(self.pool())
        .await
        .context("upsert node")?;
        Ok(())
    }

    pub async fn upsert_layout(&self, row: &crate::store::LayoutRow) -> anyhow::Result<()> {
        store::upsert_layout(self.pool(), row).await
    }

    pub async fn upsert_set(&self, set: &SetRow) -> anyhow::Result<()> {
        store::ensure_lease(self.pool(), &set.lease).await?;
        sqlx::query(
            "INSERT INTO repl_set (set_id, pool, namespace, database, copies, ack, lease)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (set_id) DO UPDATE
               SET pool = EXCLUDED.pool, namespace = EXCLUDED.namespace,
                   database = EXCLUDED.database, copies = EXCLUDED.copies,
                   ack = EXCLUDED.ack, lease = EXCLUDED.lease",
        )
        .bind(&set.set_id)
        .bind(&set.pool)
        .bind(&set.namespace)
        .bind(&set.database)
        .bind(set.copies)
        .bind(set.ack)
        .bind(&set.lease)
        .execute(self.pool())
        .await
        .context("upsert set")?;
        Ok(())
    }

    pub async fn upsert_copy(
        &self,
        set_id: &str,
        node_id: &str,
        state: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO repl_copy (set_id, node_id, state, applied_lsn)
             VALUES ($1, $2, $3, 0)
             ON CONFLICT (set_id, node_id) DO UPDATE SET state = EXCLUDED.state",
        )
        .bind(set_id)
        .bind(node_id)
        .bind(state)
        .execute(self.pool())
        .await
        .context("upsert copy")?;
        Ok(())
    }

    pub async fn ensure(&self, set_id: &str) -> anyhow::Result<()> {
        let set = store::load_set(self.pool(), set_id).await?;
        let copies = store::load_copies(self.pool(), set_id).await?;
        for copy in copies {
            let conn = self.conn(&copy.node_id, &copy.url).await?;
            crate::link::ensure(&conn, &set.namespace, &set.database).await?;
        }
        Ok(())
    }

    /// Claim `set`'s lease. `None` means another holder still has it.
    pub async fn claim(&self, set_id: &str, holder: &str) -> anyhow::Result<Option<Writer>> {
        let set = store::load_set(self.pool(), set_id).await?;
        store::ensure_lease(self.pool(), &set.lease).await?;
        let secs = self.config().lease.as_secs_f64();
        let Some((fence, until)) =
            store::claim_lease(self.pool(), &set.lease, holder, secs).await?
        else {
            return Ok(None);
        };
        let writer = Writer::start(self.clone(), set_id, holder, fence, until).await?;
        Ok(Some(writer))
    }

    pub async fn health(&self, set_id: &str) -> anyhow::Result<Health> {
        let set = store::load_set(self.pool(), set_id).await?;
        let copies = store::load_copies(self.pool(), set_id).await?;
        Ok(store::health_of(&set, &copies))
    }

    pub async fn copies(&self, set_id: &str) -> anyhow::Result<Vec<CopyRow>> {
        store::load_copies(self.pool(), set_id).await
    }

    pub async fn fingerprint(&self, set_id: &str) -> anyhow::Result<Vec<String>> {
        store::copy_fingerprint(self.pool(), set_id).await
    }

    pub async fn expire_lease(&self, name: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE repl_lease SET lease_until = now() - interval '1 second' WHERE name = $1",
        )
        .bind(name)
        .execute(self.pool())
        .await
        .context("expire lease")?;
        Ok(())
    }

    /// Read from an in-sync copy. `min_lsn` fails that copy over when it is behind.
    pub async fn read(
        &self,
        set_id: &str,
        query: &str,
        min_lsn: Option<i64>,
    ) -> anyhow::Result<String> {
        let set = store::load_set(self.pool(), set_id).await?;
        let copies = store::load_copies(self.pool(), set_id).await?;
        let mut last = None;
        for copy in copies
            .iter()
            .filter(|c| c.state == "in_sync" && c.node_state != "down")
        {
            let conn = match self.conn(&copy.node_id, &copy.url).await {
                Ok(conn) => conn,
                Err(err) => {
                    last = Some(err);
                    continue;
                }
            };
            let guard = match min_lsn {
                Some(lsn) => format!(
                    "IF (SELECT VALUE applied_lsn FROM ONLY _repl:state) < {lsn} {{ THROW \"behind\"; }};"
                ),
                None => String::new(),
            };
            match crate::link::raw(
                &conn,
                &set.namespace,
                &set.database,
                &format!("{guard} {query}"),
            )
            .await
            {
                Ok(text) if !text.contains("behind") => return Ok(text),
                Ok(text) => last = Some(anyhow::anyhow!(text)),
                Err(err) => last = Some(err),
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("no in-sync copy")))
    }
}

/// Open a pool, create the layer tables, and close it.
pub async fn migrate(database_url: &str) -> anyhow::Result<()> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect postgres")?;
    store::migrate(&pool).await
}
