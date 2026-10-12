//! I4 substrate: occupancy-aware empirical autotuning.
//!
//! Given a probed [`CudaDeviceCaps`] snapshot and a kernel's measured
//! per-thread register pressure plus per-block shared-memory usage, compute
//! the expected hardware occupancy at a candidate workgroup size. The
//! workgroup-size picker chooses the candidate that maximises blocks/SM
//! within the device's hard limits (max_threads_per_block, warp alignment,
//! register and shared-memory ceilings).
//!
//! The estimator is intentionally pure (takes a [`CudaDeviceCaps`] by
//! reference, returns a value type) so it can be unit-tested without a
//! live CUDA context. Live ptxas register counts feed the
//! `regs_per_thread` parameter; `shared_bytes_per_block` is read directly
//! from the descriptor's shared bindings.

use crate::device::CudaDeviceCaps;

/// Per-kernel resource pressure required to compute occupancy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelResourceUsage {
    /// 32-bit registers used by each thread, as reported by ptxas
    /// `--ptxas-options=-v` for the JIT-compiled module.
    pub regs_per_thread: u32,
    /// Static shared memory bytes the kernel allocates per block.
    pub shared_bytes_per_block: u32,
}

/// Estimated occupancy at a given workgroup size on a given device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccupancyEstimate {
    /// Active blocks per streaming multiprocessor at this workgroup size.
    /// Zero when the workgroup configuration cannot run at all (exceeds
    /// per-block register or shared-memory ceiling).
    pub blocks_per_sm: u32,
    /// Active warps per SM (`blocks_per_sm * workgroup_size / warp_size`).
    pub warps_per_sm: u32,
    /// `warps_per_sm` as a fraction of the device's `max_warps_per_sm`,
    /// expressed in basis points (0..=10000) so the value is integer-only
    /// and comparable across configurations without floating-point.
    pub occupancy_bps: u32,
}

impl OccupancyEstimate {
    /// Sentinel for "this workgroup size cannot execute on this device."
    pub const ZERO: Self = Self {
        blocks_per_sm: 0,
        warps_per_sm: 0,
        occupancy_bps: 0,
    };

    /// Whether the configuration achieves at least one resident block.
    #[must_use]
    pub fn is_runnable(&self) -> bool {
        self.blocks_per_sm > 0
    }
}

/// Compute the occupancy estimate for `workgroup_size` threads/block on
/// `caps` given measured `usage`.
///
/// Returns [`OccupancyEstimate::ZERO`] when the workgroup is fundamentally
/// unrunnable (exceeds per-block register or shared-memory limits, or
/// exceeds `max_threads_per_block`). Otherwise the estimator takes the
/// minimum of:
///   - register-pressure cap: `max_registers_per_sm / (regs_per_thread * workgroup_size)`
///   - shared-memory cap: `shared_per_sm / shared_bytes_per_block`
///   - thread-residence cap: `max_threads_per_sm / workgroup_size`
#[must_use]
pub fn estimate_occupancy(
    caps: &CudaDeviceCaps,
    usage: KernelResourceUsage,
    workgroup_size: u32,
) -> OccupancyEstimate {
    let warp = match caps.warp_size_u32() {
        Some(w) if w > 0 => w,
        _ => return OccupancyEstimate::ZERO,
    };
    if workgroup_size == 0 || workgroup_size > caps.max_threads_per_block_u32() {
        return OccupancyEstimate::ZERO;
    }
    let max_regs_block = caps.max_registers_per_block_u32();
    let max_regs_sm = caps.max_registers_per_sm_u32();
    let max_threads_sm = caps.max_threads_per_sm_u32();
    let shared_per_block = caps.shared_memory_per_block_bytes();

    if max_regs_block == 0 || max_regs_sm == 0 || max_threads_sm == 0 {
        return OccupancyEstimate::ZERO;
    }

    // Per-block register requirement.
    let Some(regs_per_block) = usage.regs_per_thread.checked_mul(workgroup_size) else {
        return OccupancyEstimate::ZERO;
    };
    if regs_per_block > max_regs_block {
        return OccupancyEstimate::ZERO;
    }
    if usage.shared_bytes_per_block > shared_per_block {
        return OccupancyEstimate::ZERO;
    }

    let blocks_by_threads = max_threads_sm / workgroup_size;
    let blocks_by_regs = if regs_per_block == 0 {
        u32::MAX
    } else {
        max_regs_sm / regs_per_block
    };
    let blocks_by_shared = if usage.shared_bytes_per_block == 0 {
        u32::MAX
    } else {
        caps.shared_memory_per_sm_bytes() / usage.shared_bytes_per_block
    };

    let blocks_per_sm = blocks_by_threads.min(blocks_by_regs).min(blocks_by_shared);
    if blocks_per_sm == 0 {
        return OccupancyEstimate::ZERO;
    }

    let warps_per_block = workgroup_size.div_ceil(warp);
    let Some(warps_per_sm) = blocks_per_sm.checked_mul(warps_per_block) else {
        return OccupancyEstimate::ZERO;
    };
    let max_warps_per_sm = max_threads_sm / warp;
    let occupancy_bps = crate::numeric::CUDA_NUMERIC
        .ratio_basis_points_u64(
            u64::from(warps_per_sm),
            u64::from(max_warps_per_sm),
            0,
            "occupancy estimator",
        )
        .min(10_000);

    OccupancyEstimate {
        blocks_per_sm,
        warps_per_sm,
        occupancy_bps,
    }
}

