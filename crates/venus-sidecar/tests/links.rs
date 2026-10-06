//! M4 step 7.1: `.venus/links.json` rebuild, persist, lookup without scanning.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use venus_sidecar::catalog::{CatalogPage, CatalogWalk, OldPage};
use venus_sidecar::convert::{Converted, Sidecar};
use venus_sidecar::links::{
    self, inbound, outbound, persist_links_json, persist_on_flush, rebuild_from_wiki, serialize,
    targets_in_markdown, upsert_outbound, LINKS_JSON_REL,
};
use venus_sidecar::PAGE_DOC_UUID;

const TARGET: &str = "a1b2c3d4-e5f6-4780-abcd-ef1234567890";
const HOME: &str = "doc:home";
const SECOND: &str = "eeeeeeee-e5f6-4780-abcd-ef1234567890";
const THIRD: &str = "ffffffff-e5f6-4780-abcd-ef1234567890";

const HOME_MD: &str = "See [x](spec/a1b2c3d4-e5f6-4780-abcd-ef1234567890.md) and [plain](https://example.com).\n<!-- venus:doc:a1b2c3d4-e5f6-4780-abcd-ef1234567890 -->\n";
const SECOND_MD: &str = "Also <!-- venus:doc:a1b2c3d4-e5f6-4780-abcd-ef1234567890 -->\n";
const THIRD_MD: &str = "No comment, just [link](whatever.md)\n";

const SCENARIO_1_JSON: &str = r#"{
  "inbound": {
    "a1b2c3d4-e5f6-4780-abcd-ef1234567890": [
      "doc:home",
      "eeeeeeee-e5f6-4780-abcd-ef1234567890"
    ]
  },
  "outbound": {
    "doc:home": [
      "a1b2c3d4-e5f6-4780-abcd-ef1234567890"
    ],
    "eeeeeeee-e5f6-4780-abcd-ef1234567890": [
      "a1b2c3d4-e5f6-4780-abcd-ef1234567890"
    ]
  }
}
"#;

const PAGES_YAML: &str = r#"pages:
  spec/home.md:
    uuid: 395cd07b-bdb1-5f54-ada8-e9a3fabb6a20
    docId: "doc:home"
    name: home
    tags: []
  spec/eeeeeeee-e5f6-4780-abcd-ef1234567890.md:
    uuid: eeeeeeee-e5f6-4780-abcd-ef1234567890
    docId: eeeeeeee-e5f6-4780-abcd-ef1234567890
    name: second
    tags: []
  spec/ffffffff-e5f6-4780-abcd-ef1234567890.md:
    uuid: ffffffff-e5f6-4780-abcd-ef1234567890
    docId: ffffffff-e5f6-4780-abcd-ef1234567890
    name: third
    tags: []
folders:
  spec:
    id: "folder:spec"
    name: spec
"#;

fn write_wiki(dir: &Path) {
    fs::create_dir_all(dir.join("spec")).expect("spec");
    fs::create_dir_all(dir.join(".venus")).expect("venus");
    fs::write(dir.join("spec/home.md"), HOME_MD).expect("home.md");
    fs::write(dir.join(format!("spec/{SECOND}.md")), SECOND_MD).expect("second.md");
    fs::write(dir.join(format!("spec/{THIRD}.md")), THIRD_MD).expect("third.md");
    fs::write(dir.join(".venus/pages.yaml"), PAGES_YAML).expect("pages.yaml");
}

fn scenario_walk() -> CatalogWalk {
    CatalogWalk {
        pages: vec![
            CatalogPage {
                sql_uuid: PAGE_DOC_UUID.into(),
                doc_id: HOME.into(),
                name: "home".into(),
                git_path: "spec/home.md".into(),
            },
            CatalogPage {
                sql_uuid: SECOND.into(),
                doc_id: SECOND.into(),
                name: "second".into(),
                git_path: format!("spec/{SECOND}.md"),
            },
            CatalogPage {
                sql_uuid: THIRD.into(),
                doc_id: THIRD.into(),
                name: "third".into(),
                git_path: format!("spec/{THIRD}.md"),
            },
        ],
        folders: vec![],
    }
}

fn converted(doc_id: &str, markdown: &str) -> Converted {
    Converted {
        markdown: markdown.into(),
        sidecar: Sidecar {
            doc_id: doc_id.into(),
            clock: "0".into(),
            blocks: vec![],
        },
    }
}

#[test]
fn rebuild_from_missing_links_json_matches_scenario_1() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_wiki(tmp.path());
    assert!(!tmp.path().join(LINKS_JSON_REL).exists());
    let index = rebuild_from_wiki(tmp.path(), &HashMap::new()).expect("rebuild");
    let bytes = serialize(&index).expect("serialize");
    assert_eq!(
        std::str::from_utf8(&bytes).expect("utf8"),
        SCENARIO_1_JSON,
        "rebuild JSON must match host dialect"
    );
    assert_eq!(inbound(&index, TARGET), [HOME, SECOND]);
    assert_eq!(outbound(&index, HOME), [TARGET]);
    assert!(index.inbound.get(THIRD).is_none());
    assert!(index.outbound.get(THIRD).is_none());
}

