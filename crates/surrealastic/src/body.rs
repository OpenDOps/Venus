//! Deterministic bodies. The layer refuses raw SurrealQL that would not
//! replay to the same rows.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{bail, Context};
use serde_json::{Map, Value};

/// Statements plus the parameters they close over. Built, never hand-written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Body {
    statements: Vec<String>,
    params: Map<String, Value>,
}

impl Body {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(mut self, id: &str, content: Value) -> Self {
        let name = self.bind(content);
        let id = record_id(id);
        self.statements.push(format!("UPSERT {id} CONTENT ${name};"));
        self
    }

    pub fn delete(mut self, id: &str) -> Self {
        let id = record_id(id);
        self.statements.push(format!("DELETE {id};"));
        self
    }

    pub fn delete_range(mut self, table: &str, from: i64, to: i64) -> Self {
        let table = ident(table);
        self.statements
            .push(format!("DELETE {table}:{from}..={to};"));
        self
    }

    pub fn insert_relation(mut self, id: &str, inn: &str, out: &str, content: Value) -> Self {
        let id = record_id(id);
        let inn = record_id(inn);
        let out = record_id(out);
        let name = self.bind(content);
        self.statements
            .push(format!("RELATE {inn}->{id}->{out} CONTENT ${name};"));
        self
    }

    /// Rendered statements. Parameters stay bound, not interpolated.
    pub fn sql(&self) -> String {
        self.statements.join("\n")
    }

    pub fn params(&self) -> &Map<String, Value> {
        &self.params
    }

    /// Object stored in `_repl_log.body`: the statements and their parameters.
    pub fn logged(&self) -> Value {
        serde_json::json!({
            "statements": self.statements,
            "params": self.params,
        })
    }

    /// Rebuild a body that was stored in a log row or an item row.
    pub fn from_logged(value: &Value) -> anyhow::Result<Self> {
        let Some(statements) = value.get("statements").and_then(|v| v.as_array()) else {
            bail!("logged body has no statements");
        };
        let statements = statements
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .context("logged statement is not a string")
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let params = value
            .get("params")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        Ok(Self { statements, params })
    }

    /// One statement that already passes the body rule.
    pub fn statement(mut self, sql: &str) -> anyhow::Result<Self> {
        let sql = sql.trim().trim_end_matches(';').trim();
        if sql.is_empty() {
            bail!("empty statement");
        }
        accept_sql(sql)?;
        self.statements.push(format!("{sql};"));
        Ok(self)
    }

    /// A schema change. Only `DEFINE` and `REMOVE` statements.
    pub fn define(self, sql: &str) -> anyhow::Result<Self> {
        let mut body = self;
        for part in sql.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let head = part.split_whitespace().next().unwrap_or("");
            if !head.eq_ignore_ascii_case("define") && !head.eq_ignore_ascii_case("remove") {
                bail!("define refuses a non-DDL statement");
            }
            body = body.statement(part)?;
        }
        Ok(body)
    }

    /// Splice another body, rebinding its parameters so names do not clash.
    pub fn append(mut self, other: &Body) -> Self {
        let mut map = std::collections::HashMap::new();
        for (key, value) in &other.params {
            let bound = self.bind(value.clone());
            map.insert(key.clone(), bound);
        }
        let mut keys: Vec<_> = map.keys().cloned().collect();
        keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
        for statement in &other.statements {
            let mut rewritten = statement.clone();
            for key in &keys {
                rewritten = rewritten.replace(&format!("${key}"), &format!("${}", map[key]));
            }
            self.statements.push(rewritten);
        }
        self
    }

    pub(crate) fn bind_value(&mut self, value: Value) -> String {
        self.bind(value)
    }

    fn bind(&mut self, value: Value) -> String {
        let name = format!("p{}", self.params.len());
        self.params.insert(name.clone(), value);
        name
    }
}

/// Refuse a body that would not give the same result on every copy.
/// Builder output passes. Nothing is sent when this returns an error.
pub fn accept_sql(sql: &str) -> anyhow::Result<()> {
    let lower = sql.to_ascii_lowercase();
    if lower.contains("rand::") {
        bail!("body refuses rand::");
    }
    if lower.contains("time::now") {
        bail!("body refuses time::now");
    }
    if sql.contains("+=") {
        bail!("body refuses +=");
    }
    if sql.contains("-=") {
        bail!("body refuses -=");
    }
    if create_without_id(sql) {
        bail!("body refuses CREATE without an id");
    }
    Ok(())
}

fn create_without_id(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(pos) = lower[i..].find("create") {
        let abs = i + pos;
        let before_ok = abs == 0 || !bytes[abs - 1].is_ascii_alphanumeric();
        let after = abs + "create".len();
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            let rest = sql[after..].trim_start();
            let token = rest
                .split(|c: char| c.is_whitespace() || c == ';' || c == '(' || c == ',')
                .find(|t| !t.is_empty())
                .unwrap_or("");
            if !token.contains(':') {
                return true;
            }
        }
        i = after.max(i + 1);
    }
    false
}

fn record_id(id: &str) -> String {
    assert!(
        is_record_id(id),
        "record id must be table:key, got {id}"
    );
    id.to_string()
}

fn ident(name: &str) -> String {
    assert!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "table name {name}"
    );
    name.to_string()
}

fn is_record_id(id: &str) -> bool {
    let Some((table, key)) = id.split_once(':') else {
        return false;
    };
    !table.is_empty()
        && !key.is_empty()
        && table
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && table.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
}
