//! Fuzz target inventory test suite.

const TARGETS: &str = include_str!("../../docs/optimization/FUZZ_TARGET_INVENTORY.toml");
const CORPORA: &str = include_str!("../../docs/optimization/FUZZ_CORPUS_MINIMIZATION.toml");

#[test]
fn fuzz_target_inventory_names_owner_entrypoint_oracle_and_resource_bounds() {
    for required in [
        "target_id",
        "owning_crate",
        "entrypoint",
        "input_domain",
        "oracle",
        "timeout_ms",
        "memory_limit_bytes",
        "crash_dedup_key",
        "privacy_class",
    ] {
        assert!(
            TARGETS.contains(required),
            "fuzz target inventory must include {required}"
        );
    }
}

#[test]
fn fuzz_corpus_minimization_records_seeds_mutators_and_differential_lanes() {
    for required in [
        "corpus_id",
        "seed_source",
        "seed_digest",
        "minimizer",
        "structured_mutator",
        "differential_lanes",
        "reduced_input_digest",
        "owner_lane",
        "wire_format",
        "source_dialect_envelope",
    ] {
        assert!(
            CORPORA.contains(required),
            "fuzz corpus minimization contract must include {required}"
        );
    }
}