#[test]
fn rebuild_from_corrupt_brace_then_second_rebuild_same_bytes() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_wiki(tmp.path());
    fs::write(tmp.path().join(LINKS_JSON_REL), "{").expect("corrupt");
    persist_links_json(tmp.path(), &HashMap::new(), &[], &[]).expect("persist rebuild");
    let first = fs::read(tmp.path().join(LINKS_JSON_REL)).expect("read");
    assert_eq!(std::str::from_utf8(&first).expect("utf8"), SCENARIO_1_JSON);

    persist_links_json(tmp.path(), &HashMap::new(), &[], &[]).expect("second persist");
    let second = fs::read(tmp.path().join(LINKS_JSON_REL)).expect("read2");
    assert_eq!(second, first, "second rebuild/persist is a no-op");
}

#[test]
fn persist_upsert_then_drop_clears_stale_inbound() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_wiki(tmp.path());
    persist_on_flush(tmp.path(), &scenario_walk(), &[], &HashMap::new()).expect("seed");

    let empty_home = converted(HOME, "# Venus\n");
    persist_on_flush(
        tmp.path(),
        &scenario_walk(),
        &[(PAGE_DOC_UUID.into(), empty_home)],
        &HashMap::new(),
    )
    .expect("upsert home empty");
    let after_upsert = links::load(tmp.path()).expect("load");
    assert_eq!(inbound(&after_upsert, TARGET), [SECOND]);
    assert!(after_upsert.outbound.get(HOME).is_none());

    let mut old = HashMap::new();
    old.insert(
        TARGET.to_string(),
        OldPage {
            sql_uuid: TARGET.into(),
            doc_id: TARGET.into(),
            git_path: format!("spec/{TARGET}.md"),
            name: "target".into(),
        },
    );
    // Drop the *target* page: walk without TARGET as a page; TARGET is the
    // linked uuid, not SECOND. Delete SECOND's target by dropping TARGET id
    // via old_pages whose sql is TARGET — TARGET is not a live page in walk.
    persist_on_flush(tmp.path(), &scenario_walk(), &[], &old).expect("drop target");
    let after_drop = links::load(tmp.path()).expect("load");
    assert!(after_drop.inbound.get(TARGET).is_none());
    assert!(after_drop.outbound.get(TARGET).is_none());
    assert!(after_drop.outbound.get(SECOND).is_none());
}

#[test]
fn persist_drop_deleted_source_id() {
    let tmp = tempfile::tempdir().expect("tmp");
    write_wiki(tmp.path());
    persist_links_json(tmp.path(), &HashMap::new(), &[], &[]).expect("seed");
    persist_links_json(tmp.path(), &HashMap::new(), &[], &[SECOND]).expect("drop second");
    let index = links::load(tmp.path()).expect("load");
    assert_eq!(inbound(&index, TARGET), [HOME]);
    assert!(index.outbound.get(SECOND).is_none());
}

#[test]
fn ordinary_markdown_url_is_not_indexed() {
    assert!(targets_in_markdown(THIRD_MD).is_empty());
    let mut index = links::empty_index();
    upsert_outbound(&mut index, THIRD, &targets_in_markdown(THIRD_MD));
    assert!(index.outbound.get(THIRD).is_none());
}

#[test]
fn lookup_is_map_get_not_a_file_walk() {
    let src = include_str!("../src/links.rs");
    let inbound_fn = src
        .split("pub fn inbound")
        .nth(1)
        .and_then(|s| s.split("pub fn outbound").next())
        .expect("inbound fn");
    assert!(
        !inbound_fn.contains("fs::")
            && !inbound_fn.contains("read_dir")
            && !inbound_fn.contains("rebuild"),
        "inbound must be O(1) map lookup"
    );
    let outbound_fn = src
        .split("pub fn outbound")
        .nth(1)
        .and_then(|s| s.split("fn remove_source_from_inbound").next())
        .expect("outbound fn");
    assert!(
        !outbound_fn.contains("fs::") && !outbound_fn.contains("read_dir"),
        "outbound must be O(1) map lookup"
    );
}

#[test]
fn source_scan_rebuild_does_not_hydrate_yjs() {
    let src = include_str!("../src/links.rs");
    for needle in [
        "hydrate_v1",
        "from_doc::",
        "y_octo",
        "MarkdownAdapter",
        "crate::hydrate",
        "crate::from_doc",
    ] {
        assert!(
            !src.contains(needle),
            "links.rs must not {needle} during rebuild"
        );
    }
}
