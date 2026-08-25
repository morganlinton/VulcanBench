//! Runtime observability snapshot for the wgpu backend.
//!
//! Extracted from `lib.rs` per audit item #78  -  the stats type is
//! lock-free observability data that has no business living inside
//! the backend trait-impl module. Re-exported through
//! `crate::WgpuBackendStats` so existing call sites do not change.

use crate::WgpuBackend;
use std::sync::Arc;

/// Runtime observability snapshot for a [`crate::WgpuBackend`].
///
/// Feed into metrics pipelines (prometheus, OpenTelemetry, Datadog)
/// for dashboards and alerting. Reads are lock-free; safe to call
/// from a hot scrape loop.
#[derive(Clone, Debug)]
pub struct WgpuBackendStats {
    /// Adapter name the backend is bound to (e.g. `"NVIDIA GeForce RTX 5090"`).
    pub adapter_name: std::sync::Arc<str>,
    /// Live entries in the pipeline cache.
    pub pipeline_cache_entries: usize,
    /// Soft cap before eviction triggers.
    pub pipeline_cache_capacity: usize,
    /// Estimated bytes retained by the pipeline cache.
    pub pipeline_cache_bytes: usize,
    /// Estimated byte cap before eviction triggers.
    pub pipeline_cache_byte_capacity: usize,
    /// Pipeline-cache lookup hits since backend construction.
    pub pipeline_cache_hits: u64,
    /// Pipeline-cache lookup misses since backend construction.
    pub pipeline_cache_misses: u64,
    /// Pipeline-cache insertions since backend construction.
    pub pipeline_cache_insertions: u64,
    /// Capacity-driven pipeline-cache evictions since backend construction.
    pub pipeline_cache_evictions: u64,
    /// Hit ratio over all pipeline-cache lookups.
    pub pipeline_cache_hit_rate: f64,
    /// Persistent buffer pool counters (allocations, hits, releases, evictions).
    pub persistent_pool: crate::buffer::BufferPoolStats,
}

impl WgpuBackend {
    /// Optimizer-facing capability snapshot for this live backend.
    ///
    /// Unlike adapter-only probes, this reflects the features that were
    /// actually enabled on the device after backend construction.
    #[must_use]
    pub fn adapter_caps(&self) -> vyre_foundation::optimizer::AdapterCaps {
        self.device_profile().into()
    }

    /// Driver-neutral capability profile for this live backend.
    #[must_use]
    pub fn device_profile(&self) -> vyre_driver::DeviceProfile {
        crate::runtime::adapter_caps_probe::from_backend_profile(
            &self.adapter_info,
            &self.device_limits,
            &self.enabled_features,
        )
    }

    /// Observability snapshot  -  pipeline cache size, buffer-pool
    /// stats, and adapter identity. SRE-friendly: consumers feed the
    /// returned numbers into prometheus / OpenTelemetry / Datadog
    /// pipelines for dashboards and alerting.
    ///
    /// Reads use atomic cache counters and the lock-free persistent-pool
    /// pointer, so the call is safe for metrics-scrape loops.
    #[must_use]
    pub fn stats(&self) -> WgpuBackendStats {
        let persistent_pool = self.current_persistent_pool().stats();
        let pipeline_cache_hits = self.pipeline_cache.hits();
        let pipeline_cache_misses = self.pipeline_cache.misses();
        let pipeline_cache_lookup_rate_denominator =
            pipeline_cache_hits as f64 + pipeline_cache_misses as f64;
        let pipeline_cache_hit_rate = if pipeline_cache_lookup_rate_denominator == 0.0 {
            0.0
        } else {
            pipeline_cache_hits as f64 / pipeline_cache_lookup_rate_denominator
        };
        WgpuBackendStats {
            adapter_name: Arc::clone(&self.adapter_name),
            pipeline_cache_entries: self.pipeline_cache.len(),
            pipeline_cache_capacity: self.pipeline_cache.max_entries(),
            pipeline_cache_bytes: self.pipeline_cache.cached_bytes(),
            pipeline_cache_byte_capacity: self.pipeline_cache.max_bytes(),
            pipeline_cache_hits,
            pipeline_cache_misses,
            pipeline_cache_insertions: self.pipeline_cache.insertions(),
            pipeline_cache_evictions: self.pipeline_cache.evictions(),
            pipeline_cache_hit_rate,
            persistent_pool,
        }
    }
}
