//! Owner hooks. Step 8 calls them. The layer does not know what they do.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Implemented by the process that owns the data inside a replica set.
pub trait Owner: Send + Sync {
    fn rebuild<'a>(&'a self, set: &'a str) -> BoxFut<'a, anyhow::Result<()>>;
    fn lost<'a>(&'a self, set: &'a str, tags: &'a [String]) -> BoxFut<'a, anyhow::Result<()>>;
}
