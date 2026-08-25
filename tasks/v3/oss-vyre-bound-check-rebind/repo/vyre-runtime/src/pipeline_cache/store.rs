//! [`PipelineCacheStore`]  -  backend trait shared by every concrete cache
//! implementation in this module.

use std::io;
use std::sync::Arc;

use super::fingerprint::PipelineFingerprint;
use super::metrics::PipelineCacheMetrics;

/// Trait for persistent pipeline-cache backends. [`super::DiskCache`] and
/// `super::RemoteCache` (when the `remote` feature is enabled) ship
/// disk- and network-backed implementations; tests here use the
/// in-memory [`super::InMemoryPipelineCache`].
pub trait PipelineCacheStore: Send + Sync {
    /// Look up a cached artifact for this fingerprint.
    ///
    /// This method is required so cache implementations cannot accidentally
    /// inherit a mutually recursive `get` / `get_arc` pair. Hot-path consumers
    /// should call [`Self::get_arc`] when they can share the payload.
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>>;

    /// V7-PERF-009: zero-clone hot-path lookup. Returns the cached artifact as
    /// an `Arc<Vec<u8>>` so multiple consumers share the underlying allocation.
    /// Default impl wraps `get`; in-memory and layered caches override this to
    /// return their internal `Arc` directly.
    fn get_arc(&self, fp: &PipelineFingerprint) -> Option<Arc<Vec<u8>>> {
        self.get(fp).map(Arc::new)
    }

    /// Insert a pre-compiled artifact. Implementations may dedupe
    /// or evict per their own policy.
    fn put(&self, fp: PipelineFingerprint, artifact: Vec<u8>);

    /// Durably flush artifacts that were accepted by [`Self::put`].
    ///
    /// Volatile stores can keep the default no-op. Disk stores override this
    /// so insertion stays cheap while callers still have an explicit
    /// crash-durable boundary.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when a durable backend cannot flush pending data.
    fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    /// Snapshot cache instrumentation for latency, throughput, and eviction
    /// gates. Backends that do not maintain counters return zeros.
    #[must_use]
    fn metrics(&self) -> PipelineCacheMetrics {
        PipelineCacheMetrics::default()
    }
}
