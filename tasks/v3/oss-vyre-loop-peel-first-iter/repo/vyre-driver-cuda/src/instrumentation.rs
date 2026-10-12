//! Cached CUDA instrumentation environment controls.
//!
//! Dispatch and lowering paths are hot enough that optional diagnostics cannot
//! repeatedly query process environment variables. This module centralizes those
//! controls behind one-time reads.

use std::sync::OnceLock;

pub(crate) const CUDA_CANONICAL_PREEMIT_ENV: &str = "VYRE_CUDA_CANONICAL_PREEMIT";
pub(crate) const CUDA_DESCRIPTOR_REWRITES_ENV: &str = "VYRE_CUDA_DESCRIPTOR_REWRITES";
pub(crate) const CUDA_RESIDENT_BORROWED_FALLBACK_ENV: &str = "VYRE_CUDA_RESIDENT_BORROWED_FALLBACK";
pub(crate) const CUDA_ALLOW_BORROWED_FALLBACK_ENV: &str = "VYRE_CUDA_ALLOW_BORROWED_FALLBACK";

pub(crate) fn cuda_stage_trace_enabled() -> bool {
    cached_flag("VYRE_CUDA_STAGE_TRACE", &CUDA_STAGE_TRACE)
}

pub(crate) fn cuda_profiler_ranges_enabled() -> bool {
    cached_flag("VYRE_CUDA_NVTX_RANGES", &CUDA_NVTX_RANGES)
        || cached_flag("VYRE_CUDA_PROFILE_RANGES", &CUDA_PROFILE_RANGES)
}

pub(crate) fn cuda_descriptor_audit_enabled() -> bool {
    cached_flag("VYRE_CUDA_DESCRIPTOR_AUDIT", &CUDA_DESCRIPTOR_AUDIT)
}

pub(crate) fn cuda_canonical_preemit_enabled() -> bool {
    cached_enabled_default_true(CUDA_CANONICAL_PREEMIT_ENV, &CUDA_CANONICAL_PREEMIT_DISABLED)
}

pub(crate) fn cuda_descriptor_rewrites_enabled() -> bool {
    cached_enabled_default_true(
        CUDA_DESCRIPTOR_REWRITES_ENV,
        &CUDA_DESCRIPTOR_REWRITES_DISABLED,
    )
}

pub(crate) fn cuda_graph_replay_enabled() -> bool {
    cached_enabled_default_true("VYRE_CUDA_GRAPH_REPLAY", &CUDA_GRAPH_REPLAY_DISABLED)
}

pub(crate) fn cuda_dispatch_validation_enabled() -> bool {
    cached_enabled_default_true(
        "VYRE_CUDA_VALIDATE_DISPATCH",
        &CUDA_VALIDATE_DISPATCH_DISABLED,
    )
}

pub(crate) fn cuda_resident_borrowed_fallback_enabled() -> bool {
    cached_resident_borrowed_fallback_policy().enabled
}

pub(crate) fn cuda_resident_sync_before_launch_enabled() -> bool {
    cached_flag(
        "VYRE_CUDA_RESIDENT_SYNC_BEFORE_LAUNCH",
        &CUDA_RESIDENT_SYNC_BEFORE_LAUNCH,
    )
}

static CUDA_STAGE_TRACE: OnceLock<bool> = OnceLock::new();
static CUDA_NVTX_RANGES: OnceLock<bool> = OnceLock::new();
static CUDA_PROFILE_RANGES: OnceLock<bool> = OnceLock::new();
static CUDA_DESCRIPTOR_AUDIT: OnceLock<bool> = OnceLock::new();
static CUDA_CANONICAL_PREEMIT_DISABLED: OnceLock<bool> = OnceLock::new();
static CUDA_DESCRIPTOR_REWRITES_DISABLED: OnceLock<bool> = OnceLock::new();
static CUDA_GRAPH_REPLAY_DISABLED: OnceLock<bool> = OnceLock::new();
static CUDA_VALIDATE_DISPATCH_DISABLED: OnceLock<bool> = OnceLock::new();
static CUDA_RESIDENT_BORROWED_FALLBACK_POLICY: OnceLock<ResidentBorrowedFallbackPolicy> =
    OnceLock::new();
static CUDA_RESIDENT_SYNC_BEFORE_LAUNCH: OnceLock<bool> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentBorrowedFallbackPolicy {
    enabled: bool,
}

fn cached_resident_borrowed_fallback_policy() -> ResidentBorrowedFallbackPolicy {
    *CUDA_RESIDENT_BORROWED_FALLBACK_POLICY.get_or_init(compute_resident_borrowed_fallback_policy)
}