/// Pick the workgroup size from `candidates` that maximises occupancy on
/// `caps` for the measured `usage`. Ties resolve toward the smaller size
/// so launch latency stays low when occupancy is identical. Returns
/// `None` when no candidate is runnable.
#[must_use]
pub fn pick_workgroup_size_for_occupancy(
    caps: &CudaDeviceCaps,
    usage: KernelResourceUsage,
    candidates: &[u32],
) -> Option<u32> {
    let mut best: Option<(u32, OccupancyEstimate)> = None;
    for &candidate in candidates {
        let est = estimate_occupancy(caps, usage, candidate);
        if !est.is_runnable() {
            continue;
        }
        match best {
            None => best = Some((candidate, est)),
            Some((_, current)) if est.occupancy_bps > current.occupancy_bps => {
                best = Some((candidate, est))
            }
            Some((current_size, current))
                if est.occupancy_bps == current.occupancy_bps && candidate < current_size =>
            {
                best = Some((candidate, est))
            }
            _ => {}
        }
    }
    best.map(|(size, _)| size)
}

/// Maximum whole-grid block count that can be resident for a cooperative
/// launch under the thread-residency ceiling alone.
///
/// CUDA cooperative kernels require every block in the grid to be resident at
/// once. Register and shared-memory pressure can tighten this further at
/// module-load time, but the thread ceiling is available from the probed device
/// caps and catches impossible grids before the release path crosses the FFI
/// boundary into `cuLaunchCooperativeKernel`.
#[must_use]
pub fn cooperative_thread_residency_block_limit(caps: &CudaDeviceCaps, workgroup_size: u32) -> u64 {
    if workgroup_size == 0 || !caps.cooperative_launch || caps.compute_capability < (6, 0) {
        return 0;
    }
    let blocks_per_sm = caps.max_threads_per_sm_u32() / workgroup_size;
    u64::from(blocks_per_sm) * u64::from(caps.multi_processor_count_u32())
}

/// Decision returned by [`can_launch_concurrently`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrentLaunchDecision {
    /// The two kernels can launch concurrently on the same SM with
    /// neither one's per-SM resource budget exceeded.
    Concurrent,
    /// At least one resource (registers, threads, or shared memory)
    /// would be over-subscribed; the dispatcher should serialize.
    Serialize {
        /// Human-readable reason naming the over-subscribed resource.
        reason: ConcurrentLaunchBlocker,
    },
}

/// Reason a co-launch was rejected. Useful for telemetry / diagnostics
/// so operators can understand why concurrency wasn't achieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrentLaunchBlocker {
    /// Device does not support concurrent kernels at all
    /// (`CU_DEVICE_ATTRIBUTE_CONCURRENT_KERNELS == 0`).
    DeviceUnsupported,
    /// Either kernel alone would not run (occupancy estimate ZERO).
    KernelUnrunnable,
    /// Combined warps/SM exceed the device's hardware ceiling.
    WarpResidency,
    /// Combined registers/SM exceed the per-SM register file.
    RegisterPressure,
    /// Combined per-block shared bytes exceed the per-block ceiling
    /// (each kernel still has to fit its own block's shared budget).
    SharedMemory,
}

