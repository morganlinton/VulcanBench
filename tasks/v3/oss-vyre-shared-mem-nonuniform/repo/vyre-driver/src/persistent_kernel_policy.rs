//! D1 substrate: persistent-kernel-mode decision policy.
//!
//! When a workload submits many small kernel launches with the same
//! pipeline, the launch overhead dominates execution time (~5 µs
//! per native launch, ~10–50 µs per portable queue submit). Replacing the
//! N launches with ONE persistent kernel that polls a device-side
//! work queue eliminates the per-launch cost entirely  -  a 100×
//! speedup on workloads where kernel duration < 50 µs.
//!
//! Persistent mode has a one-time setup cost (allocate the work queue,
//! launch the persistent kernel, signal shutdown at the end). This
//! amortises only when the batch is large enough. The decision policy
//! here owns the threshold: given the measured per-launch overhead and
//! per-item kernel duration, should the dispatcher run N standard
//! launches or one persistent kernel?
//!
//! Pure decision  -  no kernel launch, no Program walk. Caller passes
//! the measurements; the substrate produces a verdict.

/// Inputs to the persistent-kernel decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistentKernelInputs {
    /// Number of small launches in the upcoming batch.
    pub batch_size: u32,
    /// Average per-launch host-side overhead in nanoseconds. Measured
    /// on the live backend at startup; native is typically ~5_000 ns,
    /// portable typically ~25_000 ns.
    pub per_launch_overhead_ns: u64,
    /// Average per-item kernel duration in nanoseconds. The
    /// dispatcher measures this on the warmup pass before the batch.
    pub per_item_kernel_ns: u64,
    /// Setup cost of bringing up persistent mode (work-queue alloc,
    /// initial launch, signal handshake) in nanoseconds. native: ~50_000
    /// for a fresh queue; portable: ~200_000.
    pub persistent_setup_overhead_ns: u64,
}

/// Verdict returned by [`decide_persistent_kernel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentKernelDecision {
    /// Use the standard launch path  -  N separate kernel launches.
    /// Either the batch is too small to amortise persistent setup,
    /// or the per-item kernel is large enough that launch overhead
    /// is negligible.
    StandardLaunches,
    /// Use persistent kernel mode  -  one launch + device-side queue
    /// polling for `batch_size` work items.
    PersistentKernel {
        /// Predicted total time saved (in nanoseconds) by using the
        /// persistent path vs N standard launches. Useful for
        /// telemetry and for the autotune store.
        savings_ns: u128,
    },
}

