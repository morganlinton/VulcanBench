//! N2 substrate (foundation half): per-rewrite speculation-as-substrate
//! decision policy.
//!
//! Generalizes I2's trace-JIT speculation to ANY "probably profitable"
//! rewrite (vec_pack, shared_promote, async_load_promote, ...). For each
//! candidate rewrite the runtime keeps two compiled variants  -  a
//! conservative baseline and a speculative variant  -  and races them
//! against the autotune DB's recorded winner.
//!
//! This module owns the pure *decision*: given the speculative variant's
//! observed cost vs the baseline (recorded by I3 [`crate::autotune_store`]),
//! return [`SpeculationVerdict::Adopt`] (replace baseline with speculative
//! in the cache) or [`SpeculationVerdict::Reject`] (drop speculative,
//! stop racing). Pure arithmetic; no I/O, no allocation.
//!
//! The runtime side (compiling both variants on a side pipeline cache
//! key, dispatching them in alternation, recording observations to
//! [`crate::autotune_store`]) lives in `runtime_megakernel` and is
//! Codex's lane. This module is the half that's safe to land before
//! that wiring exists  -  every consumer reads the same decision contract.

/// Per-shape observation feeding the speculation decision.
#[derive(Debug, Clone, Copy)]
pub struct SpeculationObservation {
    /// Number of times the baseline variant was dispatched. Used to
    /// gate how confident we are in `baseline_mean_ns`.
    pub baseline_dispatches: u32,
    /// Mean wall-clock dispatch latency of the baseline variant in
    /// nanoseconds.
    pub baseline_mean_ns: u64,
    /// Number of times the speculative variant was dispatched.
    pub speculative_dispatches: u32,
    /// Mean wall-clock dispatch latency of the speculative variant.
    pub speculative_mean_ns: u64,
    /// Side-compile cost (one-time amortized over future dispatches).
    /// Treated as overhead the speculative variant must pay back.
    pub side_compile_cost_ns: u64,
}

/// Verdict returned by [`decide_speculation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculationVerdict {
    /// Speculative variant wins  -  replace the baseline in the cache.
    /// Future dispatches use the speculative variant directly.
    Adopt,
    /// Speculative variant loses or is statistically inconclusive  -
    /// drop it from the cache and stop racing on this shape.
    Reject,
    /// Not enough samples yet  -  keep racing.
    KeepRacing,
}

/// Minimum number of dispatches per variant before a verdict can be
/// rendered. Below this threshold the variance dominates and the
/// decision is unreliable; the runtime keeps racing both variants.
pub const MIN_DISPATCHES_FOR_VERDICT: u32 = 8;

/// Minimum savings in basis points (1 bp = 0.01%) the speculative
/// variant must show over the baseline to be adopted, after side-compile
/// cost amortization. 1500 bps = 15%  -  tuned conservative so adopting
/// is rare but high-confidence.
pub const MIN_ADOPT_SAVINGS_BPS: u64 = 1500;

