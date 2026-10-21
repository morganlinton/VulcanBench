use std::path::Path;

use crate::benchmark_evidence_semantics::{
    benchmark_duplicate_source_artifact_paths, benchmark_source_artifact_count,
    benchmark_source_artifact_entry_count, cpu_sota_100x_source_artifact_issues,
    duplicate_nonblank_string_array_values,
};

use super::super::checks::*;
use super::super::types::Requirement;

pub(super) fn check(requirement: &Requirement, base_dir: &Path, failures: &mut Vec<String>) {
    let Some(matrix) = first_json_evidence(
        requirement,
        base_dir,
        "release-workload-matrix.json",
        failures,
    ) else {
        return;
    };
    let contracts = matrix
        .get("cpu_sota_100x_contract_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if contracts < 10 {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` has {contracts} CPU-SOTA 100x contract(s) in the workload matrix; needs at least 10"
        ));
    }
    let covered_families = matrix
        .get("cpu_sota_100x_family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if covered_families < 10 {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` has {covered_families} covered workload family/families with a CPU-SOTA 100x contract; needs at least 10"
        ));
    }
    let required_hundred_x = matrix
        .get("required_cpu_sota_100x_families")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_hundred_x < 10 {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` matrix lists only {required_hundred_x} required 100x family/families; needs at least 10 release 100x families"
        ));
    }
    check_duplicate_string_array_values(
        "workload matrix",
        &matrix,
        "required_cpu_sota_100x_families",
        failures,
    );
    let missing_required_hundred_x = matrix
        .get("missing_required_cpu_sota_100x_families")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_hundred_x != 0 {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` matrix reports {missing_required_hundred_x} missing required 100x family/families"
        ));
    }
    let contract_cases = matrix
        .get("cpu_sota_100x_contract_cases")
        .and_then(serde_json::Value::as_array)
        .map(|cases| {
            cases
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if contract_cases.is_empty() {
        failures.push(
            "requirement `cpu-only-100x-proof` workload matrix does not list the active 100x contract case ids"
                .to_string(),
        );
    }
    check_duplicate_string_array_values(
        "workload matrix",
        &matrix,
        "cpu_sota_100x_contract_cases",
        failures,
    );
    if let Some(proof) =
        first_json_evidence(requirement, base_dir, "cpu-only-100x-proof.json", failures)
    {
        let proof_blockers = proof
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if proof_blockers != 0 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof reports {proof_blockers} blocker(s)"
            ));
        }
        check_cpu_100x_aggregate_source_provenance(&proof, failures);
        if proof.get("git").is_none_or(serde_json::Value::is_null) {
            failures.push(
                "requirement `cpu-only-100x-proof` aggregate proof must preserve git provenance object"
                    .to_string(),
            );
        }
        let workspace_root = base_dir
            .file_name()
            .is_some_and(|name| name == "release")
            .then(|| base_dir.parent())
            .flatten()
            .unwrap_or(base_dir);
        check_cpu_100x_source_artifact_counts(&proof, workspace_root, failures);
        let required_proof_cases = proof
            .get("required_cpu_sota_100x_cases")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if required_proof_cases < 10 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof lists {required_proof_cases} required 100x case(s); needs at least 10 release 100x cases"
            ));
        }
        let missing_proof_cases = proof
            .get("missing_required_cpu_sota_100x_cases")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if missing_proof_cases != 0 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof reports {missing_proof_cases} missing required 100x case(s)"
            ));
        }
        check_duplicate_string_array_values(
            "aggregate proof",
            &proof,
            "required_cpu_sota_100x_cases",
            failures,
        );
        let proof_contract_case_count = proof
            .get("cases")
            .and_then(serde_json::Value::as_array)
            .map(|cases| {
                check_required_cpu_100x_cases(&proof, cases, failures);
                cases
                    .iter()
                    .filter(|case| {
                        case.get("id")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|id| contract_cases.contains(&id))
                    })
                    .count()
            })
            .unwrap_or(0);
        if proof_contract_case_count < 10 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof artifact contains {proof_contract_case_count} case(s) listed in cpu_sota_100x_contract_cases; needs at least 10"
            ));
        }
        check_cpu_100x_aggregate_case_counts(&proof, failures);
        let aggregate_contract_cases = proof
            .get("cpu_sota_100x_contract_case_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if aggregate_contract_cases < 10 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof has {aggregate_contract_cases} CPU-SOTA 100x contract case(s); needs at least 10"
            ));
        }
        let aggregate_passing_cases = proof
            .get("cpu_sota_100x_passing_case_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if aggregate_passing_cases < 10 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof has {aggregate_passing_cases} passing CPU-SOTA 100x case(s); needs at least 10"
            ));
        }
        let min_wall_samples = proof
            .get("min_wall_samples")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if min_wall_samples < 30 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof min_wall_samples={min_wall_samples}; needs at least 30"
            ));
        }
        let min_baseline_wall_samples = proof
            .get("min_baseline_wall_samples")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if min_baseline_wall_samples < 30 {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof min_baseline_wall_samples={min_baseline_wall_samples}; needs at least 30"
            ));
        }
        for field in [
            "min_wall_p50",
            "min_wall_p95",
            "min_wall_p99",
            "min_baseline_wall_p50",
            "min_baseline_wall_p95",
            "min_baseline_wall_p99",
        ] {
            if proof
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `cpu-only-100x-proof` aggregate proof has non-positive `{field}`"
                ));
            }
        }
    }
    check_benchmark_evidence_reports(
        requirement,
        base_dir,
        "cpu-only-100x-proof.json",
        true,
        Some(100.0),
        failures,
    );
}

