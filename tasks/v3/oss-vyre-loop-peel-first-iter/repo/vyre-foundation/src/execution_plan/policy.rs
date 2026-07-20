//! Shared scheduling and launch-shape policy for execution backends.

use crate::optimizer::AdapterCaps;

/// Backend route category emitted by the shared scheduling policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PolicyRoute {
    /// Explicit diagnostic/reference route.
    ///
    /// `SchedulingPolicy::standard()` never emits this route; CPU execution is
    /// allowed only when a caller opts into an oracle/test policy.
    CpuSimd,
    /// Standard compiled GPU pipeline. Kept for explicit non-persistent
    /// policies and backend diagnostics; it is not the standard release route.
    GpuPipeline,
    /// Persistent megakernel runtime used by the standard release route.
    PersistentMegakernel,
}

/// Central contract for scheduling, routing, and launch-grid thresholds.
///
/// The values are private on purpose: callers ask policy questions instead of
/// copying numeric thresholds into each crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SchedulingPolicy {
    persistent_runtime_node_max: usize,
    cpu_fast_path_node_max: usize,
    cpu_fast_path_static_bytes_below: u64,
    megakernel_node_count_above: usize,
    fused_over_dispatch_multiplier: u64,
    default_worker_count: u32,
    occupancy_worker_divisor: u32,
    max_dispatch_workgroups: u32,
    powerful_invocation_threshold: u32,
    powerful_min_worker_groups: u32,
    min_workgroup_x: u32,
    default_workgroup_x: u32,
    max_portable_workgroup_x: u32,
}

impl Default for SchedulingPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

