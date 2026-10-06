//! Apply the graph schema to one process and the search schema to each search node.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use crate::schema::{graph_surql, search_surql};
use crate::WORKSPACE_ID;

/// Graph on `graph_endpoint` (`host:port`). Search schema on each search endpoint.
/// Safe to call again.
pub async fn migrate_surreal(
    graph_endpoint: &str,
    search_endpoints: &[&str],
    user: &str,
    pass: &str,
) -> anyhow::Result<()> {
    apply(graph_endpoint, user, pass, "graph", &graph_surql())
        .await
        .context("migrate graph")?;
    let search = search_surql();
    for endpoint in search_endpoints {
        apply(endpoint, user, pass, "search", &search)
            .await
            .with_context(|| format!("migrate search {endpoint}"))?;
    }
    Ok(())
}

async fn apply(
    endpoint: &str,
    user: &str,
    pass: &str,
    namespace: &str,
    body: &str,
) -> anyhow::Result<()> {
    let db = Surreal::new::<Ws>(endpoint)
        .await
        .with_context(|| format!("connect {endpoint}"))?;
    db.signin(Root {
        username: user,
        password: pass,
    })
    .await
    .context("signin")?;
    db.query(format!("DEFINE NAMESPACE IF NOT EXISTS {namespace}"))
        .await
        .context("define namespace")?
        .check()
        .context("define namespace failed")?;
    db.query(format!(
        "USE NS {namespace}; DEFINE DATABASE IF NOT EXISTS ⟨{WORKSPACE_ID}⟩"
    ))
    .await
    .context("define database")?
    .check()
    .context("define database failed")?;
    db.use_ns(namespace)
        .use_db(WORKSPACE_ID)
        .await
        .context("use database")?;
    db.query(body)
        .await
        .context("define tables")?
        .check()
        .context("define tables failed")?;
    Ok(())
}
