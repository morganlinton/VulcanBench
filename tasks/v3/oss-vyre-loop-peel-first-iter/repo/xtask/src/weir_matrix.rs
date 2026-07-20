//! Weir analysis API and integration evidence.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::hash::sha256_hex;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct WeirMatrix {
    schema_version: u32,
    package_name: String,
    package_version: String,
    release_package_token: String,
    analyses: Vec<WeirAnalysis>,
    feature_flags: Vec<WeirFeatureFlag>,
    required_feature_count: usize,
    missing_feature_count: usize,
    inventory_registered_count: usize,
    required_api_item_count: usize,
    missing_api_item_count: usize,
    property_test_count: usize,
    parity_test_count: usize,
    adversarial_test_count: usize,
    perf_test_count: usize,
    fuzz_test_count: usize,
    gap_test_count: usize,
    standalone_example_count: usize,
    standalone_serde_evidence_count: usize,
    standalone_serde_feature_guard_count: usize,
    standalone_example_scan_errors: Vec<String>,
    standalone_examples: Vec<ComponentFile>,
    bench_suite_count: usize,
    resident_benchmark_evidence_count: usize,
    fuzz_target_count: usize,
    fuzz_release_evidence_count: usize,
    bench_suites: Vec<WeirSourceArtifact>,
    resident_benchmark_evidence: Vec<WeirResidentBenchmarkEvidence>,
    fuzz_targets: Vec<WeirSourceArtifact>,
    fuzz_release_evidence: Vec<WeirFuzzReleaseEvidence>,
    corpus_manifest: WeirCorpusManifestArtifact,
    declared_release_artifacts: Vec<WeirDeclaredReleaseArtifact>,
    untested_analyses: Vec<&'static str>,
    integration_tests: Vec<WeirTest>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirAnalysis {
    id: &'static str,
    path: String,
    exists: bool,
    public_exported: bool,
    source_bytes: usize,
    has_public_api: bool,
    required_api_items: Vec<&'static str>,
    missing_api_items: Vec<&'static str>,
    required_policy_items: Vec<&'static str>,
    missing_policy_items: Vec<&'static str>,
    declares_op_id: bool,
    inventory_registered: bool,
    unresolved_markers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirTest {
    id: &'static str,
    path: String,
    exists: bool,
    source_bytes: usize,
    has_test_entrypoint: bool,
    assertion_count: usize,
    unresolved_markers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct ComponentFile {
    path: String,
    exists: bool,
    source_bytes: usize,
    read_error: Option<String>,
    has_main: bool,
    uses_weir_crate: bool,
    has_serde_evidence: bool,
    api_reference_count: usize,
    unresolved_markers: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirFeatureFlag {
    name: &'static str,
    cargo_declared: bool,
    readme_documented: bool,
}

#[derive(Debug, Clone, Serialize)]
struct WeirSourceArtifact {
    id: String,
    kind: &'static str,
    path: String,
    exists: bool,
    source_bytes: usize,
    read_error: Option<String>,
    required_tokens: Vec<&'static str>,
    missing_tokens: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirCorpusManifestArtifact {
    id: &'static str,
    kind: &'static str,
    path: String,
    exists: bool,
    source_bytes: usize,
    read_error: Option<String>,
    generator_command: &'static str,
    seed_count: Option<u64>,
    rng_seed: Option<u64>,
    category_ids: Vec<String>,
    seed_file_count: usize,
    seed_total_bytes: u64,
    corpus_fingerprint: String,
    required_fields: Vec<&'static str>,
    missing_fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirDeclaredReleaseArtifact {
    path: &'static str,
    documented: bool,
    expected_generator: &'static str,
    owner_lane: &'static str,
    generator_command: &'static str,
    source_fingerprint: String,
    freshness_fingerprint: String,
}

#[derive(Debug, Serialize)]
struct WeirIntegrationEvidence {
    schema_version: u32,
    tests: Vec<WeirTest>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WeirFlowReleaseContracts {
    schema_version: u32,
    package_name: String,
    package_version: String,
    release_package_token: String,
    feature_flags: Vec<WeirFeatureFlag>,
    bench_suites: Vec<WeirSourceArtifact>,
    resident_benchmark_evidence: Vec<WeirResidentBenchmarkEvidence>,
    fuzz_targets: Vec<WeirSourceArtifact>,
    fuzz_release_evidence: Vec<WeirFuzzReleaseEvidence>,
    corpus_manifest: WeirCorpusManifestArtifact,
    declared_release_artifacts: Vec<WeirDeclaredReleaseArtifact>,
    blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
struct WeirReadmeEvidence {
    schema_version: u32,
    path: String,
    exists: bool,
    source_bytes: usize,
    required_tokens: Vec<&'static str>,
    missing_tokens: Vec<&'static str>,
    example_count: usize,
    blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct WeirResidentBenchmarkEvidence {
    id: &'static str,
    path: String,
    exists: bool,
    source_bytes: usize,
    read_error: Option<String>,
    backend_id: &'static str,
    device_signature: &'static str,
    source_fingerprint: String,
    bench_command: String,
    required_fields: Vec<&'static str>,
    missing_fields: Vec<&'static str>,
    has_output_digest_field: bool,
    has_transfer_byte_fields: bool,
}

#[derive(Debug, Clone, Serialize)]
struct WeirFuzzReleaseEvidence {
    id: String,
    source_path: String,
    source_exists: bool,
    source_bytes: usize,
    source_read_error: Option<String>,
    source_fingerprint: String,
    corpus_path: String,
    corpus_exists: bool,
    corpus_file_count: usize,
    corpus_total_bytes: u64,
    corpus_digest: String,
    artifacts_path: String,
    artifact_file_count: usize,
    artifact_total_bytes: u64,
    artifact_digest: String,
    replay_command: String,
    corpus_replay_command: String,
    crash_replay_command: String,
    required_metadata: Vec<&'static str>,
    missing_metadata: Vec<&'static str>,
}

#[derive(Debug, Clone)]
struct WeirFuzzDirectoryDigest {
    exists: bool,
    file_count: usize,
    total_bytes: u64,
    digest: String,
}

const ANALYSES: &[(&str, &str)] = &[
    ("ssa", "src/ssa.rs"),
    ("def_use", "src/def_use.rs"),
    ("reaching", "src/reaching.rs"),
    ("reaching_def", "src/reaching_def.rs"),
    ("points_to", "src/points_to.rs"),
    ("may_alias", "src/may_alias.rs"),
    ("ifds", "src/ifds.rs"),
    ("ifds_gpu", "src/ifds_gpu.rs"),
    ("callgraph", "src/callgraph.rs"),
    ("control_dependence", "src/control_dependence.rs"),
    ("cross_language", "src/cross_language.rs"),
    ("dominators", "src/dominators.rs"),
    ("escape", "src/escape.rs"),
    ("escapes", "src/escapes.rs"),
    ("live", "src/live.rs"),
    ("live_at", "src/live_at.rs"),
    ("slice", "src/slice.rs"),
    ("summary", "src/summary.rs"),
    ("loop_sum", "src/loop_sum.rs"),
    ("must_init", "src/must_init.rs"),
    ("post_dominates", "src/post_dominates.rs"),
    ("range", "src/range.rs"),
    ("range_check", "src/range_check.rs"),
    ("reachability_witness", "src/reachability_witness.rs"),
    ("scc_query", "src/scc_query.rs"),
    ("soundness", "src/soundness.rs"),
    ("value_set", "src/value_set.rs"),
];

const TESTS: &[(&str, &str)] = &[
    ("adversarial_oracles", "tests/df_adversarial_oracles.rs"),
    ("anchor_bit_codegen", "tests/df_anchor_bit_codegen.rs"),
    ("cross_arm_raw_atomic", "tests/df_cross_arm_raw_atomic.rs"),
    ("construction_def_use", "tests/df_def_use.rs"),
    (
        "construction_dominators",
        "tests/df_dominators_construction.rs",
    ),
    ("construction_ifds", "tests/df_ifds_construction.rs"),
    ("construction_live", "tests/df_live_construction.rs"),
    (
        "construction_may_alias",
        "tests/df_may_alias_construction.rs",
    ),
    ("construction_reaching", "tests/df_reaching_construction.rs"),
    (
        "construction_range_check",
        "tests/df_range_check_construction.rs",
    ),
    ("cross_language", "tests/df_cross_language.rs"),
    (
        "cross_primitive_composition",
        "tests/df_cross_primitive_composition.rs",
    ),
    (
        "escape_callgraph_range",
        "tests/df_escape_callgraph_range.rs",
    ),
    ("live_at_escapes", "tests/df_live_at_escapes.rs"),
    ("must_init_scc_query", "tests/df_must_init_scc_query.rs"),
    (
        "parity_exact_primitives",
        "tests/df_parity_exact_primitives.rs",
    ),
    ("parity_dominators", "tests/df_parity_dominators.rs"),
    (
        "parity_inventory_sweep",
        "tests/df_parity_inventory_sweep.rs",
    ),
    ("parity_may_alias", "tests/df_parity_may_alias.rs"),
    ("reachability_witness", "tests/df_reachability_witness.rs"),
    (
        "slice_reaching_def_control_dep",
        "tests/df_slice_reaching_def_control_dep.rs",
    ),
    ("soundness_tags", "tests/df_soundness_tags.rs"),
    ("ssa_dominators", "tests/df_ssa_dominators.rs"),
    (
        "value_set_post_dominates",
        "tests/df_value_set_post_dominates.rs",
    ),
    ("property_points_to", "tests/df_property_points_to.rs"),
    ("property_may_alias", "tests/df_property_may_alias.rs"),
    ("property_ifds", "tests/df_property_ifds.rs"),
    (
        "property_control_dependence",
        "tests/df_property_control_dependence.rs",
    ),
    (
        "property_cross_language",
        "tests/df_property_cross_language.rs",
    ),
    ("property_def_use", "tests/df_property_def_use.rs"),
    ("property_dominators", "tests/df_property_dominators.rs"),
    ("property_range_check", "tests/df_property_range_check.rs"),
    ("property_range_escape", "tests/df_property_range_escape.rs"),
    (
        "property_reachability_witness",
        "tests/df_property_reachability_witness.rs",
    ),
    (
        "property_reaching_def_escapes",
        "tests/df_property_reaching_def_escapes.rs",
    ),
    ("property_slice", "tests/df_property_slice_construction.rs"),
    ("property_ssa", "tests/df_property_ssa_dominators.rs"),
    (
        "property_summary_callgraph",
        "tests/df_property_summary_callgraph.rs",
    ),
    ("property_value_set", "tests/df_property_value_set.rs"),
    (
        "property_bitset_oracles",
        "tests/df_property_bitset_oracles.rs",
    ),
    ("fuzz_bitset_oracles", "tests/df_fuzz_bitset_oracles.rs"),
    (
        "gap_bitset_oracle_edges",
        "tests/df_gap_bitset_oracle_edges.rs",
    ),
    ("resolve_family_node39", "tests/df_resolve_family_node39.rs"),
    ("summary_loop_points", "tests/df_summary_loop_points.rs"),
    ("three_arm_fusion", "tests/df_three_arm_fusion.rs"),
    ("perf_oracle", "tests/df_perf_oracle_throughput.rs"),
    ("scale_oracle", "tests/df_scale_oracle_no_oom.rs"),
];

const UNRESOLVED_MARKERS: &[&str] = &[
    "todo",
    "fixme",
    "placeholder",
    "stub",
    "todo!",
    "unimplemented!",
    "panic!(\"not implemented",
    "tbd",
];

const MIN_PROPERTY_TEST_FAMILIES: usize = 15;
const MIN_PARITY_TEST_FAMILIES: usize = 4;
const MIN_ADVERSARIAL_TEST_FAMILIES: usize = 1;
const MIN_PERF_TEST_FAMILIES: usize = 2;
const MIN_FUZZ_TEST_FAMILIES: usize = 1;
const MIN_GAP_TEST_FAMILIES: usize = 1;
const MIN_BENCH_SUITE_COUNT: usize = 10;
const MIN_FUZZ_TARGET_COUNT: usize = 9;
const MAX_WEIR_EVIDENCE_SOURCE_BYTES: u64 = 2_097_152;
const WEIR_CORPUS_GENERATOR_COMMAND: &str = "cargo_full run --bin corpus_expander";

const RESIDENT_BENCHMARK_REQUIRED_FIELDS: &[&str] = &[
    "ResidentBenchmarkEvidence",
    "backend_id",
    "device_signature",
    "source_fingerprint",
    "output_digest",
    "upload_transfer_bytes",
    "readback_transfer_bytes",
];

const RESIDENT_BENCHMARK_SUITES: &[(&str, &str, &str, &str)] = &[
    (
        "ifds_direct_resident_hot_path",
        "benches/ifds_direct_resident_hot_path.rs",
        "wgpu",
        "wgpu-live-device-required",
    ),
    (
        "resident_fixed_point_hot_path",
        "benches/resident_fixed_point_hot_path.rs",
        "weir_bench_resident_fixed_point",
        "mock-resident-sequence-window",
    ),
];

const FUZZ_RELEASE_REQUIRED_METADATA: &[&str] = &[
    "source_path",
    "source_fingerprint",
    "corpus_path",
    "corpus_digest",
    "replay_command",
    "corpus_replay_command",
    "crash_replay_command",
];

const REQUIRED_FEATURE_FLAGS: &[&str] = &[
    "cpu-parity",
    "gpu-telemetry",
    "serde",
    "simd-oracle",
    "test-harness",
];

const WEIR_MATRIX_OWNER_LANE: &str = "flow_weir";
const WEIR_MATRIX_GENERATOR_COMMAND: &str = "xtask weir-matrix";
const DECLARED_RELEASE_ARTIFACTS: &[&str] = &[
    "release/evidence/weir/weir-analysis-api-matrix.json",
    "release/evidence/weir/weir-vyre-integration-tests.json",
    "release/evidence/weir/weir-readme-contracts.json",
    "release/evidence/weir/weir-flow-release-contracts.json",
];

pub(crate) fn run(args: &[String]) {
    let output = match parse_output(args) {
        Ok(output) => output,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let weir_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|root| root.join("libs/dataflow/weir"))
        .unwrap_or_else(|| PathBuf::from("../../../../libs/dataflow/weir"));
    let mut blockers = Vec::new();
    let lib_rs_path = weir_root.join("src/lib.rs");
    let lib_rs = match read_text_bounded(&lib_rs_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "Weir public export scan could not read {}: {error}",
                lib_rs_path.display()
            ));
            String::new()
        }
    };
    let mut analyses = Vec::new();
    for &(id, relative) in ANALYSES {
        let path = weir_root.join(relative);
        let exists = path.is_file();
        let text = if exists {
            match read_text_bounded(&path) {
                Ok(text) => text,
                Err(error) => {
                    blockers.push(format!(
                        "Weir analysis `{id}` could not be read at {}: {error}",
                        path.display()
                    ));
                    String::new()
                }
            }
        } else {
            String::new()
        };
        let lowered = text.to_ascii_lowercase();
        let module_scope_text =
            analysis_module_scope_text(&weir_root, relative, &text, &mut blockers);
        let has_public_api = module_scope_text.contains("pub fn ")
            || module_scope_text.contains("pub struct ")
            || module_scope_text.contains("pub enum ")
            || module_scope_text.contains("pub type ")
            || text.contains("pub use ");
        let declares_op_id = text.contains("OP_ID");
        let inventory_registered = module_scope_text.contains("inventory::submit!")
            && module_scope_text.contains("vyre_harness::OpEntry::new");
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        let module_name = relative
            .strip_prefix("src/")
            .and_then(|value| value.strip_suffix(".rs"))
            .unwrap_or(id);
        let public_exported = lib_rs.contains(&format!("pub mod {module_name};"));
        if !exists {
            blockers.push(format!(
                "Weir analysis `{id}` is missing at {}",
                path.display()
            ));
        } else if text.trim().is_empty() {
            blockers.push(format!("Weir analysis `{id}` source file is empty"));
        }
        if !public_exported {
            blockers.push(format!(
                "Weir analysis `{id}` is not publicly exported from src/lib.rs"
            ));
        }
        if exists && !has_public_api {
            blockers.push(format!("Weir analysis `{id}` exposes no public API item"));
        }
        let required_api_items = required_api_items_for(id);
        let missing_api_items = required_api_items
            .iter()
            .copied()
            .filter(|required| !text.contains(required))
            .collect::<Vec<_>>();
        for required in &missing_api_items {
            blockers.push(format!(
                "Weir analysis `{id}` is missing required public API item `{required}`"
            ));
        }
        let required_policy_items = required_policy_items_for(id);
        let missing_policy_items = required_policy_items
            .iter()
            .copied()
            .filter(|required| !text.contains(required))
            .collect::<Vec<_>>();
        if id == "soundness" {
            for required in &missing_policy_items {
                blockers.push(format!(
                    "Weir soundness API is missing required policy item `{required}`"
                ));
            }
        }
        if exists && declares_op_id && !inventory_registered {
            blockers.push(format!(
                "Weir analysis `{id}` declares OP_ID but does not submit a vyre_harness::OpEntry"
            ));
        }
        for marker in &unresolved_markers {
            blockers.push(format!(
                "Weir analysis `{id}` contains unresolved marker `{marker}`"
            ));
        }
        analyses.push(WeirAnalysis {
            id,
            path: path.display().to_string(),
            exists,
            public_exported,
            source_bytes: text.len(),
            has_public_api,
            required_api_items,
            missing_api_items,
            required_policy_items,
            missing_policy_items,
            declares_op_id,
            inventory_registered,
            unresolved_markers,
        });
    }
    let mut integration_tests = Vec::new();
    for &(id, relative) in TESTS {
        let path = weir_root.join(relative);
        let exists = path.is_file();
        let text = if exists {
            match read_text_bounded(&path) {
                Ok(text) => text,
                Err(error) => {
                    blockers.push(format!(
                        "Weir integration test `{id}` could not be read at {}: {error}",
                        path.display()
                    ));
                    String::new()
                }
            }
        } else {
            String::new()
        };
        let lowered = text.to_ascii_lowercase();
        let has_test_entrypoint = text.contains("#[test]") || text.contains("proptest!");
        let assertion_count = assertion_count(&text);
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        if !exists {
            blockers.push(format!(
                "Weir integration test `{id}` is missing at {}",
                path.display()
            ));
        } else if text.trim().is_empty() {
            blockers.push(format!("Weir integration test `{id}` is empty"));
        }
        if exists && !has_test_entrypoint {
            blockers.push(format!(
                "Weir integration test `{id}` has no #[test] or proptest! entrypoint"
            ));
        }
        if exists && assertion_count == 0 {
            blockers.push(format!(
                "Weir integration test `{id}` has no assertion or property assertion"
            ));
        }
        for marker in &unresolved_markers {
            blockers.push(format!(
                "Weir integration test `{id}` contains unresolved marker `{marker}`"
            ));
        }
        integration_tests.push(WeirTest {
            id,
            path: path.display().to_string(),
            exists,
            source_bytes: text.len(),
            has_test_entrypoint,
            assertion_count,
            unresolved_markers,
        });
    }
    let property_test_count = integration_tests
        .iter()
        .filter(|test| test.id.starts_with("property_"))
        .count();
    let parity_test_count = integration_tests
        .iter()
        .filter(|test| test.id.starts_with("parity_"))
        .count();
    let adversarial_test_count = integration_tests
        .iter()
        .filter(|test| test.id.contains("adversarial"))
        .count();
    let perf_test_count = integration_tests
        .iter()
        .filter(|test| test.id.contains("perf") || test.id.contains("scale"))
        .count();
    let fuzz_test_count = integration_tests
        .iter()
        .filter(|test| test.id.contains("fuzz"))
        .count();
    let gap_test_count = integration_tests
        .iter()
        .filter(|test| test.id.contains("gap"))
        .count();
    let untested_analyses = analyses
        .iter()
        .filter(|analysis| !analysis_has_release_test(analysis.id, &integration_tests))
        .map(|analysis| analysis.id)
        .collect::<Vec<_>>();
    for analysis in &untested_analyses {
        blockers.push(format!(
            "Weir analysis `{analysis}` has no release integration, property, parity, fuzz, gap, perf, or scale test"
        ));
    }
    if property_test_count < MIN_PROPERTY_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {property_test_count} property test families; release requires at least {MIN_PROPERTY_TEST_FAMILIES}"
        ));
    }
    if parity_test_count < MIN_PARITY_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {parity_test_count} parity test families; release requires at least {MIN_PARITY_TEST_FAMILIES}"
        ));
    }
    if adversarial_test_count < MIN_ADVERSARIAL_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {adversarial_test_count} adversarial test families; release requires at least {MIN_ADVERSARIAL_TEST_FAMILIES}"
        ));
    }
    if perf_test_count < MIN_PERF_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {perf_test_count} perf/scale test families; release requires at least {MIN_PERF_TEST_FAMILIES}"
        ));
    }
    if fuzz_test_count < MIN_FUZZ_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {fuzz_test_count} fuzz test families; release requires at least {MIN_FUZZ_TEST_FAMILIES}"
        ));
    }
    if gap_test_count < MIN_GAP_TEST_FAMILIES {
        blockers.push(format!(
            "Weir matrix has {gap_test_count} gap test families; release requires at least {MIN_GAP_TEST_FAMILIES}"
        ));
    }
    let mut standalone_example_scan_errors = Vec::new();
    let standalone_examples = collect_standalone_examples(
        &weir_root,
        &mut blockers,
        &mut standalone_example_scan_errors,
    );
    if standalone_examples.len() < 2 {
        blockers.push(format!(
            "Weir matrix has {} standalone example(s); release requires at least 2 examples outside tests",
            standalone_examples.len()
        ));
    }
    let standalone_serde_evidence_count = standalone_examples
        .iter()
        .filter(|example| example.has_serde_evidence)
        .count();
    if standalone_serde_evidence_count == 0 {
        blockers.push(
            "Weir matrix has no standalone example proving serde evidence for witness or soundness API types"
                .to_string(),
        );
    }
    let cargo_toml_path = weir_root.join("Cargo.toml");
    let cargo_toml = match read_text_bounded(&cargo_toml_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "Weir Cargo.toml could not be read at {}: {error}",
                cargo_toml_path.display()
            ));
            String::new()
        }
    };
    let readme_path = weir_root.join("README.md");
    let readme = match read_text_bounded(&readme_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "Weir README could not be read for release surface evidence at {}: {error}",
                readme_path.display()
            ));
            String::new()
        }
    };
    let package_name =
        manifest_string_value(&cargo_toml, "name").unwrap_or_else(|| "weir".to_string());
    let package_version =
        manifest_string_value(&cargo_toml, "version").unwrap_or_else(|| "unknown".to_string());
    let release_package_token = format!("{package_name}@{package_version}");
    let feature_flags = collect_feature_flags(&cargo_toml, &readme);
    for feature in &feature_flags {
        if !feature.cargo_declared {
            blockers.push(format!(
                "Weir required feature `{}` is missing from Cargo.toml",
                feature.name
            ));
        }
        if !feature.readme_documented {
            blockers.push(format!(
                "Weir required feature `{}` is missing from README.md",
                feature.name
            ));
        }
    }
    let bench_suites = collect_source_artifacts(&weir_root, "benches", "bench", &[], &mut blockers);
    if bench_suites.len() < MIN_BENCH_SUITE_COUNT {
        blockers.push(format!(
            "Weir matrix has {} benchmark suite(s); release requires at least {MIN_BENCH_SUITE_COUNT}",
            bench_suites.len()
        ));
    }
    let resident_benchmark_evidence =
        collect_resident_benchmark_evidence(&weir_root, &mut blockers);
    let fuzz_cargo_toml_path = weir_root.join("fuzz/Cargo.toml");
    let fuzz_cargo_toml = match read_text_bounded(&fuzz_cargo_toml_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "Weir fuzz Cargo.toml could not be read at {}: {error}",
                fuzz_cargo_toml_path.display()
            ));
            String::new()
        }
    };
    let fuzz_targets = collect_source_artifacts(
        &weir_root,
        "fuzz/fuzz_targets",
        "fuzz",
        &["fuzz_target!"],
        &mut blockers,
    );
    if fuzz_targets.len() < MIN_FUZZ_TARGET_COUNT {
        blockers.push(format!(
            "Weir matrix has {} fuzz target(s); release requires at least {MIN_FUZZ_TARGET_COUNT}",
            fuzz_targets.len()
        ));
    }
    let fuzz_release_evidence =
        collect_fuzz_release_evidence(&weir_root, &fuzz_cargo_toml, &mut blockers);
    if fuzz_release_evidence.len() < MIN_FUZZ_TARGET_COUNT {
        blockers.push(format!(
            "Weir matrix has {} fuzz release evidence target(s); release requires at least {MIN_FUZZ_TARGET_COUNT}",
            fuzz_release_evidence.len()
        ));
    }
    let corpus_manifest = collect_corpus_manifest(&weir_root, &mut blockers);
    let declared_release_artifacts = declared_release_artifacts(&readme);
    for artifact in &declared_release_artifacts {
        if !artifact.documented {
            blockers.push(format!(
                "Weir README is missing declared release artifact `{}`",
                artifact.path
            ));
        }
    }
    let standalone_serde_feature_guard_count = usize::from(
        cargo_toml.contains("name = \"serde_evidence\"")
            && cargo_toml.contains("required-features = [\"serde\"]"),
    );
    if standalone_serde_evidence_count > 0 && standalone_serde_feature_guard_count == 0 {
        blockers.push(
            "Weir serde evidence example must declare required-features = [\"serde\"] in Cargo.toml"
                .to_string(),
        );
    }
    for example in &standalone_examples {
        if !example.exists {
            blockers.push(format!(
                "Weir standalone example {} is missing",
                example.path
            ));
        } else if let Some(error) = &example.read_error {
            blockers.push(format!(
                "Weir standalone example {} could not be read: {error}",
                example.path
            ));
        } else if example.source_bytes == 0 {
            blockers.push(format!("Weir standalone example {} is empty", example.path));
        }
        if example.exists && !example.has_main {
            blockers.push(format!(
                "Weir standalone example {} has no runnable fn main",
                example.path
            ));
        }
        if example.exists && !example.uses_weir_crate {
            blockers.push(format!(
                "Weir standalone example {} does not import or reference the weir crate",
                example.path
            ));
        }
        if example.exists && example.api_reference_count < 2 {
            blockers.push(format!(
                "Weir standalone example {} references {} dataflow API token(s); release requires at least 2",
                example.path, example.api_reference_count
            ));
        }
        for marker in &example.unresolved_markers {
            blockers.push(format!(
                "Weir standalone example {} contains unresolved marker `{marker}`",
                example.path
            ));
        }
    }
    let matrix = WeirMatrix {
        schema_version: 6,
        package_name,
        package_version,
        release_package_token,
        inventory_registered_count: analyses
            .iter()
            .filter(|analysis| analysis.inventory_registered)
            .count(),
        required_feature_count: feature_flags.len(),
        missing_feature_count: feature_flags
            .iter()
            .filter(|feature| !feature.cargo_declared || !feature.readme_documented)
            .count(),
        feature_flags,
        required_api_item_count: analyses
            .iter()
            .map(|analysis| analysis.required_api_items.len())
            .sum(),
        missing_api_item_count: analyses
            .iter()
            .map(|analysis| analysis.missing_api_items.len())
            .sum(),
        property_test_count,
        parity_test_count,
        adversarial_test_count,
        perf_test_count,
        fuzz_test_count,
        gap_test_count,
        standalone_example_count: standalone_examples.len(),
        standalone_serde_evidence_count,
        standalone_serde_feature_guard_count,
        standalone_example_scan_errors,
        standalone_examples,
        bench_suite_count: bench_suites.len(),
        resident_benchmark_evidence_count: resident_benchmark_evidence.len(),
        fuzz_target_count: fuzz_targets.len(),
        fuzz_release_evidence_count: fuzz_release_evidence.len(),
        bench_suites,
        resident_benchmark_evidence,
        fuzz_targets,
        fuzz_release_evidence,
        corpus_manifest,
        declared_release_artifacts,
        untested_analyses,
        analyses,
        integration_tests,
        blockers,
    };
    let json = match serde_json::to_string_pretty(&matrix) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize Weir matrix: {error}");
            std::process::exit(1);
        }
    };
    if let Some(parent) = output.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("Fix: failed to create `{}`: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&output, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", output.display());
        std::process::exit(1);
    }
    write_sibling_artifacts(&output, &matrix);
    println!("weir-matrix: wrote {}", output.display());
    if !matrix.blockers.is_empty() {
        std::process::exit(1);
    }
}

