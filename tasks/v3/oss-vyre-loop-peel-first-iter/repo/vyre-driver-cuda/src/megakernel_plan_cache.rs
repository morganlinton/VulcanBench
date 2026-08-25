//! Bounded CUDA megakernel plan cache.
//!
//! The cache stores topology decisions keyed by stable graph layout,
//! analysis family, CUDA device feature signature, and coarse runtime-pressure
//! buckets. The first three fields are the architectural identity of a plan;
//! pressure buckets prevent a sparse first query from poisoning dense later
//! queries over the same resident graph.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rustc_hash::FxHashMap;

use crate::backend::ordering::sort_unstable_by_key_if_needed;
use crate::backend::staging_reserve::reserve_vec;
use crate::device::CudaDeviceCaps;
use crate::megakernel_scheduler::{
    plan_cuda_megakernel_memory_budget, select_cuda_megakernel_topology,
    select_cuda_megakernel_topology_stable, CudaMegakernelExecutionPlan, CudaMegakernelGraphShape,
    CudaMegakernelMemoryBudget, CudaMegakernelMemoryError, CudaMegakernelScheduleSample,
    CudaMegakernelTopology, CudaMegakernelTopologyDecision,
};

const DEFAULT_MAX_MEGAKERNEL_PLANS: usize = 256;
const PRESSURE_BUCKET_BPS: u32 = 1_000;
const DENSITY_BUCKETS: u16 = 16;
const READBACK_BUCKET_SHIFT: u32 = 12;

/// Analysis family for a cached CUDA megakernel plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum CudaMegakernelAnalysisKind {
    /// Generic graph dataflow wave.
    Dataflow,
    /// IFDS/IDE-style exploded-supergraph propagation.
    Ifds,
    /// Reaching-definitions propagation.
    ReachingDefinitions,
    /// Live-variable propagation.
    Liveness,
    /// Points-to propagation.
    PointsTo,
    /// Source-token or parser-frontier wave.
    ParserFrontend,
    /// Caller-owned analysis family identified by a stable numeric tag.
    Custom(u64),
}

/// CUDA device feature signature that invalidates cached megakernel plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CudaMegakernelDeviceKey {
    /// CUDA SM major version.
    pub sm_major: u16,
    /// CUDA SM minor version.
    pub sm_minor: u16,
    /// Hardware warp size.
    pub warp_size: u16,
    /// Whether cooperative grid synchronization is available.
    pub supports_grid_sync: bool,
    /// Whether tensor-core lowering is available for this backend session.
    pub supports_tensor_cores: bool,
    /// Maximum threads accepted for one workgroup/block.
    pub max_workgroup_size: u32,
}

impl From<&CudaDeviceCaps> for CudaMegakernelDeviceKey {
    fn from(caps: &CudaDeviceCaps) -> Self {
        Self {
            sm_major: caps.compute_capability.0.min(u32::from(u16::MAX)) as u16,
            sm_minor: caps.compute_capability.1.min(u32::from(u16::MAX)) as u16,
            warp_size: caps.required_warp_size_u32().min(u32::from(u16::MAX)) as u16,
            supports_grid_sync: caps.compute_capability >= (6, 0) && caps.cooperative_launch,
            supports_tensor_cores: caps.hardware_supports_tensor_cores(),
            max_workgroup_size: caps.max_threads_per_block_u32(),
        }
    }
}

/// Stable key for cached CUDA megakernel plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CudaMegakernelPlanCacheKey {
    /// Stable hash of the normalized resident graph layout.
    pub graph_layout_hash: u64,
    /// Analysis family consuming the graph layout.
    pub analysis_kind: CudaMegakernelAnalysisKind,
    /// CUDA device feature signature.
    pub device: CudaMegakernelDeviceKey,
    /// Coarse active-frontier density bucket.
    pub frontier_density_bucket: u16,
    /// Coarse memory-pressure bucket in basis points.
    pub memory_pressure_bucket: u32,
    /// Coarse output/readback pressure bucket.
    pub readback_pressure_bucket: u16,
    /// Coarse launch-over-dispatch pressure bucket in basis points.
    pub launch_pressure_bucket: u32,
    /// Coarse caller-provided fusion-pressure bucket.
    pub fusion_pressure_bucket: u32,
}

impl CudaMegakernelPlanCacheKey {
    /// Build a cache key from stable identity fields and runtime pressure.
    #[must_use]
    pub fn new(
        graph_layout_hash: u64,
        analysis_kind: CudaMegakernelAnalysisKind,
        device: CudaMegakernelDeviceKey,
        frontier_density: f64,
        memory_pressure_bps: u32,
        readback_bytes: u64,
        launch_pressure_bps: u32,
        fusion_pressure: f64,
    ) -> Self {
        Self {
            graph_layout_hash,
            analysis_kind,
            device,
            frontier_density_bucket: density_bucket(frontier_density),
            memory_pressure_bucket: pressure_bucket(memory_pressure_bps),
            readback_pressure_bucket: readback_bucket(readback_bytes),
            launch_pressure_bucket: pressure_bucket(launch_pressure_bps),
            fusion_pressure_bucket: fusion_bucket(fusion_pressure),
        }
    }