fn check_duplicate_string_array_values(
    label: &str,
    value: &serde_json::Value,
    field: &str,
    failures: &mut Vec<String>,
) {
    let duplicates = duplicate_nonblank_string_array_values(value, field);
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        failures.push(format!(
            "requirement `cpu-only-100x-proof` {label} has duplicate {field}: {duplicates}"
        ));
    }
}

fn check_required_cpu_100x_cases(
    proof: &serde_json::Value,
    cases: &[serde_json::Value],
    failures: &mut Vec<String>,
) {
    for required_case in [
        "release.condition_eval.1m",
        "release.string_bitmap_scatter.1m",
        "release.offset_count_aggregation.1m",
        "release.entropy_window.1m",
        "release.quantified_condition_loops.1m",
        "release.alias_reaching_def.1m",
        "release.ifds_witness.1m",
        "release.c_ast_traversal.1m",
        "release.megakernel_queue.1m",
        "release.egraph_saturation.1m",
        "sparse.compaction.count.1m",
    ] {
        if !cases.iter().any(|case| {
            let case_backend = case
                .get("backend_id")
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    proof
                        .get("selected_backend")
                        .and_then(serde_json::Value::as_str)
                });
            case.get("id").and_then(serde_json::Value::as_str) == Some(required_case)
                && crate::benchmark_evidence_semantics::benchmark_case_proves_cpu_sota_100x(
                    case,
                    case_backend,
                )
        }) {
            failures.push(format!(
                "requirement `cpu-only-100x-proof` aggregate proof required case `{required_case}` does not prove a passing 100x CUDA win"
            ));
        }
    }
}

fn check_cpu_100x_aggregate_source_provenance(
    proof: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    if proof
        .get("source_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        failures.push(
            "requirement `cpu-only-100x-proof` aggregate proof must preserve source_fingerprint"
                .to_string(),
        );
    }
    if proof
        .get("source_tree_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        failures.push(
            "requirement `cpu-only-100x-proof` aggregate proof must preserve source_tree_fingerprint"
                .to_string(),
        );
    }
}

fn check_cpu_100x_source_artifact_counts(
    proof: &serde_json::Value,
    workspace_root: &Path,
    failures: &mut Vec<String>,
) {
    let unique_source_artifacts = benchmark_source_artifact_count(proof) as u64;
    let raw_source_artifacts = benchmark_source_artifact_entry_count(proof) as u64;
    let declared_source_artifacts = proof
        .get("source_artifact_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if declared_source_artifacts != unique_source_artifacts {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof source_artifact_count={declared_source_artifacts}, but unique source_artifacts={unique_source_artifacts}"
        ));
    }
    if unique_source_artifacts < 10 {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof has {unique_source_artifacts} unique source artifact(s); needs at least 10"
        ));
    }
    if raw_source_artifacts != unique_source_artifacts {
        let duplicates = benchmark_duplicate_source_artifact_paths(proof)
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof has duplicate source_artifacts: {duplicates}"
        ));
    }
    for issue in cpu_sota_100x_source_artifact_issues(workspace_root, proof) {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof {issue}"
        ));
    }
}

