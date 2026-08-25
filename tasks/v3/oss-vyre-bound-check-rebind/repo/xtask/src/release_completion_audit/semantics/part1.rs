pub(crate) fn inspect_evidence_semantics(evidence: &str, path: &Path, blockers: &mut Vec<String>) {
    if evidence.ends_with(".json") {
        inspect_json_evidence(evidence, path, blockers);
    } else if evidence.ends_with(".md") {
        inspect_markdown_evidence(evidence, path, blockers);
    } else if evidence.ends_with("BENCH_TARGETS.toml") {
        inspect_bench_targets_toml(evidence, path, blockers);
    }
}

fn inspect_bench_targets_toml(evidence: &str, path: &Path, blockers: &mut Vec<String>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "{evidence}: failed to read benchmark target table: {error}"
            ));
            return;
        }
    };
    let target_count = text.matches("[[target]]").count();
    if target_count < 17 {
        blockers.push(format!(
            "{evidence}: benchmark target table contains {target_count} target(s); needs at least 17 including release workloads and optimization-proof targets"
        ));
    }
    for required in [
        "release.workload.condition_eval",
        "release.workload.string_bitmap_scatter",
        "release.workload.offset_count_aggregation",
        "release.workload.pe_metadata",
        "release.workload.entropy_window",
        "release.workload.for_any_all_n",
        "release.workload.alias_reaching_def",
        "release.workload.ifds_witness",
        "release.workload.callgraph_reachability",
        "release.workload.c_ast_traversal",
        "release.workload.megakernel_stream",
        "release.workload.egraph_saturation",
        "release.workload.conformance_sparse_readback",
        "release.optimization.lower_rewrite_impact",
        "release.optimization.foundation_optimizer_impact",
    ] {
        if !text.contains(required) {
            blockers.push(format!(
                "{evidence}: missing release benchmark target `{required}`"
            ));
        }
    }
    if !text.contains("\"cpu_sota\"") || !text.contains("min_speedup_over_cpu_sota") {
        blockers.push(format!(
            "{evidence}: benchmark target table must declare CPU-SOTA classes and speedup thresholds"
        ));
    }
}

