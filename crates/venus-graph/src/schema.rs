//! SurrealQL for namespace `graph` and namespace `search`.
//! The graph script refuses a `SEARCH` index. The search script does not
//! create `links_to` or namespace `graph`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use crate::WORKSPACE_ID;

const EDGE_SOURCE: &str =
    "string ASSERT $value IN ['outline', 'link', 'glossary', 'symbol', 'extract', 'model']";

/// Graph namespace script. No full-text index.
pub fn graph_surql() -> String {
    let sql = render_graph();
    accept_graph_schema(&sql).expect("graph schema");
    sql
}

/// Search namespace script. Indexes only. No graph edges.
pub fn search_surql() -> String {
    let sql = render_search();
    accept_search_schema(&sql).expect("search schema");
    sql
}

/// Reject a graph script that defines a full-text index.
pub fn accept_graph_schema(sql: &str) -> Result<(), &'static str> {
    if sql
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "SEARCH")
    {
        return Err("graph schema refuses a SEARCH index");
    }
    Ok(())
}

/// Reject a search script that creates graph edges or the graph namespace.
pub fn accept_search_schema(sql: &str) -> Result<(), &'static str> {
    if sql.contains("links_to") {
        return Err("search schema does not create links_to");
    }
    if sql.contains("NAMESPACE graph") {
        return Err("search schema does not create namespace graph");
    }
    Ok(())
}

fn render_graph() -> String {
    let mut sql = String::new();
    table(
        &mut sql,
        "graph_meta",
        &[
            ("schema", "int"),
            ("glossary_id", "option<string>"),
            ("ner", "bool DEFAULT false"),
            ("wiki_sha", "option<string>"),
        ],
    );
    table(
        &mut sql,
        "page",
        &[
            ("git_path", "string"),
            ("title", "string"),
            ("indexed_sha", "string"),
            ("search_sha", "option<string>"),
            ("search_error", "option<string>"),
            ("pass", "int"),
            ("gist", "option<string>"),
            ("index_error", "option<string>"),
            ("semantic_error", "option<string>"),
            ("incremental_count", "int DEFAULT 0"),
        ],
    );
    table(
        &mut sql,
        "heading",
        &[
            ("doc_id", "string"),
            ("block_id", "string"),
            ("level", "int"),
            ("text", "string"),
            ("body", "string"),
            ("body_hash", "string"),
            ("gist", "option<string>"),
            ("indexed_sha", "string"),
        ],
    );
    table(
        &mut sql,
        "mention",
        &[
            ("doc_id", "string"),
            ("block_id", "string"),
            ("kind", "string"),
            ("text", "string"),
            ("norm", "string"),
            ("start", "int"),
            ("end", "int"),
            ("pass", "int"),
            ("indexed_sha", "string"),
        ],
    );
    table(
        &mut sql,
        "term",
        &[
            ("name", "string"),
            ("doc_id", "string"),
            ("block_id", "string"),
        ],
    );
    table(
        &mut sql,
        "symbol",
        &[
            ("doc_id", "string"),
            ("lang", "string"),
            ("kind", "string"),
            ("name", "string"),
        ],
    );

    for (name, inn, out, logical) in [
        ("contains", "page | heading", "page | heading", false),
        ("links_to", "page | heading", "page | heading", false),
        ("transcludes", "heading", "page", false),
        ("same_page", "heading", "heading", false),
        ("mentions", "heading", "mention", false),
        ("of", "mention", "term | symbol", false),
        ("defines", "heading", "heading | term | symbol", true),
        ("depends_on", "heading", "heading", true),
        ("constrains", "heading", "heading", true),
        ("contradicts", "heading", "heading", true),
        ("supersedes", "heading", "heading", true),
    ] {
        sql.push_str(&format!(
            "DEFINE TABLE IF NOT EXISTS {name} TYPE RELATION IN {inn} OUT {out} SCHEMAFULL;\n"
        ));
        field(&mut sql, name, "source", EDGE_SOURCE);
        field(&mut sql, name, "from_doc", "string");
        field(&mut sql, name, "indexed_sha", "string");
        if name == "links_to" {
            field(&mut sql, name, "resolved", "bool");
            field(&mut sql, name, "href", "option<string>");
            field(&mut sql, name, "via", "string");
        }
        if logical {
            field(&mut sql, name, "quote", "option<string>");
            field(&mut sql, name, "confidence", "option<float>");
        }
    }
    let _ = WORKSPACE_ID;
    sql
}

fn render_search() -> String {
    let mut sql = String::new();
    table(
        &mut sql,
        "page",
        &[
            ("git_path", "string"),
            ("title", "string"),
            ("indexed_sha", "string"),
        ],
    );
    table(
        &mut sql,
        "heading",
        &[
            ("doc_id", "string"),
            ("block_id", "string"),
            ("level", "int"),
            ("text", "string"),
            ("body", "string"),
            ("gist", "option<string>"),
            ("indexed_sha", "string"),
        ],
    );
    table(
        &mut sql,
        "mention",
        &[
            ("doc_id", "string"),
            ("block_id", "string"),
            ("kind", "string"),
            ("text", "string"),
            ("norm", "string"),
            ("indexed_sha", "string"),
        ],
    );
    sql.push_str("DEFINE ANALYZER IF NOT EXISTS wiki TOKENIZERS class FILTERS lowercase, ascii;\n");
    sql.push_str(
        "DEFINE INDEX IF NOT EXISTS heading_text ON heading FIELDS text, body SEARCH ANALYZER wiki BM25;\n",
    );
    sql.push_str(
        "DEFINE INDEX IF NOT EXISTS mention_text ON mention FIELDS text, norm SEARCH ANALYZER wiki BM25;\n",
    );
    sql
}

fn table(sql: &mut String, name: &str, fields: &[(&str, &str)]) {
    sql.push_str(&format!("DEFINE TABLE IF NOT EXISTS {name} SCHEMAFULL;\n"));
    for (field_name, ty) in fields {
        field(sql, name, field_name, ty);
    }
}

fn field(sql: &mut String, table: &str, name: &str, ty: &str) {
    sql.push_str(&format!(
        "DEFINE FIELD IF NOT EXISTS {name} ON {table} TYPE {ty};\n"
    ));
}