/// Decide whether to adopt the speculative variant, reject it, or keep
/// racing. Pure arithmetic; widened throughout so adversarial inputs cannot
/// panic or silently clamp a release-path adoption decision.
#[must_use]
pub fn decide_speculation(obs: SpeculationObservation) -> SpeculationVerdict {
    if obs.baseline_dispatches < MIN_DISPATCHES_FOR_VERDICT
        || obs.speculative_dispatches < MIN_DISPATCHES_FOR_VERDICT
    {
        return SpeculationVerdict::KeepRacing;
    }
    if obs.baseline_mean_ns == 0 {
        // Degenerate baseline  -  keep racing rather than divide-by-zero.
        return SpeculationVerdict::KeepRacing;
    }

    // Amortized speculative cost: per-dispatch latency plus
    // side-compile-cost / dispatches-so-far. The further we go, the
    // less the side-compile bites.
    let amortized_overhead_ns = obs
        .side_compile_cost_ns
        .checked_div(u64::from(obs.speculative_dispatches.max(1)))
        .unwrap_or(u64::MAX);
    let effective_speculative_ns =
        u128::from(obs.speculative_mean_ns) + u128::from(amortized_overhead_ns);
    let baseline_mean_ns = u128::from(obs.baseline_mean_ns);

    if effective_speculative_ns >= baseline_mean_ns {
        return SpeculationVerdict::Reject;
    }
    let savings_ns = u64::try_from(baseline_mean_ns - effective_speculative_ns).unwrap_or(u64::MAX);
    let savings_bps = crate::numeric::ratio_basis_points_u64_wide(
        savings_ns,
        obs.baseline_mean_ns,
        0,
        "speculation savings",
        "driver",
    );
    if savings_bps >= MIN_ADOPT_SAVINGS_BPS {
        SpeculationVerdict::Adopt
    } else {
        // Speculative wins but by less than the threshold  -  keep
        // racing in case the gap widens with more samples.
        SpeculationVerdict::KeepRacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(b_n: u32, b_ns: u64, s_n: u32, s_ns: u64, sc_ns: u64) -> SpeculationObservation {
        SpeculationObservation {
            baseline_dispatches: b_n,
            baseline_mean_ns: b_ns,
            speculative_dispatches: s_n,
            speculative_mean_ns: s_ns,
            side_compile_cost_ns: sc_ns,
        }
    }

    #[test]
    fn under_threshold_keeps_racing() {
        // baseline only sampled 3 times  -  too few to verdict.
        let v = decide_speculation(obs(3, 100_000, 100, 50_000, 0));
        assert_eq!(v, SpeculationVerdict::KeepRacing);
    }

    #[test]
    fn speculative_clearly_faster_adopts() {
        // baseline 100us, speculative 50us, no side-compile cost.
        // savings = 50%, well over 15% threshold.
        let v = decide_speculation(obs(50, 100_000, 50, 50_000, 0));
        assert_eq!(v, SpeculationVerdict::Adopt);
    }

    #[test]
    fn speculative_slower_rejects() {
        let v = decide_speculation(obs(50, 50_000, 50, 100_000, 0));
        assert_eq!(v, SpeculationVerdict::Reject);
    }

    #[test]
    fn speculative_marginally_faster_keeps_racing() {
        // baseline 100us, speculative 95us → 5% savings, under 15%.
        let v = decide_speculation(obs(50, 100_000, 50, 95_000, 0));
        assert_eq!(v, SpeculationVerdict::KeepRacing);
    }

    #[test]
    fn side_compile_cost_amortizes_into_decision() {
        // baseline 100us, speculative 50us, but side-compile = 1ms.
        // After 50 dispatches, amortized overhead = 1ms/50 = 20us.
        // Effective speculative = 50us + 20us = 70us → 30% savings.
        let v = decide_speculation(obs(50, 100_000, 50, 50_000, 1_000_000));
        assert_eq!(v, SpeculationVerdict::Adopt);
    }

    #[test]
    fn side_compile_cost_can_dominate_early() {
        // Same shape but only 8 speculative dispatches.
        // Amortized overhead = 1ms/8 = 125us. Effective = 50us + 125us = 175us
        // > baseline 100us → reject.
        let v = decide_speculation(obs(50, 100_000, 8, 50_000, 1_000_000));
        assert_eq!(v, SpeculationVerdict::Reject);
    }

    #[test]
    fn zero_baseline_keeps_racing_rather_than_dividing_by_zero() {
        let v = decide_speculation(obs(50, 0, 50, 50_000, 0));
        assert_eq!(v, SpeculationVerdict::KeepRacing);
    }

    #[test]
    fn extreme_inputs_do_not_panic() {
        assert_eq!(
            decide_speculation(obs(u32::MAX, u64::MAX, u32::MAX, u64::MAX, u64::MAX)),
            SpeculationVerdict::Reject
        );
        assert_eq!(
            decide_speculation(obs(u32::MAX, 1, u32::MAX, u64::MAX, 0)),
            SpeculationVerdict::Reject
        );
    }

    #[test]
    fn huge_savings_use_widened_arithmetic_not_saturation() {
        assert_eq!(
            decide_speculation(obs(u32::MAX, u64::MAX, u32::MAX, 1, 0)),
            SpeculationVerdict::Adopt
        );
    }

    #[test]
    fn speculation_policy_source_uses_exact_widened_arithmetic() {
        let source = include_str!("speculation_substrate.rs");

        assert!(
            !source.contains(concat!("saturating", "_add"))
                && !source.contains(concat!("saturating", "_mul")),
            "Fix: speculation adoption policy must use widened exact arithmetic, not saturating math that can hide release-path cost corruption."
        );
        assert!(
            source.contains("u128::from(obs.speculative_mean_ns)")
                && source.contains("u128::from(obs.baseline_mean_ns)")
                && source.contains("crate::numeric::ratio_basis_points_u64_wide"),
            "Fix: speculation adoption policy must compute effective cost and savings in widened integer space."
        );
    }
}