    fn identity(self) -> CudaMegakernelPlanIdentityKey {
        CudaMegakernelPlanIdentityKey {
            graph_layout_hash: self.graph_layout_hash,
            analysis_kind: self.analysis_kind,
            device: self.device,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct CudaMegakernelPlanIdentityKey {
    graph_layout_hash: u64,
    analysis_kind: CudaMegakernelAnalysisKind,
    device: CudaMegakernelDeviceKey,
}

/// Cached CUDA megakernel plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CudaMegakernelCachedPlan {
    /// Selected topology for this key.
    pub topology: CudaMegakernelTopology,
    /// Full decision telemetry used when the plan was inserted.
    pub decision: CudaMegakernelTopologyDecision,
}

/// Runtime counters for [`CudaMegakernelPlanCache`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CudaMegakernelPlanCacheStats {
    /// Cache lookup hits.
    pub hits: u64,
    /// Cache lookup misses.
    pub misses: u64,
    /// Entries evicted by the bounded LRU policy.
    pub evictions: u64,
    /// Current entry count.
    pub entries: usize,
}

#[derive(Clone, Copy, Debug)]
struct CudaMegakernelPlanCacheEntry {
    plan: CudaMegakernelCachedPlan,
    last_seen: u64,
}

/// Bounded LRU cache for CUDA megakernel topology plans.
#[derive(Debug)]
pub struct CudaMegakernelPlanCache {
    entries: FxHashMap<CudaMegakernelPlanCacheKey, CudaMegakernelPlanCacheEntry>,
    latest_by_identity: FxHashMap<CudaMegakernelPlanIdentityKey, (u64, CudaMegakernelTopology)>,
    eviction_queue: BinaryHeap<Reverse<(u64, CudaMegakernelPlanCacheKey)>>,
    max_entries: usize,
    serial: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
}

fn increment_plan_cache_counter(counter: &mut u64, field: &'static str) {
    vyre_driver::accounting::pinning_increment_u64(counter, || {
        tracing::error!(
            "CUDA megakernel {field} overflowed u64; pinning counter at u64::MAX. Fix: scrape metrics more frequently or shard the cache."
        );
    });
}

impl Default for CudaMegakernelPlanCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CudaMegakernelPlanCache {
    /// Create a cache with the default production entry bound.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_entries(DEFAULT_MAX_MEGAKERNEL_PLANS)
    }

    /// Create a cache with an explicit entry bound.
    #[must_use]
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            entries: FxHashMap::default(),
            latest_by_identity: FxHashMap::default(),
            eviction_queue: BinaryHeap::new(),
            max_entries,
            serial: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Return a cached plan or insert a newly selected topology decision.
    pub fn get_or_insert_with(
        &mut self,
        key: CudaMegakernelPlanCacheKey,
        build: impl FnOnce() -> CudaMegakernelTopologyDecision,
    ) -> Result<CudaMegakernelCachedPlan, CudaMegakernelMemoryError> {
        let serial = self.advance_serial()?;
        if let Some(entry) = self.entries.get_mut(&key) {
            increment_plan_cache_counter(&mut self.hits, "megakernel plan-cache hit counter");
            entry.last_seen = serial;
            let plan = entry.plan;
            self.eviction_queue.push(Reverse((serial, key)));
            self.update_latest_identity(key.identity(), serial, plan.topology);
            return Ok(plan);
        }
        increment_plan_cache_counter(&mut self.misses, "megakernel plan-cache miss counter");
        if self.max_entries == 0 {
            let decision = build();
            return Ok(CudaMegakernelCachedPlan {
                topology: decision.topology,
                decision,
            });
        }
        self.evict_until_below_limit()?;
        let decision = build();
        let plan = CudaMegakernelCachedPlan {
            topology: decision.topology,
            decision,
        };
        self.entries.insert(
            key,
            CudaMegakernelPlanCacheEntry {
                plan,
                last_seen: serial,
            },
        );
        self.eviction_queue.push(Reverse((serial, key)));
        self.update_latest_identity(key.identity(), serial, plan.topology);
        Ok(plan)
    }