fn collect_standalone_examples(
    weir_root: &Path,
    blockers: &mut Vec<String>,
    scan_errors: &mut Vec<String>,
) -> Vec<ComponentFile> {
    let examples_root = weir_root.join("examples");
    let entries = match fs::read_dir(&examples_root) {
        Ok(entries) => entries,
        Err(error) => {
            let message = format!(
                "Weir examples directory could not be read at {}: {error}",
                examples_root.display()
            );
            blockers.push(message.clone());
            scan_errors.push(message);
            return Vec::new();
        }
    };
    let mut examples = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let message = format!(
                    "Weir examples entry could not be read in {}: {error}",
                    examples_root.display()
                );
                blockers.push(message.clone());
                scan_errors.push(message);
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let (text, read_error) = match read_text_bounded(&path) {
            Ok(text) => (text, None),
            Err(error) => (String::new(), Some(error.to_string())),
        };
        let lowered = text.to_ascii_lowercase();
        let unresolved_markers = UNRESOLVED_MARKERS
            .iter()
            .copied()
            .filter(|marker| lowered.contains(marker))
            .collect::<Vec<_>>();
        let api_reference_count = [
            "ssa",
            "reaching",
            "reachingdef",
            "points",
            "alias",
            "ifds",
            "callgraph",
            "slice",
            "summary",
            "loop",
            "fixpoint",
            "soundness",
        ]
        .iter()
        .filter(|token| lowered.contains(**token))
        .count();
        let has_serde_evidence = lowered.contains("serde")
            && lowered.contains("serialize")
            && (lowered.contains("deserialize") || lowered.contains("deserializeowned"))
            && (lowered.contains("pathseed") || lowered.contains("soundness"));
        examples.push(ComponentFile {
            path: path.display().to_string(),
            exists: path.is_file(),
            source_bytes: text.len(),
            read_error,
            has_main: text.contains("fn main(") || text.contains("fn main ()"),
            uses_weir_crate: text.contains("weir::") || text.contains("use weir"),
            has_serde_evidence,
            api_reference_count,
            unresolved_markers,
        });
    }
    examples.sort_by(|left, right| left.path.cmp(&right.path));
    examples
}

