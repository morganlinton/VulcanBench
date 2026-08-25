fn inspect_parser_contract_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let component_id = value
        .get("component_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let expected = if evidence.ends_with("vyrec-cli-contracts.json") {
        "vyrec"
    } else {
        evidence
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix("-contracts.json"))
            .unwrap_or("")
    };
    if component_id != expected {
        blockers.push(format!(
            "{evidence}: component_id `{component_id}` does not match expected `{expected}`"
        ));
    }
    if value
        .get("role")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|role| role.is_empty())
    {
        blockers.push(format!("{evidence}: parser contract role is empty"));
    }
    if value
        .get("root")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|root| root.is_empty())
    {
        blockers.push(format!("{evidence}: parser contract root is empty"));
    }
    let required_terms = value
        .get("required_terms")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_terms == 0 {
        blockers.push(format!("{evidence}: parser contract has no required_terms"));
    }
    let missing_terms = value
        .get("missing_terms")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_terms != 0 {
        blockers.push(format!(
            "{evidence}: parser contract reports {missing_terms} missing term(s)"
        ));
    }
    let required_contract_topics = value
        .get("required_contract_topics")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_contract_topics == 0 {
        blockers.push(format!(
            "{evidence}: parser contract has no required_contract_topics"
        ));
    }
    let missing_contract_topics = value
        .get("missing_contract_topics")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_contract_topics != 0 {
        blockers.push(format!(
            "{evidence}: parser contract reports {missing_contract_topics} missing contract topic(s)"
        ));
    }
    let required_test_categories = value
        .get("required_test_categories")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_test_categories == 0 {
        blockers.push(format!(
            "{evidence}: parser contract has no required_test_categories"
        ));
    }
    let missing_test_categories = value
        .get("missing_test_categories")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_test_categories != 0 {
        blockers.push(format!(
            "{evidence}: parser contract reports {missing_test_categories} missing test categor(ies)"
        ));
    }
    let required_evidence_trees = value
        .get("required_evidence_trees")
        .and_then(serde_json::Value::as_array);
    if required_evidence_trees.is_none_or(|trees| trees.len() < 3) {
        blockers.push(format!(
            "{evidence}: parser contract must list tests, benches, and fuzz evidence trees"
        ));
    }
    inspect_duplicate_parser_contract_object_rows(
        evidence,
        value,
        "required_evidence_trees",
        "tree",
        blockers,
    );
    if let Some(trees) = required_evidence_trees {
        for tree in trees {
            let tree_name = tree
                .get("tree")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if tree.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
                blockers.push(format!(
                    "{evidence}: parser contract evidence tree `{tree_name}` does not exist"
                ));
            }
            let source_bytes = tree
                .get("source_bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if source_bytes == 0 {
                blockers.push(format!(
                    "{evidence}: parser contract evidence tree `{tree_name}` has zero source bytes"
                ));
            }
            let unreadable = tree
                .get("unreadable_file_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX);
            if unreadable != 0 {
                blockers.push(format!(
                    "{evidence}: parser contract evidence tree `{tree_name}` has {unreadable} unreadable source file(s)"
                ));
            }
        }
    }
    let unresolved_ownership_markers = value
        .get("unresolved_ownership_markers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if unresolved_ownership_markers != 0 {
        blockers.push(format!(
            "{evidence}: parser contract reports {unresolved_ownership_markers} unresolved ownership marker(s)"
        ));
    }
    let Some(files) = value
        .get("required_files")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!(
            "{evidence}: parser contract missing required_files"
        ));
        return;
    };
    if files.is_empty() {
        blockers.push(format!(
            "{evidence}: parser contract required_files is empty"
        ));
    }
    inspect_duplicate_parser_contract_object_rows(
        evidence,
        value,
        "required_files",
        "path",
        blockers,
    );
    for file in files {
        let path = file
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if file.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
            blockers.push(format!("{evidence}: required file `{path}` does not exist"));
        }
        if file
            .get("source_bytes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!("{evidence}: required file `{path}` is empty"));
        }
        let read_error = file.get("read_error");
        if !read_error.is_some_and(serde_json::Value::is_null) {
            blockers.push(format!(
                "{evidence}: required file `{path}` read_error={}",
                read_error
                    .map(serde_json::Value::to_string)
                    .unwrap_or_else(|| "<missing>".to_string())
            ));
        }
    }
}

