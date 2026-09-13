//! Load BlockSuite y-blocks from a y-octo-hydrated pin.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use y_octo::{Any, Array, Doc, Map, Text, TextDeltaOp, TextInsert, Value};

pub const RANGE_FLAVOURS: &[&str] = &[
    "affine:paragraph",
    "affine:list",
    "affine:code",
    "affine:divider",
    "affine:image",
    "affine:embed-linked-doc",
];

#[derive(Debug, Clone)]
pub struct Delta {
    pub insert: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub underline: bool,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bulleted,
    Numbered,
    Todo,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub flavour: String,
    pub children: Vec<String>,
    pub para_type: String,
    pub deltas: Vec<Delta>,
    pub title: String,
    pub checked: bool,
    pub language: Option<String>,
    #[allow(dead_code)]
    pub source_id: Option<String>,
    pub page_id: Option<String>,
    #[allow(dead_code)]
    pub caption: Option<String>,
    pub list_kind: Option<ListKind>,
}

impl Block {
    pub fn is_ranged(&self) -> bool {
        RANGE_FLAVOURS.iter().any(|f| *f == self.flavour)
    }

    pub fn is_empty_text_paragraph(&self) -> bool {
        self.flavour == "affine:paragraph"
            && (self.para_type == "text" || self.para_type.is_empty())
            && self.plain_text().is_empty()
    }

    pub fn plain_text(&self) -> String {
        self.deltas.iter().map(|d| d.insert.as_str()).collect()
    }
}

pub struct BlockTree {
    pub blocks: HashMap<String, Block>,
    pub page_id: String,
    pub workspace_id: String,
}

impl BlockTree {
    pub fn load(doc: &Doc, workspace_id: &str) -> Result<Self> {
        let map = doc
            .get_map("blocks")
            .map_err(|e| anyhow!("y-octo blocks map: {e}"))?;
        let mut blocks = HashMap::new();
        for (id, value) in map.iter() {
            let Value::Map(bm) = value else {
                continue;
            };
            let block = load_block(&id, &bm)?;
            blocks.insert(block.id.clone(), block);
        }
        let page_id = blocks
            .values()
            .find(|b| b.flavour == "affine:page")
            .map(|b| b.id.clone())
            .ok_or_else(|| anyhow!("pin has no affine:page root"))?;
        Ok(Self {
            blocks,
            page_id,
            workspace_id: workspace_id.to_string(),
        })
    }

    pub fn page(&self) -> Result<&Block> {
        self.blocks
            .get(&self.page_id)
            .ok_or_else(|| anyhow!("missing page {}", self.page_id))
    }

    pub fn get(&self, id: &str) -> Result<&Block> {
        self.blocks
            .get(id)
            .ok_or_else(|| anyhow!("missing block {id}"))
    }

    pub fn note_ids(&self) -> Result<Vec<String>> {
        let page = self.page()?;
        for cid in &page.children {
            let child = self.get(cid)?;
            if child.flavour == "affine:note" {
                return Ok(child.children.clone());
            }
        }
        bail!("affine:page has no affine:note child");
    }

    /// Note-walked ranged blocks in tree order, with list nesting depth.
    pub fn collect_ranged(&self) -> Result<Vec<(String, usize)>> {
        let mut out = Vec::new();
        let page = self.page()?;
        collect_ranged(self, &page.children, 0, &mut out)?;
        Ok(out)
    }
}

fn collect_ranged(
    tree: &BlockTree,
    ids: &[String],
    list_depth: usize,
    out: &mut Vec<(String, usize)>,
) -> Result<()> {
    for id in ids {
        let block = tree.get(id)?;
        if block.flavour == "affine:surface" {
            continue;
        }
        if block.is_ranged() {
            let depth = if block.flavour == "affine:list" {
                list_depth
            } else {
                0
            };
            out.push((block.id.clone(), depth));
        }
        let next = if block.flavour == "affine:list" {
            list_depth + 1
        } else {
            list_depth
        };
        collect_ranged(tree, &block.children, next, out)?;
    }
    Ok(())
}

fn load_block(id: &str, map: &Map) -> Result<Block> {
    let flavour = map_string(map, "sys:flavour").unwrap_or_default();
    let sys_id = map_string(map, "sys:id").unwrap_or_else(|| id.to_string());
    let children = map
        .get("sys:children")
        .and_then(|v| match v {
            Value::Array(a) => Some(array_ids(&a)),
            _ => None,
        })
        .unwrap_or_default();
    let para_type = map_string(map, "prop:type").unwrap_or_else(|| "text".into());
    let list_kind = match flavour.as_str() {
        "affine:list" => Some(match para_type.as_str() {
            "numbered" => ListKind::Numbered,
            "todo" => ListKind::Todo,
            _ => ListKind::Bulleted,
        }),
        _ => None,
    };
    let deltas = map
        .get("prop:text")
        .and_then(|v| match v {
            Value::Text(t) => Some(text_deltas(&t)),
            _ => None,
        })
        .unwrap_or_default();
    let title = map
        .get("prop:title")
        .and_then(|v| match v {
            Value::Text(t) => Some(t.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let checked = matches!(map.get("prop:checked"), Some(Value::Any(Any::True)));
    let language = map_string(map, "prop:language");
    let source_id = map_string(map, "prop:sourceId");
    let page_id = map_string(map, "prop:pageId");
    let caption = map_string(map, "prop:caption");
    Ok(Block {
        id: sys_id,
        flavour,
        children,
        para_type,
        deltas,
        title,
        checked,
        language,
        source_id,
        page_id,
        caption,
        list_kind,
    })
}

fn map_string(map: &Map, key: &str) -> Option<String> {
    match map.get(key)? {
        Value::Any(Any::String(s)) => Some(s),
        Value::Text(t) => Some(t.to_string()),
        other => {
            let s = other.to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

fn array_ids(arr: &Array) -> Vec<String> {
    arr.iter()
        .filter_map(|v| match v {
            Value::Any(Any::String(s)) => Some(s),
            other => {
                let s = other.to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
        })
        .collect()
}

fn text_deltas(text: &Text) -> Vec<Delta> {
    text.to_delta()
        .into_iter()
        .filter_map(|op| match op {
            TextDeltaOp::Insert {
                insert: TextInsert::Text(insert),
                format,
            } => {
                let attrs = format.unwrap_or_default();
                Some(Delta {
                    insert,
                    bold: flag(&attrs, "bold"),
                    italic: flag(&attrs, "italic"),
                    strike: flag(&attrs, "strike"),
                    code: flag(&attrs, "code"),
                    underline: flag(&attrs, "underline"),
                    link: attrs.get("link").and_then(|a| match a {
                        Any::String(s) if !s.is_empty() => Some(s.clone()),
                        _ => None,
                    }),
                })
            }
            _ => None,
        })
        .collect()
}

fn flag(attrs: &y_octo::TextAttributes, key: &str) -> bool {
    match attrs.get(key) {
        Some(Any::True) => true,
        Some(Any::String(s)) => s == "true",
        _ => false,
    }
}