fn collect_feature_flags(cargo_toml: &str, readme: &str) -> Vec<WeirFeatureFlag> {
    let readme_lower = readme.to_ascii_lowercase();
    REQUIRED_FEATURE_FLAGS
        .iter()
        .copied()
        .map(|name| {
            let quoted = format!("\"{name}\"");
            let unquoted = format!("{name} =");
            WeirFeatureFlag {
                name,
                cargo_declared: cargo_toml.contains(&quoted) || cargo_toml.contains(&unquoted),
                readme_documented: readme_lower.contains(&name.to_ascii_lowercase()),
            }
        })
        .collect()
}

fn collect_source_artifacts(
    weir_root: &Path,
    relative_dir: &str,
    kind: &'static str,
    required_tokens: &[&'static str],
    blockers: &mut Vec<String>,
) -> Vec<WeirSourceArtifact> {
    let root = weir_root.join(relative_dir);
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) => {
            blockers.push(format!(
                "Weir {kind} directory could not be read at {}: {error}",
                root.display()
            ));
            return Vec::new();
        }
    };
    let mut artifacts = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "Weir {kind} entry could not be read in {}: {error}",
                    root.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let (text, read_error) = match read_text_bounded(&path) {
            Ok(text) => (text, None),
            Err(error) => (String::new(), Some(error.to_string())),
        };
        let missing_tokens = required_tokens
            .iter()
            .copied()
            .filter(|token| !text.contains(token))
            .collect::<Vec<_>>();
        for token in &missing_tokens {
            blockers.push(format!(
                "Weir {kind} artifact {} is missing required token `{token}`",
                path.display()
            ));
        }
        if let Some(error) = &read_error {
            blockers.push(format!(
                "Weir {kind} artifact {} could not be read: {error}",
                path.display()
            ));
        }
        if text.trim().is_empty() && read_error.is_none() {
            blockers.push(format!("Weir {kind} artifact {} is empty", path.display()));
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(kind)
            .to_string();
        artifacts.push(WeirSourceArtifact {
            id,
            kind,
            path: path.display().to_string(),
            exists: path.is_file(),
            source_bytes: text.len(),
            read_error,
            required_tokens: required_tokens.to_vec(),
            missing_tokens,
        });
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    artifacts
}