/// Decide whether two kernels can launch concurrently on the same CUDA
/// device under the same SM resources. Pure decision  -  does not perform
/// the launch, only validates that the device + measured per-kernel
/// `KernelResourceUsage` would fit a co-resident schedule.
///
/// Resource model: concurrent kernels need at least one block from each
/// kernel to be co-resident on an SM. Full single-kernel occupancy is not
/// required for overlap; CUDA can interleave blocks as resources free up.
/// This check therefore first proves each kernel is individually runnable,
/// then checks the combined one-block register, warp, and shared-memory
/// footprint against per-SM caps.
///
/// `concurrent_kernels = false` on the device short-circuits to
/// `Serialize { DeviceUnsupported }`.
#[must_use]
pub fn can_launch_concurrently(
    caps: &CudaDeviceCaps,
    usage_a: KernelResourceUsage,
    workgroup_a: u32,
    usage_b: KernelResourceUsage,
    workgroup_b: u32,
) -> ConcurrentLaunchDecision {
    if !caps.concurrent_kernels {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::DeviceUnsupported,
        };
    }

    let est_a = estimate_occupancy(caps, usage_a, workgroup_a);
    let est_b = estimate_occupancy(caps, usage_b, workgroup_b);
    if !est_a.is_runnable() || !est_b.is_runnable() {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::KernelUnrunnable,
        };
    }

    let warp = match caps.warp_size_u32() {
        Some(w) if w > 0 => w,
        _ => {
            return ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::DeviceUnsupported,
            };
        }
    };
    let max_threads_sm = caps.max_threads_per_sm_u32();
    let max_warps_sm = max_threads_sm / warp;
    let warps_a = workgroup_a.div_ceil(warp);
    let warps_b = workgroup_b.div_ceil(warp);
    let Some(total_warps) = warps_a.checked_add(warps_b) else {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::WarpResidency,
        };
    };
    if total_warps > max_warps_sm {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::WarpResidency,
        };
    }

    let max_regs_sm = caps.max_registers_per_sm_u32();
    let Some(regs_a) = usage_a.regs_per_thread.checked_mul(workgroup_a) else {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::RegisterPressure,
        };
    };
    let Some(regs_b) = usage_b.regs_per_thread.checked_mul(workgroup_b) else {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::RegisterPressure,
        };
    };
    let Some(total_regs) = regs_a.checked_add(regs_b) else {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::RegisterPressure,
        };
    };
    if total_regs > max_regs_sm {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::RegisterPressure,
        };
    }

    let shared_per_sm = caps.shared_memory_per_sm_bytes();
    let shared_a = usage_a.shared_bytes_per_block;
    let shared_b = usage_b.shared_bytes_per_block;
    let Some(total_shared) = shared_a.checked_add(shared_b) else {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::SharedMemory,
        };
    };
    if total_shared > shared_per_sm {
        return ConcurrentLaunchDecision::Serialize {
            reason: ConcurrentLaunchBlocker::SharedMemory,
        };
    }

    ConcurrentLaunchDecision::Concurrent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic_device_caps::blackwell_sm120_caps_default;

    #[test]
    fn occupancy_production_paths_do_not_panic_on_release_capability_math() {
        let source = include_str!("occupancy.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: occupancy source must contain production section");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else(")
                && !production.contains(".expect("),
            "Fix: CUDA occupancy production arithmetic must return unrunnable/serialized decisions instead of panicking."
        );
        assert!(
            production.contains("OccupancyEstimate::ZERO")
                && production.contains("ConcurrentLaunchDecision::Serialize")
                && production.contains("cooperative_thread_residency_block_limit"),
            "Fix: CUDA occupancy must keep launch selection and cooperative residency on explicit non-panicking decision paths."
        );
    }

    #[test]
    fn estimate_zero_when_workgroup_exceeds_max_threads_per_block() {
        let caps = blackwell_sm120_caps_default();
        let usage = KernelResourceUsage {
            regs_per_thread: 32,
            shared_bytes_per_block: 0,
        };
        let est = estimate_occupancy(&caps, usage, 4096);
        assert_eq!(est, OccupancyEstimate::ZERO);
    }

    #[test]
    fn estimate_zero_when_register_pressure_too_high() {
        let caps = blackwell_sm120_caps_default();
        // 256 regs/thread * 256 threads = 65_536 → fits exactly per block.
        // 256 regs/thread * 257 threads = 65_792 → busts per-block ceiling.
        let usage = KernelResourceUsage {
            regs_per_thread: 256,
            shared_bytes_per_block: 0,
        };
        let busts = estimate_occupancy(&caps, usage, 257);
        assert_eq!(busts, OccupancyEstimate::ZERO);
        let fits = estimate_occupancy(&caps, usage, 256);
        assert!(fits.is_runnable());
    }

    #[test]
    fn estimate_zero_when_register_requirement_overflows() {
        let mut caps = blackwell_sm120_caps_default();
        caps.max_threads_per_block = i32::MAX;
        caps.max_threads_per_sm = i32::MAX;
        caps.max_registers_per_block = i32::MAX;
        caps.max_registers_per_sm = i32::MAX;
        let usage = KernelResourceUsage {
            regs_per_thread: u32::MAX,
            shared_bytes_per_block: 0,
        };
        let est = estimate_occupancy(&caps, usage, 2);
        assert_eq!(
            est,
            OccupancyEstimate::ZERO,
            "Fix: CUDA occupancy must reject overflowing register products instead of saturating them into plausible resource pressure."
        );
    }

    #[test]
    fn estimate_full_occupancy_on_lightweight_kernel() {
        let caps = blackwell_sm120_caps_default();
        // 16 regs/thread, no shared. At 256 threads → blocks-by-regs =
        // 65_536 / (16*256) = 16; blocks-by-threads = 2048/256 = 8 →
        // 8 blocks/SM. Warps/SM = 8 * 8 = 64 = max_threads_per_sm/warp =
        // 2048/32 = 64 → 100% occupancy.
        let usage = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        let est = estimate_occupancy(&caps, usage, 256);
        assert_eq!(est.blocks_per_sm, 8);
        assert_eq!(est.warps_per_sm, 64);
        assert_eq!(est.occupancy_bps, 10_000);
    }

    #[test]
    fn picker_chooses_smaller_size_on_tie() {
        let caps = blackwell_sm120_caps_default();
        let usage = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        // 128 and 256 both reach 100% occupancy; picker should choose 128.
        let chosen = pick_workgroup_size_for_occupancy(&caps, usage, &[128, 256, 512]);
        assert_eq!(chosen, Some(128));
    }

    #[test]
    fn picker_returns_none_when_no_candidate_runnable() {
        let caps = blackwell_sm120_caps_default();
        // 65_537 regs/thread per block is impossible at any block size > 0.
        let usage = KernelResourceUsage {
            regs_per_thread: 65_537,
            shared_bytes_per_block: 0,
        };
        let chosen = pick_workgroup_size_for_occupancy(&caps, usage, &[32, 64, 128]);
        assert_eq!(chosen, None);
    }

    #[test]
    fn estimate_zero_when_shared_memory_exceeds_per_block_limit() {
        let caps = blackwell_sm120_caps_default();
        let usage = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 256 * 1024,
        };
        let est = estimate_occupancy(&caps, usage, 64);
        assert_eq!(est, OccupancyEstimate::ZERO);
    }

    #[test]
    fn estimate_uses_probed_per_sm_shared_memory_not_block_multiplier() {
        let mut caps = blackwell_sm120_caps_default();
        caps.shared_memory_per_block = 128 * 1024;
        caps.shared_memory_per_sm = 192 * 1024;
        let usage = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 96 * 1024,
        };

        let est = estimate_occupancy(&caps, usage, 256);

        assert_eq!(
            est.blocks_per_sm, 2,
            "Fix: CUDA occupancy must use probed per-SM shared memory instead of assuming a 4x per-block budget."
        );
    }

    #[test]
    fn occupancy_bps_is_proportional_to_warps_per_sm() {
        let caps = blackwell_sm120_caps_default();
        // High-pressure kernel: 64 regs/thread, 256 threads. Blocks/SM =
        // min(2048/256, 65536/(64*256)) = min(8, 4) = 4.
        // Warps/SM = 4 * 8 = 32. max_warps_per_sm = 64.
        // occupancy_bps = (32 * 10000) / 64 = 5000.
        let usage = KernelResourceUsage {
            regs_per_thread: 64,
            shared_bytes_per_block: 0,
        };
        let est = estimate_occupancy(&caps, usage, 256);
        assert_eq!(est.blocks_per_sm, 4);
        assert_eq!(est.warps_per_sm, 32);
        assert_eq!(est.occupancy_bps, 5_000);
    }

    #[test]
    fn picker_prefers_higher_occupancy_over_smaller_size() {
        let caps = blackwell_sm120_caps_default();
        // At 32 threads, 64 regs/thread → blocks_by_regs = 65536/2048 = 32,
        // blocks_by_threads = 2048/32 = 64 → 32 blocks * 1 warp = 32 warps/SM = 50%.
        // At 256 threads, 64 regs/thread → 32 warps/SM = 50% (computed above).
        // Tie → picker prefers smaller size (32).
        let usage = KernelResourceUsage {
            regs_per_thread: 64,
            shared_bytes_per_block: 0,
        };
        let chosen = pick_workgroup_size_for_occupancy(&caps, usage, &[32, 256]);
        assert_eq!(chosen, Some(32));
    }

    #[test]
    fn cooperative_residency_limit_uses_sm_thread_ceiling() {
        let caps = blackwell_sm120_caps_default();
        assert_eq!(
            cooperative_thread_residency_block_limit(&caps, 256),
            1_360,
            "Fix: CUDA cooperative launch preflight must reject grids larger than blocks_per_sm * sm_count before calling cuLaunchCooperativeKernel."
        );
        assert_eq!(cooperative_thread_residency_block_limit(&caps, 0), 0);
    }

    // ── D5: concurrent-launch decision policy tests ─────────────────

    #[test]
    fn co_launch_two_kernels_with_headroom_fits_concurrently() {
        let caps = blackwell_sm120_caps_default();
        let light = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        let decision = can_launch_concurrently(&caps, light, 256, light, 256);
        assert_eq!(decision, ConcurrentLaunchDecision::Concurrent);
    }

    #[test]
    fn co_launch_two_full_occupancy_kernels_overflows_warp_cap() {
        let mut caps = blackwell_sm120_caps_default();
        caps.max_threads_per_sm = 512;
        let full = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        let decision = can_launch_concurrently(&caps, full, 512, full, 512);
        assert_eq!(
            decision,
            ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::WarpResidency
            }
        );
    }

    #[test]
    fn co_launch_register_heavy_kernels_serializes_on_register_pressure() {
        let caps = blackwell_sm120_caps_default();
        let heavy = KernelResourceUsage {
            regs_per_thread: 129,
            shared_bytes_per_block: 0,
        };
        let decision = can_launch_concurrently(&caps, heavy, 256, heavy, 256);
        assert_eq!(
            decision,
            ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::RegisterPressure
            }
        );
    }

    #[test]
    fn co_launch_with_unrunnable_kernel_returns_kernel_unrunnable() {
        let caps = blackwell_sm120_caps_default();
        let runnable = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        let too_big = KernelResourceUsage {
            regs_per_thread: 65_537, // exceeds per-block register cap
            shared_bytes_per_block: 0,
        };
        let decision = can_launch_concurrently(&caps, runnable, 128, too_big, 256);
        assert_eq!(
            decision,
            ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::KernelUnrunnable
            }
        );
    }

    #[test]
    fn co_launch_on_device_without_concurrency_short_circuits() {
        let mut caps = blackwell_sm120_caps_default();
        caps.concurrent_kernels = false;
        let usage = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 0,
        };
        let decision = can_launch_concurrently(&caps, usage, 64, usage, 64);
        assert_eq!(
            decision,
            ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::DeviceUnsupported
            }
        );
    }

    #[test]
    fn co_launch_with_shared_memory_headroom_fits() {
        let caps = blackwell_sm120_caps_default();
        let shared = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 96 * 1024,
        };
        let decision = can_launch_concurrently(&caps, shared, 128, shared, 128);
        assert_eq!(decision, ConcurrentLaunchDecision::Concurrent);
    }

    #[test]
    fn co_launch_shared_memory_uses_exact_per_sm_limit() {
        let mut caps = blackwell_sm120_caps_default();
        caps.shared_memory_per_sm = 160 * 1024;
        let shared = KernelResourceUsage {
            regs_per_thread: 16,
            shared_bytes_per_block: 96 * 1024,
        };

        let decision = can_launch_concurrently(&caps, shared, 128, shared, 128);

        assert_eq!(
            decision,
            ConcurrentLaunchDecision::Serialize {
                reason: ConcurrentLaunchBlocker::SharedMemory
            },
            "Fix: CUDA concurrent-launch policy must reject co-resident shared-memory pressure using the probed SM budget, not a guessed multiplier."
        );
    }

    #[test]
    fn occupancy_arithmetic_is_checked_not_saturating() {
        let source = include_str!("occupancy.rs");
        assert!(
            !source.contains(concat!(".", "saturating_add"))
                && !source.contains(concat!(".", "saturating_mul"))
                && !source.contains(concat!(".", "saturating_sub")),
            "Fix: CUDA occupancy planning must use checked or proven-exact arithmetic, not saturating math that hides impossible resource states."
        );
    }
}
