//! Maturity-gate contract for the beta C frontend.
//!
//! The crate may remain beta, but the conditions for leaving beta must be
//! explicit, evidence-backed, and linked from the README.

use std::fs;
use std::path::Path;

#[test]
fn maturity_gate_exists_and_pins_beta_status_with_promotion_criteria() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let maturity_path = root.join("MATURITY.md");
    let maturity = fs::read_to_string(&maturity_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", maturity_path.display()));

    for required in [
        "Current maturity: **beta / active development**.",
        "## Scope boundary",
        "## Promotion gate",
        "## Production criteria",
        "## Evidence artifacts",
        "Parser correctness",
        "GPU-first execution",
        "Preprocessor parity",
        "Object ABI stability",
        "Clang differential evidence",
        "Adversarial inputs",
        "Performance evidence",
        "Fuzz readiness",
    ] {
        assert!(
            maturity.contains(required),
            "MATURITY.md must contain `{required}`"
        );
    }
}

#[test]
fn readme_links_to_maturity_gate_instead_of_freeform_beta_claims() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme_path = root.join("README.md");
    let readme = fs::read_to_string(&readme_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", readme_path.display()));

    assert!(
        readme.contains("[`MATURITY.md`](MATURITY.md)"),
        "README beta status must link to the authoritative maturity gate"
    );
}

#[test]
fn maturity_evidence_artifacts_exist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "parity/PARITY_MANIFEST_V1.md",
        "parity/PREPROCESS_BENCHMARK_V1.md",
        "fuzz/README.md",
        "benches/parser_pipeline.rs",
        "benches/real_corpus.rs",
        "tests/parity_release_gate.rs",
        "tests/preprocess_differential_benchmark.rs",
        "tests/object_version_stability.rs",
        "tests/cuda_first_no_host_paths.rs",
        "tests/gpu_directive_kernels_real_gpu.rs",
        "tests/gpu_prep_kernel_libc_shape.rs",
        "tests/gpu_prepare_tu_source_e2e.rs",
        "tests/linux_grade_constructs_gpu_e2e.rs",
    ] {
        assert!(
            root.join(relative).is_file(),
            "maturity evidence artifact must exist: {relative}"
        );
    }
}

#[test]
fn maturity_gate_pins_real_gpu_and_linux_grade_artifacts_in_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let maturity_path = root.join("MATURITY.md");
    let maturity = fs::read_to_string(&maturity_path)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", maturity_path.display()));

    for required in [
        "tests/gpu_directive_kernels_real_gpu.rs",
        "tests/gpu_prep_kernel_libc_shape.rs",
        "tests/gpu_prepare_tu_source_e2e.rs",
        "tests/linux_grade_constructs_gpu_e2e.rs",
    ] {
        assert!(
            maturity.contains(required),
            "MATURITY.md must pin real GPU/Linux-grade evidence artifact `{required}`"
        );
    }
}
