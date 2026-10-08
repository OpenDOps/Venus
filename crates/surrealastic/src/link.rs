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
pub async fn dial_wss(
    addr: &str,
    ca_pem: Option<&str>,
    user: &str,
    pass: &str,
) -> anyhow::Result<()> {
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

pub async fn cached(
    map: &Mutex<HashMap<String, Arc<Conn>>>,
    node_id: &str,
    url: &str,
    config: &Config,
) -> anyhow::Result<Arc<Conn>> {
    if let Some(conn) = map.lock().await.get(node_id) {
        return Ok(Arc::clone(conn));
    }
    let conn = Arc::new(Conn {
        db: dial(url, config).await?,
        gate: Mutex::new(()),
    });
    let mut guard = map.lock().await;
    if let Some(existing) = guard.get(node_id) {
        return Ok(Arc::clone(existing));
    }
    guard.insert(node_id.to_string(), Arc::clone(&conn));
    Ok(conn)
}

fn quoted(name: &str) -> String {
    crate::guard::quote_ident(name)
}

pub async fn ensure(conn: &Conn, namespace: &str, database: &str) -> anyhow::Result<()> {
    let ns = quoted(namespace);
    let db = quoted(database);
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
DEFINE FIELD IF NOT EXISTS body ON _repl_log FLEXIBLE TYPE object;
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

pub async fn apply(
    conn: &Conn,
    namespace: &str,
    database: &str,
    entry: &Entry,
) -> anyhow::Result<ApplyReply> {
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
    let mut real_err: Option<surrealdb::Error> = None;
    let mut missed = 0_u32;
    for index in 0usize..4096 {
        match response.take::<surrealdb::Value>(index) {
            Ok(value) => {
                missed = 0;
                let text = value.to_string();
                if text == "NONE" {
                    continue;
                }
                if let Some(reply) = reply_from_text(&text) {
                    return Ok(reply);
                }
            }
            Err(err) => {
                let text = error_text(&err);
                if text.contains("Query index") || text.contains("out of bounds") {
                    break;
                }
                if text.contains("not executed") {
                    continue;
                }
                if let Some(status) = classify(&text) {
                    return Ok(ApplyReply {
                        applied_lsn: gap_at(&status),
                        applied_fence: 0,
                        status,
                    });
                }
                missed = missed.saturating_add(1);
                if real_err.is_none() && missed < 4 {
                    fallback = text;
                    real_err = Some(err);
                }
                if missed >= 4 {
                    break;
                }
            }
        }
    }
    if let Some(err) = real_err {
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
    let rest = text[at + "status".len()..]
        .trim_start_matches(|c: char| c == ':' || c == ' ' || c == '"' || c == '\'' || c == '=');
    let rest = rest.to_ascii_lowercase();
    if rest.starts_with("already") {
        Some("already")
    } else if rest.starts_with("ok") {
        Some("ok")
    } else {
        None
    }
}

pub async fn log_ids(
    conn: &Conn,
    namespace: &str,
    database: &str,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<i64>> {
    let sql = format!(
        "USE NS {} DB {}; SELECT id FROM _repl_log:{from}..={to};",
        quoted(namespace),
        quoted(database)
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("log range")?;
    Ok(record_ids(&response_text(&mut response)))
}

pub async fn explain_log(
    conn: &Conn,
    namespace: &str,
    database: &str,
    from: i64,
    to: i64,
) -> anyhow::Result<String> {
    let sql = format!(
        "USE NS {} DB {}; SELECT * FROM _repl_log:{from}..={to} EXPLAIN;",
        quoted(namespace),
        quoted(database)
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("explain")?;
    Ok(response_text(&mut response))
}

pub async fn state_of(conn: &Conn, namespace: &str, database: &str) -> anyhow::Result<(i64, i64)> {
    let sql = format!(
        "USE NS {} DB {}; SELECT applied_lsn, applied_fence FROM ONLY _repl:state;",
        quoted(namespace),
        quoted(database)
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn.db.query(sql).await.context("read state")?;
    let text = response_text(&mut response);
    Ok((
        field_i64(&text, "applied_lsn").unwrap_or(0),
        field_i64(&text, "applied_fence").unwrap_or(0),
    ))
}

pub async fn query_json(
    conn: &Conn,
    namespace: &str,
    database: &str,
    sql: &str,
) -> anyhow::Result<serde_json::Value> {
    let script = format!(
        "USE NS {} DB {}; {sql}",
        quoted(namespace),
        quoted(database)
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn
        .db
        .query(script)
        .await
        .with_context(|| format!("query {sql}"))?;
    for index in 0usize..16 {
        let Ok(value) = response.take::<surrealdb::Value>(index) else {
            continue;
        };
        let text = value.to_string();
        if text == "NONE" {
            continue;
        }
        if let Ok(json) = surrealdb::value::from_value::<serde_json::Value>(value.clone()) {
            return Ok(json);
        }
        if let Some(parsed) = surreal_display_json(&text) {
            return Ok(parsed);
        }
        return Ok(serde_json::Value::String(text));
    }
    Ok(serde_json::Value::Null)
}

/// Surreal's `Display` is not JSON: `{ id: row:1, n: 1 }`. Record ids become strings.
pub(crate) fn surreal_display_json(text: &str) -> Option<serde_json::Value> {
    let mut parser = Disp { s: text, i: 0 };
    let value = parser.parse_value()?;
    parser.skip();
    if parser.i != parser.s.len() {
        return None;
    }
    Some(value)
}

struct Disp<'a> {
    s: &'a str,
    i: usize,
}

impl Disp<'_> {
    fn skip(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if !c.is_whitespace() {
                break;
            }
            self.i += c.len_utf8();
        }
    }

    fn rest(&self) -> &str {
        &self.s[self.i..]
    }

    fn peek(&mut self) -> Option<char> {
        self.skip();
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.rest().chars().next()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, want: char) -> bool {
        if self.peek() == Some(want) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn parse_value(&mut self) -> Option<serde_json::Value> {
        match self.peek()? {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => self.parse_string('"').map(serde_json::Value::String),
            '\'' => self.parse_string('\'').map(serde_json::Value::String),
            '-' | '0'..='9' => self.parse_number(),
            _ => self.parse_word(),
        }
    }

    fn parse_object(&mut self) -> Option<serde_json::Value> {
        if !self.eat('{') {
            return None;
        }
        let mut map = serde_json::Map::new();
        loop {
            if self.eat('}') {
                break;
            }
            let key = if matches!(self.peek(), Some('"' | '\'')) {
                let quote = self.peek()?;
                self.parse_string(quote)?
            } else {
                self.ident()?
            };
            if !self.eat(':') {
                return None;
            }
            let value = self.parse_value()?;
            map.insert(key, value);
            if self.eat(',') {
                continue;
            }
            if !self.eat('}') {
                return None;
            }
            break;
        }
        Some(serde_json::Value::Object(map))
    }

    fn parse_array(&mut self) -> Option<serde_json::Value> {
        if !self.eat('[') {
            return None;
        }
        let mut items = Vec::new();
        loop {
            if self.eat(']') {
                break;
            }
            items.push(self.parse_value()?);
            if self.eat(',') {
                continue;
            }
            if !self.eat(']') {
                return None;
            }
            break;
        }
        Some(serde_json::Value::Array(items))
    }

    fn parse_string(&mut self, quote: char) -> Option<String> {
        if !self.eat(quote) {
            return None;
        }
        let mut out = String::new();
        loop {
            let c = self.bump()?;
            if c == quote {
                break;
            }
            if c == '\\' {
                match self.bump()? {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    other => out.push(other),
                }
            } else {
                out.push(c);
            }
        }
        Some(out)
    }

    fn parse_number(&mut self) -> Option<serde_json::Value> {
        self.skip();
        let start = self.i;
        if self.rest().starts_with('-') {
            self.i += 1;
        }
        let digits = self
            .rest()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits == 0 {
            return None;
        }
        self.i += digits;
        if self.rest().starts_with('.') {
            let frac = self.rest()[1..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if frac > 0 {
                self.i += 1 + frac;
            }
        }
        let text = &self.s[start..self.i];
        let number = if let Ok(n) = text.parse::<i64>() {
            serde_json::Number::from(n)
        } else {
            serde_json::Number::from_f64(text.parse().ok()?)?
        };
        Some(serde_json::Value::Number(number))
    }

    fn parse_word(&mut self) -> Option<serde_json::Value> {
        let word = self.ident()?;
        match word.as_str() {
            "true" => return Some(serde_json::Value::Bool(true)),
            "false" => return Some(serde_json::Value::Bool(false)),
            "none" | "null" | "NONE" | "NULL" => return Some(serde_json::Value::Null),
            _ => {}
        }
        if matches!(word.as_str(), "d" | "u") && matches!(self.peek(), Some('\'' | '"')) {
            let quote = self.peek()?;
            return self.parse_string(quote).map(serde_json::Value::String);
        }
        if self.peek() == Some(':') {
            self.bump();
            let id = self.record_key()?;
            return Some(serde_json::Value::String(format!("{word}:{id}")));
        }
        Some(serde_json::Value::String(word))
    }

    fn record_key(&mut self) -> Option<String> {
        match self.peek()? {
            '"' => self.parse_string('"'),
            '\'' => self.parse_string('\''),
            '⟨' => {
                self.bump();
                let start = self.i;
                while self.rest().chars().next()? != '⟩' {
                    self.bump();
                }
                let key = self.s[start..self.i].to_string();
                self.bump();
                Some(key)
            }
            '-' | '0'..='9' => match self.parse_number()? {
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            },
            _ => self.ident(),
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.skip();
        let mut out = String::new();
        while let Some(c) = self.rest().chars().next() {
            if c.is_ascii_alphanumeric() || c == '_' {
                out.push(c);
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }
}

/// Run a script that already names its namespace. Used to remove a database.
pub async fn exec(conn: &Conn, sql: &str) -> anyhow::Result<String> {
    let _gate = conn.gate.lock().await;
    let mut response = conn
        .db
        .query(sql)
        .await
        .with_context(|| format!("script {sql}"))?;
    Ok(response_text(&mut response))
}

pub async fn raw(
    conn: &Conn,
    namespace: &str,
    database: &str,
    sql: &str,
) -> anyhow::Result<String> {
    let script = format!(
        "USE NS {} DB {}; {sql}",
        quoted(namespace),
        quoted(database)
    );
    let _gate = conn.gate.lock().await;
    let mut response = conn
        .db
        .query(script)
        .await
        .with_context(|| format!("raw {namespace} {database}"))?
        .check()
        .with_context(|| format!("raw {namespace} {database}"))?;
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

#[cfg(test)]
mod tests {
    use super::surreal_display_json;

    #[test]
    fn record_ids_become_json_strings() {
        let parsed = surreal_display_json("[{ id: row:1, n: 1 }]").unwrap();
        assert_eq!(parsed, serde_json::json!([{"id": "row:1", "n": 1}]));
    }

    #[test]
    fn a_logged_body_round_trips() {
        let text = r#"{ statements: ["UPSERT row:1 SET n = $p0;"], params: { p0: "a" }, at: d'2026-10-08T00:00:00Z' }"#;
        let parsed = surreal_display_json(text).unwrap();
        assert_eq!(parsed["statements"][0], "UPSERT row:1 SET n = $p0;");
        assert_eq!(parsed["params"]["p0"], "a");
        assert_eq!(parsed["at"], "2026-10-08T00:00:00Z");
    }
}
