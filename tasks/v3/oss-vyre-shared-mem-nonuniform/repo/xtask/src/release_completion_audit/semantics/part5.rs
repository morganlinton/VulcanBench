fn inspect_optimization_corpus_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let required = value
        .get("required_min_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(4_096);
    let generated = value
        .get("generated_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let verified = value
        .get("verified_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let optimized = value
        .get("optimized_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dataflow_analysis_cases = value
        .get("dataflow_analysis_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dataflow_analysis_optimized = value
        .get("dataflow_analysis_optimized_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let non_converged = value
        .get("non_converged_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let ops_before = value
        .get("total_ops_before")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let ops_after = value
        .get("total_ops_after")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if required < 4_096 {
        blockers.push(format!(
            "{evidence}: required_min_cases is {required}; release floor is 4096"
        ));
    }
    if generated < required || generated < 4_096 {
        blockers.push(format!(
            "{evidence}: generated_cases is {generated}; needs at least {required} and never below 4096"
        ));
    }
    if verified != generated {
        blockers.push(format!(
            "{evidence}: verified_cases {verified} does not equal generated_cases {generated}"
        ));
    }
    if optimized == 0 {
        blockers.push(format!(
            "{evidence}: optimized_cases is zero; corpus does not prove optimizer firing"
        ));
    }
    if dataflow_analysis_cases == 0 {
        blockers.push(format!(
            "{evidence}: dataflow_analysis_cases is zero; corpus does not prove Weir-aware optimizer firing"
        ));
    }
    if dataflow_analysis_optimized < dataflow_analysis_cases {
        blockers.push(format!(
            "{evidence}: dataflow_analysis_optimized_cases {dataflow_analysis_optimized} is below dataflow_analysis_cases {dataflow_analysis_cases}"
        ));
    }
    if non_converged != 0 {
        blockers.push(format!(
            "{evidence}: non_converged_cases is {non_converged}; release requires zero"
        ));
    }
    if ops_before == 0 || ops_after == 0 {
        blockers.push(format!(
            "{evidence}: total_ops_before={ops_before}, total_ops_after={ops_after}; corpus must include real IR size evidence"
        ));
    }
}

fn read_referenced_release_json(
    manifest_path: &Path,
    artifact: &str,
    blockers: &mut Vec<String>,
) -> Option<serde_json::Value> {
    let artifact_path = if Path::new(artifact).is_absolute() {
        PathBuf::from(artifact)
    } else if artifact.starts_with("release/") {
        manifest_path
            .ancestors()
            .nth(4)
            .map(|workspace| workspace.join(artifact))
            .unwrap_or_else(|| PathBuf::from(artifact))
    } else {
        manifest_path
            .parent()
            .map(|parent| parent.join(artifact))
            .unwrap_or_else(|| PathBuf::from(artifact))
    };
    let text = match read_text_bounded(&artifact_path) {
        Ok(text) => text,
        Err(error) => {
            blockers.push(format!(
                "{}: failed to read referenced benchmark artifact `{}`: {error}",
                manifest_path.display(),
                artifact_path.display()
            ));
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            blockers.push(format!(
                "{}: referenced benchmark artifact `{}` is invalid JSON: {error}",
                manifest_path.display(),
                artifact_path.display()
            ));
            None
        }
    }
}

fn benchmark_report_has_metric(report: &serde_json::Value, metric: &str) -> bool {
    report
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|cases| {
            cases.iter().any(|case| {
                case.get("metrics")
                    .and_then(serde_json::Value::as_object)
                    .is_some_and(|metrics| metrics.contains_key(metric))
            })
        })
}

fn benchmark_report_has_positive_metric(report: &serde_json::Value, metric: &str) -> bool {
    report
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|cases| {
            cases.iter().any(|case| {
                case.get("metrics")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|metrics| metrics.get(metric))
                    .and_then(|value| metric_p50(Some(value)))
                    .is_some_and(|value| value > 0.0)
            })
        })
}

fn inspect_benchmark_cuda_environment_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some(environment) = value.get("environment") else {
        blockers.push(format!(
            "{evidence}: CUDA benchmark missing environment provenance"
        ));
        return;
    };
    let gpu_devices = environment
        .get("gpu_devices")
        .and_then(serde_json::Value::as_array);
    let first_gpu = gpu_devices.and_then(|devices| devices.first());
    if gpu_devices.is_none_or(|devices| devices.is_empty()) {
        blockers.push(format!(
            "{evidence}: CUDA benchmark has no nvidia-smi gpu_devices provenance"
        ));
    }
    if first_gpu
        .and_then(|device| device.get("name"))
        .and_then(serde_json::Value::as_str)
        .is_none_or(str::is_empty)
    {
        blockers.push(format!(
            "{evidence}: CUDA benchmark has no GPU model from nvidia-smi"
        ));
    }
    match first_gpu
        .and_then(|device| device.get("memory_total_mib"))
        .and_then(serde_json::Value::as_u64)
    {
        Some(mib) if mib >= 16 * 1024 => {}
        Some(mib) => blockers.push(format!(
            "{evidence}: CUDA benchmark GPU memory is {mib} MiB, below release floor 16384 MiB"
        )),
        None => blockers.push(format!(
            "{evidence}: CUDA benchmark has no GPU memory_total_mib from nvidia-smi"
        )),
    }
    match (
        first_gpu
            .and_then(|device| device.get("compute_capability_major"))
            .and_then(serde_json::Value::as_u64),
        first_gpu
            .and_then(|device| device.get("compute_capability_minor"))
            .and_then(serde_json::Value::as_u64),
    ) {
        (Some(major), Some(minor)) if (major, minor) >= (8, 0) => {}
        (Some(major), Some(minor)) => blockers.push(format!(
            "{evidence}: CUDA benchmark compute capability is {major}.{minor}, below release floor 8.0"
        )),
        _ => blockers.push(format!(
            "{evidence}: CUDA benchmark has no compute capability from nvidia-smi"
        )),
    }
    for field in ["nvidia_driver_version", "nvidia_cuda_version"] {
        if environment
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            blockers.push(format!(
                "{evidence}: CUDA benchmark environment is missing `{field}` from nvidia-smi"
            ));
        }
    }
}

fn inspect_hygiene_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("finding_summary")
        .and_then(serde_json::Value::as_array)
        .is_none()
    {
        blockers.push(format!("{evidence}: missing finding_summary"));
    }
    let finding_count = value
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let summary_count = value
        .get("finding_summary")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("count").and_then(serde_json::Value::as_u64))
                .sum::<u64>() as usize
        })
        .unwrap_or(usize::MAX);
    if finding_count != summary_count {
        blockers.push(format!(
            "{evidence}: finding_summary count {summary_count} does not match findings count {finding_count}"
        ));
    }
    inspect_hygiene_release_surface_coverage(evidence, value, blockers);
    let Some(roots) = value
        .get("scanned_roots")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!("{evidence}: missing scanned_roots"));
        return;
    };
    for required_root in [
        "libs/performance/matching/vyre",
        "libs/dataflow/weir",
        "tools/vyrec",
        "libs/surge/surgec",
        "libs/performance/matching/vyre/vyre-grammar-gen",
    ] {
        if !roots.iter().any(|root| {
            root.as_str()
                .is_some_and(|root| root.contains(required_root))
        }) {
            blockers.push(format!(
                "{evidence}: scanned_roots is missing `{required_root}`"
            ));
        }
    }
}