fn collect_resident_benchmark_evidence(
    weir_root: &Path,
    blockers: &mut Vec<String>,
) -> Vec<WeirResidentBenchmarkEvidence> {
    let mut evidence = Vec::new();
    for &(id, relative, backend_id, device_signature) in RESIDENT_BENCHMARK_SUITES {
        let path = weir_root.join(relative);
        let exists = path.is_file();
        let (text, read_error) = if exists {
            match read_text_bounded(&path) {
                Ok(text) => (text, None),
                Err(error) => {
                    let message = error.to_string();
                    blockers.push(format!(
                        "Weir resident benchmark `{id}` could not be read at {}: {message}",
                        path.display()
                    ));
                    (String::new(), Some(message))
                }
            }
        } else {
            blockers.push(format!(
                "Weir resident benchmark `{id}` is missing at {}",
                path.display()
            ));
            (String::new(), None)
        };
        let missing_fields = RESIDENT_BENCHMARK_REQUIRED_FIELDS
            .iter()
            .copied()
            .filter(|field| !text.contains(field))
            .collect::<Vec<_>>();
        for field in &missing_fields {
            blockers.push(format!(
                "Weir resident benchmark `{id}` is missing required evidence field `{field}`"
            ));
        }
        if exists && text.trim().is_empty() && read_error.is_none() {
            blockers.push(format!("Weir resident benchmark `{id}` source is empty"));
        }
        let has_output_digest_field = text.contains("output_digest");
        let has_transfer_byte_fields =
            text.contains("upload_transfer_bytes") && text.contains("readback_transfer_bytes");
        if !has_output_digest_field {
            blockers.push(format!(
                "Weir resident benchmark `{id}` must expose output_digest evidence"
            ));
        }
        if !has_transfer_byte_fields {
            blockers.push(format!(
                "Weir resident benchmark `{id}` must expose upload/readback transfer byte evidence"
            ));
        }
        let source_fingerprint = format!(
            "weir-resident-bench-source:v1:{}",
            fnv1a64_hex(text.as_bytes())
        );
        let bench_name = Path::new(relative)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(id);
        evidence.push(WeirResidentBenchmarkEvidence {
            id,
            path: path.display().to_string(),
            exists,
            source_bytes: text.len(),
            read_error,
            backend_id,
            device_signature,
            source_fingerprint,
            bench_command: format!("cargo_full bench --bench {bench_name}"),
            required_fields: RESIDENT_BENCHMARK_REQUIRED_FIELDS.to_vec(),
            missing_fields,
            has_output_digest_field,
            has_transfer_byte_fields,
        });
    }
    evidence.sort_by(|left, right| left.id.cmp(right.id));
    evidence
}