impl SchedulingPolicy {
    /// Return the standard CUDA-first megakernel policy used by vyre's built-in planners.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            persistent_runtime_node_max: 64,
            cpu_fast_path_node_max: 64,
            cpu_fast_path_static_bytes_below: 1 << 16,
            megakernel_node_count_above: 1024,
            fused_over_dispatch_multiplier: 4,
            default_worker_count: 64,
            occupancy_worker_divisor: 256,
            max_dispatch_workgroups: 1024,
            powerful_invocation_threshold: 4096,
            powerful_min_worker_groups: 64,
            min_workgroup_x: 32,
            default_workgroup_x: 64,
            max_portable_workgroup_x: 256,
        }
    }

    /// Return true when a program should use persistent runtime dispatch.
    #[must_use]
    pub const fn use_persistent_runtime(&self, _node_count: usize) -> bool {
        true
    }

    /// Return true when dispatch-shape autotuning should measure variants.
    #[must_use]
    pub const fn recommend_autotune(&self, node_count: usize) -> bool {
        node_count > self.persistent_runtime_node_max
    }

    /// Route a plan represented by node count and static bytes.
    #[must_use]
    pub const fn route(&self, node_count: usize, static_bytes: u64) -> PolicyRoute {
        if self.use_persistent_megakernel(node_count) {
            PolicyRoute::PersistentMegakernel
        } else if self.use_cpu_fast_path(node_count, static_bytes) {
            PolicyRoute::CpuSimd
        } else {
            PolicyRoute::GpuPipeline
        }
    }

    /// Return true when a tiny static workload should stay on CPU SIMD.
    ///
    /// The standard release policy is GPU-only and therefore returns `false`;
    /// CPU SIMD remains an explicit oracle/test concept, not an implicit route.
    #[must_use]
    pub const fn use_cpu_fast_path(&self, _node_count: usize, _static_bytes: u64) -> bool {
        false
    }

    /// Return true when the persistent megakernel is the preferred route.
    #[must_use]
    pub const fn use_persistent_megakernel(&self, _node_count: usize) -> bool {
        true
    }

    /// Return true when an axis-wise fused launch stays within policy.
    #[must_use]
    pub const fn allow_fused_threads(&self, fused_threads: u64, max_arm_threads: u64) -> bool {
        fused_threads <= max_arm_threads.saturating_mul(self.fused_over_dispatch_multiplier)
    }

    /// Multiplier used to reject pathological axis-wise fused launch shapes.
    #[must_use]
    pub const fn fused_over_dispatch_multiplier(&self) -> u64 {
        self.fused_over_dispatch_multiplier
    }

    /// Default persistent worker workgroup count.
    #[must_use]
    pub const fn default_worker_count(&self) -> u32 {
        self.default_worker_count
    }

    /// Clamp a requested worker count into the legal workgroup x dimension.
    #[must_use]
    pub const fn worker_workgroup_size(&self, worker_count: u32, max_workgroup_size_x: u32) -> u32 {
        let max_workgroup_size_x = if max_workgroup_size_x > 1 {
            max_workgroup_size_x
        } else {
            1
        };
        if worker_count == 0 {
            1
        } else if worker_count > max_workgroup_size_x {
            max_workgroup_size_x
        } else {
            worker_count
        }
    }

    /// Round a logical slot count up to a whole worker workgroup.
    #[must_use]
    pub const fn padded_slot_count(&self, slot_count: u32, workgroup_size_x: u32) -> u32 {
        let workgroup_size_x = if workgroup_size_x > 1 {
            workgroup_size_x
        } else {
            1
        };
        let groups = slot_count
            .saturating_add(workgroup_size_x - 1)
            .saturating_div(workgroup_size_x);
        let groups = if groups > 1 { groups } else { 1 };
        groups.saturating_mul(workgroup_size_x)
    }

    /// Compute the backend dispatch grid for a logical queue length.
    #[must_use]
    pub const fn dispatch_grid_for(
        &self,
        worker_count: u32,
        queue_len: u32,
        max_workgroup_size_x: u32,
    ) -> [u32; 3] {
        let workgroup_width = if max_workgroup_size_x > 1 {
            max_workgroup_size_x
        } else {
            1
        };
        let requested_workers = if worker_count > 1 { worker_count } else { 1 };
        let workgroups = queue_len
            .saturating_add(workgroup_width - 1)
            .saturating_div(workgroup_width);
        let workgroups = if workgroups > 1 { workgroups } else { 1 };
        let final_workgroups = min3(workgroups, requested_workers, self.max_dispatch_workgroups);
        [final_workgroups, 1, 1]
    }

    /// Compute a persistent-worker ceiling from adapter limits.
    #[must_use]
    pub const fn default_worker_groups_from_limits(
        &self,
        max_compute_workgroups_per_dimension: u32,
        max_compute_invocations_per_workgroup: u32,
    ) -> u32 {
        let occupancy_based = clamp_between(
            max_compute_workgroups_per_dimension / self.occupancy_worker_divisor,
            1,
            self.max_dispatch_workgroups,
        );
        let min_for_powerful =
            if max_compute_invocations_per_workgroup >= self.powerful_invocation_threshold {
                self.powerful_min_worker_groups
            } else {
                1
            };
        if occupancy_based > min_for_powerful {
            occupancy_based
        } else {
            min_for_powerful
        }
    }

    /// Choose a 1-D workgroup width from real adapter limits and known shape.
    ///
    /// This is the deterministic fallback used when a backend has no live
    /// profiling loop available. It rejects illegal widths up front, estimates
    /// waves per workgroup from subgroup size, and prefers shapes that avoid
    /// tails when shape facts prove divisibility.
    #[must_use]
    pub fn select_workgroup_x(
        &self,
        declared_x: u32,
        problem_size: Option<u32>,
        caps: &AdapterCaps,
    ) -> u32 {
        let max_x = self.legal_workgroup_x_ceiling(caps);
        let min_x = self.min_workgroup_x.min(max_x).max(1);
        let floor = if caps.subgroup_size > 0 {
            caps.subgroup_size.min(max_x).max(1)
        } else {
            min_x
        };

        let declared = normalize_power_of_two(declared_x, min_x, max_x);
        if declared_x >= min_x
            && declared_x <= max_x
            && declared_x.is_power_of_two()
            && Self::workgroup_x_score(declared, problem_size, caps)
                >= Self::workgroup_x_score(
                    self.default_workgroup_x.min(max_x).max(min_x),
                    problem_size,
                    caps,
                )
        {
            return declared;
        }

        let mut best = normalize_power_of_two(self.default_workgroup_x, floor, max_x);
        let mut best_score = Self::workgroup_x_score(best, problem_size, caps);
        let mut candidate = floor.next_power_of_two().min(max_x).max(1);
        while candidate <= max_x {
            let score = Self::workgroup_x_score(candidate, problem_size, caps);
            if score > best_score || (score == best_score && candidate > best) {
                best = candidate;
                best_score = score;
            }
            match candidate.checked_mul(2) {
                Some(next) if next > candidate => candidate = next,
                _ => break,
            }
        }
        best
    }

    /// Choose the preferred workgroup tile for kernels whose lowering can
    /// consume a tile shape.
    #[must_use]
    pub fn select_workgroup_tile(
        &self,
        declared: [u32; 3],
        problem_size: Option<u32>,
        caps: &AdapterCaps,
    ) -> [u32; 3] {
        if legal_tile(caps.ideal_workgroup_tile, caps) {
            return caps.ideal_workgroup_tile;
        }
        if legal_tile(declared, caps) {
            return declared;
        }
        [
            self.select_workgroup_x(declared[0], problem_size, caps),
            1,
            1,
        ]
    }

    /// Choose a vector pack width in bits from device-signature facts.
    #[must_use]
    pub const fn select_vector_pack_bits(&self, element_bits: u32, caps: &AdapterCaps) -> u32 {
        let minimum = if element_bits > 0 { element_bits } else { 32 };
        let preferred = caps.ideal_vector_pack_bits;
        if preferred >= minimum && preferred % minimum == 0 {
            preferred
        } else if caps.l2_cache_bytes >= 32 * 1024 * 1024 && minimum <= 128 {
            128
        } else if minimum <= 64 {
            64
        } else {
            minimum
        }
    }

    /// Choose an unroll depth from device-signature facts and register limits.
    #[must_use]
    pub const fn select_unroll_depth(
        &self,
        loop_trip_count: Option<u32>,
        caps: &AdapterCaps,
    ) -> u32 {
        let mut preferred = if caps.ideal_unroll_depth > 0 {
            caps.ideal_unroll_depth
        } else if caps.regs_per_thread_max >= 128 {
            8
        } else {
            4
        };
        if caps.regs_per_thread_max > 0 && caps.regs_per_thread_max < 64 && preferred > 4 {
            preferred = 4;
        }
        if let Some(trip_count) = loop_trip_count {
            if trip_count > 0 && preferred > trip_count {
                preferred = trip_count;
            }
        }
        if preferred > 16 {
            16
        } else if preferred > 0 {
            preferred
        } else {
            1
        }
    }

    /// Maximum legal 1-D workgroup width for this adapter and policy.
    #[must_use]
    pub const fn legal_workgroup_x_ceiling(&self, caps: &AdapterCaps) -> u32 {
        let adapter_x = if caps.max_workgroup_size[0] > 0 {
            caps.max_workgroup_size[0]
        } else {
            1
        };
        let adapter_invocations = if caps.max_invocations_per_workgroup > 0 {
            caps.max_invocations_per_workgroup
        } else {
            adapter_x
        };
        let limit = min3(
            adapter_x,
            adapter_invocations,
            self.max_portable_workgroup_x,
        );
        if limit > 1 {
            limit
        } else {
            1
        }
    }

    fn workgroup_x_score(x: u32, problem_size: Option<u32>, caps: &AdapterCaps) -> u32 {
        let subgroup = effective_subgroup_size(caps);
        let waves = x.saturating_add(subgroup - 1).saturating_div(subgroup);
        let profile_preferred =
            preferred_workgroup_x(caps)
                .map_or(0, |preferred| if preferred == x { 1000 } else { 0 });
        let occupancy = waves.min(8).saturating_mul(100);
        let subgroup_fit = if x % subgroup == 0 { 250 } else { 0 };
        let specialization = if caps.supports_specialization_constants {
            30
        } else {
            0
        };
        let tail = match problem_size {
            Some(size) if size > 0 && size % x == 0 => 200,
            Some(size) if size > 0 => {
                let rem = size % x;
                120u32.saturating_sub(rem.saturating_mul(120) / x)
            }
            _ => 0,
        };
        occupancy
            .saturating_add(profile_preferred)
            .saturating_add(subgroup_fit)
            .saturating_add(specialization)
            .saturating_add(tail)
    }
}

