//! Tiered runtime cache primitives.

pub(crate) mod pipeline;

pub use lru::AccessTracker;
pub use tiered_cache::{AccessStats, CacheEntry, CacheError, CacheTier, LruPolicy, TieredCache};

/// LRU tracking.
pub mod lru;
/// Multi-tier cache storage, policy, and errors.
pub mod tiered_cache;

/// Cache test suites.
#[cfg(test)]
pub mod tests;
