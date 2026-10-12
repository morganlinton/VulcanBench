//! Semiring registry test suite.

const REGISTRY: &str = include_str!("../../docs/optimization/SEMIRING_REGISTRY.toml");

#[test]
fn semiring_registry_names_identities_masks_and_backend_eligibility() {
    for required in [
        "add_identity",
        "mul_identity",
        "annihilator",
        "mask_policy",
        "sparsity_class",
        "backend_eligibility",
        "boolean-reachability",
        "min-plus-distance",
    ] {
        assert!(
            REGISTRY.contains(required),
            "semiring registry must include {required}"
        );
    }
}
