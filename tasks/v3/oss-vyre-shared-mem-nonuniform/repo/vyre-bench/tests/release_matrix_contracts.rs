//! Release matrix coverage contracts. Asserts that every release
//! workload family declared in `vyre-bench::release_matrix` ships with
//! a registered runner and committed evidence, so a release sweep cannot
//! silently skip a family.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use vyre_bench::api::case::{BaselineClass, WorkloadClass};
use vyre_bench::report::json::{
    REQUIRED_BENCHMARK_CASE_FIELDS, REQUIRED_BENCHMARK_METRIC_FIELDS,
};

#[path = "release_matrix_contracts/command_contracts.rs"]
mod command_contracts;
#[path = "release_matrix_contracts/family_exclusions.rs"]
mod family_exclusions;
#[path = "release_matrix_contracts/readme_contracts.rs"]
mod readme_contracts;
#[path = "release_matrix_contracts/suite_artifacts.rs"]
mod suite_artifacts;
#[path = "release_matrix_contracts/thesis_axes.rs"]
mod thesis_axes;

use vyre_bench::api::suite::SuiteKind;

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Fix: vyre-bench must live under the workspace root.")
        .to_path_buf()
}

fn bench_targets_manifest() -> toml::Value {
    let workspace = workspace_root();
    let targets_text =
        std::fs::read_to_string(workspace.join("docs/optimization/BENCH_TARGETS.toml"))
            .expect("Fix: BENCH_TARGETS.toml must be readable.");
    toml::from_str(&targets_text).expect("Fix: BENCH_TARGETS.toml must parse as TOML.")
}

fn bench_target_rows(targets: &toml::Value) -> &[toml::Value] {
    targets
        .get("target")
        .and_then(toml::Value::as_array)
        .expect("Fix: BENCH_TARGETS.toml must contain target rows.")
}

#[test]
fn release_matrix_covers_required_workload_families() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    assert!(
        matrix.required_closed_families >= 12,
        "Fix: release matrix declares {} required workload families; release requires at least 12 proof workloads.",
        matrix.required_closed_families
    );
    assert!(
        matrix
            .families
            .iter()
            .filter(|family| family.required)
            .count()
            >= 12,
        "Fix: release matrix must enumerate at least 12 required proof workload families."
    );
    assert!(
        matrix.matched_required_families >= matrix.required_closed_families,
        "Fix: release matrix covers {} workload families, but release requires at least {}. Blockers: {:?}",
        matrix.matched_required_families,
        matrix.required_closed_families,
        matrix.blockers
    );
}

#[test]
fn release_matrix_advertises_required_benchmark_evidence_schema() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    assert_eq!(
        matrix.benchmark_evidence_schema_version, 1,
        "Fix: release matrix must version the benchmark evidence schema."
    );
    assert_eq!(
        matrix.required_benchmark_case_fields,
        REQUIRED_BENCHMARK_CASE_FIELDS,
        "Fix: release matrix required case fields must share the report validation constants."
    );
    assert_eq!(
        matrix.required_benchmark_metric_fields,
        REQUIRED_BENCHMARK_METRIC_FIELDS,
        "Fix: release matrix required metric fields must share the report validation constants."
    );
}

#[test]
fn release_matrix_has_cpu_sota_hundred_x_contract() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let targets = bench_targets_manifest();
    let target_rows = bench_target_rows(&targets);
    let mut required_cases = BTreeSet::new();
    for family in matrix.families.iter().filter(|family| {
        matrix
            .required_cpu_sota_100x_families
            .iter()
            .any(|required| *required == family.id)
    }) {
        assert!(
            !family.bench_target_ids.is_empty(),
            "Fix: required CPU-SOTA 100x family `{}` must link to BENCH_TARGETS.toml target data.",
            family.id
        );
        for target_id in &family.bench_target_ids {
            let target = target_rows
                .iter()
                .find(|target| {
                    target.get("id").and_then(toml::Value::as_str) == Some(*target_id)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Fix: required CPU-SOTA 100x family `{}` references missing BENCH_TARGETS target `{target_id}`.",
                        family.id
                    )
                });
            let case_id = target
                .get("bench_case_id")
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "Fix: required CPU-SOTA 100x BENCH_TARGETS target `{target_id}` must declare bench_case_id."
                    )
                });
            required_cases.insert(case_id.to_string());
        }
    }
    assert!(
        !required_cases.is_empty(),
        "Fix: release matrix must derive required CPU-SOTA 100x case ids from BENCH_TARGETS.toml."
    );
    assert!(
        matrix.cpu_sota_100x_contract_count >= 10,
        "Fix: release matrix must include at least ten CPU-SOTA 100x contracts for the CUDA release proof workloads."
    );
    assert!(
        matrix.cpu_sota_100x_family_count >= 10,
        "Fix: release matrix must cover at least ten CPU-SOTA 100x workload families."
    );
    assert!(
        matrix.missing_required_cpu_sota_100x_families.is_empty(),
        "Fix: release matrix is missing required CPU-SOTA 100x family/families: {:?}",
        matrix.missing_required_cpu_sota_100x_families
    );
    for case_id in required_cases {
        assert!(
            matrix
                .cpu_sota_100x_contract_cases
                .iter()
                .any(|actual| actual == &case_id),
            "Fix: release matrix does not list required CPU-SOTA 100x case `{case_id}`."
        );
    }
}

#[test]
fn release_matrix_contains_current_required_family_ids() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let required_families = [
        "condition-eval",
        "string-bitmap-scatter",
        "offset-count-aggregation",
        "metadata-conditions",
        "entropy-window",
        "quantified-condition-loops",
        "alias-reaching-def",
        "ifds-witness",
        "c-ast-traversal",
        "megakernel-queued-batches",
        "egraph-saturation",
        "sparse-output-compaction",
        "callgraph-reachability",
    ];
    for family_id in required_families {
        let Some(family) = matrix.families.iter().find(|family| family.id == family_id) else {
            panic!("Fix: release matrix is missing required family `{family_id}`.");
        };
        assert!(
            family.required,
            "Fix: release matrix family `{family_id}` must be release-required."
        );
        assert!(
            !family.matched_cases.is_empty(),
            "Fix: release matrix family `{family_id}` has no active release case."
        );
        assert!(
            family
                .bench_target_ids
                .iter()
                .all(|target| target.starts_with("release.workload.")),
            "Fix: release matrix family `{family_id}` must map to canonical release benchmark target ids."
        );
    }
}
