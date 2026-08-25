//! Device-profile-aware cost helpers for [`vyre_foundation::optimizer::eqsat::extract_best`].
//!
//! ROADMAP A7. The egraph extraction substrate (`extract_best`) accepts an
//! arbitrary `Fn(&L) -> u64` cost function. Each consumer Family used to
//! roll its own  -  passing a flat per-op cost table that ignored the
//! current device's tensor-core throughput, hot/cold path heat, and
//! FP16-eligibility hints.
//!
//! This module gives every Family one shared place to build a cost
//! closure from `(DeviceProfile, hot_path_flag, base_cost_fn)`. The
//! closure scales the base cost by:
//!
//! 1. **Hot/cold-path multiplier.** Hot-path nodes pay less per
//!    abstract cost unit because the optimizer is willing to spend
//!    more rewriter budget on them; cold-path nodes pay more so the
//!    extractor prefers smaller (less optimised) representations.
//! 2. **Tensor-core scaling for FP16-eligible ALU work.** When the
//!    profile reports `supports_tensor_cores && supports_f16`, ALU
//!    nodes flagged as `fp16_eligible` are scaled by the
//!    profile's tensor-core throughput multiplier (default `0.25`  -
//!    i.e. 4× cheaper than scalar f32) so the extractor prefers
//!    FP16-eligible variants on supporting hardware.
//!
//! Every multiplier is a `f32` clamped into `[0.0, 4.0]` and applied
//! to the base cost before truncation back to `u64`. The base cost
//! function still drives the *shape* of the cost landscape; the
//! profile only nudges relative weights.
//!
//! Pure functional value: no global state, no allocation, no I/O.
//! Two profiles with identical capability bits produce identical
//! closures so the extractor result is deterministic per device.

use vyre_foundation::optimizer::eqsat::ENodeLang;

use crate::device_profile::DeviceProfile;

/// Default cost multiplier for hot-path nodes.
///
/// Hot-path nodes are nodes the dispatcher recently saw fire (per the
/// I1 hot-path-hint substrate). The extractor prefers cheaper
/// representations on the cold path and is willing to pay more
/// extractor work on hot paths.
pub const HOT_PATH_COST_SCALE: f32 = 0.5;
/// Integer basis-point form of [`HOT_PATH_COST_SCALE`] used by the release
/// extraction path.
pub const HOT_PATH_COST_SCALE_BPS: u32 = 5_000;

/// Default cost multiplier for cold-path nodes.
pub const COLD_PATH_COST_SCALE: f32 = 1.5;
/// Integer basis-point form of [`COLD_PATH_COST_SCALE`] used by the release
/// extraction path.
pub const COLD_PATH_COST_SCALE_BPS: u32 = 15_000;

/// Default tensor-core throughput multiplier for FP16-eligible ALU
/// work on a profile that reports both `supports_tensor_cores` and
/// `supports_f16`. `0.25` = roughly 4× cheaper than f32 ALU.
pub const TENSOR_CORE_COST_SCALE: f32 = 0.25;
/// Integer basis-point form of [`TENSOR_CORE_COST_SCALE`] used by the release
/// extraction path.
pub const TENSOR_CORE_COST_SCALE_BPS: u32 = 2_500;

const MAX_SCALE_BPS: u32 = 40_000;

/// Per-node hint bits derived from the foundation analyses.
///
/// Callers populate this from the substrate they already have:
/// `PrecisionHints::lookup(digest)` for `fp16_eligible`, the F1
/// `vsa_specialization_key` for `compile_time_constant`. The cost
/// helper does not compute these  -  it only consumes them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NodeHints {
    /// Foundation precision_hint analysis flagged this node as
    /// representable in F16. The extractor will prefer this node when
    /// the device profile reports tensor-core support.
    pub fp16_eligible: bool,
    /// F1 specialization detected this node's value as a compile-time
    /// constant. Reserved for the F3 dtype-spec wiring; not yet
    /// consumed by this helper.
    pub compile_time_constant: bool,
}

/// Build a cost closure for `extract_best` parameterised on the
/// neutral device profile and a hot-path flag.
///
/// `base_cost_fn` gives the ABSTRACT per-op cost (e.g. 1 for a const,
/// 2 for an Add, 4 for a Div). `hint_lookup` answers per-node hint
/// bits  -  typically a wrapper over `PrecisionHints::lookup`.
///
/// The returned closure is `Fn(&L) -> u64` and can be passed
/// straight into `extract_best`. It owns its arguments by value so
/// the closure outlives the call frame.
#[must_use]
pub fn device_aware_cost<L, B, H>(
    profile: &DeviceProfile,
    hot: bool,
    base_cost_fn: B,
    hint_lookup: H,
) -> impl Fn(&L) -> u64
where
    L: ENodeLang,
    B: Fn(&L) -> u64,
    H: Fn(&L) -> NodeHints,
{
    let path_scale_bps = if hot {
        HOT_PATH_COST_SCALE_BPS
    } else {
        COLD_PATH_COST_SCALE_BPS
    };
    let tensor_scale_bps = if profile.supports_tensor_cores && profile.supports_f16 {
        TENSOR_CORE_COST_SCALE_BPS
    } else {
        crate::numeric::BASIS_POINTS_DENOMINATOR
    };
    move |node: &L| {
        let base = base_cost_fn(node);
        let hints = hint_lookup(node);
        let mut scale_bps = path_scale_bps;
        if hints.fp16_eligible {
            scale_bps = compose_scale_basis_points(scale_bps, tensor_scale_bps);
        }
        scale_cost_basis_points(base, scale_bps)
    }
}

