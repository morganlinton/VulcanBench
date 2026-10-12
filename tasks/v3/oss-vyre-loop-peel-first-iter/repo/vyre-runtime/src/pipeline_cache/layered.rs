//! [`LayeredPipelineCache`]  -  composite store that reads from every
//! backend in order and writes only to the first.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::fingerprint::PipelineFingerprint;
use super::metrics::PipelineCacheMetrics;
use super::store::PipelineCacheStore;

/// Composite store that reads from every backend and writes to
/// the first. Lets callers compose `[RamStore, DiskStore, RemoteStore]`
/// so a miss at the fast layer falls through to slower layers.
pub struct LayeredPipelineCache {
    layers: Vec<Arc<dyn PipelineCacheStore>>,
    promotions: LayeredPromotionCounters,
}

impl LayeredPipelineCache {
    /// Construct from an ordered list (fastest-first). Lookups
    /// consult every layer in order; writes land in the first layer
    /// only  -  downstream layers are expected to be populated
    /// independently (e.g., from a pre-compiled blob bundle).
    #[must_use]
    pub fn new(layers: Vec<Arc<dyn PipelineCacheStore>>) -> Self {
        Self {
            layers,
            promotions: LayeredPromotionCounters::default(),
        }
    }

    /// Snapshot promotion evidence for lower-layer hits copied into faster
    /// preceding layers.
    #[must_use]
    pub fn promotion_report(&self) -> LayeredPromotionReport {
        self.promotions.snapshot()
    }

    fn promote_hit_to_faster_layers(
        &self,
        fp: PipelineFingerprint,
        artifact: &Arc<Vec<u8>>,
        source_layer: usize,
    ) {
        if source_layer == 0 {
            return;
        }
        let promoted_bytes = artifact.len() as u64;
        let mut promoted_layers = 0u64;
        for layer in &self.layers[..source_layer] {
            layer.put(fp, artifact.as_ref().clone());
            promoted_layers = promoted_layers.saturating_add(1);
        }
        self.promotions.record(
            source_layer,
            promoted_layers,
            promoted_bytes.saturating_mul(promoted_layers),
        );
    }
}

#[derive(Debug, Default)]
struct LayeredPromotionCounters {
    events: AtomicU64,
    promoted_layers: AtomicU64,
    promoted_bytes: AtomicU64,
    last_source_layer: AtomicU64,
    last_promoted_layers: AtomicU64,
    last_promoted_bytes: AtomicU64,
}

impl LayeredPromotionCounters {
    fn record(&self, source_layer: usize, promoted_layers: u64, promoted_bytes: u64) {
        self.events.fetch_add(1, Ordering::Relaxed);
        self.promoted_layers
            .fetch_add(promoted_layers, Ordering::Relaxed);
        self.promoted_bytes
            .fetch_add(promoted_bytes, Ordering::Relaxed);
        self.last_source_layer
            .store(source_layer as u64, Ordering::Relaxed);
        self.last_promoted_layers
            .store(promoted_layers, Ordering::Relaxed);
        self.last_promoted_bytes
            .store(promoted_bytes, Ordering::Relaxed);
    }

    fn snapshot(&self) -> LayeredPromotionReport {
        LayeredPromotionReport {
            events: self.events.load(Ordering::Relaxed),
            promoted_layers: self.promoted_layers.load(Ordering::Relaxed),
            promoted_bytes: self.promoted_bytes.load(Ordering::Relaxed),
            last_source_layer: self.last_source_layer.load(Ordering::Relaxed),
            last_promoted_layers: self.last_promoted_layers.load(Ordering::Relaxed),
            last_promoted_bytes: self.last_promoted_bytes.load(Ordering::Relaxed),
        }
    }
}

/// Layered-cache promotion evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayeredPromotionReport {
    /// Number of lower-layer hit events that caused promotion.
    pub events: u64,
    /// Total faster-layer writes caused by promotion.
    pub promoted_layers: u64,
    /// Total artifact bytes copied into faster layers.
    pub promoted_bytes: u64,
    /// Index of the most recent source layer that supplied a promoted hit.
    pub last_source_layer: u64,
    /// Number of faster layers written by the most recent promotion.
    pub last_promoted_layers: u64,
    /// Artifact bytes copied by the most recent promotion.
    pub last_promoted_bytes: u64,
}

