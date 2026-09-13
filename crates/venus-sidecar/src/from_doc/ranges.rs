//! Sidecar range placement — same rules as `from-doc.js` (UTF-16 `[start, end)`).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, bail, Result};

use super::markdown::{indent_slice, own_markdown};
use super::tree::BlockTree;
use crate::convert::SidecarBlock;

#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}

enum Placed {
    Empty,
    Span(Span),
}

pub struct RangedItem {
    pub id: String,
    pub flavour: String,
    pub slice: String,
    pub list_depth: usize,
}

pub fn ranged_items(tree: &BlockTree) -> Result<Vec<RangedItem>> {
    let mut items = Vec::new();
    for (id, list_depth) in tree.collect_ranged()? {
        let block = tree.get(&id)?;
        let slice = own_markdown(tree, &id, 0)?;
        // Unresolved `affine:image` (no blob map) has an empty adapter slice.
        // Empty text paragraphs stay in the list for last-N newline assignment.
        if slice.is_empty() && block.flavour != "affine:paragraph" {
            continue;
        }
        items.push(RangedItem {
            id,
            flavour: block.flavour.clone(),
            slice,
            list_depth,
        });
    }
    Ok(items)
}

pub fn place_page_title(tree: &BlockTree, markdown: &str) -> Result<Option<SidecarBlock>> {
    let page = tree.page()?;
    if page.title.is_empty() {
        return Ok(None);
    }
    let heading = format!("# {}\n", page.title);
    let heading_u16: Vec<u16> = heading.encode_utf16().collect();
    let md_u16: Vec<u16> = markdown.encode_utf16().collect();
    if md_u16.len() < heading_u16.len() || md_u16[..heading_u16.len()] != heading_u16[..] {
        bail!(
            "Page title {:?} not at start of fromDoc: {:?}",
            page.title,
            utf16_to_string(&md_u16[..md_u16.len().min(80)])
        );
    }
    Ok(Some(SidecarBlock {
        id: page.id.clone(),
        start: 0,
        end: heading_u16.len(),
    }))
}

pub fn build_ranges(
    markdown: &str,
    items: &[RangedItem],
    title: Option<SidecarBlock>,
) -> Result<Vec<SidecarBlock>> {
    let md: Vec<u16> = markdown.encode_utf16().collect();
    let leading_start = title.as_ref().map(|t| t.end).unwrap_or(0);
    let mut placed: Vec<Placed> = Vec::with_capacity(items.len());
    let mut cursor = leading_start;
    for item in items {
        if item.flavour == "affine:paragraph" && core_of(&item.slice).is_empty() {
            placed.push(Placed::Empty);
            continue;
        }
        let indented = indent_slice(&item.slice, item.list_depth);
        let loc = place_from_cursor(&md, &indented, cursor)
            .or_else(|| place_from_cursor(&md, &item.slice, cursor))
            .ok_or_else(|| {
                anyhow!(
                    "Could not place block {} ({}) slice={:?} at {cursor} near {:?}",
                    item.id,
                    item.flavour,
                    item.slice,
                    utf16_to_string(&md[cursor..md.len().min(cursor + 80)])
                )
            })?;
        cursor = loc.end;
        placed.push(Placed::Span(loc));
    }
    assign_empty_paragraphs(&md, items, &mut placed, leading_start)?;
    let mut blocks = Vec::new();
    if let Some(title) = title {
        blocks.push(title);
    }
    for (item, p) in items.iter().zip(placed.iter()) {
        let Placed::Span(span) = p else {
            bail!("empty paragraph {} was not assigned a newline", item.id);
        };
        blocks.push(SidecarBlock {
            id: item.id.clone(),
            start: span.start,
            end: span.end,
        });
    }
    Ok(blocks)
}

fn core_of(slice: &str) -> String {
    slice.trim_end_matches('\n').to_string()
}

fn placement_candidates(slice: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |s: String| {
        if !s.is_empty() && seen.insert(s.clone()) {
            out.push(s);
        }
    };
    push(slice.to_string());
    let core = core_of(slice);
    if !core.is_empty() {
        push(format!("{core}\n"));
        push(core);
    }
    out
}

fn place_from_cursor(md: &[u16], slice: &str, from: usize) -> Option<Span> {
    let candidates = placement_candidates(slice);
    let mut i = from;
    while i <= md.len() {
        for cand in &candidates {
            let c = encode(cand);
            if starts_with(md, i, &c) {
                return Some(Span {
                    start: i,
                    end: i + c.len(),
                });
            }
            if let Some(n) = match_numbered_at(md, cand, i) {
                return Some(n);
            }
        }
        if i < md.len() && md[i] == u16::from(b'\n') {
            i += 1;
            continue;
        }
        break;
    }
    place_by_index_of(md, &candidates, from)
}

