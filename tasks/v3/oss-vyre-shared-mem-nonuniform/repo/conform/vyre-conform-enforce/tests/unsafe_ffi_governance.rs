//! Unsafe ffi governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/UNSAFE_FFI_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn unsafe_ffi_primary_sources_are_registered() {
    for key in [
        "RUST_BOOK_UNSAFE",
        "RUST_NOMICON_FFI",
        "RUST_UNSAFE_CODE_GUIDELINES",
        "RUST_RFC_2945_C_UNWIND",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn unsafe_ffi_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-961..VX-980",
        "unsafe_block_safety_contracts",
        "ffi_abi_boundary_contracts",
        "memory_layout_aliasing_alignment",
        "backend_handle_lifetime_provenance",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "unsafe FFI governance tranche coverage must include {required}"
        );
    }
}
