//! One WebSocket per node. Writes go through the guarded script only.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use surrealdb::engine::remote::ws::{Client, Ws, Wss};
use surrealdb::opt::auth::Root;
use surrealdb::opt::Config as SdConfig;
use surrealdb::Surreal;
use tokio::sync::Mutex;

use crate::body::accept_sql;
use crate::config::Config;
use crate::guard::{classify, script, ApplyReply, ApplyStatus, Entry};

pub struct Conn {
    db: Surreal<Client>,
    gate: Mutex<()>,
}

pub async fn dial(url: &str, config: &Config) -> anyhow::Result<Surreal<Client>> {
    let sd = SdConfig::new().query_timeout(Duration::from_secs(8));
    let db = if let Some(rest) = url.strip_prefix("wss://") {
        let addr = rest.trim_end_matches('/');
        let tls = tls_config(&config.tls_ca)?;
        let sd = sd.rustls(tls);
        Surreal::new::<Wss>((addr, sd))
            .await
            .with_context(|| format!("wss {addr}"))?
    } else {
        let addr = url
            .trim_start_matches("ws://")
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/');
        Surreal::new::<Ws>((addr, sd))
            .await
            .with_context(|| format!("ws {addr}"))?
    };
    db.signin(Root {
        username: &config.surreal_user,
        password: &config.surreal_pass,
    })
    .await
    .context("signin")?;
    Ok(db)
}

fn tls_config(ca_path: &str) -> anyhow::Result<rustls::ClientConfig> {
    let pem = std::fs::read_to_string(ca_path).with_context(|| format!("read CA {ca_path}"))?;
    let mut roots = rustls::RootCertStore::empty();
    let mut reader = std::io::BufReader::new(pem.as_bytes());
    for cert in rustls_pemfile::certs(&mut reader) {
        roots.add(cert.context("parse CA")?).context("add CA")?;
    }
    let provider = rustls::crypto::ring::default_provider();
    Ok(rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .context("tls versions")?
        .with_root_certificates(roots)
        .with_no_client_auth())
}

/// Connect with an explicit PEM (the TLS test). `None` uses web PKI roots only.
pub async fn dial_wss(addr: &str, ca_pem: Option<&str>, user: &str, pass: &str) -> anyhow::Result<()> {
    let sd = if let Some(pem) = ca_pem {
        let mut roots = rustls::RootCertStore::empty();
        let mut reader = std::io::BufReader::new(pem.as_bytes());
        for cert in rustls_pemfile::certs(&mut reader) {
            roots.add(cert.context("parse CA")?).context("add CA")?;
        }
        let provider = rustls::crypto::ring::default_provider();
        let tls = rustls::ClientConfig::builder_with_provider(provider.into())
            .with_safe_default_protocol_versions()
            .context("tls versions")?
            .with_root_certificates(roots)
            .with_no_client_auth();
        SdConfig::new().rustls(tls)
    } else {
        SdConfig::new()
    };
    let db = Surreal::new::<Wss>((addr, sd))
        .await
        .with_context(|| format!("wss {addr}"))?;
    db.signin(Root {
        username: user,
        password: pass,
    })
    .await
    .context("signin")?;
    Ok(())
}

pub async fn cached(map: &Mutex<HashMap<String, Arc<Conn>>>, node_id: &str, url: &str, config: &Config) -> anyhow::Result<Arc<Conn>> {
    let mut guard = map.lock().await;
    if let Some(conn) = guard.get(node_id) {
        return Ok(Arc::clone(conn));
    }
    let conn = Arc::new(Conn {
        db: dial(url, config).await?,
        gate: Mutex::new(()),
    });
    guard.insert(node_id.to_string(), Arc::clone(&conn));
    Ok(conn)
}

pub async fn ensure(conn: &Conn, namespace: &str, database: &str) -> anyhow::Result<()> {
    let ns = namespace.to_string();
    let db = database.to_string();
    let sql = format!(
        r#"
DEFINE NAMESPACE IF NOT EXISTS {ns};
USE NS {ns};
DEFINE DATABASE IF NOT EXISTS {db};
USE NS {ns} DB {db};
DEFINE TABLE IF NOT EXISTS _repl SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS fence ON _repl TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS applied_lsn ON _repl TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS applied_fence ON _repl TYPE int DEFAULT 0;
DEFINE TABLE IF NOT EXISTS _repl_log SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS fence ON _repl_log TYPE int;
DEFINE FIELD IF NOT EXISTS tag ON _repl_log TYPE string;
DEFINE FIELD IF NOT EXISTS body ON _repl_log TYPE object;
DEFINE FIELD IF NOT EXISTS at ON _repl_log TYPE datetime;
UPSERT _repl:state SET fence = fence ?? 0, applied_lsn = applied_lsn ?? 0, applied_fence = applied_fence ?? 0;
"#
    );
    let _gate = conn.gate.lock().await;
    conn.db
        .query(sql)
        .await
        .context("ensure schema")?
        .check()
        .context("ensure schema failed")?;
    Ok(())
}