fn place_by_index_of(md: &[u16], candidates: &[String], from: usize) -> Option<Span> {
    for cand in candidates {
        let c = encode(cand);
        if let Some(idx) = find_sub(md, &c, from) {
            return Some(Span {
                start: idx,
                end: idx + c.len(),
            });
        }
    }
    for cand in candidates {
        if let Some(n) = place_numbered_by_search(md, cand, from) {
            return Some(n);
        }
    }
    None
}

fn numbered_parts(slice: &str) -> Option<(String, String)> {
    // /^((?:  )*)(\d+)\.([\s\S]*)$/
    let bytes = slice.as_bytes();
    let mut pad = 0usize;
    while pad + 1 < bytes.len() && bytes[pad] == b' ' && bytes[pad + 1] == b' ' {
        pad += 2;
    }
    let rest = &slice[pad..];
    let mut nlen = 0usize;
    for c in rest.chars() {
        if c.is_ascii_digit() {
            nlen += 1;
        } else {
            break;
        }
    }
    if nlen == 0 {
        return None;
    }
    let after_num = &rest[nlen..];
    if !after_num.starts_with('.') {
        return None;
    }
    Some((slice[..pad].to_string(), after_num[1..].to_string()))
}

fn match_numbered_at(md: &[u16], slice: &str, i: usize) -> Option<Span> {
    let (pad, rest) = numbered_parts(slice)?;
    let pad_u = encode(&pad);
    if !starts_with(md, i, &pad_u) {
        return None;
    }
    let after_pad = i + pad_u.len();
    let mut j = after_pad;
    while j < md.len() && md[j] >= u16::from(b'0') && md[j] <= u16::from(b'9') {
        j += 1;
    }
    if j == after_pad || j >= md.len() || md[j] != u16::from(b'.') {
        return None;
    }
    let rest_u = encode(&rest);
    let rest_at = j + 1;
    if !starts_with(md, rest_at, &rest_u) {
        return None;
    }
    Some(Span {
        start: i,
        end: rest_at + rest_u.len(),
    })
}

fn place_numbered_by_search(md: &[u16], slice: &str, from: usize) -> Option<Span> {
    let (pad, rest) = numbered_parts(slice)?;
    let pad_u = encode(&pad);
    let rest_u = encode(&rest);
    let mut i = from;
    while i < md.len() {
        if starts_with(md, i, &pad_u) {
            let after_pad = i + pad_u.len();
            let mut j = after_pad;
            while j < md.len() && md[j] >= u16::from(b'0') && md[j] <= u16::from(b'9') {
                j += 1;
            }
            if j > after_pad && j < md.len() && md[j] == u16::from(b'.') {
                let rest_at = j + 1;
                if starts_with(md, rest_at, &rest_u) {
                    return Some(Span {
                        start: i,
                        end: rest_at + rest_u.len(),
                    });
                }
            }
        }
        i += 1;
    }
    None
}

fn assign_empty_paragraphs(
    md: &[u16],
    items: &[RangedItem],
    placed: &mut [Placed],
    leading_start: usize,
) -> Result<()> {
    let mut i = 0;
    while i < items.len() {
        if !matches!(placed[i], Placed::Empty) {
            i += 1;
            continue;
        }
        let mut group = Vec::new();
        while i < items.len() && matches!(placed[i], Placed::Empty) {
            group.push(i);
            i += 1;
        }
        let prev = group[0] as isize - 1;
        let next = group[group.len() - 1] + 1;
        let interval_start = if prev >= 0 {
            match placed[prev as usize] {
                Placed::Span(s) => s.end,
                Placed::Empty => leading_start,
            }
        } else {
            leading_start
        };
        let interval_end = if next < items.len() {
            match placed[next] {
                Placed::Span(s) => s.start,
                Placed::Empty => md.len(),
            }
        } else {
            md.len()
        };
        let mut nl = Vec::new();
        for (k, ch) in md[interval_start..interval_end].iter().enumerate() {
            if *ch == u16::from(b'\n') {
                nl.push(interval_start + k);
            }
        }
        if nl.len() < group.len() {
            bail!(
                "Need {} empty-paragraph newline(s), found {} in {:?}",
                group.len(),
                nl.len(),
                utf16_to_string(&md[interval_start..interval_end])
            );
        }
        let take = &nl[nl.len() - group.len()..];
        for (g, start) in group.iter().zip(take.iter()) {
            placed[*g] = Placed::Span(Span {
                start: *start,
                end: *start + 1,
            });
        }
    }
    Ok(())
}

fn encode(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn starts_with(md: &[u16], i: usize, needle: &[u16]) -> bool {
    i + needle.len() <= md.len() && md[i..i + needle.len()] == needle[..]
}

fn find_sub(md: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() || from > md.len() {
        return None;
    }
    md[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

fn utf16_to_string(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}
