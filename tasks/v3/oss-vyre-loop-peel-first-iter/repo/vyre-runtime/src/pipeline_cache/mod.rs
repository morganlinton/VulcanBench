//! P4.3  -  content-addressed pipeline cache.
//!
//! Every compiled `Program` has a stable fingerprint =
//! `blake3(canonicalize(program).to_wire())`. The fingerprint
//! becomes the cache key: two authors who write the same
//! computation via different spellings share cached target binaries /
//! native-backend artifacts, skipping recompilation.
//!
//! The cache is deliberately composable at this layer. Hot paths use
//! [`InMemoryPipelineCache`], persistent process-restart reuse uses
//! [`DiskCache`], and callers that want both compose them with
//! [`LayeredPipelineCache`].

#![allow(clippy::missing_const_for_thread_local, clippy::explicit_auto_deref)]

mod disk;
mod fingerprint;
mod in_memory;
mod layered;
mod metrics;
#[cfg(feature = "remote-cache")]
mod remote;
mod store;

#[cfg(test)]
pub(super) mod test_helpers;

pub use disk::{
    DiskCache, DiskCacheDurabilityReport, DiskCacheError, PersistentPipelineCacheStore,
};
pub use fingerprint::PipelineFingerprint;
pub use in_memory::{InMemoryEvictionReason, InMemoryEvictionReport, InMemoryPipelineCache};
pub use layered::{LayeredPipelineCache, LayeredPromotionReport};
pub use metrics::{PipelineCacheMetricError, PipelineCacheMetrics};
#[cfg(feature = "remote-cache")]
pub use remote::RemoteCache;
pub use store::PipelineCacheStore;