fn inspect_hygiene_release_surface_coverage(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some(coverage) = value.get("release_surface_coverage") else {
        blockers.push(format!("{evidence}: missing release_surface_coverage"));
        return;
    };
    for field in [
        "vyre_workspace",
        "cuda_driver_crate",
        "wgpu_driver_crate",
        "weir_crate",
        "vyrec_tool",
        "surgec_tool",
        "surgec_grammar_gen",
        "release_scripts",
        "github_workflows",
        "branch_protection_controls",
    ] {
        if coverage.get(field).and_then(serde_json::Value::as_bool) != Some(true) {
            blockers.push(format!(
                "{evidence}: release_surface_coverage.{field} must be true"
            ));
        }
    }
    for (field, required) in [
        (
            "resource_bound_patterns",
            &[
                "std_thread_sleep",
                "thread_sleep",
                "tokio_sleep",
                "unbounded_read",
            ][..],
        ),
        (
            "hidden_fallback_patterns",
            &[
                "silent_gpu_skip",
                "silent_gpu_skipped",
                "gpu_unavailable_skip",
                "cfg_not_gpu",
                "cpu_fallback",
                "software_fallback",
                "fallback_dispatch",
                "falling_back_to_cpu",
                "fallback_to_cpu",
                "synthetic_gpu_timing",
                "fake_gpu_timing_formula",
            ][..],
        ),
        (
            "release_tooling_patterns",
            &[
                "raw_workspace_cargo",
                "invalid_cargo_full_xtask",
                "heredoc",
                "missing_cargo_wrapper",
            ][..],
        ),
    ] {
        let values = coverage.get(field).and_then(serde_json::Value::as_array);
        for required_value in required {
            if !values.is_some_and(|values| {
                values
                    .iter()
                    .any(|value| value.as_str() == Some(*required_value))
            }) {
                blockers.push(format!(
                    "{evidence}: release_surface_coverage.{field} is missing `{required_value}`"
                ));
            }
        }
    }
}