fn collect_fuzz_release_evidence(
    weir_root: &Path,
    fuzz_cargo_toml: &str,
    blockers: &mut Vec<String>,
) -> Vec<WeirFuzzReleaseEvidence> {
    let fuzz_root = weir_root.join("fuzz");
    let declared_targets = collect_declared_fuzz_targets(fuzz_cargo_toml, blockers);
    if declared_targets.is_empty() {
        blockers.push(format!(
            "Weir fuzz manifest at {} declares no [[bin]] fuzz targets",
            fuzz_root.join("Cargo.toml").display()
        ));
    }

    let mut evidence = Vec::new();
    for (id, source_relative) in declared_targets {
        let source_path = fuzz_root.join(&source_relative);
        let source_exists = source_path.is_file();
        let (source_text, source_read_error) = if source_exists {
            match read_text_bounded(&source_path) {
                Ok(text) => (text, None),
                Err(error) => {
                    let message = error.to_string();
                    blockers.push(format!(
                        "Weir fuzz target `{id}` could not be read at {}: {message}",
                        source_path.display()
                    ));
                    (String::new(), Some(message))
                }
            }
        } else {
            blockers.push(format!(
                "Weir fuzz target `{id}` source is missing at {}",
                source_path.display()
            ));
            (String::new(), None)
        };
        let source_fingerprint = format!(
            "weir-fuzz-target-source:v1:{}",
            fnv1a64_hex(source_text.as_bytes())
        );
        let corpus_path = fuzz_root.join("corpus").join(&id);
        let corpus_digest = collect_fuzz_directory_digest(
            &corpus_path,
            &format!("Weir fuzz corpus `{id}`"),
            true,
            blockers,
        );
        let artifacts_path = fuzz_root.join("artifacts").join(&id);
        let artifact_digest = collect_fuzz_directory_digest(
            &artifacts_path,
            &format!("Weir fuzz crash artifacts `{id}`"),
            false,
            blockers,
        );
        let replay_command = format!("cd {} && cargo_full fuzz run {id}", fuzz_root.display());
        let corpus_replay_command = format!(
            "cd {} && for seed in corpus/{id}/*; do test -f \"$seed\" && cargo_full fuzz run {id} \"$seed\"; done",
            fuzz_root.display()
        );
        let crash_replay_command = format!(
            "cd {} && for crash in artifacts/{id}/crash-*; do test -f \"$crash\" && cargo_full fuzz run {id} \"$crash\"; done",
            fuzz_root.display()
        );
        let mut missing_metadata = Vec::new();
        if !source_exists || source_text.trim().is_empty() || source_read_error.is_some() {
            missing_metadata.push("source_path");
            missing_metadata.push("source_fingerprint");
        }
        if !corpus_digest.exists || corpus_digest.file_count == 0 {
            missing_metadata.push("corpus_path");
            missing_metadata.push("corpus_digest");
        }
        if replay_command.trim().is_empty() {
            missing_metadata.push("replay_command");
        }
        if corpus_replay_command.trim().is_empty() {
            missing_metadata.push("corpus_replay_command");
        }
        if crash_replay_command.trim().is_empty() {
            missing_metadata.push("crash_replay_command");
        }
        for field in &missing_metadata {
            blockers.push(format!(
                "Weir fuzz target `{id}` is missing release evidence metadata `{field}`"
            ));
        }
        evidence.push(WeirFuzzReleaseEvidence {
            id,
            source_path: source_path.display().to_string(),
            source_exists,
            source_bytes: source_text.len(),
            source_read_error,
            source_fingerprint,
            corpus_path: corpus_path.display().to_string(),
            corpus_exists: corpus_digest.exists,
            corpus_file_count: corpus_digest.file_count,
            corpus_total_bytes: corpus_digest.total_bytes,
            corpus_digest: corpus_digest.digest,
            artifacts_path: artifacts_path.display().to_string(),
            artifact_file_count: artifact_digest.file_count,
            artifact_total_bytes: artifact_digest.total_bytes,
            artifact_digest: artifact_digest.digest,
            replay_command,
            corpus_replay_command,
            crash_replay_command,
            required_metadata: FUZZ_RELEASE_REQUIRED_METADATA.to_vec(),
            missing_metadata,
        });
    }
    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    evidence
}