fn check_cpu_100x_aggregate_case_counts(proof: &serde_json::Value, failures: &mut Vec<String>) {
    let (derived_contract_cases, derived_passing_cases) =
        crate::benchmark_evidence_semantics::cpu_sota_100x_case_counts(proof);
    let aggregate_contract_cases = proof
        .get("cpu_sota_100x_contract_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if aggregate_contract_cases != derived_contract_cases {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof cpu_sota_100x_contract_case_count={aggregate_contract_cases}, but cases prove {derived_contract_cases}"
        ));
    }
    let aggregate_passing_cases = proof
        .get("cpu_sota_100x_passing_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if aggregate_passing_cases != derived_passing_cases {
        failures.push(format!(
            "requirement `cpu-only-100x-proof` aggregate proof cpu_sota_100x_passing_case_count={aggregate_passing_cases}, but cases prove {derived_passing_cases}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_100x_gate_rejects_duplicate_source_artifact_count_inflation() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for CPU-SOTA duplicate source artifact test.");
        let proof = serde_json::json!({
            "source_artifact_count": 10,
            "source_artifacts": [
                "release/evidence/benchmarks/workload-01-condition-eval.json",
                "release/evidence/benchmarks/workload-01-condition-eval.json"
            ]
        });
        let mut failures = Vec::new();

        check_cpu_100x_source_artifact_counts(&proof, dir.path(), &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "source_artifact_count=10, but unique source_artifacts=1"
            )),
            "Fix: CPU-SOTA release gate must reject declared source_artifact_count inflation; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "duplicate source_artifacts: release/evidence/benchmarks/workload-01-condition-eval.json"
            )),
            "Fix: CPU-SOTA release gate must reject duplicate aggregate source_artifacts; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_absolute_source_artifact_path() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for CPU-SOTA absolute source artifact test.");
        let external_artifact = dir.path().join("external-source-artifact.json");
        std::fs::write(&external_artifact, "{}")
            .expect("Fix: write external CPU-SOTA source artifact.");
        let proof = serde_json::json!({
            "source_artifact_count": 10,
            "source_artifacts": [
                external_artifact.display().to_string(),
                "release/evidence/benchmarks/workload-02.json",
                "release/evidence/benchmarks/workload-03.json",
                "release/evidence/benchmarks/workload-04.json",
                "release/evidence/benchmarks/workload-05.json",
                "release/evidence/benchmarks/workload-06.json",
                "release/evidence/benchmarks/workload-07.json",
                "release/evidence/benchmarks/workload-08.json",
                "release/evidence/benchmarks/workload-09.json",
                "release/evidence/benchmarks/workload-10.json"
            ]
        });
        let mut failures = Vec::new();

        check_cpu_100x_source_artifact_counts(&proof, dir.path(), &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "aggregate proof source_artifact `"
            ) && failure.contains("must be a relative release path")),
            "Fix: CPU-SOTA release gate must reject existing absolute aggregate source_artifact paths; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_weak_source_artifact_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for CPU-SOTA source provenance gate test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let aggregate_source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut source_artifacts = Vec::new();
        for index in 0..10 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            let source_fingerprint = if index == 4 {
                "git:abc123:dirty=true"
            } else {
                "git:aggregate:dirty=false"
            };
            std::fs::write(
                dir.path().join(&artifact),
                serde_json::to_string_pretty(&serde_json::json!({
                    "selected_backend": "cuda",
                    "source_fingerprint": source_fingerprint,
                    "source_tree_fingerprint": &aggregate_source_tree_fingerprint,
                    "summary": {"total_cases": 0, "passed": 0, "failed": 0},
                    "cases": []
                }))
                .expect("Fix: serialize CPU-SOTA source artifact."),
            )
            .expect("Fix: write CPU-SOTA source artifact.");
            source_artifacts.push(artifact);
        }
        let proof = serde_json::json!({
            "source_fingerprint": "git:aggregate:dirty=false",
            "source_tree_fingerprint": aggregate_source_tree_fingerprint,
            "source_artifact_count": 10,
            "source_artifacts": source_artifacts
        });
        let mut failures = Vec::new();

        check_cpu_100x_source_artifact_counts(&proof, dir.path(), &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "requirement `cpu-only-100x-proof` aggregate proof source_artifact `release/evidence/benchmarks/workload-04.json` source_fingerprint `git:abc123:dirty=true` is dirty but has no worktree digest"
            )),
            "Fix: CPU-SOTA gate must reject weak dirty source artifacts listed by a clean aggregate proof; failures={failures:?}"
        );
        assert!(
            !failures.iter().any(|failure| failure.contains(
                "source_artifact `release/evidence/benchmarks/workload-04.json` source_fingerprint `git:abc123:dirty=true` does not match aggregate source"
            )),
            "Fix: CPU-SOTA gate must rely on source_tree_fingerprint for aggregate source identity instead of raw evidence commit equality; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_inflated_aggregate_case_counts() {
        let proof = serde_json::json!({
            "selected_backend": "cuda",
            "cpu_sota_100x_contract_case_count": 10,
            "cpu_sota_100x_passing_case_count": 10,
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"p50": 10},
                        "baseline_wall_ns": {"p50": 2000}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                }
            ]
        });
        let mut failures = Vec::new();

        check_cpu_100x_aggregate_case_counts(&proof, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "cpu_sota_100x_contract_case_count=10, but cases prove 1"
            )),
            "Fix: CPU-SOTA release gate must reject inflated aggregate contract case counts; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "cpu_sota_100x_passing_case_count=10, but cases prove 1"
            )),
            "Fix: CPU-SOTA release gate must reject inflated aggregate passing case counts; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_claimed_speedup_without_measured_100x() {
        let proof = serde_json::json!({
            "selected_backend": "cuda",
            "cpu_sota_100x_contract_case_count": 1,
            "cpu_sota_100x_passing_case_count": 1,
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "pass",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "metrics": {
                        "wall_ns": {"p50": 100},
                        "baseline_wall_ns": {"p50": 1000}
                    },
                    "performance": {"contract_passed": true, "speedup_x": 200.0}
                }
            ]
        });
        let mut failures = Vec::new();

        check_cpu_100x_aggregate_case_counts(&proof, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "cpu_sota_100x_passing_case_count=1, but cases prove 0"
            )),
            "Fix: CPU-SOTA release gate must reject aggregate passing counts backed only by claimed speedup_x instead of measured baseline_wall_ns / wall_ns; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_requires_required_cases_to_prove_passing_100x() {
        let proof = serde_json::json!({"selected_backend": "cuda"});
        let cases = vec![serde_json::json!({
            "id": "release.condition_eval.1m",
            "backend_id": "cuda",
            "status": "fail",
            "contract": {
                "baselines": [
                    {
                        "class": "CpuSota",
                        "backend_ids": ["cuda"],
                        "min_speedup_x": 100.0
                    }
                ]
            },
            "metrics": {
                "wall_ns": {"p50": 10},
                "baseline_wall_ns": {"p50": 2000}
            },
            "performance": {"contract_passed": true, "speedup_x": 200.0}
        })];
        let mut failures = Vec::new();

        check_required_cpu_100x_cases(&proof, &cases, &mut failures);

        assert!(
            failures.iter().any(|failure| failure.contains(
                "required case `release.condition_eval.1m` does not prove a passing 100x CUDA win"
            )),
            "Fix: CPU-SOTA release gate must reject required case IDs that are present but failed or unproven; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_duplicate_case_array_entries() {
        let matrix = serde_json::json!({
            "required_cpu_sota_100x_families": [
                "release.condition-eval",
                "release.condition-eval"
            ],
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.condition_eval.1m"
            ]
        });
        let proof = serde_json::json!({
            "required_cpu_sota_100x_cases": [
                "release.entropy_window.1m",
                "release.entropy_window.1m"
            ]
        });
        let mut failures = Vec::new();

        check_duplicate_string_array_values(
            "workload matrix",
            &matrix,
            "required_cpu_sota_100x_families",
            &mut failures,
        );
        check_duplicate_string_array_values(
            "workload matrix",
            &matrix,
            "cpu_sota_100x_contract_cases",
            &mut failures,
        );
        check_duplicate_string_array_values(
            "aggregate proof",
            &proof,
            "required_cpu_sota_100x_cases",
            &mut failures,
        );

        assert!(
            failures.iter().any(|failure| failure.contains(
                "workload matrix has duplicate required_cpu_sota_100x_families: release.condition-eval"
            )),
            "Fix: CPU-SOTA gate must reject duplicate matrix required 100x families; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "workload matrix has duplicate cpu_sota_100x_contract_cases: release.condition_eval.1m"
            )),
            "Fix: CPU-SOTA gate must reject duplicate matrix contract case ids; failures={failures:?}"
        );
        assert!(
            failures.iter().any(|failure| failure.contains(
                "aggregate proof has duplicate required_cpu_sota_100x_cases: release.entropy_window.1m"
            )),
            "Fix: CPU-SOTA gate must reject duplicate aggregate required case ids; failures={failures:?}"
        );
    }

    #[test]
    fn cpu_100x_gate_rejects_blank_aggregate_source_provenance() {
        let proof = serde_json::json!({
            "source_fingerprint": "   ",
            "source_tree_fingerprint": "\t"
        });
        let mut failures = Vec::new();

        check_cpu_100x_aggregate_source_provenance(&proof, &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("must preserve source_fingerprint")),
            "Fix: CPU-SOTA release gate must reject blank aggregate source_fingerprint; failures={failures:?}"
        );
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("must preserve source_tree_fingerprint")),
            "Fix: CPU-SOTA release gate must reject blank aggregate source_tree_fingerprint; failures={failures:?}"
        );
    }
}