    /// Return a cached topology plan or select and cache one from the current
    /// CUDA telemetry sample.
    ///
    /// This is the hot-path convenience API: callers provide stable graph,
    /// analysis, device, and telemetry inputs, while the cache owns the
    /// pressure bucketing needed to avoid stale sparse/dense decisions.
    pub fn get_or_select_topology(
        &mut self,
        graph_layout_hash: u64,
        analysis_kind: CudaMegakernelAnalysisKind,
        device: CudaMegakernelDeviceKey,
        sample: CudaMegakernelScheduleSample,
        graph: CudaMegakernelGraphShape,
        memory: CudaMegakernelMemoryBudget,
        launch_overhead_ns: f64,
        fusion_pressure: f64,
    ) -> Result<CudaMegakernelCachedPlan, CudaMegakernelMemoryError> {
        let effective_fusion_pressure = if device.supports_grid_sync {
            fusion_pressure
        } else {
            0.0
        };
        let key = CudaMegakernelPlanCacheKey::new(
            graph_layout_hash,
            analysis_kind,
            device,
            sample.frontier_density,
            pressure_bps(memory.required_bytes, memory.budget_bytes),
            sample.readback_bytes,
            launch_pressure_bps(sample.dispatch_cost_ns, launch_overhead_ns),
            effective_fusion_pressure,
        );
        let previous_topology =
            self.latest_topology_for_identity(graph_layout_hash, analysis_kind, device);
        self.get_or_insert_with(key, || {
            if let Some(previous_topology) = previous_topology {
                select_cuda_megakernel_topology_stable(
                    sample,
                    graph,
                    memory,
                    launch_overhead_ns,
                    effective_fusion_pressure,
                    previous_topology,
                )
            } else {
                select_cuda_megakernel_topology(
                    sample,
                    graph,
                    memory,
                    launch_overhead_ns,
                    effective_fusion_pressure,
                )
            }
        })
    }