fn inspect_optimization_analysis_fixture_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let missing_required = value
        .get("missing_required_families")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required != 0 {
        blockers.push(format!(
            "{evidence}: missing_required_families has {missing_required} entrie(s), expected zero"
        ));
    }
    let total_fixture_cases = value
        .get("total_fixture_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_triggered_cases = value
        .get("total_triggered_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if total_fixture_cases < 512 || total_triggered_cases != total_fixture_cases {
        blockers.push(format!(
            "{evidence}: total_fixture_cases={total_fixture_cases}, total_triggered_cases={total_triggered_cases}; needs 512 fully-triggered A13-A16 cases"
        ));
    }
    let Some(families) = value.get("families").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing families array"));
        return;
    };
    inspect_duplicate_analysis_fixture_family_rows(evidence, value, blockers);
    for required in [
        "A13-coalesce-fixture",
        "A14-shared-mem-promote-fixture",
        "A15-bank-conflict-fixture",
        "A16-vec-pack-fixture",
    ] {
        let Some(family) = families.iter().find(|family| {
            family.get("family").and_then(serde_json::Value::as_str) == Some(required)
        }) else {
            blockers.push(format!(
                "{evidence}: missing analysis fixture family `{required}`"
            ));
            continue;
        };
        let cases = family
            .get("cases")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let triggered = family
            .get("triggered_cases")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let analysis_sites = family
            .get("analysis_sites")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if cases < 128 || triggered != cases || analysis_sites < cases {
            blockers.push(format!(
                "{evidence}: analysis fixture `{required}` has cases={cases}, triggered_cases={triggered}, analysis_sites={analysis_sites}; needs at least 128 cases, every case triggered, and at least one analysis site per case"
            ));
        }
        match required {
            "A13-coalesce-fixture" => {
                for field in [
                    "coalesced_unit_stride_sites",
                    "strided_sites",
                    "broadcast_sites",
                ] {
                    if family
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        blockers.push(format!("{evidence}: A13 fixture has zero `{field}`"));
                    }
                }
            }
            "A14-shared-mem-promote-fixture" => {
                for field in ["shared_mem_candidates", "shared_mem_tile_bytes"] {
                    if family
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        blockers.push(format!("{evidence}: A14 fixture has zero `{field}`"));
                    }
                }
            }
            "A15-bank-conflict-fixture" => {
                for field in ["bank_conflict_sites", "bank_conflict_critical_sites"] {
                    if family
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        blockers.push(format!("{evidence}: A15 fixture has zero `{field}`"));
                    }
                }
            }
            "A16-vec-pack-fixture" => {
                for field in ["vec_pack_chains", "vec_pack_ops_eliminated"] {
                    if family
                        .get(field)
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        == 0
                    {
                        blockers.push(format!("{evidence}: A16 fixture has zero `{field}`"));
                    }
                }
            }
            _ => {}
        }
    }
}

fn inspect_duplicate_analysis_fixture_family_rows(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            value, "families", "family",
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!(
            "{evidence}: duplicate analysis fixture family rows: {duplicates}"
        ));
    }
}

#[cfg(test)]
mod part5_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_duplicate_analysis_fixture_family_rows() {
        let manifest = serde_json::json!({
            "missing_required_families": [],
            "total_fixture_cases": 512,
            "total_triggered_cases": 512,
            "families": [
                {
                    "family": "A13-coalesce-fixture",
                    "cases": 128,
                    "triggered_cases": 128,
                    "analysis_sites": 128,
                    "coalesced_unit_stride_sites": 1,
                    "strided_sites": 1,
                    "broadcast_sites": 1
                },
                {
                    "family": "A13-coalesce-fixture",
                    "cases": 128,
                    "triggered_cases": 128,
                    "analysis_sites": 128,
                    "coalesced_unit_stride_sites": 1,
                    "strided_sites": 1,
                    "broadcast_sites": 1
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_optimization_analysis_fixture_semantics(
            "optimization-analysis-fixtures.json",
            &manifest,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| {
                blocker.contains("duplicate analysis fixture family rows: A13-coalesce-fixture")
            }),
            "Fix: completion audit must reject duplicate analysis fixture family rows before totals can prove A13-A16 coverage; blockers={blockers:?}"
        );
    }
}