fn inspect_duplicate_parser_contract_object_rows(
    evidence: &str,
    value: &serde_json::Value,
    array_field: &str,
    object_field: &str,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            value,
            array_field,
            object_field,
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!(
            "{evidence}: duplicate parser contract {array_field}.{object_field} rows: {duplicates}"
        ));
    }
}

fn inspect_cpu_100x_benchmark_semantics(
    evidence: &str,
    path: &std::path::Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let selected_backend = value
        .get("selected_backend")
        .and_then(serde_json::Value::as_str);
    if selected_backend != Some("cuda") {
        blockers.push(format!("{evidence}: selected_backend must be cuda"));
    }
    inspect_cpu_100x_aggregate_source_provenance(evidence, value, blockers);
    inspect_cpu_100x_source_artifact_counts(evidence, path, value, blockers);
    inspect_duplicate_array_values(evidence, value, "required_cpu_sota_100x_cases", blockers);
    let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing cases array"));
        return;
    };
    inspect_cpu_100x_aggregate_case_counts(evidence, value, blockers);
    inspect_required_cpu_100x_cases(evidence, value, cases, blockers);
    for case in cases {
        inspect_cpu_100x_case(evidence, value, case, blockers);
    }
}

fn inspect_required_cpu_100x_cases(
    evidence: &str,
    value: &serde_json::Value,
    cases: &[serde_json::Value],
    blockers: &mut Vec<String>,
) {
    let selected_backend = value
        .get("selected_backend")
        .and_then(serde_json::Value::as_str);
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
                .or(selected_backend);
            case.get("id").and_then(serde_json::Value::as_str) == Some(required_case)
                && crate::benchmark_evidence_semantics::benchmark_case_proves_cpu_sota_100x(
                    case,
                    case_backend,
                )
        }) {
            blockers.push(format!(
                "{evidence}: required CPU-SOTA 100x proof case `{required_case}` does not prove a passing 100x CUDA win"
            ));
        }
    }
}