fn legal_tile(tile: [u32; 3], caps: &AdapterCaps) -> bool {
    if tile.contains(&0) {
        return false;
    }
    let invocations = tile[0].saturating_mul(tile[1]).saturating_mul(tile[2]);
    invocations > 0
        && invocations <= caps.max_invocations_per_workgroup.max(1)
        && tile[0] <= caps.max_workgroup_size[0].max(1)
        && tile[1] <= caps.max_workgroup_size[1].max(1)
        && tile[2] <= caps.max_workgroup_size[2].max(1)
}

fn preferred_workgroup_x(caps: &AdapterCaps) -> Option<u32> {
    if !legal_tile(caps.ideal_workgroup_tile, caps) {
        return None;
    }
    Some(normalize_power_of_two(
        caps.ideal_workgroup_tile[0]
            .saturating_mul(caps.ideal_workgroup_tile[1])
            .saturating_mul(caps.ideal_workgroup_tile[2]),
        1,
        caps.max_workgroup_size[0]
            .min(caps.max_invocations_per_workgroup)
            .max(1),
    ))
}

const fn clamp_between(value: u32, min: u32, max: u32) -> u32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

const fn min3(a: u32, b: u32, c: u32) -> u32 {
    let ab = if a < b { a } else { b };
    if ab < c {
        ab
    } else {
        c
    }
}