fn collect_declared_fuzz_targets(
    fuzz_cargo_toml: &str,
    blockers: &mut Vec<String>,
) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let mut in_bin = false;
    let mut name = None::<String>;
    let mut path = None::<String>;
    for line in fuzz_cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bin]]" {
            if in_bin {
                push_declared_fuzz_target(&mut targets, name.take(), path.take(), blockers);
            }
            in_bin = true;
            continue;
        }
        if !in_bin {
            continue;
        }
        if let Some(value) = manifest_line_string_value(trimmed, "name") {
            name = Some(value);
        } else if let Some(value) = manifest_line_string_value(trimmed, "path") {
            path = Some(value);
        }
    }
    if in_bin {
        push_declared_fuzz_target(&mut targets, name, path, blockers);
    }
    targets.sort_by(|left, right| left.0.cmp(&right.0));
    targets
}

fn push_declared_fuzz_target(
    targets: &mut Vec<(String, String)>,
    name: Option<String>,
    path: Option<String>,
    blockers: &mut Vec<String>,
) {
    let Some(name) = name else {
        blockers.push("Weir fuzz manifest has [[bin]] without name".to_string());
        return;
    };
    let Some(path) = path else {
        blockers.push(format!("Weir fuzz manifest target `{name}` has no path"));
        return;
    };
    if !path.starts_with("fuzz_targets/") || !path.ends_with(".rs") {
        blockers.push(format!(
            "Weir fuzz manifest target `{name}` path `{path}` must point at fuzz_targets/*.rs"
        ));
    }
    if targets.iter().any(|(existing, _)| existing == &name) {
        blockers.push(format!(
            "Weir fuzz manifest declares duplicate target `{name}`"
        ));
        return;
    }
    targets.push((name, path));
}

fn collect_fuzz_directory_digest(
    root: &Path,
    label: &str,
    missing_is_blocker: bool,
    blockers: &mut Vec<String>,
) -> WeirFuzzDirectoryDigest {
    if !root.is_dir() {
        if missing_is_blocker {
            blockers.push(format!("{label} directory is missing at {}", root.display()));
        }
        return WeirFuzzDirectoryDigest {
            exists: false,
            file_count: 0,
            total_bytes: 0,
            digest: format!(
                "weir-fuzz-file-tree:v1:{}",
                fnv1a64_hex(format!("missing:{}", root.display()).as_bytes())
            ),
        };
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            blockers.push(format!("{label} directory {} could not be read: {error}", root.display()));
            return WeirFuzzDirectoryDigest {
                exists: true,
                file_count: 0,
                total_bytes: 0,
                digest: format!(
                    "weir-fuzz-file-tree:v1:{}",
                    fnv1a64_hex(format!("unreadable:{}", root.display()).as_bytes())
                ),
            };
        }
    };
    let mut file_records = Vec::new();
    let mut total_bytes = 0u64;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "{label} entry in {} could not be read: {error}",
                    root.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                blockers.push(format!("{label} file {} metadata failed: {error}", path.display()));
                continue;
            }
        };
        let len = metadata.len();
        if len > MAX_WEIR_EVIDENCE_SOURCE_BYTES {
            blockers.push(format!(
                "{label} file {} is {len} bytes, above evidence digest cap {MAX_WEIR_EVIDENCE_SOURCE_BYTES}",
                path.display()
            ));
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                blockers.push(format!("{label} file {} could not be read: {error}", path.display()));
                continue;
            }
        };
        total_bytes = total_bytes.saturating_add(len);
        file_records.push(format!(
            "{}:{len}:{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<non-utf8>"),
            fnv1a64_hex(&bytes)
        ));
    }
    file_records.sort();
    WeirFuzzDirectoryDigest {
        exists: true,
        file_count: file_records.len(),
        total_bytes,
        digest: format!(
            "weir-fuzz-file-tree:v1:{}",
            fnv1a64_hex(file_records.join("\n").as_bytes())
        ),
    }
}

fn collect_corpus_manifest(
    weir_root: &Path,
    blockers: &mut Vec<String>,
) -> WeirCorpusManifestArtifact {
    let manifest_path = weir_root.join("tests/corpus/seeds/manifest.json");
    let required_fields = vec![
        "seed_count",
        "categories",
        "rng_seed",
        "manifest_path",
        "output_directory",
    ];
    let exists = manifest_path.is_file();
    let mut read_error = None;
    let mut text = String::new();
    if exists {
        match read_text_bounded(&manifest_path) {
            Ok(value) => text = value,
            Err(error) => {
                let message = error.to_string();
                blockers.push(format!(
                    "Weir corpus manifest {} could not be read: {message}",
                    manifest_path.display()
                ));
                read_error = Some(message);
            }
        }
    } else {
        blockers.push(format!(
            "Weir corpus manifest is missing at {}. Fix: run `{WEIR_CORPUS_GENERATOR_COMMAND}` from {}.",
            manifest_path.display(),
            weir_root.display()
        ));
    }

    let mut seed_count = None;
    let mut rng_seed = None;
    let mut category_ids = Vec::new();
    let mut missing_fields = required_fields.clone();
    if !text.is_empty() {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => {
                missing_fields = required_fields
                    .iter()
                    .copied()
                    .filter(|field| value.get(*field).is_none())
                    .collect();
                seed_count = value.get("seed_count").and_then(serde_json::Value::as_u64);
                rng_seed = value.get("rng_seed").and_then(serde_json::Value::as_u64);
                if let Some(categories) =
                    value.get("categories").and_then(serde_json::Value::as_object)
                {
                    category_ids = categories.keys().cloned().collect();
                    category_ids.sort();
                }
            }
            Err(error) => {
                blockers.push(format!(
                    "Weir corpus manifest {} is not valid JSON: {error}",
                    manifest_path.display()
                ));
            }
        }
    }
    for field in &missing_fields {
        blockers.push(format!(
            "Weir corpus manifest {} is missing required field `{field}`",
            manifest_path.display()
        ));
    }

    let seeds_dir = weir_root.join("tests/corpus/seeds");
    let (seed_file_count, seed_total_bytes, seed_listing_fingerprint) =
        corpus_seed_listing_fingerprint(&seeds_dir, blockers);
    if let Some(expected) = seed_count {
        if expected != seed_file_count as u64 {
            blockers.push(format!(
                "Weir corpus manifest declares seed_count={expected}, but {seed_file_count} .bin fixture(s) exist in {}",
                seeds_dir.display()
            ));
        }
    }
    if category_ids.is_empty() && exists && read_error.is_none() {
        blockers.push(format!(
            "Weir corpus manifest {} has no category ids",
            manifest_path.display()
        ));
    }

    let corpus_fingerprint = fnv1a64_hex(
        format!(
            "{}\n{}\n{}\n{}",
            text, seed_file_count, seed_total_bytes, seed_listing_fingerprint
        )
        .as_bytes(),
    );

    WeirCorpusManifestArtifact {
        id: "weir_corpus_manifest",
        kind: "corpus",
        path: manifest_path.display().to_string(),
        exists,
        source_bytes: text.len(),
        read_error,
        generator_command: WEIR_CORPUS_GENERATOR_COMMAND,
        seed_count,
        rng_seed,
        category_ids,
        seed_file_count,
        seed_total_bytes,
        corpus_fingerprint,
        required_fields,
        missing_fields,
    }
}