impl PipelineCacheStore for LayeredPipelineCache {
    fn get(&self, fp: &PipelineFingerprint) -> Option<Vec<u8>> {
        self.get_arc(fp).map(|artifact| (*artifact).clone())
    }

    /// V7-PERF-009: forward through to each layer's zero-clone path so
    /// the hit propagates without an intermediate `Vec<u8>` allocation.
    fn get_arc(&self, fp: &PipelineFingerprint) -> Option<Arc<Vec<u8>>> {
        for (index, layer) in self.layers.iter().enumerate() {
            if let Some(arc) = layer.get_arc(fp) {
                self.promote_hit_to_faster_layers(*fp, &arc, index);
                return Some(arc);
            }
        }
        None
    }

    fn put(&self, fp: PipelineFingerprint, artifact: Vec<u8>) {
        if let Some(first) = self.layers.first() {
            first.put(fp, artifact);
        }
    }

    fn flush(&self) -> io::Result<()> {
        for layer in &self.layers {
            layer.flush()?;
        }
        Ok(())
    }

    fn metrics(&self) -> PipelineCacheMetrics {
        self.layers
            .iter()
            .fold(PipelineCacheMetrics::default(), |acc, layer| {
                acc.checked_add(layer.metrics())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_cache::test_helpers::tiny_program;
    use crate::pipeline_cache::InMemoryPipelineCache;

    #[test]
    fn layered_cache_prefers_first_hit() {
        let fast = Arc::new(InMemoryPipelineCache::new());
        let slow = Arc::new(InMemoryPipelineCache::new());
        let fp = PipelineFingerprint::of(&tiny_program());
        slow.put(fp, b"fallback".to_vec());
        let cache = LayeredPipelineCache::new(vec![fast.clone(), slow]);
        // Miss in fast, hit in slow.
        assert_eq!(cache.get(&fp).unwrap(), b"fallback".to_vec());
        // Put lands in fast only.
        cache.put(fp, b"warmed".to_vec());
        assert_eq!(fast.get(&fp).unwrap(), b"warmed".to_vec());
    }

    #[test]
    fn layered_cache_metrics_aggregate_layers() {
        let fast = Arc::new(InMemoryPipelineCache::new());
        let slow = Arc::new(InMemoryPipelineCache::new());
        let fp = PipelineFingerprint::of(&tiny_program());
        slow.put(fp, b"slow".to_vec());
        let cache = LayeredPipelineCache::new(vec![fast, slow]);

        assert_eq!(cache.get(&fp).unwrap(), b"slow".to_vec());
        let metrics = cache.metrics();
        assert_eq!(metrics.lookups, 2);
        assert_eq!(metrics.hits, 1);
        assert_eq!(metrics.misses, 1);
    }

    #[test]
    fn layered_cache_promotes_lower_layer_hit_to_faster_layers_with_report() {
        let fast = Arc::new(InMemoryPipelineCache::new());
        let slow = Arc::new(InMemoryPipelineCache::new());
        let fp = PipelineFingerprint::of(&tiny_program());
        slow.put(fp, b"fallback".to_vec());
        let cache = LayeredPipelineCache::new(vec![fast.clone(), slow]);

        assert_eq!(cache.get(&fp).unwrap(), b"fallback".to_vec());
        assert_eq!(fast.get(&fp).unwrap(), b"fallback".to_vec());

        let report = cache.promotion_report();
        assert_eq!(report.events, 1);
        assert_eq!(report.promoted_layers, 1);
        assert_eq!(report.promoted_bytes, b"fallback".len() as u64);
        assert_eq!(report.last_source_layer, 1);
        assert_eq!(report.last_promoted_layers, 1);
        assert_eq!(report.last_promoted_bytes, b"fallback".len() as u64);
    }
}
