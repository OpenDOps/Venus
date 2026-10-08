//! Placement and the body rule. No database.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;

use surrealastic::{accept_sql, homes, set_health, Body, Health, Member, NodeState};

fn member(id: &str, zone: &str, weight: i32, state: NodeState) -> Member {
    Member {
        node_id: id.to_string(),
        zone: zone.to_string(),
        weight,
        state,
    }
}

fn nodes(n: usize) -> Vec<Member> {
    (0..n)
        .map(|i| member(&format!("{i:02}"), &format!("z{i:02}"), 1, NodeState::Up))
        .collect()
}

fn sets(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("set-{i:05}")).collect()
}

#[test]
fn placement() {
    for n in 1..=10 {
        for copies in 1..=3 {
            let members = nodes(n);
            let ids = sets(400);
            let mut first = Vec::new();
            for id in &ids {
                let chosen = homes(id, copies, &members);
                let again = homes(id, copies, &members);
                assert_eq!(chosen, again, "{id} stable");
                let reversed = {
                    let mut m = members.clone();
                    m.reverse();
                    homes(id, copies, &m)
                };
                assert_eq!(chosen, reversed, "{id} ignores member order");
                let zones: HashSet<_> = chosen
                    .iter()
                    .map(|node| {
                        members
                            .iter()
                            .find(|m| m.node_id == *node)
                            .unwrap()
                            .zone
                            .clone()
                    })
                    .collect();
                assert_eq!(zones.len(), chosen.len(), "distinct zones");
                assert!(chosen.len() as u32 <= copies);
                assert_eq!(chosen.len() as u32, copies.min(n as u32));
                first.push(chosen);
            }

            let extra = member("new", "znew", 1, NodeState::Up);
            let mut with = members.clone();
            with.push(extra);
            let mut moved = 0usize;
            let mut includes = 0usize;
            for (id, before) in ids.iter().zip(first.iter()) {
                let after = homes(id, copies, &with);
                let has = after.iter().any(|n| n == "new");
                if has {
                    includes += 1;
                }
                if before != &after {
                    moved += 1;
                    assert!(has, "a moved set must now include the new node");
                } else {
                    assert!(!has, "an unmoved set must not include the new node");
                }
            }
            assert_eq!(moved, includes);
            if (copies as usize) <= n {
                let expect = copies as f64 / (n as f64 + 1.0);
                let got = includes as f64 / ids.len() as f64;
                assert!(
                    (got - expect).abs() < 0.12,
                    "N={n} copies={copies} share {got} expected about {expect}"
                );
            }

            if n >= 2 {
                let gone = members[0].node_id.clone();
                let rest: Vec<_> = members.iter().skip(1).cloned().collect();
                for id in &ids {
                    let before = homes(id, copies, &members);
                    let after = homes(id, copies, &rest);
                    if !before.iter().any(|n| n == &gone) {
                        assert_eq!(before, after, "removing a node moves only its own copies");
                    }
                }
            }
        }
    }
}

#[test]
fn weight() {
    let members = vec![
        member("heavy", "zh", 2, NodeState::Up),
        member("light", "zl", 1, NodeState::Up),
    ];
    let mut heavy = 0;
    let mut light = 0;
    for id in sets(10_000) {
        let home = homes(&id, 1, &members);
        assert_eq!(home.len(), 1);
        match home[0].as_str() {
            "heavy" => heavy += 1,
            "light" => light += 1,
            other => panic!("unexpected home {other}"),
        }
    }
    let ratio = heavy as f64 / light as f64;
    assert!(
        (1.8..=2.2).contains(&ratio),
        "weight 2 held {heavy}, weight 1 held {light}, ratio {ratio}"
    );
}

#[test]
fn zones() {
    let shared = vec![
        member("a", "z", 1, NodeState::Up),
        member("b", "z", 1, NodeState::Up),
    ];
    for id in sets(50) {
        let home = homes(id.as_str(), 2, &shared);
        assert_eq!(home.len(), 1, "one zone cannot hold two copies");
    }
    assert_eq!(set_health(2, 1, 1), Health::Yellow);

    let mixed = vec![
        member("a", "z", 1, NodeState::Up),
        member("b", "z", 1, NodeState::Up),
        member("c", "y", 1, NodeState::Up),
    ];
    for id in sets(50) {
        let home = homes(id.as_str(), 2, &mixed);
        assert_eq!(home.len(), 2);
        let in_z = home.iter().filter(|n| *n == "a" || *n == "b").count();
        assert_eq!(in_z, 1, "never two homes in one zone");
    }
}

#[test]
fn draining() {
    let members = vec![
        member("up", "a", 1, NodeState::Up),
        member("down", "b", 1, NodeState::Down),
        member("drain", "c", 1, NodeState::Draining),
    ];
    for id in sets(40) {
        let home = homes(id.as_str(), 3, &members);
        assert!(
            home.iter().any(|n| n == "down"),
            "a down node stays a member"
        );
        assert!(
            !home.iter().any(|n| n == "drain"),
            "a draining node is never a home"
        );
    }
}

#[test]
fn body_rule() {
    for sql in [
        "rand::uuid()",
        "time::now()",
        "CREATE t",
        "x += 1",
        "x -= 1",
        "COMMIT TRANSACTION",
        "CANCEL TRANSACTION",
        "BEGIN TRANSACTION",
        "LET $x = 1; COMMIT;",
        "{ CANCEL }",
    ] {
        assert!(accept_sql(sql).is_err(), "{sql} must be refused");
    }
    accept_sql("UPSERT _layout_item:1 CONTENT { commit: 1 };").expect("field named commit");
    let built = Body::new()
        .upsert("item:1", serde_json::json!({"n": 1}))
        .delete("item:1")
        .delete_range("item", 1, 4)
        .insert_relation("edge:1", "item:1", "item:2", serde_json::json!({"k": "v"}));
    accept_sql(&built.sql()).expect("builder body");
    assert!(built.sql().contains("UPSERT item:1"));
    assert!(
        built.sql().contains("DELETE item:1..=4") || built.sql().contains("DELETE item:1..=4;")
    );
}

#[test]
fn append_keeps_param_names() {
    let mut body = Body::new();
    for n in 0..6 {
        let part = Body::new()
            .upsert(&format!("row:a{n}"), serde_json::json!({ "a": n }))
            .upsert(&format!("row:b{n}"), serde_json::json!({ "b": n }))
            .upsert(&format!("row:c{n}"), serde_json::json!({ "c": n }));
        body = body.append(&part);
    }
    let sql = body.sql();
    let mut rest = sql.as_str();
    let mut seen = 0_usize;
    while let Some(at) = rest.find("$p") {
        let name: String = rest[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        assert!(
            body.params().contains_key(&name),
            "${name} is not a bound parameter"
        );
        seen += 1;
        rest = &rest[at + 1 + name.len()..];
    }
    assert_eq!(seen, 18);
    assert_eq!(body.params().len(), 18);
}