fn corpus_seed_listing_fingerprint(
    seeds_dir: &Path,
    blockers: &mut Vec<String>,
) -> (usize, u64, String) {
    let entries = match fs::read_dir(seeds_dir) {
        Ok(entries) => entries,
        Err(error) => {
            blockers.push(format!(
                "Weir corpus seed directory {} could not be read: {error}",
                seeds_dir.display()
            ));
            return (0, 0, fnv1a64_hex(b"missing-weir-corpus-seeds"));
        }
    };
    let mut seed_files = Vec::new();
    let mut total_bytes = 0u64;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "Weir corpus seed entry in {} could not be read: {error}",
                    seeds_dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("bin") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                blockers.push(format!(
                    "Weir corpus seed {} metadata could not be read: {error}",
                    path.display()
                ));
                continue;
            }
        };
        let len = metadata.len();
        total_bytes = total_bytes.saturating_add(len);
        seed_files.push(format!(
            "{}:{len}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<non-utf8>")
        ));
    }
    seed_files.sort();
    let fingerprint = fnv1a64_hex(seed_files.join("\n").as_bytes());
    (seed_files.len(), total_bytes, fingerprint)
}

fn declared_release_artifacts(readme: &str) -> Vec<WeirDeclaredReleaseArtifact> {
    DECLARED_RELEASE_ARTIFACTS
        .iter()
        .map(|path| {
            let (source_fingerprint, freshness_fingerprint) =
                declared_release_artifact_fingerprints(path);
            WeirDeclaredReleaseArtifact {
            path,
            documented: readme.contains(path),
                expected_generator: WEIR_MATRIX_GENERATOR_COMMAND,
                owner_lane: WEIR_MATRIX_OWNER_LANE,
                generator_command: WEIR_MATRIX_GENERATOR_COMMAND,
                source_fingerprint,
                freshness_fingerprint,
            }
        })
        .collect()
}

fn declared_release_artifact_fingerprints(path: &str) -> (String, String) {
    let source_material = format!(
        "weir-matrix-declared-artifact:v1\nowner_lane={WEIR_MATRIX_OWNER_LANE}\ngenerator={WEIR_MATRIX_GENERATOR_COMMAND}\nartifact={path}\n"
    );
    let source_hash = sha256_hex(source_material.as_bytes());
    let freshness_material = format!(
        "release-evidence-freshness:v1\nartifact={path}\ngenerator={WEIR_MATRIX_GENERATOR_COMMAND}\nsource={source_hash}\n"
    );
    (
        format!("release-evidence-source:v1:{source_hash}"),
        format!(
            "release-evidence-freshness:v1:{}",
            sha256_hex(freshness_material.as_bytes())
        ),
    )
}

fn manifest_string_value(manifest: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    manifest.lines().find_map(|line| {
        manifest_line_string_value(line.trim(), key).or_else(|| {
            let value = line.trim().strip_prefix(&prefix)?;
            let value = value.trim();
            let value = value.strip_prefix('"')?.strip_suffix('"')?;
            Some(value.to_string())
        })
    })
}

fn manifest_line_string_value(trimmed: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    let value = trimmed.strip_prefix(&prefix)?;
    let value = value.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.to_string())
}

fn write_sibling_artifacts(output: &Path, matrix: &WeirMatrix) {
    let Some(parent) = output.parent() else {
        eprintln!(
            "Fix: Weir matrix output `{}` has no parent directory.",
            output.display()
        );
        std::process::exit(1);
    };
    let blockers = matrix
        .integration_tests
        .iter()
        .flat_map(|test| {
            let mut blockers = Vec::new();
            if !test.exists {
                blockers.push(format!(
                    "Weir integration test `{}` is missing at {}",
                    test.id, test.path
                ));
            }
            if test.exists && test.source_bytes == 0 {
                blockers.push(format!("Weir integration test `{}` is empty", test.id));
            }
            if test.exists && !test.has_test_entrypoint {
                blockers.push(format!(
                    "Weir integration test `{}` has no #[test] or proptest! entrypoint",
                    test.id
                ));
            }
            if test.exists && test.assertion_count == 0 {
                blockers.push(format!(
                    "Weir integration test `{}` has no assertion or property assertion",
                    test.id
                ));
            }
            for marker in &test.unresolved_markers {
                blockers.push(format!(
                    "Weir integration test `{}` contains unresolved marker `{marker}`",
                    test.id
                ));
            }
            blockers
        })
        .collect::<Vec<_>>();
    write_json(
        &parent.join("weir-vyre-integration-tests.json"),
        &WeirIntegrationEvidence {
            schema_version: 2,
            tests: matrix.integration_tests.clone(),
            blockers,
        },
    );
    write_json(
        &parent.join("weir-flow-release-contracts.json"),
        &WeirFlowReleaseContracts {
            schema_version: 4,
            package_name: matrix.package_name.clone(),
            package_version: matrix.package_version.clone(),
            release_package_token: matrix.release_package_token.clone(),
            feature_flags: matrix.feature_flags.clone(),
            bench_suites: matrix.bench_suites.clone(),
            resident_benchmark_evidence: matrix.resident_benchmark_evidence.clone(),
            fuzz_targets: matrix.fuzz_targets.clone(),
            fuzz_release_evidence: matrix.fuzz_release_evidence.clone(),
            corpus_manifest: matrix.corpus_manifest.clone(),
            declared_release_artifacts: matrix.declared_release_artifacts.clone(),
            blockers: matrix
                .blockers
                .iter()
                .filter(|blocker| {
                    blocker.contains("feature")
                        || blocker.contains("benchmark")
                        || blocker.contains("fuzz")
                        || blocker.contains("corpus")
                        || blocker.contains("release artifact")
                })
                .cloned()
                .collect(),
        },
    );
    write_weir_readme_artifact(parent);
}

fn write_weir_readme_artifact(parent: &Path) {
    let weir_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|root| root.join("libs/dataflow/weir"))
        .unwrap_or_else(|| PathBuf::from("../../../../libs/dataflow/weir"));
    let readme = weir_root.join("README.md");
    let exists = readme.is_file();
    let mut blockers = Vec::new();
    let text = if exists {
        match read_text_bounded(&readme) {
            Ok(text) => text,
            Err(error) => {
                blockers.push(format!(
                    "Weir README could not be read at {}: {error}",
                    readme.display()
                ));
                String::new()
            }
        }
    } else {
        String::new()
    };
    let lowered = text.to_ascii_lowercase();
    let required_tokens = vec![
        "0.1.0",
        "dataflow",
        "vyre",
        "ssa",
        "def-use",
        "reaching",
        "reaching-definition",
        "points-to",
        "may-alias",
        "ifds",
        "callgraph",
        "control-dependence",
        "cross-language",
        "dominators",
        "escape",
        "live",
        "must-init",
        "post-dominates",
        "range",
        "range-check",
        "scc",
        "slice",
        "summary",
        "value-set",
        "soundness",
        "serde",
        "default feature",
        "serde_evidence",
        "required-features",
        "precisioncontract",
        "primitive soundness",
        "cargo add weir",
    ];
    let missing_tokens = required_tokens
        .iter()
        .copied()
        .filter(|token| !lowered.contains(&token.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    let example_count = text.matches("```rust").count() + text.matches("```toml").count();
    if !exists {
        blockers.push(format!("Weir README is missing at {}", readme.display()));
    }
    if exists && text.trim().is_empty() {
        blockers.push("Weir README is empty".to_string());
    }
    for token in &missing_tokens {
        blockers.push(format!("Weir README is missing required token `{token}`"));
    }
    if example_count == 0 {
        blockers
            .push("Weir README must include at least one Rust or TOML example block".to_string());
    }
    write_json(
        &parent.join("weir-readme-contracts.json"),
        &WeirReadmeEvidence {
            schema_version: 2,
            path: readme.display().to_string(),
            exists,
            source_bytes: text.len(),
            required_tokens,
            missing_tokens,
            example_count,
            blockers,
        },
    );
}

fn write_json(path: &Path, value: &impl Serialize) {
    let json = match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Fix: failed to serialize `{}`: {error}", path.display());
            std::process::exit(1);
        }
    };
    if let Err(error) = fs::write(path, format!("{json}\n")) {
        eprintln!("Fix: failed to write `{}`: {error}", path.display());
        std::process::exit(1);
    }
}

