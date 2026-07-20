//! Prototype kernel comparator test suite.

const COMPARATOR: &str = include_str!("../../docs/optimization/PROTOTYPE_KERNEL_COMPARATOR.toml");

#[test]
fn prototype_kernel_comparator_requires_vyre_owned_promotion_evidence() {
    for required in [
        "prototype_tool",
        "promotion_criteria",
        "abi_check",
        "output_parity",
        "active_ns",
        "toolchain_digest",
        "production_owner",
        "vyre-driver",
    ] {
        assert!(
            COMPARATOR.contains(required),
            "prototype kernel comparator must include {required}"
        );
    }
}