    /// Return a cache-backed, memory-validated CUDA megakernel execution plan.
    ///
    /// The cache key uses sparse-plan memory pressure because sparse is the
    /// lower-bound resident footprint shared by every topology. A cache hit
    /// reuses the prior topology decision, then this method validates the exact
    /// current dense/fused/sparse byte budget before returning a launchable
    /// plan. If the cached non-sparse topology no longer fits, the method
    /// downgrades to sparse only after proving the sparse plan fits.
    pub fn get_or_plan_execution(
        &mut self,
        graph_layout_hash: u64,
        analysis_kind: CudaMegakernelAnalysisKind,
        device: CudaMegakernelDeviceKey,
        sample: CudaMegakernelScheduleSample,
        graph: CudaMegakernelGraphShape,
        bytes_per_node: u64,
        bytes_per_edge: u64,
        frontier_bytes: u64,
        scratch_bytes: u64,
        output_bytes: u64,
        budget_bytes: u64,
        launch_overhead_ns: f64,
        fusion_pressure: f64,
    ) -> Result<CudaMegakernelExecutionPlan, CudaMegakernelMemoryError> {
        let sparse_memory = plan_cuda_megakernel_memory_budget(
            CudaMegakernelTopology::SparseFrontier,
            graph,
            bytes_per_node,
            bytes_per_edge,
            frontier_bytes,
            scratch_bytes,
            output_bytes,
            u64::MAX,
        )?;
        let cached = self.get_or_select_topology(
            graph_layout_hash,
            analysis_kind,
            device,
            sample,
            graph,
            CudaMegakernelMemoryBudget {
                required_bytes: sparse_memory.required_bytes,
                budget_bytes,
            },
            launch_overhead_ns,
            fusion_pressure,
        )?;
        match plan_cuda_megakernel_memory_budget(
            cached.topology,
            graph,
            bytes_per_node,
            bytes_per_edge,
            frontier_bytes,
            scratch_bytes,
            output_bytes,
            budget_bytes,
        ) {
            Ok(memory) => Ok(CudaMegakernelExecutionPlan {
                topology: cached.topology,
                memory,
                downgraded_to_sparse: false,
            }),
            Err(CudaMegakernelMemoryError::OverBudget { .. })
                if cached.topology != CudaMegakernelTopology::SparseFrontier =>
            {
                let memory = plan_cuda_megakernel_memory_budget(
                    CudaMegakernelTopology::SparseFrontier,
                    graph,
                    bytes_per_node,
                    bytes_per_edge,
                    frontier_bytes,
                    scratch_bytes,
                    output_bytes,
                    budget_bytes,
                )?;
                Ok(CudaMegakernelExecutionPlan {
                    topology: CudaMegakernelTopology::SparseFrontier,
                    memory,
                    downgraded_to_sparse: true,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Return cache counters.
    #[must_use]
    pub fn stats(&self) -> CudaMegakernelPlanCacheStats {
        CudaMegakernelPlanCacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            entries: self.entries.len(),
        }
    }

    /// Drop every cached plan and preserve counters for observability.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.latest_by_identity.clear();
        self.eviction_queue.clear();
    }

    fn latest_topology_for_identity(
        &self,
        graph_layout_hash: u64,
        analysis_kind: CudaMegakernelAnalysisKind,
        device: CudaMegakernelDeviceKey,
    ) -> Option<CudaMegakernelTopology> {
        self.latest_by_identity
            .get(&CudaMegakernelPlanIdentityKey {
                graph_layout_hash,
                analysis_kind,
                device,
            })
            .map(|(_, topology)| *topology)
    }

    fn update_latest_identity(
        &mut self,
        identity: CudaMegakernelPlanIdentityKey,
        serial: u64,
        topology: CudaMegakernelTopology,
    ) {
        match self.latest_by_identity.get(&identity) {
            Some((latest_serial, _)) if *latest_serial > serial => {}
            _ => {
                self.latest_by_identity.insert(identity, (serial, topology));
            }
        }
    }

    fn recompute_latest_identity(&mut self, identity: CudaMegakernelPlanIdentityKey) {
        let latest = self
            .entries
            .iter()
            .filter(|(key, _)| key.identity() == identity)
            .max_by_key(|(_, entry)| entry.last_seen)
            .map(|(_, entry)| (entry.last_seen, entry.plan.topology));
        if let Some(latest) = latest {
            self.latest_by_identity.insert(identity, latest);
        } else {
            self.latest_by_identity.remove(&identity);
        }
    }

    fn evict_until_below_limit(&mut self) -> Result<(), CudaMegakernelMemoryError> {
        while self.entries.len() >= self.max_entries {
            let Some(Reverse((last_seen, lru_key))) = self.eviction_queue.pop() else {
                break;
            };
            let Some(entry) = self.entries.get(&lru_key) else {
                continue;
            };
            if entry.last_seen != last_seen {
                continue;
            }
            let identity = lru_key.identity();
            let evicted_topology = entry.plan.topology;
            self.entries.remove(&lru_key);
            if matches!(
                self.latest_by_identity.get(&identity),
                Some((latest_seen, latest_topology))
                    if *latest_seen == last_seen && *latest_topology == evicted_topology
            ) {
                self.recompute_latest_identity(identity);
            }
            increment_plan_cache_counter(
                &mut self.evictions,
                "megakernel plan-cache eviction counter",
            );
        }
        Ok(())
    }

    fn advance_serial(&mut self) -> Result<u64, CudaMegakernelMemoryError> {
        if let Some(next) = self.serial.checked_add(1) {
            self.serial = next;
            return Ok(next);
        }
        self.rebase_lru_serials()?;
        self.serial =
            self.serial
                .checked_add(1)
                .ok_or(CudaMegakernelMemoryError::ByteCountOverflow {
                    field: "megakernel plan-cache LRU serial after rebase",
                })?;
        Ok(self.serial)
    }

    fn rebase_lru_serials(&mut self) -> Result<(), CudaMegakernelMemoryError> {
        let mut ordered = Vec::new();
        reserve_vec(
            &mut ordered,
            self.entries.len(),
            "megakernel plan-cache LRU rebase scratch",
        )
        .map_err(|_| CudaMegakernelMemoryError::ByteCountOverflow {
            field: "megakernel plan-cache LRU rebase scratch",
        })?;
        for (key, entry) in &self.entries {
            ordered.push((entry.last_seen, *key));
        }
        sort_unstable_by_key_if_needed(&mut ordered, |(last_seen, key)| (*last_seen, *key));
        self.eviction_queue.clear();
        self.latest_by_identity.clear();
        let mut serial = 0_u64;
        for (_, key) in ordered {
            serial = serial
                .checked_add(1)
                .ok_or(CudaMegakernelMemoryError::ByteCountOverflow {
                    field: "megakernel plan-cache LRU rebase serial",
                })?;
            let topology = if let Some(entry) = self.entries.get_mut(&key) {
                entry.last_seen = serial;
                Some(entry.plan.topology)
            } else {
                None
            };
            if let Some(topology) = topology {
                self.eviction_queue.push(Reverse((serial, key)));
                self.update_latest_identity(key.identity(), serial, topology);
            }
        }
        self.serial = serial;
        Ok(())
    }
}

fn density_bucket(frontier_density: f64) -> u16 {
    if !frontier_density.is_finite() {
        return 0;
    }
    let clamped = frontier_density.clamp(0.0, 1.0);
    rounded_f64_to_u16_bucket(
        clamped * f64::from(DENSITY_BUCKETS - 1),
        "frontier-density bucket",
    )
}

fn pressure_bucket(memory_pressure_bps: u32) -> u32 {
    memory_pressure_bps / PRESSURE_BUCKET_BPS
}

fn pressure_bps(numerator: u64, denominator: u64) -> u32 {
    crate::numeric::CUDA_NUMERIC.ratio_basis_points_u64(
        numerator,
        denominator,
        if numerator == 0 { 0 } else { u32::MAX },
        "megakernel pressure",
    )
}

fn launch_pressure_bps(dispatch_cost_ns: f64, launch_overhead_ns: f64) -> u32 {
    crate::numeric::CUDA_NUMERIC.finite_f64_ratio_basis_points_trunc(
        launch_overhead_ns,
        dispatch_cost_ns,
        u32::MAX,
        0,
        "launch-pressure basis-points",
    )
}

fn readback_bucket(readback_bytes: u64) -> u16 {
    if readback_bytes == 0 {
        return 0;
    }
    let shifted = readback_bytes >> READBACK_BUCKET_SHIFT;
    let bucket = u64::BITS - shifted.leading_zeros();
    bucket.min(u32::from(u16::MAX)) as u16
}

fn fusion_bucket(fusion_pressure: f64) -> u32 {
    pressure_bucket(
        crate::numeric::CUDA_NUMERIC.finite_f64_unit_basis_points_trunc(
            fusion_pressure,
            0,
            "fusion-pressure basis-points",
        ),
    )
}

fn rounded_f64_to_u16_bucket(value: f64, label: &'static str) -> u16 {
    let rounded = value.round();
    if !rounded.is_finite() || rounded < 0.0 || rounded > f64::from(u16::MAX) {
        tracing::error!(
            "CUDA megakernel {label} value {rounded} cannot fit u16. Fix: reduce bucket resolution or shard cache domains."
        );
        return if rounded.is_sign_negative() {
            0
        } else {
            u16::MAX
        };
    }
    rounded as u16
}

#[cfg(test)]
mod tests {
    use super::{
        CudaMegakernelAnalysisKind, CudaMegakernelDeviceKey, CudaMegakernelPlanCache,
        CudaMegakernelPlanCacheKey,
    };
    use crate::megakernel_scheduler::{
        CudaMegakernelGraphShape, CudaMegakernelScheduleSample, CudaMegakernelTopology,
        CudaMegakernelTopologyDecision,
    };
    use crate::synthetic_device_caps::blackwell_sm120_caps_default;

    fn device() -> CudaMegakernelDeviceKey {
        CudaMegakernelDeviceKey {
            sm_major: 12,
            sm_minor: 0,
            warp_size: 32,
            supports_grid_sync: true,
            supports_tensor_cores: true,
            max_workgroup_size: 1024,
        }
    }

    fn key(
        graph_layout_hash: u64,
        analysis_kind: CudaMegakernelAnalysisKind,
        frontier_density: f64,
        memory_pressure_bps: u32,
    ) -> CudaMegakernelPlanCacheKey {
        CudaMegakernelPlanCacheKey::new(
            graph_layout_hash,
            analysis_kind,
            device(),
            frontier_density,
            memory_pressure_bps,
            0,
            0,
            0.0,
        )
    }

    fn decision(topology: CudaMegakernelTopology) -> CudaMegakernelTopologyDecision {
        CudaMegakernelTopologyDecision {
            topology,
            memory_pressure_bps: 1_000,
            average_degree_bps: 20_000,
            launch_pressure_bps: 2_000,
        }
    }

    #[test]
    fn cache_reuses_plan_for_same_graph_analysis_device_and_pressure_bucket() {
        let mut cache = CudaMegakernelPlanCache::new();
        let key = key(42, CudaMegakernelAnalysisKind::Ifds, 0.52, 2_400);
        let first = cache
            .get_or_insert_with(key, || decision(CudaMegakernelTopology::FusedWave))
            .expect("Fix: CUDA megakernel plan-cache insert should fit telemetry counters.");
        let second = cache
            .get_or_insert_with(key, || decision(CudaMegakernelTopology::SparseFrontier))
            .expect("Fix: CUDA megakernel plan-cache hit should fit telemetry counters.");

        assert_eq!(first, second);
        assert_eq!(second.topology, CudaMegakernelTopology::FusedWave);
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn device_key_is_derived_from_cuda_caps() {
        assert_eq!(
            CudaMegakernelDeviceKey::from(&blackwell_sm120_caps_default()),
            device()
        );
    }

    #[test]
    fn cache_separates_analysis_family_density_and_device_features() {
        let ifds = key(42, CudaMegakernelAnalysisKind::Ifds, 0.01, 1_000);
        let liveness = key(42, CudaMegakernelAnalysisKind::Liveness, 0.01, 1_000);
        let dense = key(42, CudaMegakernelAnalysisKind::Ifds, 0.95, 1_000);
        let mut other_device = device();
        other_device.sm_minor = 1;
        let device_changed = CudaMegakernelPlanCacheKey::new(
            42,
            CudaMegakernelAnalysisKind::Ifds,
            other_device,
            0.01,
            1_000,
            0,
            0,
            0.0,
        );

        assert_ne!(ifds, liveness);
        assert_ne!(ifds, dense);
        assert_ne!(ifds, device_changed);
    }

    #[test]
    fn bounded_cache_evicts_lru_entry() {
        let mut cache = CudaMegakernelPlanCache::with_max_entries(2);
        let first = key(1, CudaMegakernelAnalysisKind::Dataflow, 0.1, 1_000);
        let second = key(2, CudaMegakernelAnalysisKind::Dataflow, 0.1, 1_000);
        let third = key(3, CudaMegakernelAnalysisKind::Dataflow, 0.1, 1_000);

        cache
            .get_or_insert_with(first, || decision(CudaMegakernelTopology::SparseFrontier))
            .expect("Fix: CUDA megakernel plan-cache insert should fit telemetry counters.");
        cache
            .get_or_insert_with(second, || decision(CudaMegakernelTopology::HybridFrontier))
            .expect("Fix: CUDA megakernel plan-cache insert should fit telemetry counters.");
        cache
            .get_or_insert_with(first, || decision(CudaMegakernelTopology::DenseFrontier))
            .expect("Fix: CUDA megakernel plan-cache hit should fit telemetry counters.");
        cache
            .get_or_insert_with(third, || decision(CudaMegakernelTopology::FusedWave))
            .expect("Fix: CUDA megakernel plan-cache eviction should fit telemetry counters.");

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 2);
        let reloaded_second = cache
            .get_or_insert_with(second, || decision(CudaMegakernelTopology::DenseFrontier))
            .expect("Fix: CUDA megakernel plan-cache reload should fit telemetry counters.");
        assert_eq!(
            reloaded_second.topology,
            CudaMegakernelTopology::DenseFrontier
        );
    }

    #[test]
    fn cache_selects_topology_and_reuses_pressure_bucket_plan() {
        let mut cache = CudaMegakernelPlanCache::new();
        let sample = crate::megakernel_scheduler::CudaMegakernelScheduleSample {
            dispatch_cost_ns: 1_000.0,
            frontier_density: 0.90,
            readback_bytes: 1 << 20,
        };
        let graph = crate::megakernel_scheduler::CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
            required_bytes: 1_024,
            budget_bytes: 16_384,
        };
        let first = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                sample,
                graph,
                memory,
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology selection should fit telemetry counters.");
        let second = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                crate::megakernel_scheduler::CudaMegakernelScheduleSample {
                    frontier_density: 0.91,
                    ..sample
                },
                graph,
                crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
                    required_bytes: 1_100,
                    budget_bytes: 16_384,
                },
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology cache hit should fit telemetry counters.");

        assert_eq!(first, second);
        assert_eq!(first.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn cache_stabilizes_topology_across_adjacent_pressure_buckets() {
        let mut cache = CudaMegakernelPlanCache::new();
        let graph = crate::megakernel_scheduler::CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
            required_bytes: 1_024,
            budget_bytes: 16_384,
        };
        let dense = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                crate::megakernel_scheduler::CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.70,
                    readback_bytes: 512,
                },
                graph,
                memory,
                100.0,
                0.0,
            )
            .expect("Fix: CUDA megakernel topology selection should fit telemetry counters.");
        let near_dense = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                crate::megakernel_scheduler::CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.68,
                    readback_bytes: 512,
                },
                graph,
                memory,
                100.0,
                0.0,
            )
            .expect("Fix: CUDA megakernel topology stabilization should fit telemetry counters.");

        assert_eq!(dense.topology, CudaMegakernelTopology::DenseFrontier);
        assert_eq!(near_dense.topology, CudaMegakernelTopology::DenseFrontier);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn cache_reselects_when_memory_pressure_bucket_changes() {
        let mut cache = CudaMegakernelPlanCache::new();
        let sample = crate::megakernel_scheduler::CudaMegakernelScheduleSample {
            dispatch_cost_ns: 1_000.0,
            frontier_density: 0.90,
            readback_bytes: 1 << 20,
        };
        let graph = crate::megakernel_scheduler::CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let low_pressure = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                sample,
                graph,
                crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
                    required_bytes: 1_024,
                    budget_bytes: 16_384,
                },
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology selection should fit telemetry counters.");
        let red_zone = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                sample,
                graph,
                crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
                    required_bytes: 15_500,
                    budget_bytes: 16_384,
                },
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology reselection should fit telemetry counters.");

        assert_eq!(low_pressure.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(red_zone.topology, CudaMegakernelTopology::SparseFrontier);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn cache_pressure_bucket_uses_exact_u128_math() {
        let low = CudaMegakernelPlanCacheKey::new(
            1,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            0.5,
            super::pressure_bps(1_u64 << 62, 1_u64 << 63),
            0,
            0,
            0.0,
        );
        let high = CudaMegakernelPlanCacheKey::new(
            1,
            CudaMegakernelAnalysisKind::Dataflow,
            device(),
            0.5,
            super::pressure_bps(1_u64 << 63, 1_u64 << 63),
            0,
            0,
            0.0,
        );

        assert_eq!(low.memory_pressure_bucket, 5);
        assert_eq!(high.memory_pressure_bucket, 10);
    }

    #[test]
    fn cache_reselects_when_readback_launch_or_fusion_pressure_changes() {
        let mut cache = CudaMegakernelPlanCache::new();
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let memory = crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
            required_bytes: 1_024,
            budget_bytes: 16_384,
        };
        let low_pressure = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.50,
                    readback_bytes: 0,
                },
                graph,
                memory,
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology selection should fit telemetry counters.");
        let high_pressure = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.50,
                    readback_bytes: 1 << 20,
                },
                graph,
                memory,
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology pressure split should fit telemetry counters.");

        assert_ne!(low_pressure.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(high_pressure.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn cache_never_selects_fused_wave_without_grid_sync_support() {
        let mut cache = CudaMegakernelPlanCache::new();
        let mut no_grid_sync = device();
        no_grid_sync.supports_grid_sync = false;

        let plan = cache
            .get_or_select_topology(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                no_grid_sync,
                CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.50,
                    readback_bytes: 1 << 20,
                },
                CudaMegakernelGraphShape {
                    node_count: 1_000,
                    edge_count: 4_000,
                },
                crate::megakernel_scheduler::CudaMegakernelMemoryBudget {
                    required_bytes: 1_024,
                    budget_bytes: 16_384,
                },
                250.0,
                0.95,
            )
            .expect("Fix: CUDA megakernel topology selection should fit telemetry counters.");

        assert_ne!(
            plan.topology,
            CudaMegakernelTopology::FusedWave,
            "Fix: CUDA megakernel planner must not select cooperative fused-wave topology when the device key says grid sync is unavailable."
        );
    }

    #[test]
    fn cached_execution_plan_reuses_topology_bucket_and_validates_memory() {
        let mut cache = CudaMegakernelPlanCache::new();
        let sample = CudaMegakernelScheduleSample {
            dispatch_cost_ns: 1_000.0,
            frontier_density: 0.90,
            readback_bytes: 1 << 20,
        };
        let graph = CudaMegakernelGraphShape {
            node_count: 1_000,
            edge_count: 4_000,
        };
        let first = cache
            .get_or_plan_execution(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                sample,
                graph,
                16,
                8,
                4_096,
                2_048,
                512,
                128 * 1024,
                250.0,
                0.95,
            )
            .expect("Fix: cache-backed fused CUDA execution plan should fit the explicit budget.");
        let second = cache
            .get_or_plan_execution(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                CudaMegakernelScheduleSample {
                    frontier_density: 0.91,
                    ..sample
                },
                graph,
                16,
                8,
                4_096,
                2_048,
                512,
                128 * 1024,
                250.0,
                0.95,
            )
            .expect("Fix: equivalent CUDA execution pressure bucket should reuse the cached topology and still validate memory.");

        assert_eq!(first.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(second.topology, CudaMegakernelTopology::FusedWave);
        assert_eq!(second.memory.scratch_bytes, 8_192);
        assert!(!second.downgraded_to_sparse);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn cached_execution_plan_downgrades_non_sparse_topology_when_exact_budget_fails() {
        let mut cache = CudaMegakernelPlanCache::new();
        let plan = cache
            .get_or_plan_execution(
                99,
                CudaMegakernelAnalysisKind::Dataflow,
                device(),
                CudaMegakernelScheduleSample {
                    dispatch_cost_ns: 1_000.0,
                    frontier_density: 0.50,
                    readback_bytes: 1 << 20,
                },
                CudaMegakernelGraphShape {
                    node_count: 1_000,
                    edge_count: 4_000,
                },
                16,
                8,
                4_096,
                10_000,
                512,
                80_000,
                250.0,
                0.90,
            )
            .expect("Fix: sparse CUDA downgrade must fit after cached fused topology exceeds exact budget.");

        assert_eq!(plan.topology, CudaMegakernelTopology::SparseFrontier);
        assert!(plan.downgraded_to_sparse);
        assert_eq!(plan.memory.scratch_bytes, 10_000);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().entries, 1);
    }

    #[test]
    fn cache_rebases_lru_serial_instead_of_failing_dispatch() {
        let mut cache = CudaMegakernelPlanCache::with_max_entries(2);
        let first = key(1, CudaMegakernelAnalysisKind::Ifds, 0.10, 1_000);
        let second = key(2, CudaMegakernelAnalysisKind::Ifds, 0.20, 1_000);
        cache
            .get_or_insert_with(first, || decision(CudaMegakernelTopology::SparseFrontier))
            .expect("Fix: first plan insert should fit");
        cache
            .get_or_insert_with(second, || decision(CudaMegakernelTopology::DenseFrontier))
            .expect("Fix: second plan insert should fit");
        cache.serial = u64::MAX;

        cache
            .get_or_insert_with(first, || decision(CudaMegakernelTopology::FusedWave))
            .expect(
                "Fix: LRU serial exhaustion must rebase instead of failing the CUDA dispatch path",
            );

        let first_seen = cache
            .entries
            .get(&first)
            .expect("Fix: first entry must remain")
            .last_seen;
        let second_seen = cache
            .entries
            .get(&second)
            .expect("Fix: second entry must remain")
            .last_seen;
        assert!(first_seen > second_seen);
        assert_eq!(cache.stats().hits, 1);
    }

    #[test]
    fn cache_counters_pin_instead_of_failing_dispatch() {
        let mut cache = CudaMegakernelPlanCache::new();
        let key = key(3, CudaMegakernelAnalysisKind::Ifds, 0.10, 1_000);
        cache
            .get_or_insert_with(key, || decision(CudaMegakernelTopology::SparseFrontier))
            .expect("Fix: plan insert should fit");
        cache.hits = u64::MAX;

        cache
            .get_or_insert_with(key, || decision(CudaMegakernelTopology::DenseFrontier))
            .expect("Fix: counter exhaustion must not fail the CUDA dispatch path");

        assert_eq!(cache.stats().hits, u64::MAX);
    }

    #[test]
    fn cache_eviction_is_queue_backed_not_map_scanned() {
        let src = include_str!("megakernel_plan_cache.rs");
        assert!(
            !src.contains(concat!(".iter()", ".min_by_key")),
            "Fix: CUDA megakernel plan-cache eviction must use the ordered eviction queue, not scan every cached plan on cold insert."
        );
        assert!(
            src.contains("BinaryHeap<Reverse<(u64, CudaMegakernelPlanCacheKey)>>"),
            "Fix: CUDA megakernel plan cache must keep an ordered LRU queue for hot-path eviction."
        );
        assert!(
            src.contains("increment_plan_cache_counter")
                && !src.contains(concat!(".", "saturating_add")),
            "Fix: CUDA megakernel plan-cache telemetry counters must pin loudly on overflow without hiding it behind saturating_add."
        );
        assert!(
            !src.contains(concat!("panic!", "(\"Fix: CUDA megakernel plan-cache")),
            "Fix: CUDA megakernel plan-cache overflow must return typed planner errors instead of panicking."
        );
        let production = src
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: megakernel plan-cache source must contain production section");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".expect(")
                && !production.contains(".unwrap_or_else(")
                && !production.contains("assert!("),
            "Fix: CUDA megakernel plan-cache production pressure bucketing and accounting must not panic."
        );
        assert!(
            production.contains("pub memory_pressure_bucket: u32")
                && production.contains("pub launch_pressure_bucket: u32")
                && production.contains("pub fusion_pressure_bucket: u32")
                && production.contains("tracing::error!"),
            "Fix: CUDA megakernel plan-cache pressure buckets must be wide enough for release telemetry and overflow must remain observable."
        );
        assert!(
            !src.contains(concat!(".", "wrapping_add"))
                && src.contains("fn rebase_lru_serials")
                && src.contains("fn advance_serial"),
            "Fix: CUDA megakernel plan-cache LRU serial must rebase on overflow, not wrap or fail hot dispatch."
        );
        assert!(
            production.contains("use crate::backend::ordering::sort_unstable_by_key_if_needed;")
                && production.contains("sort_unstable_by_key_if_needed(&mut ordered"),
            "Fix: CUDA megakernel plan-cache LRU rebase must use the shared monotonic sort fast path instead of a bespoke unconditional sort."
        );
        assert!(
            !production.contains(".sort_unstable_by_key("),
            "Fix: CUDA megakernel plan-cache production code must not reintroduce unconditional key sorting."
        );
        let latest_lookup = production
            .split("fn latest_topology_for_identity")
            .nth(1)
            .expect("Fix: CUDA megakernel plan-cache must expose previous-topology lookup")
            .split("fn update_latest_identity")
            .next()
            .expect("Fix: CUDA megakernel plan-cache lookup function must be bounded");
        assert!(
            latest_lookup.contains("latest_by_identity")
                && latest_lookup.contains(".get(&CudaMegakernelPlanIdentityKey")
                && !latest_lookup.contains(".iter()"),
            "Fix: previous-topology lookup must use the identity index instead of scanning every cached plan on cache miss."
        );
    }
}
