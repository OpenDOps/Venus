//! `venus-graph serve` migrates, then holds `writer:{ws}` for each workspace
//! in `VENUS_GRAPH_WORKSPACES`. Postgres is `DATABASE_URL`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;

use anyhow::Context;
use surrealastic::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("serve") {
        anyhow::bail!("usage: venus-graph serve");
    }
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL is unset")?;
    let workspaces =
        env::var("VENUS_GRAPH_WORKSPACES").context("VENUS_GRAPH_WORKSPACES is unset")?;
    let list: Vec<String> = workspaces
        .split(',')
        .map(str::trim)
        .filter(|ws| !ws.is_empty())
        .map(str::to_string)
        .collect();
    let holder = format!("venus-graph-{}", std::process::id());
    let held = venus_graph::serve(&database_url, &list, &holder, Config::from_env()).await?;
    if held.iter().all(Option::is_none) {
        return Ok(());
    }
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("graph_jobs postgres")?;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                for (ws, layout) in list.iter().zip(held.iter()) {
                    if layout.is_none() {
                        continue;
                    }
                    match venus_graph::claim_job(&pool, ws, &holder).await {
                        Ok(Some(job)) => match venus_graph::record_sha(&pool, &job, &holder).await {
                            Ok(sha) => eprintln!("graph_jobs {ws} {sha}"),
                            Err(err) => eprintln!("graph_jobs record {ws}: {err:#}"),
                        },
                        Ok(None) => {}
                        Err(err) => eprintln!("graph_jobs claim {ws}: {err:#}"),
                    }
                }
            }
        }
    }
    Ok(())
}