fn compute_resident_borrowed_fallback_policy() -> ResidentBorrowedFallbackPolicy {
    if !std::env::var_os(CUDA_RESIDENT_BORROWED_FALLBACK_ENV).is_some() {
        return ResidentBorrowedFallbackPolicy { enabled: false };
    }

    #[cfg(debug_assertions)]
    {
        return ResidentBorrowedFallbackPolicy { enabled: true };
    }

    #[cfg(not(debug_assertions))]
    {
        if env_value_enables_explicit_true(
            &std::env::var(CUDA_ALLOW_BORROWED_FALLBACK_ENV).unwrap_or_default(),
        ) {
            eprintln!(
                "\n\
                 ================================================================================\n\
                 WARNING: VYRE_CUDA_ALLOW_BORROWED_FALLBACK=1\n\
                 CUDA resident dispatch is using the host-buffer BORROWED FALLBACK path.\n\
                 This path downloads resident buffers, runs dispatch_borrowed, and re-uploads.\n\
                 Release performance and parity evidence are INVALID while this escape hatch is on.\n\
                 Unset {CUDA_RESIDENT_BORROWED_FALLBACK_ENV} and {CUDA_ALLOW_BORROWED_FALLBACK_ENV} \
                 before collecting release metrics.\n\
                 ================================================================================\n"
            );
            return ResidentBorrowedFallbackPolicy { enabled: true };
        }

        eprintln!(
            "\n\
             ERROR: {CUDA_RESIDENT_BORROWED_FALLBACK_ENV} is set but resident borrowed fallback \
             is refused on release builds.\n\
             Fix: unset {CUDA_RESIDENT_BORROWED_FALLBACK_ENV} and use native CUDA resident dispatch, \
             or set {CUDA_ALLOW_BORROWED_FALLBACK_ENV}=1 only for local debugging (invalidates release perf).\n"
        );
        ResidentBorrowedFallbackPolicy { enabled: false }
    }
}

fn env_value_enables_explicit_true(value: &str) -> bool {
    matches!(value, "1" | "true" | "TRUE" | "on" | "ON")
}

fn cached_flag(name: &'static str, slot: &OnceLock<bool>) -> bool {
    *slot.get_or_init(|| std::env::var_os(name).is_some())
}

fn cached_enabled_default_true(name: &'static str, disabled_slot: &OnceLock<bool>) -> bool {
    !*disabled_slot.get_or_init(|| {
        std::env::var(name)
            .map(|value| env_value_disables_default_true(&value))
            .unwrap_or(false)
    })
}

fn env_value_disables_default_true(value: &str) -> bool {
    matches!(value, "0" | "false" | "FALSE" | "off" | "OFF")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_true_env_parser_accepts_only_explicit_false_values() {
        for value in ["0", "false", "FALSE", "off", "OFF"] {
            assert!(
                env_value_disables_default_true(value),
                "Fix: CUDA default-on instrumentation knob value `{value}` must disable the feature."
            );
        }
        for value in ["", "1", "true", "TRUE", "on", "ON", "False", "Off"] {
            assert!(
                !env_value_disables_default_true(value),
                "Fix: CUDA default-on instrumentation knob value `{value}` must not accidentally disable the feature."
            );
        }
    }

    #[test]
    fn cached_flags_are_stable_after_first_read() {
        let slot = OnceLock::new();
        assert!(!cached_flag("VYRE_CUDA_TEST_UNSET_FLAG", &slot));
        assert!(!cached_flag("VYRE_CUDA_TEST_UNSET_FLAG", &slot));
    }

    #[test]
    fn explicit_true_env_parser_accepts_only_release_escape_hatch_values() {
        for value in ["1", "true", "TRUE", "on", "ON"] {
            assert!(
                env_value_enables_explicit_true(value),
                "Fix: CUDA resident borrowed-fallback allow value `{value}` must enable the escape hatch."
            );
        }
        for value in ["", "0", "false", "FALSE", "off", "OFF", "yes"] {
            assert!(
                !env_value_enables_explicit_true(value),
                "Fix: CUDA resident borrowed-fallback allow value `{value}` must not accidentally enable the escape hatch."
            );
        }
    }

    #[test]
    fn resident_borrowed_fallback_is_not_opt_in_on_release_path() {
        let source = include_str!("instrumentation.rs");
        assert!(
            source.contains(CUDA_RESIDENT_BORROWED_FALLBACK_ENV)
                && source.contains(CUDA_ALLOW_BORROWED_FALLBACK_ENV)
                && source.contains("compute_resident_borrowed_fallback_policy")
                && source.contains("#[cfg(debug_assertions)]")
                && source.contains("#[cfg(not(debug_assertions))]"),
            "Fix: CUDA resident borrowed fallback must stay debug-opt-in and require an explicit release escape hatch."
        );
        assert!(
            !source.contains("cached_flag(\n        \"VYRE_CUDA_RESIDENT_BORROWED_FALLBACK\""),
            "Fix: CUDA resident borrowed fallback must not be a bare env-presence flag on release builds."
        );
    }
}
