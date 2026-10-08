//! One transaction: the fence check, the body, the log row, the position.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use crate::body::Body;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub lsn: i64,
    pub prev: i64,
    pub prev_fence: i64,
    pub fence: i64,
    pub tag: String,
    pub at: String,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReply {
    pub status: ApplyStatus,
    pub applied_lsn: i64,
    pub applied_fence: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyStatus {
    Ok,
    Already,
    Fenced,
    Gap { at: i64 },
    Divergent,
}

/// SurrealQL for one guarded write. The body is spliced as its own statements.
pub fn script(namespace: &str, database: &str, entry: &Entry) -> String {
    let ns = quote_ident(namespace);
    let db = quote_ident(database);
    let body = entry.body.sql();
    format!(
        r#"
USE NS {ns} DB {db};
BEGIN TRANSACTION;
LET $row = (SELECT * FROM ONLY _repl:state);
LET $sf = $row.fence ?? 0;
LET $sl = $row.applied_lsn ?? 0;
LET $saf = $row.applied_fence ?? 0;
IF $sf > $fence {{
    THROW "fenced";
}};
IF $sl = $lsn AND $saf = $fence {{
    RETURN {{ status: "already", applied_lsn: $sl, applied_fence: $saf }};
}};
IF $sl < $prev {{
    THROW string::concat("gap at ", <string> $sl);
}};
IF $sl > $prev OR $saf != $prev_fence {{
    THROW "divergent";
}};
{body}
UPSERT type::thing("_repl_log", $lsn) CONTENT {{
    fence: $fence,
    tag: $tag,
    body: $logbody,
    at: <datetime> $at
}};
UPDATE _repl:state SET fence = $fence, applied_lsn = $lsn, applied_fence = $fence;
RETURN {{ status: "ok", applied_lsn: $lsn, applied_fence: $fence }};
COMMIT TRANSACTION;
"#
    )
}

pub fn classify(err: &str) -> Option<ApplyStatus> {
    if err.contains("fenced") {
        return Some(ApplyStatus::Fenced);
    }
    if let Some(rest) = err.split("gap at ").nth(1) {
        let num: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        if let Ok(at) = num.parse() {
            return Some(ApplyStatus::Gap { at });
        }
    }
    if err.contains("divergent") {
        return Some(ApplyStatus::Divergent);
    }
    None
}

pub(crate) fn quote_ident(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        name.to_string()
    } else {
        format!("⟨{name}⟩")
    }
}
