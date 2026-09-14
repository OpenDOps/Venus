//! Pin generation buffer: `Map<docId, { bytes, clock }>` filled by the MVCC cut.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;

/// Frozen CRDT bytes for one flush: `Map<docId, { bytes, clock }>`.
#[derive(Debug, Default)]
pub struct PinMap {
    docs: HashMap<String, PinEntry>,
    blobs: HashMap<String, Vec<u8>>,
    /// Catalog v0 path, set in the same collect as the page bytes.
    pub git_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PinEntry {
    pub bytes: Vec<u8>,
    pub clock: i64,
}

impl PinMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn get(&self, doc_id: &str) -> Option<&PinEntry> {
        self.docs.get(doc_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &PinEntry)> {
        self.docs.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn blob(&self, hash: &str) -> Option<&[u8]> {
        self.blobs.get(hash).map(|b| b.as_slice())
    }

    pub fn blobs(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.blobs.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    /// Cut fills this. Claim / observe must not call it.
    pub fn insert(&mut self, doc_id: String, entry: PinEntry) {
        self.docs.insert(doc_id, entry);
    }

    pub fn insert_blob(&mut self, hash: String, bytes: Vec<u8>) {
        self.blobs.insert(hash, bytes);
    }

    pub fn clear(&mut self) {
        self.docs.clear();
        self.blobs.clear();
        self.git_path = None;
    }
}