const fn normalize_power_of_two(value: u32, min: u32, max: u32) -> u32 {
    let bounded = if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    };
    if bounded <= 1 {
        return 1;
    }
    if bounded.is_power_of_two() {
        bounded
    } else {
        1u32 << bounded.ilog2()
    }
}

const fn effective_subgroup_size(caps: &AdapterCaps) -> u32 {
    if caps.subgroup_size > 0 {
        caps.subgroup_size
    } else {
        32
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    fn policy() -> SchedulingPolicy {
        SchedulingPolicy::standard()
    }

    // --- Routing ---

    #[test]
    fn route_tiny_workload_uses_megakernel() {
        assert_eq!(
            policy().route(10, 100),
            PolicyRoute::PersistentMegakernel,
            "small node count + small bytes uses standard megakernel release path"
        );
    }

    #[test]
    fn route_large_bytes_uses_megakernel() {
        // Below node threshold but above static bytes threshold.
        assert_eq!(
            policy().route(10, 1 << 20),
            PolicyRoute::PersistentMegakernel,
            "small nodes but large bytes uses standard megakernel release path"
        );
    }

    #[test]
    fn route_large_node_count_uses_megakernel() {
        assert_eq!(
            policy().route(2000, 0),
            PolicyRoute::PersistentMegakernel,
            "2000 nodes → megakernel"
        );
    }

    #[test]
    fn route_medium_node_count_uses_megakernel() {
        assert_eq!(
            policy().route(500, 1 << 20),
            PolicyRoute::PersistentMegakernel,
            "500 nodes uses standard megakernel release path"
        );
    }

    // --- Persistent runtime ---

    #[test]
    fn persistent_runtime_is_standard_for_all_node_counts() {
        let p = policy();
        assert!(p.use_persistent_runtime(64));
        assert!(p.use_persistent_runtime(65));
    }

    // --- Autotune ---

    #[test]
    fn autotune_recommended_for_large_programs() {
        let p = policy();
        assert!(!p.recommend_autotune(64));
        assert!(p.recommend_autotune(65));
    }

    // --- Worker clamping ---

    #[test]
    fn worker_workgroup_size_clamps_to_max() {
        assert_eq!(policy().worker_workgroup_size(512, 256), 256);
    }

    #[test]
    fn worker_workgroup_size_zero_becomes_one() {
        assert_eq!(policy().worker_workgroup_size(0, 256), 1);
    }

    #[test]
    fn worker_workgroup_size_within_range_preserved() {
        assert_eq!(policy().worker_workgroup_size(128, 256), 128);
    }

    // --- Slot padding ---

    #[test]
    fn padded_slot_count_rounds_up() {
        assert_eq!(policy().padded_slot_count(65, 64), 128);
    }

    #[test]
    fn padded_slot_count_exact_multiple_unchanged() {
        assert_eq!(policy().padded_slot_count(128, 64), 128);
    }

    #[test]
    fn padded_slot_count_minimum_is_one_workgroup() {
        assert_eq!(policy().padded_slot_count(1, 64), 64);
    }

    // --- Dispatch grid ---

    #[test]
    fn dispatch_grid_single_workgroup() {
        let grid = policy().dispatch_grid_for(64, 32, 64);
        assert_eq!(grid, [1, 1, 1]);
    }

    #[test]
    fn dispatch_grid_capped_at_max() {
        let grid = policy().dispatch_grid_for(9999, 999999, 64);
        // Should be capped at max_dispatch_workgroups (1024).
        assert!(grid[0] <= 1024);
    }

    // --- Fusion limit ---

    #[test]
    fn allow_fused_threads_within_multiplier() {
        assert!(policy().allow_fused_threads(100, 100));
        assert!(policy().allow_fused_threads(400, 100)); // 4x
        assert!(!policy().allow_fused_threads(401, 100)); // >4x
    }

    // --- Default worker groups ---

    #[test]
    fn default_worker_groups_from_powerful_adapter() {
        let groups = policy().default_worker_groups_from_limits(65536, 4096);
        assert!(
            groups >= 64,
            "powerful adapter should get at least 64 groups: {groups}"
        );
    }

    #[test]
    fn default_worker_groups_from_weak_adapter() {
        let groups = policy().default_worker_groups_from_limits(256, 128);
        assert!(groups >= 1);
    }

    #[test]
    fn select_workgroup_uses_real_adapter_ceiling() {
        let caps = AdapterCaps {
            max_workgroup_size: [128, 1, 1],
            max_invocations_per_workgroup: 128,
            subgroup_size: 32,
            ..AdapterCaps::conservative()
        };
        assert_eq!(policy().select_workgroup_x(1024, Some(4096), &caps), 128);
    }

    #[test]
    fn select_workgroup_prefers_divisible_occupancy_shape() {
        let caps = AdapterCaps::high_end();
        assert_eq!(policy().select_workgroup_x(1, Some(4096), &caps), 256);
    }

    #[test]
    fn select_workgroup_respects_small_adapter() {
        let caps = AdapterCaps {
            max_workgroup_size: [16, 1, 1],
            max_invocations_per_workgroup: 16,
            subgroup_size: 8,
            ..AdapterCaps::conservative()
        };
        assert_eq!(policy().select_workgroup_x(1, Some(64), &caps), 16);
    }

    #[test]
    fn device_signature_tile_bias_changes_workgroup_choice() {
        let base = AdapterCaps {
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            subgroup_size: 32,
            ideal_workgroup_tile: [8, 8, 1],
            ..AdapterCaps::conservative()
        };
        let wide = AdapterCaps {
            ideal_workgroup_tile: [16, 16, 1],
            ..base
        };

        assert_eq!(policy().select_workgroup_x(1, Some(4096), &base), 64);
        assert_eq!(policy().select_workgroup_x(1, Some(4096), &wide), 256);
    }

    #[test]
    fn device_signature_selects_tile_vector_and_unroll() {
        let caps = AdapterCaps {
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            regs_per_thread_max: 255,
            l2_cache_bytes: 96 * 1024 * 1024,
            ideal_unroll_depth: 8,
            ideal_vector_pack_bits: 128,
            ideal_workgroup_tile: [16, 16, 1],
            ..AdapterCaps::conservative()
        };

        assert_eq!(
            policy().select_workgroup_tile([1, 1, 1], Some(4096), &caps),
            [16, 16, 1]
        );
        assert_eq!(policy().select_vector_pack_bits(32, &caps), 128);
        assert_eq!(policy().select_unroll_depth(Some(32), &caps), 8);
    }
}