/// Apply an integer basis-point multiplier to a `u64` cost with checked,
/// deterministic rounding.
///
/// Scale is clamped to `[1, 40000]` basis points before scaling; zero
/// falls back to the base cost to preserve the old invalid-scale contract.
fn scale_cost_basis_points(base: u64, scale_bps: u32) -> u64 {
    crate::numeric::scale_u64_by_basis_points_round_clamped(
        base,
        scale_bps,
        base,
        MAX_SCALE_BPS,
        "extraction cost",
        "driver",
    )
}

fn compose_scale_basis_points(left_bps: u32, right_bps: u32) -> u32 {
    crate::numeric::compose_basis_points_u32(
        left_bps,
        right_bps,
        "extraction cost scale composition",
        "driver",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::optimizer::eqsat::{EChildren, ENodeLang};

    /// Trivial language for the cost-helper tests: just a `Const(u32)`
    /// and a synthetic `Heavy` with no children. The base cost
    /// function distinguishes them so we can observe scaling.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Toy {
        Const(u32),
        Heavy,
    }

    impl ENodeLang for Toy {
        fn children(&self) -> EChildren {
            EChildren::new()
        }
        fn with_children(&self, _children: &[vyre_foundation::optimizer::eqsat::EClassId]) -> Self {
            self.clone()
        }
    }

    fn base_cost(node: &Toy) -> u64 {
        match node {
            Toy::Const(_) => 1,
            Toy::Heavy => 100,
        }
    }

    fn no_hints(_: &Toy) -> NodeHints {
        NodeHints::default()
    }

    #[test]
    fn cold_path_inflates_base_cost() {
        let profile = DeviceProfile::conservative("test");
        let cost = device_aware_cost(&profile, /*hot=*/ false, base_cost, no_hints);
        assert_eq!(
            cost(&Toy::Heavy),
            scale_cost_basis_points(100, COLD_PATH_COST_SCALE_BPS)
        );
        assert_eq!(
            cost(&Toy::Const(0)),
            scale_cost_basis_points(1, COLD_PATH_COST_SCALE_BPS)
        );
    }

    #[test]
    fn hot_path_shrinks_base_cost() {
        let profile = DeviceProfile::conservative("test");
        let cost = device_aware_cost(&profile, /*hot=*/ true, base_cost, no_hints);
        assert_eq!(
            cost(&Toy::Heavy),
            scale_cost_basis_points(100, HOT_PATH_COST_SCALE_BPS)
        );
        assert_eq!(
            cost(&Toy::Const(0)),
            scale_cost_basis_points(1, HOT_PATH_COST_SCALE_BPS)
        );
    }

    #[test]
    fn tensor_core_profile_scales_fp16_eligible_nodes() {
        let mut profile = DeviceProfile::conservative("test");
        profile.supports_tensor_cores = true;
        profile.supports_f16 = true;
        let mark_eligible = |node: &Toy| match node {
            Toy::Heavy => NodeHints {
                fp16_eligible: true,
                compile_time_constant: false,
            },
            _ => NodeHints::default(),
        };
        let cost = device_aware_cost(&profile, /*hot=*/ true, base_cost, mark_eligible);
        let expected = scale_cost_basis_points(
            100,
            compose_scale_basis_points(HOT_PATH_COST_SCALE_BPS, TENSOR_CORE_COST_SCALE_BPS),
        );
        assert_eq!(cost(&Toy::Heavy), expected);
        // Const is not fp16-eligible  -  only hot-path scaling applies.
        assert_eq!(
            cost(&Toy::Const(0)),
            scale_cost_basis_points(1, HOT_PATH_COST_SCALE_BPS)
        );
    }

    #[test]
    fn no_tensor_core_support_ignores_fp16_hint() {
        let profile = DeviceProfile::conservative("test");
        assert!(!profile.supports_tensor_cores);
        let mark_eligible = |_: &Toy| NodeHints {
            fp16_eligible: true,
            compile_time_constant: false,
        };
        let cost = device_aware_cost(&profile, /*hot=*/ true, base_cost, mark_eligible);
        // FP16 hint is ignored on a profile that doesn't support tensor cores.
        assert_eq!(
            cost(&Toy::Heavy),
            scale_cost_basis_points(100, HOT_PATH_COST_SCALE_BPS)
        );
    }

    #[test]
    fn scale_cost_clamps_high_multiplier_basis_points() {
        assert_eq!(scale_cost_basis_points(10, 1_000_000), 40); // 10 * 4.0 cap
    }

    #[test]
    fn zero_basis_point_scale_preserves_invalid_scale_contract() {
        assert_eq!(scale_cost_basis_points(7, 0), 7);
    }

    #[test]
    fn extraction_cost_release_path_uses_integer_scaling() {
        let source = include_str!("extraction_cost.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: extraction-cost production source must precede tests");

        assert!(
            production.contains("scale_cost_basis_points")
                && production.contains("compose_scale_basis_points")
                && production.contains("crate::numeric::"),
            "Fix: extraction cost scaling must use deterministic integer basis-point arithmetic."
        );
        assert!(
            !production.contains("base as f32")
                && !production.contains("scaled.round()")
                && !production.contains("scale *= tensor_scale"),
            "Fix: extraction cost release path must not use lossy float scaling."
        );
    }

    #[test]
    fn deterministic_for_identical_profiles() {
        let p1 = DeviceProfile::conservative("a");
        let p2 = DeviceProfile::conservative("b");
        let c1 = device_aware_cost(&p1, false, base_cost, no_hints);
        let c2 = device_aware_cost(&p2, false, base_cost, no_hints);
        // Backend name differs but capability bits are identical → same
        // cost output.
        assert_eq!(c1(&Toy::Heavy), c2(&Toy::Heavy));
        assert_eq!(c1(&Toy::Const(7)), c2(&Toy::Const(7)));
    }
}
