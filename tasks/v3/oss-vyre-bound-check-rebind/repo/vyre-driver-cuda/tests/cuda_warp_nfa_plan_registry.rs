//! Cuda warp nfa plan registry test suite.

const PLANS: &str = include_str!("../../docs/optimization/CUDA_WARP_NFA_PLANS.toml");

#[test]
fn cuda_warp_nfa_plan_registry_records_route_metrics_and_parity() {
    for required in [
        "active_ns",
        "divergence_proxy",
        "match_parity",
        "warp_program_digest",
        "unsupported_reason",
        "route_id = \"warp_bit_parallel_nfa\"",
        "backend = \"cuda\"",
        "warp_ownership = \"one-warp-per-pattern-frontier\"",
        "match_parity_required = true",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: CUDA warp NFA plan registry must include `{required}`"
        );
    }
}

#[test]
fn cuda_warp_nfa_plan_registry_requires_verifier_and_refusal_reasons() {
    for required in [
        "verifier_required = true",
        "comparator = \"cpu_regex_reference\"",
        "VYRE_CUDA_SCAN_UNSUPPORTED_CONSTRUCT",
        "VYRE_CUDA_SCAN_WARP_PROGRAM_TOO_LARGE",
    ] {
        assert!(
            PLANS.contains(required),
            "Fix: CUDA warp NFA plan registry must include `{required}`"
        );
    }
    assert_eq!(
        PLANS
            .matches("evidence_path = \"vyre-driver-cuda/tests/cuda_warp_nfa_plan_registry.rs\"")
            .count(),
        PLANS.matches("[[route]]").count() + PLANS.matches("[[refusal]]").count(),
        "Fix: every CUDA warp NFA row must point at this proof gate"
    );
}