fn inspect_json_evidence(evidence: &str, path: &Path, blockers: &mut Vec<String>) {
    let text = match read_text_bounded(path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!("{evidence}: failed to read JSON evidence: {error}"));
            return;
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(format!("{evidence}: invalid JSON evidence: {error}"));
            return;
        }
    };
    inspect_current_source_fingerprint_freshness(evidence, path, &value, blockers);
    inspect_generated_blockers_array(evidence, &value, blockers);
    let failed = value
        .get("summary")
        .and_then(|summary| summary.get("failed"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let failed_cases = crate::benchmark_evidence_semantics::benchmark_failed_case_summaries(&value);
    let case_failed = failed_cases.len() as u64;
    if value.get("summary").is_some() || value.get("cases").is_some() {
        if let Some(mismatch) =
            crate::benchmark_evidence_semantics::benchmark_report_summary_case_evidence_mismatch(
                &value,
            )
        {
            blockers.push(format!(
                "{evidence}: benchmark summary is invalid: {mismatch}"
            ));
        }
        check_case_backend_matches_selected_backend(evidence, &value, blockers);
        inspect_contract_baselines_apply_to_backend(evidence, &value, blockers);
        check_cuda_telemetry_labels_match_counters(evidence, &value, blockers);
        if value.get("cases").is_some()
            || value
                .get("selected_backend")
                .and_then(serde_json::Value::as_str)
                .is_some()
        {
            inspect_benchmark_report_provenance(evidence, path, &value, blockers);
        }
    }
    if failed != 0 || case_failed != 0 {
        let detail = if failed_cases.is_empty() {
            String::new()
        } else {
            format!(": {}", failed_cases.join("; "))
        };
        let count_detail = if failed == case_failed {
            String::new()
        } else {
            format!("; case evidence reports {case_failed} failed case(s)")
        };
        blockers.push(format!(
            "{evidence}: benchmark summary reports {failed} failed case(s){count_detail}{detail}"
        ));
    }
    if let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) {
        if cases.is_empty() {
            blockers.push(format!("{evidence}: cases array is empty"));
        }
    }
    if !evidence.ends_with("cuda-release-suite.json")
        && !evidence.ends_with("wgpu-fallback-suite.json")
        && !evidence.contains("/conformance/")
        && (evidence.contains("cuda")
            || value
                .get("selected_backend")
                .and_then(serde_json::Value::as_str)
                == Some("cuda"))
    {
        inspect_benchmark_cuda_environment_semantics(evidence, &value, blockers);
    }
    if let Some(families) = value.get("families").and_then(serde_json::Value::as_array) {
        if families.is_empty() {
            blockers.push(format!("{evidence}: workload families array is empty"));
        }
    }
    if let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array) {
        if packages.is_empty() {
            blockers.push(format!("{evidence}: packages array is empty"));
        }
    }
    if let Some(entries) = value.get("entries").and_then(serde_json::Value::as_array) {
        if entries.is_empty() {
            blockers.push(format!("{evidence}: entries array is empty"));
        }
    }
    if value
        .get("op_count")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|count| count == 0)
    {
        blockers.push(format!("{evidence}: op_count is zero"));
    }
    if value
        .get("total_files")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|count| count == 0)
    {
        blockers.push(format!("{evidence}: total_files is zero"));
    }
    if value
        .get("scanned_files")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|count| count == 0)
    {
        blockers.push(format!("{evidence}: scanned_files is zero"));
    }
    if evidence.ends_with("hygiene-matrix.json") {
        inspect_hygiene_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("version-matrix.json") {
        inspect_version_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("backend-matrix.json") {
        inspect_backend_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("feature-matrix.json") {
        inspect_feature_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("metadata-matrix.json") {
        inspect_metadata_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("publish-readiness.json") {
        inspect_package_readiness_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("public-launch-state.json") {
        inspect_public_launch_state_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("docs-matrix.json") {
        inspect_docs_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("bench-release-axes.json") {
        inspect_release_axes_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("cuda-release-suite.json")
        || evidence.ends_with("wgpu-fallback-suite.json")
    {
        inspect_backend_suite_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("cuda-ptx-patterns.json") {
        inspect_cuda_ptx_pattern_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("megakernel-condition-cuda.json") {
        inspect_megakernel_condition_cuda_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("megakernel-latency-cuda.json") {
        inspect_megakernel_latency_cuda_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("release-tag-plan.json") {
        inspect_release_tag_plan_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("release-workload-matrix.json") {
        inspect_release_workload_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.contains("release/evidence/benchmarks/workload-")
        && evidence.ends_with(".json")
        && !evidence.ends_with("release-workload-matrix.json")
    {
        inspect_workload_benchmark_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("cpu-only-100x-proof.json") {
        inspect_cpu_100x_benchmark_semantics(evidence, path, &value, blockers);
    }
    if is_before_after_benchmark_evidence(evidence) {
        inspect_before_after_benchmark_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("optimization-corpus.json")
        || evidence.ends_with("optimization-corpus-contracts.json")
    {
        inspect_optimization_corpus_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("optimization-family-manifest.json") {
        inspect_optimization_family_manifest_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("optimization-analysis-fixtures.json") {
        inspect_optimization_analysis_fixture_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("optimization-case-manifest.json") {
        inspect_optimization_case_manifest_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("pass-family-benchmark-manifest.json") {
        inspect_pass_family_benchmark_manifest_semantics(evidence, path, &value, blockers);
    }
    if is_marker_evidence(evidence) {
        inspect_marker_evidence_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("weir-analysis-api-matrix.json") {
        inspect_weir_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("weir-vyre-integration-tests.json") {
        inspect_schema_version_at_least(evidence, &value, 2, blockers);
    }
    if evidence.ends_with("weir-readme-contracts.json") {
        inspect_weir_readme_contract_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("vyre-readme-contracts.json") {
        inspect_weir_readme_contract_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("cuda-conformance.json")
        || evidence.ends_with("wgpu-conformance.json")
        || evidence.ends_with("reference-conformance.json")
    {
        inspect_backend_conformance_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("conformance-matrix.json") {
        inspect_conformance_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("release-gate-log.json") {
        inspect_release_conformance_log_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("release-evidence-run.json") {
        inspect_release_evidence_run_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("c-parser-linux-subsystem.json") {
        inspect_c_parser_corpus_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("linux-subsystem-corpus-manifest.json") {
        inspect_c_parser_manifest_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("c-parser-diagnostics-summary.json") {
        inspect_c_parser_diagnostics_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("c-parser-throughput.json") {
        inspect_c_parser_throughput_semantics(evidence, path, &value, blockers);
    }
    if evidence.ends_with("distributed-parser-map.json") {
        inspect_distributed_parser_map_semantics(evidence, &value, blockers);
    }
    if is_parser_contract_evidence(evidence) {
        inspect_parser_contract_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("oversized-test-closure.json") {
        inspect_oversized_test_closure_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("test-matrix.json") {
        inspect_test_matrix_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("modularization-map.json") {
        inspect_modularization_map_semantics(evidence, &value, blockers);
    }
    if evidence.ends_with("release-surface-suite-coverage.json") {
        inspect_surface_coverage_semantics(evidence, &value, blockers);
    }
    if is_test_suite_evidence(evidence) {
        inspect_suite_evidence_semantics(evidence, &value, blockers);
    }
    let open = value
        .get("blocked_or_open_requirements")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if open != 0 {
        blockers.push(format!(
            "{evidence}: completion audit reports {open} blocked/open requirement(s)"
        ));
    }
}

fn inspect_generated_blockers_array(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let requires_blockers = evidence_requires_blockers_array(evidence);
    match value.get("blockers") {
        Some(blockers_value) => {
            let Some(blocker_items) = blockers_value.as_array() else {
                blockers.push(format!("{evidence}: blockers must be an array"));
                return;
            };
            if !blocker_items.is_empty() {
                blockers.push(format!(
                    "{evidence}: reports {} blocker(s)",
                    blocker_items.len()
                ));
            }
        }
        None if requires_blockers => blockers.push(format!("{evidence}: missing blockers array")),
        None => {}
    }
}

fn evidence_requires_blockers_array(evidence: &str) -> bool {
    evidence.ends_with("cuda-release-suite.json")
        || evidence.ends_with("wgpu-fallback-suite.json")
        || evidence.ends_with("bench-release-axes.json")
        || evidence.ends_with("cpu-only-100x-proof.json")
        || evidence.ends_with("pass-family-benchmark-manifest.json")
        || evidence.ends_with("release-workload-matrix.json")
}

fn inspect_current_source_fingerprint_freshness(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some((field, source_fingerprint)) =
        crate::benchmark_evidence_semantics::report_freshness_fingerprint(value)
    else {
        return;
    };
    let Some(current_source_fingerprint) =
        crate::benchmark_evidence_semantics::current_freshness_fingerprint_for_report(path, value)
    else {
        return;
    };
    for issue in crate::benchmark_evidence_semantics::source_fingerprint_freshness_issues(
        source_fingerprint,
        &current_source_fingerprint,
    ) {
        match issue {
            crate::benchmark_evidence_semantics::SourceFingerprintFreshnessIssue::Mismatch {
                source_fingerprint,
                current_source_fingerprint,
            } => blockers.push(format!(
                "{evidence}: {field} `{source_fingerprint}` does not match current workspace source `{current_source_fingerprint}`"
            )),
        }
    }
}

fn is_marker_evidence(evidence: &str) -> bool {
    evidence.ends_with("alias-aware-dse.json")
        || evidence.ends_with("alias-aware-stlf.json")
        || evidence.ends_with("alias-aware-licm.json")
        || evidence.ends_with("alias-aware-fusion-fission.json")
        || evidence.ends_with("weir-facts-pass-firing.json")
        || evidence.ends_with("egraph-saturation-matrix.json")
        || evidence.ends_with("egraph-semantic-contracts.json")
}

fn inspect_marker_evidence_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let markers = value
        .get("markers")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if markers == 0 {
        blockers.push(format!("{evidence}: marker evidence contains zero markers"));
    }
    for required in required_marker_ids_for_evidence(evidence) {
        if !value
            .get("markers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|markers| {
                markers.iter().any(|marker| {
                    marker.get("id").and_then(serde_json::Value::as_str) == Some(required)
                })
            })
        {
            blockers.push(format!(
                "{evidence}: missing required optimization marker `{required}`"
            ));
        }
    }
    if !value
        .get("source_matrix")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|source| source.ends_with("optimization-integration-matrix.json"))
    {
        blockers.push(format!(
            "{evidence}: source_matrix must reference optimization-integration-matrix.json"
        ));
    }
}

fn required_marker_ids_for_evidence(evidence: &str) -> &'static [&'static str] {
    if evidence.ends_with("alias-aware-dse.json") {
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
        ]
    } else if evidence.ends_with("alias-aware-stlf.json") {
        &[
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
            "dataflow-analysis-stlf-firing-test",
        ]
    } else if evidence.ends_with("alias-aware-licm.json") {
        &[
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
        ]
    } else if evidence.ends_with("alias-aware-fusion-fission.json") {
        &[
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
        ]
    } else if evidence.ends_with("weir-facts-pass-firing.json") {
        &[
            "alias-aware-dse-entrypoint",
            "reaching-def-dse-entrypoint",
            "alias-aware-stlf-entrypoint",
            "reaching-def-stlf-entrypoint",
            "alias-aware-licm-entrypoint",
            "reaching-def-licm-entrypoint",
            "alias-aware-loop-fusion-entrypoint",
            "reaching-def-loop-fusion-entrypoint",
            "alias-aware-loop-fission-entrypoint",
            "reaching-def-loop-fission-entrypoint",
            "weir-alias-analysis-api",
            "weir-reaching-def-analysis-api",
            "dataflow-analysis-pipeline-entrypoint",
        ]
    } else if evidence.ends_with("egraph-saturation-matrix.json")
        || evidence.ends_with("egraph-semantic-contracts.json")
    {
        &[
            "egraph-saturation",
            "egraph-canonical-pipeline-entrypoint",
            "egraph-algebraic-reassociation",
            "egraph-bitwise-reassociation",
        ]
    } else {
        &[]
    }
}

#[cfg(test)]
mod part1_tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    #[test]
    fn completion_audit_rejects_hidden_failed_case_summary_zero() {
        let dir = TempDir::new()
            .expect("Fix: create temporary workspace for hidden benchmark audit test.");
        let path = dir.path().join("wgpu-hidden-invalid.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "summary": {"failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass",
                        "correctness": {
                            "Invalid": {
                                "reason": "CUDA/WGPU output mismatch at row 17"
                            }
                        },
                        "performance": {"contract_passed": true}
                    }
                ]
            }))
            .expect("Fix: serialize hidden failed benchmark JSON."),
        )
        .expect("Fix: write hidden failed benchmark JSON.");

        let mut blockers = Vec::new();
        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-hidden-invalid.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "benchmark summary reports 0 failed case(s); case evidence reports 1 failed case(s): `release.condition_eval.1m`: CUDA/WGPU output mismatch at row 17"
            )),
            "Fix: completion audit must reject benchmark case failures hidden behind summary.failed=0; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_stale_summary_passed_count() {
        let dir = TempDir::new()
            .expect("Fix: create temporary workspace for stale benchmark summary audit test.");
        let path = dir.path().join("wgpu-stale-passed.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 0, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize stale benchmark summary JSON."),
        )
        .expect("Fix: write stale benchmark summary JSON.");

        let mut blockers = Vec::new();
        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-stale-passed.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "benchmark summary is invalid: summary total/pass/fail (Some(1)/Some(0)/Some(0)) contradicts case evidence (1/1/0)"
            )),
            "Fix: completion audit must reject stale summary.passed even when summary.failed is zero; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_generic_benchmark_backend_drift() {
        let dir = TempDir::new()
            .expect("Fix: create temporary workspace for backend drift benchmark audit test.");
        let path = dir.path().join("wgpu-backend-drift.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "summary": {"total_cases": 1, "passed": 1, "failed": 0},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "cuda",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize backend drift benchmark JSON."),
        )
        .expect("Fix: write backend drift benchmark JSON.");

        let mut blockers = Vec::new();
        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-backend-drift.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/wgpu-backend-drift.json: case `release.condition_eval.1m` backend_id `cuda` does not match selected_backend `wgpu`"
            )),
            "Fix: completion audit must reject generic benchmark cases executed on a backend other than selected_backend; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_generic_benchmark_missing_source_provenance() {
        let dir = TempDir::new()
            .expect("Fix: create temporary workspace for missing-source benchmark audit test.");
        let path = dir.path().join("wgpu-missing-source.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "selected_backend": "wgpu",
                "git": {"commit": "0123456789abcdef0123456789abcdef01234567"},
                "source_artifacts": ["", null],
                "summary": {"total_cases": 1, "passed": 1, "failed": 0, "cache_hit_rate": null},
                "environment": {"cpu_model": "test CPU"},
                "cases": [
                    {
                        "id": "release.condition_eval.1m",
                        "backend_id": "wgpu",
                        "status": "pass"
                    }
                ]
            }))
            .expect("Fix: serialize missing-source benchmark JSON."),
        )
        .expect("Fix: write missing-source benchmark JSON.");

        let mut blockers = Vec::new();
        inspect_json_evidence(
            "release/evidence/benchmarks/wgpu-missing-source.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/wgpu-missing-source.json: benchmark report must include source_fingerprint provenance"
            )),
            "Fix: completion audit must reject generic benchmark reports with no source provenance; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_generated_evidence_missing_blockers_array() {
        let dir = TempDir::new()
            .expect("Fix: create temporary workspace for generated blockers audit test.");
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n")
            .expect("Fix: write temporary workspace manifest.");
        let benchmark_dir = dir.path().join("release/evidence/benchmarks");
        std::fs::create_dir_all(&benchmark_dir)
            .expect("Fix: create benchmark evidence directory for generated blockers test.");
        let path = benchmark_dir.join("bench-release-axes.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "source_artifacts": []
            }))
            .expect("Fix: serialize generated evidence without blockers."),
        )
        .expect("Fix: write generated evidence without blockers.");

        let mut blockers = Vec::new();
        inspect_json_evidence(
            "release/evidence/benchmarks/bench-release-axes.json",
            &path,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release/evidence/benchmarks/bench-release-axes.json: missing blockers array"
            )),
            "Fix: completion audit must fail closed when generated release benchmark evidence omits blockers; blockers={blockers:?}"
        );
    }
}
