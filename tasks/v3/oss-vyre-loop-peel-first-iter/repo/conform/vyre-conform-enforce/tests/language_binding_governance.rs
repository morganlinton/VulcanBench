//! Language binding governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SURFACES: &str =
    include_str!("../../../docs/optimization/LANGUAGE_BINDING_SURFACE_MATRIX.toml");
const C_ABI: &str =
    include_str!("../../../docs/optimization/C_ABI_HEADER_LIBRARY_POLICY.toml");
const PYTHON: &str =
    include_str!("../../../docs/optimization/PYTHON_WHEEL_BINDING_POLICY.toml");
const NODE_WASM: &str =
    include_str!("../../../docs/optimization/NODE_WASM_BINDING_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_LANGUAGE_BINDING_TRANCHE_COVERAGE.toml");

#[test]
fn language_binding_sources_are_registered() {
    for key in [
        "RUST_REFERENCE_LINKAGE",
        "PYO3_USER_GUIDE",
        "MATURIN_USER_GUIDE",
        "PYTHON_WHEEL_SPEC",
        "NODE_API_ABI",
        "NAPI_RS",
        "NPM_PACKAGE_JSON",
        "WEBASSEMBLY_CORE_SPEC",
        "WEBASSEMBLY_COMPONENT_MODEL",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn language_binding_surface_matrix_keeps_binding_products_crate_boundaries_packaging_semver_verification_and_private_boundaries_distinct() {
    for required in [
        "binding_id",
        "language_surface",
        "crate_boundary",
        "artifact_form",
        "ffi_or_component_policy",
        "packaging_policy",
        "semver_policy",
        "verification_policy",
        "private_boundary_policy",
        "c-abi-library",
        "python-wheel-binding",
        "node-api-addon-binding",
        "wasm-component-binding",
    ] {
        assert!(
            SURFACES.contains(required),
            "language binding surface matrix must include {required}"
        );
    }
}

#[test]
fn c_abi_policy_records_crate_types_symbols_headers_layout_ownership_errors_unwinding_and_versioning() {
    for required in [
        "c_api_id",
        "crate_type_policy",
        "symbol_policy",
        "header_policy",
        "layout_policy",
        "ownership_policy",
        "error_policy",
        "unwind_policy",
        "versioning_policy",
        "vyre-public-c-library",
        "vyre-callback-bridge",
    ] {
        assert!(
            C_ABI.contains(required),
            "C ABI header library policy must include {required}"
        );
    }
}

#[test]
fn python_wheel_policy_records_pyo3_maturin_wheel_tags_metadata_api_mapping_import_tests_integrity_and_private_boundaries() {
    for required in [
        "wheel_id",
        "module_policy",
        "build_policy",
        "wheel_tag_policy",
        "metadata_policy",
        "api_mapping_policy",
        "import_test_policy",
        "artifact_integrity_policy",
        "private_boundary_policy",
        "vyre-python-extension",
        "vyre-python-platform-wheel",
    ] {
        assert!(
            PYTHON.contains(required),
            "Python wheel binding policy must include {required}"
        );
    }
}

#[test]
fn node_wasm_policy_records_node_api_npm_wasm_component_host_capability_api_mapping_verification_and_boundaries() {
    for required in [
        "binding_id",
        "runtime_surface",
        "abi_or_component_policy",
        "package_manifest_policy",
        "artifact_policy",
        "host_capability_policy",
        "api_mapping_policy",
        "verification_policy",
        "private_boundary_policy",
        "vyre-node-api-addon",
        "vyre-wasm-component",
    ] {
        assert!(
            NODE_WASM.contains(required),
            "Node Wasm binding policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_language_binding_rows() {
    for row in [
        "VX-1201",
        "VX-1202",
        "VX-1203",
        "VX-1204",
        "VX-1205",
        "VX-1206",
        "VX-1207",
        "VX-1208",
        "VX-1209",
        "VX-1210",
        "VX-1211",
        "VX-1212",
        "VX-1213",
        "VX-1214",
        "VX-1215",
        "VX-1216",
        "VX-1217",
        "VX-1218",
        "VX-1219",
        "VX-1220",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn language_binding_coverage_reuses_unsafe_ffi_api_crate_artifact_and_publication_authorities() {
    for required in [
        "VX-1201..VX-1220",
        "language_binding_surface_matrix",
        "c_abi_header_library_policy",
        "python_wheel_binding_policy",
        "node_wasm_binding_policy",
        "unsafe_ffi_governance",
        "public_api_semver_msrv_policy",
        "crate_boundary_feature_matrix",
        "release_artifact_integrity_index",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "language binding tranche coverage must include {required}"
        );
    }
}