fn inspect_cpu_100x_case(
    evidence: &str,
    value: &serde_json::Value,
    case: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let selected_backend = value
        .get("selected_backend")
        .and_then(serde_json::Value::as_str);
    let id = case
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<unknown>");
    if case.get("backend_id").and_then(serde_json::Value::as_str) != Some("cuda") {
        blockers.push(format!("{evidence}: case `{id}` backend_id must be cuda"));
    }
    let case_backend = case
        .get("backend_id")
        .and_then(serde_json::Value::as_str)
        .or(selected_backend);
    if !crate::benchmark_evidence_semantics::benchmark_case_has_cpu_sota_contract(
        case,
        case_backend,
        100.0,
    ) {
        blockers.push(format!(
            "{evidence}: case `{id}` must carry an applicable CPU-SOTA performance contract with min_speedup_x >= 100.00"
        ));
    }
    if !case
        .get("performance")
        .and_then(|performance| performance.get("contract_passed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        let reason = crate::benchmark_evidence_semantics::benchmark_case_failure_reason(case)
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        blockers.push(format!(
            "{evidence}: case `{id}` must pass its performance contract{reason}"
        ));
    }
    let metrics = case.get("metrics").and_then(serde_json::Value::as_object);
    let wall = metrics.and_then(active_gpu_metric_p50);
    let baseline = metrics.and_then(|metrics| metric_p50(metrics.get("baseline_wall_ns")));
    let wall_samples = metrics
        .and_then(|metrics| metric_samples(metrics.get("wall_ns")))
        .unwrap_or(0);
    if wall_samples < 30 {
        blockers.push(format!(
            "{evidence}: case `{id}` has {wall_samples} wall_ns sample(s), needs at least 30"
        ));
    }
    let baseline_wall_samples = metrics
        .and_then(|metrics| metric_samples(metrics.get("baseline_wall_ns")))
        .unwrap_or(0);
    if baseline_wall_samples < 30 {
        blockers.push(format!(
            "{evidence}: case `{id}` has {baseline_wall_samples} baseline_wall_ns sample(s), needs at least 30"
        ));
    }
    require_benchmark_metric_percentiles(evidence, id, metrics, "wall_ns", blockers);
    require_benchmark_metric_percentiles(evidence, id, metrics, "baseline_wall_ns", blockers);
    match (wall, baseline) {
        (Some(wall), Some(baseline)) if wall > 0.0 && baseline / wall >= 100.0 => {}
        (Some(wall), Some(baseline)) if wall > 0.0 => blockers.push(format!(
            "{evidence}: case `{id}` end-to-end p50 speedup is {:.2}x, needs 100.00x",
            baseline / wall
        )),
        _ => blockers.push(format!(
            "{evidence}: case `{id}` must include p50 wall_ns and baseline_wall_ns"
        )),
    }
}

fn inspect_cpu_100x_aggregate_source_provenance(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("source_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|source| source.trim().is_empty())
    {
        blockers.push(format!(
            "{evidence}: aggregate proof must preserve source_fingerprint"
        ));
    }
    if value
        .get("source_tree_fingerprint")
        .and_then(serde_json::Value::as_str)
        .is_none_or(|source| source.trim().is_empty())
    {
        blockers.push(format!(
            "{evidence}: aggregate proof must preserve source_tree_fingerprint"
        ));
    }
}

fn inspect_cpu_100x_source_artifact_counts(
    evidence: &str,
    path: &std::path::Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let unique_source_artifacts =
        crate::benchmark_evidence_semantics::benchmark_source_artifact_count(value) as u64;
    let raw_source_artifacts =
        crate::benchmark_evidence_semantics::benchmark_source_artifact_entry_count(value) as u64;
    let declared_source_artifacts = value
        .get("source_artifact_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if declared_source_artifacts != unique_source_artifacts {
        blockers.push(format!(
            "{evidence}: source_artifact_count={declared_source_artifacts}, but unique source_artifacts={unique_source_artifacts}"
        ));
    }
    if unique_source_artifacts < 10 {
        blockers.push(format!(
            "{evidence}: has {unique_source_artifacts} unique source artifact(s); needs at least 10"
        ));
    }
    if raw_source_artifacts != unique_source_artifacts {
        let duplicates =
            crate::benchmark_evidence_semantics::benchmark_duplicate_source_artifact_paths(value)
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
        blockers.push(format!(
            "{evidence}: duplicate source_artifacts: {duplicates}"
        ));
    }
    let source_artifacts =
        crate::benchmark_evidence_semantics::benchmark_source_artifact_paths(value);
    if source_artifacts.is_empty() {
        return;
    }
    let Some(workspace_root) = cpu_100x_workspace_root(path) else {
        blockers.push(format!(
            "{evidence}: could not resolve workspace root for source_artifacts from {}",
            path.display()
        ));
        return;
    };
    for issue in crate::benchmark_evidence_semantics::cpu_sota_100x_source_artifact_issues(
        workspace_root,
        value,
    ) {
        blockers.push(format!("{evidence}: {issue}"));
    }
}

fn cpu_100x_workspace_root(path: &std::path::Path) -> Option<&std::path::Path> {
    path.ancestors().find(|candidate| {
        candidate.join("Cargo.toml").is_file() && candidate.join("release").is_dir()
    })
}

fn inspect_cpu_100x_aggregate_case_counts(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let (derived_contract_cases, derived_passing_cases) =
        crate::benchmark_evidence_semantics::cpu_sota_100x_case_counts(value);
    let declared_contract_cases = value
        .get("cpu_sota_100x_contract_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if declared_contract_cases != derived_contract_cases {
        blockers.push(format!(
            "{evidence}: cpu_sota_100x_contract_case_count={declared_contract_cases}, but cases prove {derived_contract_cases}"
        ));
    }
    let declared_passing_cases = value
        .get("cpu_sota_100x_passing_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if declared_passing_cases != derived_passing_cases {
        blockers.push(format!(
            "{evidence}: cpu_sota_100x_passing_case_count={declared_passing_cases}, but cases prove {derived_passing_cases}"
        ));
    }
}

#[cfg(test)]
mod part13_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_duplicate_parser_contract_object_rows() {
        let report = serde_json::json!({
            "component_id": "vyrec",
            "role": "cli parser contract",
            "root": "crates/vyrec",
            "required_terms": ["parse"],
            "missing_terms": [],
            "required_contract_topics": ["ownership"],
            "missing_contract_topics": [],
            "required_test_categories": ["unit"],
            "missing_test_categories": [],
            "required_evidence_trees": [
                {"tree": "tests", "exists": true, "source_bytes": 128, "unreadable_file_count": 0},
                {"tree": "tests", "exists": true, "source_bytes": 128, "unreadable_file_count": 0},
                {"tree": "benches", "exists": true, "source_bytes": 128, "unreadable_file_count": 0}
            ],
            "unresolved_ownership_markers": [],
            "required_files": [
                {"path": "crates/vyrec/src/lib.rs", "exists": true, "source_bytes": 128, "read_error": null},
                {"path": "crates/vyrec/src/lib.rs", "exists": true, "source_bytes": 128, "read_error": null}
            ]
        });
        let mut blockers = Vec::new();

        inspect_parser_contract_semantics("vyrec-cli-contracts.json", &report, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate parser contract required_evidence_trees.tree rows: tests"
            )),
            "Fix: completion audit must reject duplicate parser contract evidence tree rows; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate parser contract required_files.path rows: crates/vyrec/src/lib.rs"
            )),
            "Fix: completion audit must reject duplicate parser contract required file rows; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_wrong_backend_cpu_sota_contract() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["wgpu"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": {"contract_passed": true},
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 2000, "p95": 2100, "p99": 2200}
                    }
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_cpu_100x_benchmark_semantics(
            "cpu-100x.json",
            std::path::Path::new("cpu-100x.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "must carry an applicable CPU-SOTA performance contract"
            )),
            "Fix: completion audit must expose CPU-SOTA contracts scoped to the wrong backend; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_failure_preserves_case_reason() {
        let report = serde_json::json!({
            "selected_backend": "cuda",
            "cases": [
                {
                    "id": "release.condition_eval.1m",
                    "backend_id": "cuda",
                    "status": "failed",
                    "correctness": {
                        "Invalid": {
                            "reason": "Performance contract failed: release condition eval requires 100.00x over CPU-SOTA, observed 42.00x"
                        }
                    },
                    "contract": {
                        "baselines": [
                            {
                                "class": "CpuSota",
                                "backend_ids": ["cuda"],
                                "min_speedup_x": 100.0
                            }
                        ]
                    },
                    "performance": null,
                    "metrics": {
                        "wall_ns": {"samples": 30, "p50": 10, "p95": 11, "p99": 12},
                        "baseline_wall_ns": {"samples": 30, "p50": 1000, "p95": 1001, "p99": 1002}
                    }
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_cpu_100x_benchmark_semantics(
            "cpu-100x.json",
            std::path::Path::new("cpu-100x.json"),
            &report,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "case `release.condition_eval.1m` must pass its performance contract: Performance contract failed"
            ) && blocker.contains("observed 42.00x")),
            "Fix: completion audit CPU-SOTA proof blockers must preserve failed benchmark case reasons; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_duplicate_source_artifact_count_inflation() {
        let proof = serde_json::json!({
            "source_artifact_count": 10,
            "source_artifacts": [
                "release/evidence/benchmarks/workload-01-condition-eval.json",
                "release/evidence/benchmarks/workload-01-condition-eval.json"
            ]
        });
        let mut blockers = Vec::new();

        inspect_cpu_100x_source_artifact_counts(
            "cpu-only-100x-proof.json",
            std::path::Path::new("cpu-only-100x-proof.json"),
            &proof,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "source_artifact_count=10, but unique source_artifacts=1"
            )),
            "Fix: completion audit must reject inflated CPU-SOTA source_artifact_count; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate source_artifacts: release/evidence/benchmarks/workload-01-condition-eval.json"
            )),
            "Fix: completion audit must reject duplicate CPU-SOTA source_artifacts; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_absolute_source_artifact_path() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for CPU-SOTA absolute source artifact audit.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let evidence_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Fix: create temporary CPU-SOTA evidence directory.");
        let proof_path = evidence_dir.join("cpu-only-100x-proof.json");
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
        let mut blockers = Vec::new();

        inspect_cpu_100x_source_artifact_counts(
            "release/evidence/benchmarks/cpu-only-100x-proof.json",
            &proof_path,
            &proof,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/cpu-only-100x-proof.json: source_artifact `"
            ) && blocker.contains("must be a relative release path")),
            "Fix: completion audit must reject existing absolute CPU-SOTA source_artifact paths; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_weak_source_artifact_provenance() {
        let dir = tempfile::TempDir::new()
            .expect("Fix: create temporary workspace for CPU-SOTA source provenance audit test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let evidence_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Fix: create temporary benchmark evidence directory.");
        let aggregate_source_tree_fingerprint =
            vyre_bench::probes::source_tree_fingerprint_at(dir.path());
        let mut source_artifacts = Vec::new();
        for index in 0..10 {
            let artifact = format!("release/evidence/benchmarks/workload-{index:02}.json");
            let source_fingerprint = if index == 7 {
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
        let proof_path = evidence_dir.join("cpu-only-100x-proof.json");
        let proof = serde_json::json!({
            "source_fingerprint": "git:aggregate:dirty=false",
            "source_tree_fingerprint": aggregate_source_tree_fingerprint,
            "source_artifact_count": 10,
            "source_artifacts": source_artifacts
        });
        let mut blockers = Vec::new();

        inspect_cpu_100x_source_artifact_counts(
            "release/evidence/benchmarks/cpu-only-100x-proof.json",
            &proof_path,
            &proof,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/cpu-only-100x-proof.json: source_artifact `release/evidence/benchmarks/workload-07.json` source_fingerprint `git:abc123:dirty=true` is dirty but has no worktree digest"
            )),
            "Fix: completion audit must reject weak dirty source artifacts listed by a clean CPU-SOTA aggregate proof; blockers={blockers:?}"
        );
        assert!(
            !blockers.iter().any(|blocker| blocker.contains(
                "source_artifact `release/evidence/benchmarks/workload-07.json` source_fingerprint `git:abc123:dirty=true` does not match aggregate source"
            )),
            "Fix: completion audit must rely on source_tree_fingerprint for CPU-SOTA aggregate source identity instead of raw evidence commit equality; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_inflated_aggregate_case_counts() {
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
        let mut blockers = Vec::new();

        inspect_cpu_100x_aggregate_case_counts("cpu-only-100x-proof.json", &proof, &mut blockers);

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "cpu_sota_100x_contract_case_count=10, but cases prove 1"
            )),
            "Fix: completion audit must reject inflated aggregate CPU-SOTA contract case counts; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "cpu_sota_100x_passing_case_count=10, but cases prove 1"
            )),
            "Fix: completion audit must reject inflated aggregate CPU-SOTA passing case counts; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_requires_required_cases_to_prove_passing_100x() {
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
        let mut blockers = Vec::new();

        inspect_required_cpu_100x_cases(
            "cpu-only-100x-proof.json",
            &proof,
            &cases,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "required CPU-SOTA 100x proof case `release.condition_eval.1m` does not prove a passing 100x CUDA win"
            )),
            "Fix: completion audit must reject required CPU-SOTA proof case IDs that are present but failed or unproven; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_blank_aggregate_source_provenance() {
        let proof = serde_json::json!({
            "source_fingerprint": "   ",
            "source_tree_fingerprint": "\t"
        });
        let mut blockers = Vec::new();

        inspect_cpu_100x_aggregate_source_provenance(
            "cpu-only-100x-proof.json",
            &proof,
            &mut blockers,
        );

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("must preserve source_fingerprint")),
            "Fix: completion audit must reject blank aggregate source_fingerprint; blockers={blockers:?}"
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("must preserve source_tree_fingerprint")),
            "Fix: completion audit must reject blank aggregate source_tree_fingerprint; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_cpu_100x_rejects_duplicate_required_case_ids() {
        let proof = serde_json::json!({
            "required_cpu_sota_100x_cases": [
                "release.entropy_window.1m",
                "release.entropy_window.1m"
            ]
        });
        let mut blockers = Vec::new();

        inspect_duplicate_array_values(
            "cpu-only-100x-proof.json",
            &proof,
            "required_cpu_sota_100x_cases",
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate required_cpu_sota_100x_cases: release.entropy_window.1m"
            )),
            "Fix: completion audit must reject duplicate aggregate CPU-SOTA required case ids; blockers={blockers:?}"
        );
    }
}

fn metric_p50(value: Option<&serde_json::Value>) -> Option<f64> {
    metric_percentile(value, "p50")
}

fn active_gpu_metric_p50(metrics: &serde_json::Map<String, serde_json::Value>) -> Option<f64> {
    metric_p50(metrics.get("dispatch_ns"))
        .or_else(|| metric_p50(metrics.get("kernel_execute_ns")))
        .or_else(|| metric_p50(metrics.get("wall_ns")))
}

fn metric_percentile(value: Option<&serde_json::Value>, percentile: &str) -> Option<f64> {
    value
        .and_then(|value| value.get(percentile))
        .and_then(serde_json::Value::as_f64)
        .or_else(|| {
            value
                .and_then(|value| value.get(percentile))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as f64)
        })
}

fn metric_samples(value: Option<&serde_json::Value>) -> Option<u64> {
    value?.get("samples").and_then(serde_json::Value::as_u64)
}

fn require_benchmark_metric_percentiles(
    evidence: &str,
    case_id: &str,
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
    metric_name: &str,
    blockers: &mut Vec<String>,
) {
    for percentile in ["p50", "p95", "p99"] {
        let value =
            metrics.and_then(|metrics| metric_percentile(metrics.get(metric_name), percentile));
        if !value.is_some_and(|value| value > 0.0) {
            blockers.push(format!(
                "{evidence}: case `{case_id}` must include positive {percentile} {metric_name}"
            ));
        }
    }
}

fn inspect_version_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("requested_vyre_release")
        .and_then(serde_json::Value::as_str)
        != Some("0.6.3")
    {
        blockers.push(format!(
            "{evidence}: requested_vyre_release must be `0.6.3`"
        ));
    }
    if value
        .get("requested_weir_release")
        .and_then(serde_json::Value::as_str)
        != Some("0.1.0")
    {
        blockers.push(format!(
            "{evidence}: requested_weir_release must be `0.1.0`"
        ));
    }
    if value
        .get("release_doc_tag_findings")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|findings| !findings.is_empty())
    {
        blockers.push(format!(
            "{evidence}: release_doc_tag_findings must exist and be empty"
        ));
    }
    if value
        .get("release_note_token_findings")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|findings| !findings.is_empty())
    {
        blockers.push(format!(
            "{evidence}: release_note_token_findings must exist and be empty"
        ));
    }
    if value
        .get("missing_required_release_packages")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|packages| !packages.is_empty())
    {
        blockers.push(format!(
            "{evidence}: missing_required_release_packages must exist and be empty"
        ));
    }
    let required_release_packages = value
        .get("required_release_packages")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required_package in [
        "vyre@0.6.3",
        "vyre-driver-cuda@0.6.3",
        "vyre-driver-wgpu@0.6.3",
        "weir@0.1.0",
        "vyrec@0.1.0",
        "vyre-frontend-c@0.6.3",
    ] {
        if !required_release_packages
            .iter()
            .any(|package| package.as_str() == Some(required_package))
        {
            blockers.push(format!(
                "{evidence}: required_release_packages must include `{required_package}`"
            ));
        }
    }
    let Some(tag_story) = value
        .get("tag_story")
        .and_then(serde_json::Value::as_object)
    else {
        blockers.push(format!("{evidence}: missing tag_story"));
        return;
    };
    for (field, expected) in [
        ("vyre_rc_tag", "vyre-v0.6.3-rc.1"),
        ("weir_rc_tag", "weir-v0.1.0-rc.1"),
        (
            "combined_release_train_rc_tag",
            "vyre-0.6.3-weir-0.1.0-rc.1",
        ),
        ("vyre_tag", "vyre-v0.6.3"),
        ("weir_tag", "weir-v0.1.0"),
        ("combined_release_train_tag", "vyre-0.6.3-weir-0.1.0"),
    ] {
        if tag_story.get(field).and_then(serde_json::Value::as_str) != Some(expected) {
            blockers.push(format!(
                "{evidence}: tag_story.{field} must be `{expected}`"
            ));
        }
    }
    for required in [
        "vyre 0.6.3",
        "weir 0.1.0",
        "vyre-driver-cuda@0.6.3",
        "vyre-driver-wgpu@0.6.3",
        "vyre-v0.6.3-rc.1",
        "weir-v0.1.0-rc.1",
        "vyre-0.6.3-weir-0.1.0-rc.1",
        "vyre-v0.6.3",
        "weir-v0.1.0",
        "vyre-0.6.3-weir-0.1.0",
    ] {
        let present = tag_story
            .get("required_in_release_notes")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|entries| entries.iter().any(|entry| entry.as_str() == Some(required)));
        if !present {
            blockers.push(format!(
                "{evidence}: tag_story.required_in_release_notes is missing `{required}`"
            ));
        }
    }
}

fn inspect_markdown_evidence(evidence: &str, path: &Path, blockers: &mut Vec<String>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "{evidence}: failed to read markdown evidence: {error}"
            ));
            return;
        }
    };
    if text.trim().is_empty() {
        blockers.push(format!("{evidence}: markdown evidence is empty"));
    }
    for marker in [
        "status: blocked",
        "status: open",
        "status: pending",
        "todo",
        "fixme",
        "placeholder",
        "stub",
        "tbd",
        "to be filled",
    ] {
        for line in text.lines() {
            let lowered = line.to_ascii_lowercase();
            if markdown_line_is_release_rule_text(&lowered) {
                continue;
            }
            if lowered.contains(marker) {
                blockers.push(format!(
                    "{evidence}: markdown evidence contains unresolved marker `{marker}`"
                ));
                break;
            }
        }
    }
    if evidence.starts_with("evidence/docs/") && !text.contains("Evidence sources:") {
        blockers.push(format!(
            "{evidence}: generated docs evidence does not list evidence sources"
        ));
    }
}