fn parse_output(args: &[String]) -> Result<PathBuf, String> {
    let mut output = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("Fix: --output requires a path.".to_string());
                };
                output = Some(PathBuf::from(path));
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "USAGE:\n  cargo_full run --bin xtask -- weir-matrix [--output PATH]\n\n\
                     Writes Weir analysis API and integration evidence."
                );
                std::process::exit(0);
            }
            other => return Err(format!("Fix: unknown weir-matrix option `{other}`.")),
        }
    }
    Ok(output.unwrap_or_else(default_output))
}

fn default_output() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|path| path.join("release/evidence/weir/weir-analysis-api-matrix.json"))
        .unwrap_or_else(|| PathBuf::from("release/evidence/weir/weir-analysis-api-matrix.json"))
}

fn analysis_module_scope_text(
    weir_root: &Path,
    relative: &str,
    top_level_text: &str,
    blockers: &mut Vec<String>,
) -> String {
    let mut scope = String::from(top_level_text);
    let Some(module_name) = relative
        .strip_prefix("src/")
        .and_then(|value| value.strip_suffix(".rs"))
    else {
        return scope;
    };
    let module_dir = weir_root.join("src").join(module_name);
    if !module_dir.is_dir() {
        return scope;
    }
    let entries = match fs::read_dir(&module_dir) {
        Ok(entries) => entries,
        Err(error) => {
            blockers.push(format!(
                "Weir analysis module `{module_name}` could not be scanned at {}: {error}",
                module_dir.display()
            ));
            return scope;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                blockers.push(format!(
                    "Weir analysis module `{module_name}` had unreadable entry in {}: {error}",
                    module_dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        match read_text_bounded(&path) {
            Ok(text) => {
                scope.push('\n');
                scope.push_str(&text);
            }
            Err(error) => blockers.push(format!(
                "Weir analysis module `{module_name}` could not read {} while scanning registration scope: {error}",
                path.display()
            )),
        }
    }
    scope
}

fn required_api_items_for(id: &str) -> Vec<&'static str> {
    match id {
        "ssa" => vec![
            "SsaForm",
            "Cfg",
            "ssa_phi_placement_step",
            "compute_dominators",
            "compute_dominance_frontiers",
            "place_phi_nodes",
            "rename_variables",
            "Ssa",
        ],
        "def_use" => vec![
            "def_use_chain",
            "def_use_chain_bitset",
            "def_use_query",
            "cpu_ref",
            "DefUse",
        ],
        "reaching" => vec!["reaching_defs_step", "ReachingDefs"],
        "reaching_def" => vec!["reaching_def", "cpu_ref", "ReachingDef"],
        "points_to" => vec![
            "andersen_points_to",
            "andersen_points_to_with_shape",
            "cpu_subset_closure",
            "PointsTo",
        ],
        "may_alias" => vec!["may_alias", "cpu_ref", "MayAlias"],
        "ifds" => vec!["ifds_reach_step", "ifds_reach_step_exploded", "Ifds"],
        "ifds_gpu" => vec![
            "solve_cpu",
            "IfdsShape",
            "ifds_gpu_step",
            "ifds_gpu",
            "IfdsGpu",
        ],
        "callgraph" => vec!["callgraph_build", "callgraph_build_with_count", "Callgraph"],
        "control_dependence" => {
            vec!["control_dependence", "cpu_ref", "ControlDependence"]
        }
        "cross_language" => vec![
            "EDGE_KIND_FFI",
            "EDGE_KIND_ALL",
            "cross_language",
            "cpu_ref",
            "CrossLanguage",
        ],
        "dominators" => vec![
            "dominates",
            "cpu_ref",
            "compute_cpu",
            "compute_bitmap_bytes",
            "Dominators",
        ],
        "escape" => vec!["escape_analyze", "escape_analyze_with_count", "Escape"],
        "escapes" => vec!["escapes", "cpu_ref", "Escapes"],
        "live" => vec!["live_step", "Liveness"],
        "live_at" => vec!["live_at", "cpu_ref", "LiveAt"],
        "slice" => vec![
            "backward_slice",
            "backward_slice_with_shape",
            "BackwardSlice",
        ],
        "summary" => vec![
            "summarize_function",
            "summarize_function_with_count",
            "Summary",
        ],
        "loop_sum" => vec!["loop_summarize", "loop_summarize_with_count", "LoopSum"],
        "must_init" => vec!["must_init", "cpu_ref", "MustInit"],
        "post_dominates" => vec!["post_dominates", "cpu_ref", "PostDominates"],
        "range" => vec!["range_propagate", "range_propagate_with_count", "Range"],
        "range_check" => vec!["range_check", "cpu_ref", "RangeCheck"],
        "reachability_witness" => vec![
            "PathSeed",
            "ExtractedPath",
            "PreparedWitnessGraph",
            "exploded_reachability_to_statement_mask",
            "extract_path",
            "prepare_witness_graph",
            "extract_path_prepared",
            "NodeAttr",
        ],
        "scc_query" => vec!["scc_query", "cpu_ref", "SccQuery"],
        "soundness" => vec![
            "Soundness",
            "PrecisionContract",
            "PrimitiveSoundness",
            "SoundnessViolation",
            "SoundnessTagged",
            "validate_pipeline",
            "validate_primitive",
        ],
        "value_set" => vec!["value_set", "cpu_ref", "ValueSet"],
        _ => Vec::new(),
    }
}

fn required_policy_items_for(id: &str) -> Vec<&'static str> {
    if id == "soundness" {
        vec![
            "PrecisionContract",
            "PrimitiveSoundness",
            "SoundnessViolation",
            "SoundnessTagged",
            "validate_pipeline",
            "validate_primitive",
        ]
    } else {
        Vec::new()
    }
}

fn analysis_has_release_test(id: &str, tests: &[WeirTest]) -> bool {
    let aliases = analysis_test_aliases(id);
    tests.iter().any(|test| {
        test.exists
            && test.has_test_entrypoint
            && test.assertion_count > 0
            && aliases.iter().any(|alias| test.id.contains(alias))
    })
}

fn analysis_test_aliases(id: &str) -> Vec<&str> {
    match id {
        "reaching_def" => vec!["reaching_def", "slice_reaching_def"],
        "points_to" => vec!["points_to", "points"],
        "may_alias" => vec!["may_alias", "alias"],
        "ifds_gpu" => vec!["ifds_gpu", "ifds"],
        "control_dependence" => vec!["control_dependence", "control_dep"],
        "escape" => vec!["escape", "range_escape"],
        "escapes" => vec!["escapes", "live_at_escapes"],
        "live_at" => vec!["live_at", "live_at_escapes"],
        "summary" => vec!["summary", "summary_loop_points"],
        "loop_sum" => vec!["loop_sum", "summary_loop_points"],
        "must_init" => vec!["must_init", "must_init_scc_query"],
        "post_dominates" => vec!["post_dominates", "value_set_post_dominates"],
        "range" => vec!["range", "range_escape"],
        "scc_query" => vec!["scc_query", "must_init_scc_query"],
        "soundness" => vec!["soundness", "soundness_tags"],
        other => vec![other],
    }
}

fn assertion_count(text: &str) -> usize {
    [
        "assert!(",
        "assert_eq!(",
        "assert_ne!(",
        "prop_assert!(",
        "prop_assert_eq!(",
        "prop_assert_ne!(",
    ]
    .iter()
    .map(|needle| text.matches(needle).count())
    .sum()
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_WEIR_EVIDENCE_SOURCE_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_WEIR_EVIDENCE_SOURCE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_WEIR_EVIDENCE_SOURCE_BYTES} byte Weir evidence read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}

fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
