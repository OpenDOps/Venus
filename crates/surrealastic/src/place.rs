//! Rendezvous placement. `down` nodes stay members. `draining` nodes do not.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use xxhash_rust::xxh3::xxh3_64;

/// Node membership for one pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub node_id: String,
    pub zone: String,
    pub weight: i32,
    pub state: NodeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Up,
    Suspect,
    Down,
    Draining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Green,
    Yellow,
    Red,
}

/// Homes for `set_id`. At most `copies`, one per zone, highest score first.
/// Fewer zones than `copies` returns fewer homes.
pub fn homes(set_id: &str, copies: u32, members: &[Member]) -> Vec<String> {
    let mut ranked: Vec<(f64, &Member)> = members
        .iter()
        .filter(|m| m.state != NodeState::Draining)
        .map(|m| (score(set_id, &m.node_id, m.weight), m))
        .collect();
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.node_id.cmp(&b.1.node_id))
    });
    let mut chosen: Vec<String> = Vec::new();
    let mut zones: Vec<&str> = Vec::new();
    for (_, member) in ranked {
        if zones.iter().any(|z| *z == member.zone) {
            continue;
        }
        zones.push(&member.zone);
        chosen.push(member.node_id.clone());
        if chosen.len() as u32 >= copies {
            break;
        }
    }
    chosen
}

/// Green: `copies` in-sync copies on `up` nodes. Yellow: still able to ack.
/// Red: fewer in-sync copies than `ack`.
pub fn set_health(copies: u32, ack: u32, in_sync_on_up: u32) -> Health {
    if copies > 0 && in_sync_on_up >= copies {
        Health::Green
    } else if in_sync_on_up >= ack {
        Health::Yellow
    } else {
        Health::Red
    }
}

fn score(set_id: &str, node_id: &str, weight: i32) -> f64 {
    let key = format!("{set_id}:{node_id}");
    let hash = xxh3_64(key.as_bytes());
    // (hash + 1) / 2^64, in (0, 1].
    let u = (hash as f64 + 1.0) / 18_446_744_073_709_551_616.0;
    let u = u.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON);
    let weight = if weight <= 0 { 1.0 } else { weight as f64 };
    -weight / u.ln()
}
