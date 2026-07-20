//! D9 substrate: bindless buffers / textures decision policy.
//!
//! When a kernel binds many resources (think 100+ small buffers in a
//! sparse compute graph), the per-binding setup cost  -  bind group
//! creation, descriptor set rebinds  -  dominates dispatch latency.
//! Bindless mode replaces N descriptor entries with one descriptor
//! array indexed at runtime, eliminating the rebind churn.
//!
//! Concrete backends expose bindless access through their own native
//! resource-indexing primitives. Not every adapter supports it; the
//! policy here owns the decision given a probed capability + resource
//! count.
//!
//! Pure decision: no Program walk, no descriptor scan. Caller passes
//! the resource count and the backend's bindless capability bit.

/// Backend support level for bindless resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindlessSupport {
    /// Backend has full bindless support: descriptor arrays plus
    /// dynamic indexing.
    Full,
    /// Backend supports descriptor arrays but with a fixed size and no
    /// runtime indexing of unbound slots. Useful when every slot is
    /// guaranteed bound; not useful for sparse access.
    Static,
    /// Backend has no bindless support. Always use traditional
    /// per-resource bindings.
    Unsupported,
}

/// Inputs to the bindless decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindlessInputs {
    /// Number of resources the kernel binds. Below the threshold,
    /// traditional bindings beat bindless on every backend (the
    /// per-bindless-handle setup cost has its own constant).
    pub resource_count: u32,
    /// Backend's bindless support level (probed once per backend
    /// startup).
    pub support: BindlessSupport,
    /// Whether the kernel's access pattern is dynamic (different
    /// indices per thread / per dispatch). Only `Full` support
    /// handles dynamic indexing; `Static` is wasted on dynamic
    /// access.
    pub dynamic_indexing: bool,
}

/// Verdict from [`decide_bindless`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindlessDecision {
    /// Use bindless  -  N resources go into a single descriptor array.
    Bindless,
    /// Use traditional per-resource bindings.
    TraditionalBindings,
}

/// Threshold above which bindless wins on `Full` support backends.
/// Below this count the per-handle bindless setup overhead dominates.
/// Calibrated from backend microbenchmarks: around two dozen bindings
/// is the crossover on current discrete GPUs.
pub const BINDLESS_RESOURCE_COUNT_THRESHOLD: u32 = 24;

/// Decide whether to use the bindless path for this dispatch.
///
/// Picks `Bindless` when:
///   - support is `Full`, AND
///   - resource_count >= [`BINDLESS_RESOURCE_COUNT_THRESHOLD`]
///
/// `Static` support is treated as `Bindless` only when the access
/// pattern is NOT dynamic (every slot is guaranteed bound) AND the
/// resource count clears the threshold. `Unsupported` always returns
/// `TraditionalBindings`.
#[must_use]
pub fn decide_bindless(inputs: BindlessInputs) -> BindlessDecision {
    if matches!(inputs.support, BindlessSupport::Unsupported) {
        return BindlessDecision::TraditionalBindings;
    }
    if inputs.resource_count < BINDLESS_RESOURCE_COUNT_THRESHOLD {
        return BindlessDecision::TraditionalBindings;
    }
    match inputs.support {
        BindlessSupport::Full => BindlessDecision::Bindless,
        BindlessSupport::Static if !inputs.dynamic_indexing => BindlessDecision::Bindless,
        _ => BindlessDecision::TraditionalBindings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(count: u32, support: BindlessSupport, dynamic: bool) -> BindlessInputs {
        BindlessInputs {
            resource_count: count,
            support,
            dynamic_indexing: dynamic,
        }
    }

    #[test]
    fn unsupported_always_returns_traditional() {
        for count in [0, 8, 24, 100, u32::MAX] {
            for dynamic in [false, true] {
                assert_eq!(
                    decide_bindless(inp(count, BindlessSupport::Unsupported, dynamic)),
                    BindlessDecision::TraditionalBindings
                );
            }
        }
    }

    #[test]
    fn below_threshold_returns_traditional_on_full_support() {
        // 23 < threshold(24).
        assert_eq!(
            decide_bindless(inp(23, BindlessSupport::Full, true)),
            BindlessDecision::TraditionalBindings
        );
    }

    #[test]
    fn at_threshold_returns_bindless_on_full_support() {
        assert_eq!(
            decide_bindless(inp(24, BindlessSupport::Full, true)),
            BindlessDecision::Bindless
        );
    }

    #[test]
    fn above_threshold_returns_bindless_on_full_support() {
        assert_eq!(
            decide_bindless(inp(100, BindlessSupport::Full, false)),
            BindlessDecision::Bindless
        );
    }

    #[test]
    fn static_support_with_dynamic_access_returns_traditional() {
        // Static can't satisfy dynamic indexing of unbound slots  -
        // dynamic access on Static-only support falls back.
        assert_eq!(
            decide_bindless(inp(100, BindlessSupport::Static, true)),
            BindlessDecision::TraditionalBindings
        );
    }

    #[test]
    fn static_support_with_static_access_returns_bindless() {
        // Static support with non-dynamic access is the sweet spot
        // for fixed descriptor arrays.
        assert_eq!(
            decide_bindless(inp(100, BindlessSupport::Static, false)),
            BindlessDecision::Bindless
        );
    }

    #[test]
    fn static_support_below_threshold_returns_traditional() {
        // Even with non-dynamic access, low count → traditional.
        assert_eq!(
            decide_bindless(inp(10, BindlessSupport::Static, false)),
            BindlessDecision::TraditionalBindings
        );
    }

    #[test]
    fn zero_resources_always_traditional() {
        for support in [
            BindlessSupport::Full,
            BindlessSupport::Static,
            BindlessSupport::Unsupported,
        ] {
            assert_eq!(
                decide_bindless(inp(0, support, false)),
                BindlessDecision::TraditionalBindings
            );
        }
    }

    #[test]
    fn threshold_constant_matches_documentation() {
        // Pin the calibrated threshold so casual edits don't move it
        // without a corresponding benchmark update.
        assert_eq!(BINDLESS_RESOURCE_COUNT_THRESHOLD, 24);
    }
}
