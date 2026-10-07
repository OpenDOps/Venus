//! Step 5 checks that do not need a database.
//! `cargo test -p surrealastic --test layout`
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;

use surrealastic::{pack_items, shard_for, shards_for, Body, Item, log_tail_start};

fn item(key: i64) -> Item {
    Item {
        key,
        item: format!("item-{key}"),
        tag: "t".into(),
        rids: vec![format!("row:{key}")],
        body: Body::new().upsert(&format!("row:{key}"), serde_json::json!({ "n": key })),
        delete: false,
    }
}

#[test]
fn modulo() {
    for key in 0..10 {
        assert_eq!(shard_for(key, 2), (key % 2) as i32, "key {key}");
    }
    let forward: Vec<Item> = (0..10).map(item).collect();
    let mut backward = forward.clone();
    backward.reverse();
    assert_eq!(placed(&forward), placed(&backward));
}

fn placed(items: &[Item]) -> Vec<(i64, i32)> {
    let mut pairs: Vec<_> = items
        .iter()
        .map(|item| (item.key, shard_for(item.key, 2)))
        .collect();
    pairs.sort();
    pairs
}

#[test]
fn dual_grouping() {
    assert_eq!(shards_for(2, 2, Some(4)), vec![0, 2]);
    assert_eq!(shards_for(1, 2, Some(4)), vec![1]);
    assert_eq!(shards_for(6, 2, Some(4)), vec![0, 2]);
}

#[test]
fn entry_limit() {
    let items: Vec<Item> = (0..300).map(|n| item(n * 2)).collect();
    let packs = pack_items(&items, 256, 4 * 1024 * 1024);
    assert_eq!(packs.len(), 2);
    assert!(packs.iter().all(|pack| pack.len() <= 256));
    assert!(packs.iter().all(|pack| {
        pack.iter().map(|item| item.body.logged().to_string().len()).sum::<usize>() < 4 * 1024 * 1024
    }));
    assert_eq!(packs[0].len() + packs[1].len(), 300);
}

#[test]
fn schema_define_refuses_non_ddl() {
    let err = Body::new()
        .define("UPSERT probe:1 CONTENT { n: 1 };")
        .expect_err("dml");
    assert!(err.to_string().contains("non-DDL"), "{err}");
    assert!(Body::new().define("DEFINE TABLE probe SCHEMALESS;").is_ok());
    assert!(Body::new().define("REMOVE TABLE probe;").is_ok());
}

#[test]
fn refill_log_starts_after_l0() {
    assert_eq!(log_tail_start(40), 41);
    assert!(log_tail_start(0) > 0);
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/layout.rs")).unwrap();
    assert!(
        !source.contains("_repl_log:1") && !source.contains("_repl_log:0"),
        "refill must not read the log from the start"
    );
}

#[test]
fn no_field_hash() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = 0_u32;
    visit(&root, &mut |path| {
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            return;
        }
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.contains("doc_id"), "{}", path.display());
        assert!(!text.contains("sha256"), "{}", path.display());
        files += 1;
    });
    assert!(files >= 8, "walked {files} sources");
}

fn visit(dir: &Path, each: &mut dyn FnMut(&Path)) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            visit(&path, each);
        } else {
            each(&path);
        }
    }
}