pub async fn apply(conn: &Conn, namespace: &str, database: &str, entry: &Entry) -> anyhow::Result<ApplyReply> {
    accept_sql(&entry.body.sql())?;
    let sql = script(namespace, database, entry);
    let _gate = conn.gate.lock().await;
    let mut query = conn
        .db
        .query(sql)
        .bind(("lsn", entry.lsn))
        .bind(("prev", entry.prev))
        .bind(("prev_fence", entry.prev_fence))
        .bind(("fence", entry.fence))
        .bind(("tag", entry.tag.clone()))
        .bind(("at", entry.at.clone()))
        .bind(("logbody", entry.body.logged()));
    for (key, value) in entry.body.params() {
        query = query.bind((key.clone(), value.clone()));
    }
    match query.await {
        Ok(mut response) => parse_reply(&mut response),
        Err(err) => match classify(&error_text(&err)) {
            Some(status) => Ok(ApplyReply {
                applied_lsn: gap_at(&status),
                applied_fence: 0,
                status,
            }),
            None => Err(err).context("guarded write"),
        },
    }
}

fn gap_at(status: &ApplyStatus) -> i64 {
    match status {
        ApplyStatus::Gap { at } => *at,
        _ => 0,
    }
}

fn parse_reply(response: &mut surrealdb::Response) -> anyhow::Result<ApplyReply> {
    let mut fallback = String::new();
    let mut last_err: Option<surrealdb::Error> = None;
    for index in 0usize..24 {
        match response.take::<surrealdb::Value>(index) {
            Ok(value) => {
                let text = value.to_string();
                if text == "NONE" {
                    continue;
                }
                fallback.push_str(&text);
                fallback.push('\n');
                if let Some(reply) = reply_from_text(&text) {
                    return Ok(reply);
                }
            }
            Err(err) => {
                let text = error_text(&err);
                fallback.push_str(&text);
                fallback.push('\n');
                if let Some(status) = classify(&text) {
                    return Ok(ApplyReply {
                        applied_lsn: gap_at(&status),
                        applied_fence: 0,
                        status,
                    });
                }
                last_err = Some(err);
            }
        }
    }
    if let Some(err) = last_err {
        return Err(err).context(format!("guarded write returned no status: {fallback}"));
    }
    anyhow::bail!("guarded write returned no status: {fallback}")
}

fn error_text(err: &impl std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push(' ');
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn reply_from_text(text: &str) -> Option<ApplyReply> {
    let status = status_word(text)?;
    Some(ApplyReply {
        status: if status == "already" {
            ApplyStatus::Already
        } else {
            ApplyStatus::Ok
        },
        applied_lsn: field_i64(text, "applied_lsn").unwrap_or(0),
        applied_fence: field_i64(text, "applied_fence").unwrap_or(0),
    })
}

fn status_word(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    let at = lower.find("status")?;
    let rest = text[at + "status".len()..].trim_start_matches(|c: char| {
        c == ':' || c == ' ' || c == '"' || c == '\'' || c == '='
    });
    let rest = rest.to_ascii_lowercase();
    if rest.starts_with("already") {
        Some("already")
    } else if rest.starts_with("ok") {
        Some("ok")
    } else {
        None
    }
}

pub async fn log_ids(conn: &Conn, namespace: &str, database: &str, from: i64, to: i64) -> anyhow::Result<Vec<i64>> {
    let sql = format!(
        "USE NS {namespace} DB {database}; SELECT id FROM _repl_log:{from}..={to};"
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("log range")?;
    Ok(record_ids(&response_text(&mut response)))
}

pub async fn explain_log(conn: &Conn, namespace: &str, database: &str, from: i64, to: i64) -> anyhow::Result<String> {
    let sql = format!(
        "USE NS {namespace} DB {database}; SELECT * FROM _repl_log:{from}..={to} EXPLAIN;"
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("explain")?;
    Ok(response_text(&mut response))
}

pub async fn state_of(conn: &Conn, namespace: &str, database: &str) -> anyhow::Result<(i64, i64)> {
    let sql = format!(
        "USE NS {namespace} DB {database}; SELECT applied_lsn, applied_fence FROM ONLY _repl:state;"
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("read state")?;
    let text = response_text(&mut response);
    Ok((
        field_i64(&text, "applied_lsn").unwrap_or(0),
        field_i64(&text, "applied_fence").unwrap_or(0),
    ))
}

pub async fn raw(conn: &Conn, namespace: &str, database: &str, sql: &str) -> anyhow::Result<String> {
    let script = format!("USE NS {namespace} DB {database}; {sql}");
    let _gate = conn.gate.lock().await;
    let mut response = conn
        .db
        .query(script)
        .await
        .with_context(|| format!("raw {sql}"))?;
    Ok(response_text(&mut response))
}

fn response_text(response: &mut surrealdb::Response) -> String {
    let mut text = String::new();
    for index in 0usize..16 {
        let Ok(value) = response.take::<surrealdb::Value>(index) else {
            continue;
        };
        let piece = value.to_string();
        if piece == "NONE" || piece.is_empty() {
            continue;
        }
        text.push_str(&piece);
        text.push('\n');
    }
    text
}

fn record_ids(text: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    for (at, _) in text.match_indices("_repl_log:") {
        let rest = &text[at + "_repl_log:".len()..];
        let num: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        if let Ok(n) = num.parse() {
            ids.push(n);
        }
    }
    ids
}

fn field_i64(text: &str, field: &str) -> Option<i64> {
    let rest = text.split(field).nth(1)?;
    let num: String = rest
        .chars()
        .skip_while(|c| !c.is_ascii_digit() && *c != '-')
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    num.parse().ok()
}