/// Decide whether to use persistent kernel mode for this batch.
///
/// Standard launches cost: `batch_size * per_launch_overhead + batch_size * per_item_kernel`.
/// Persistent cost: `persistent_setup + batch_size * per_item_kernel`.
/// Persistent wins iff `batch_size * per_launch_overhead > persistent_setup`.
///
/// Returns `StandardLaunches` when batch_size is 0 or 1 (persistent
/// mode never wins for a single launch  -  the setup cost dominates).
#[must_use]
pub fn decide_persistent_kernel(inputs: PersistentKernelInputs) -> PersistentKernelDecision {
    if inputs.batch_size <= 1 {
        return PersistentKernelDecision::StandardLaunches;
    }
    // Defensive: zero per-launch overhead means we have no model to
    // amortise  -  keep the standard path.
    if inputs.per_launch_overhead_ns == 0 {
        return PersistentKernelDecision::StandardLaunches;
    }
    let standard_overhead =
        u128::from(inputs.batch_size) * u128::from(inputs.per_launch_overhead_ns);
    let persistent_setup_overhead_ns = u128::from(inputs.persistent_setup_overhead_ns);
    if standard_overhead <= persistent_setup_overhead_ns {
        return PersistentKernelDecision::StandardLaunches;
    }
    let savings_ns = standard_overhead - persistent_setup_overhead_ns;
    PersistentKernelDecision::PersistentKernel { savings_ns }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(batch: u32, launch_ovh: u64, item_ns: u64, setup: u64) -> PersistentKernelInputs {
        PersistentKernelInputs {
            batch_size: batch,
            per_launch_overhead_ns: launch_ovh,
            per_item_kernel_ns: item_ns,
            persistent_setup_overhead_ns: setup,
        }
    }

    #[test]
    fn single_launch_is_always_standard() {
        // No matter how cheap the persistent setup, a 1-launch batch
        // can't beat the standard path.
        let dec = decide_persistent_kernel(inp(1, 5_000, 1_000, 1_000));
        assert_eq!(dec, PersistentKernelDecision::StandardLaunches);
    }

    #[test]
    fn zero_batch_is_standard() {
        let dec = decide_persistent_kernel(inp(0, 5_000, 1_000, 50_000));
        assert_eq!(dec, PersistentKernelDecision::StandardLaunches);
    }

    #[test]
    fn small_batch_below_amortisation_threshold_is_standard() {
        // 5 launches × 5 µs = 25 µs total; persistent setup = 50 µs →
        // standard is cheaper.
        let dec = decide_persistent_kernel(inp(5, 5_000, 1_000, 50_000));
        assert_eq!(dec, PersistentKernelDecision::StandardLaunches);
    }

    #[test]
    fn batch_at_amortisation_threshold_is_standard() {
        // Exactly equal  -  the policy uses strict `>` so equal cost
        // stays on the standard path (cheaper to keep launching).
        let dec = decide_persistent_kernel(inp(10, 5_000, 1_000, 50_000));
        assert_eq!(dec, PersistentKernelDecision::StandardLaunches);
    }

    #[test]
    fn large_batch_above_threshold_picks_persistent() {
        // 100 launches × 5 µs = 500 µs; persistent setup = 50 µs →
        // savings = 450 µs.
        let dec = decide_persistent_kernel(inp(100, 5_000, 1_000, 50_000));
        assert_eq!(
            dec,
            PersistentKernelDecision::PersistentKernel {
                savings_ns: 450_000
            }
        );
    }

    #[test]
    fn portable_typical_overheads_pick_persistent_at_modest_batch() {
        // portable submit overhead ~25 µs; persistent setup ~200 µs.
        // 10 launches × 25 µs = 250 µs > 200 µs setup → persistent.
        let dec = decide_persistent_kernel(inp(10, 25_000, 5_000, 200_000));
        assert_eq!(
            dec,
            PersistentKernelDecision::PersistentKernel { savings_ns: 50_000 }
        );
    }

    #[test]
    fn zero_per_launch_overhead_returns_standard() {
        // Defensive: a backend that reports zero launch overhead has
        // no model to amortise  -  keep the standard path.
        let dec = decide_persistent_kernel(inp(1000, 0, 100, 50_000));
        assert_eq!(dec, PersistentKernelDecision::StandardLaunches);
    }

    #[test]
    fn savings_is_strictly_positive_for_persistent_verdict() {
        let dec = decide_persistent_kernel(inp(1000, 5_000, 1_000, 50_000));
        match dec {
            PersistentKernelDecision::PersistentKernel { savings_ns } => {
                assert!(savings_ns > 0);
            }
            other => panic!("expected PersistentKernel; got {:?}", other),
        }
    }

    #[test]
    fn item_duration_does_not_affect_decision() {
        // The decision is purely about overhead vs setup; per-item
        // kernel duration appears on both sides of the inequality
        // and cancels.
        let small_kernel = decide_persistent_kernel(inp(100, 5_000, 100, 50_000));
        let large_kernel = decide_persistent_kernel(inp(100, 5_000, 1_000_000, 50_000));
        assert_eq!(small_kernel, large_kernel);
    }

    #[test]
    fn widened_arithmetic_preserves_extreme_savings() {
        // Adversarial: batch_size × per_launch_overhead near u64::MAX
        // must not panic or clamp the predicted savings.
        let dec = decide_persistent_kernel(inp(u32::MAX, u64::MAX / 2, 1, 50_000));
        match dec {
            PersistentKernelDecision::PersistentKernel { savings_ns } => {
                assert_eq!(
                    savings_ns,
                    u128::from(u32::MAX) * u128::from(u64::MAX / 2) - 50_000
                );
            }
            other => panic!("expected PersistentKernel; got {:?}", other),
        }
    }

    #[test]
    fn persistent_policy_source_uses_exact_widened_arithmetic() {
        let source = include_str!("persistent_kernel_policy.rs");

        assert!(
            !source.contains(concat!("saturating", "_mul"))
                && !source.contains(concat!("saturating", "_sub")),
            "Fix: persistent-kernel policy must use exact widened arithmetic, not saturating launch-cost math."
        );
        assert!(
            source.contains("u128::from(inputs.batch_size)")
                && source.contains("u128::from(inputs.per_launch_overhead_ns)")
                && source.contains("standard_overhead - persistent_setup_overhead_ns"),
            "Fix: persistent-kernel savings must stay widened through the verdict."
        );
    }
}
