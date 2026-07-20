fn inspect_backend_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 {
        blockers.push(format!(
            "{evidence}: schema_version is {schema_version}, expected >= 2"
        ));
    }
    if value.get("cuda_first").and_then(serde_json::Value::as_bool) != Some(true) {
        blockers.push(format!("{evidence}: cuda_first must be true"));
    }
    if value
        .get("wgpu_fallback_present")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!("{evidence}: wgpu_fallback_present must be true"));
    }
    if value
        .get("preferred_backend_gpu_only")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!(
            "{evidence}: preferred_backend_gpu_only must be true"
        ));
    }
    let preferred = value
        .get("preferred_backend_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if !matches!(preferred, "cuda" | "wgpu") {
        blockers.push(format!(
            "{evidence}: preferred_backend_id `{preferred}` must be cuda or wgpu"
        ));
    }
    if value
        .get("gpu_probe")
        .and_then(|probe| probe.get("nvidia_smi_ok"))
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        blockers.push(format!("{evidence}: gpu_probe.nvidia_smi_ok must be true"));
    }
    let gpu_devices = value
        .get("gpu_probe")
        .and_then(|probe| probe.get("nvidia_smi_devices"))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if gpu_devices == 0 {
        blockers.push(format!(
            "{evidence}: gpu_probe.nvidia_smi_devices must list at least one GPU"
        ));
    }
    let release_floor_device = value
        .get("gpu_probe")
        .and_then(|probe| probe.get("nvidia_smi_device_details"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|devices| {
            devices.iter().any(|device| {
                device
                    .get("memory_total_mib")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|mib| mib >= 16 * 1024)
                    && matches!(
                        (
                            device
                                .get("compute_capability_major")
                                .and_then(serde_json::Value::as_u64),
                            device
                                .get("compute_capability_minor")
                                .and_then(serde_json::Value::as_u64),
                        ),
                        (Some(major), Some(minor)) if (major, minor) >= (8, 0)
                    )
            })
        });
    if !release_floor_device {
        blockers.push(format!(
            "{evidence}: gpu_probe.nvidia_smi_device_details must include a CUDA GPU with >=16384 MiB VRAM and compute capability >=8.0"
        ));
    }
    for field in ["nvidia_driver_version", "nvidia_cuda_version"] {
        if value
            .get("gpu_probe")
            .and_then(|probe| probe.get(field))
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            blockers.push(format!("{evidence}: gpu_probe.{field} must be recorded"));
        }
    }
    let backends = value
        .get("backends")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in ["cuda", "wgpu"] {
        if !backends.iter().any(|backend| {
            backend.get("id").and_then(serde_json::Value::as_str) == Some(required)
                && backend
                    .get("dispatches")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                && backend
                    .get("acquire_ok")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        }) {
            blockers.push(format!(
                "{evidence}: backend `{required}` must dispatch and acquire successfully"
            ));
        }
    }
    for (field, minimum) in [
        ("cuda_feature_markers", 12usize),
        ("wgpu_feature_markers", 7usize),
    ] {
        let Some(markers) = value.get(field).and_then(serde_json::Value::as_array) else {
            blockers.push(format!("{evidence}: missing {field}"));
            continue;
        };
        if markers.len() < minimum {
            blockers.push(format!(
                "{evidence}: {field} has {} marker(s), needs at least {minimum}",
                markers.len()
            ));
        }
        let required_ids: &[&str] = match field {
            "cuda_feature_markers" => &[
                "tensor-core-fragment",
                "ldmatrix-cp-async",
                "predicated-execution",
                "instruction-scheduling",
                "ptx-vector-load-gap-scheduling",
                "ptx-compute-load-gap-scheduling",
                "ptx-vector-load-fusion",
                "ptx-vector-store-fusion",
                "async-copy-emitter",
                "mma-emitter",
                "cuda-resident-dispatch",
                "cuda-resident-io",
                "cuda-graph-launch",
                "cuda-module-cache",
                "cuda-ptx-source-cache",
                "cuda-ptx-target-probe",
                "megakernel-paired-speculation",
            ],
            "wgpu_feature_markers" => &[
                "wgpu-persistent-engine",
                "wgpu-megakernel-dispatcher",
                "wgpu-readback-ring",
                "wgpu-async-dispatch-prefetch",
                "wgpu-dispatch-scratch-reuse",
                "wgpu-disk-cache",
                "wgpu-no-cpu-fallback-test",
                "megakernel-paired-speculation",
            ],
            _ => &[],
        };
        for required_id in required_ids {
            if !markers.iter().any(|marker| {
                marker.get("id").and_then(serde_json::Value::as_str) == Some(*required_id)
            }) {
                blockers.push(format!(
                    "{evidence}: {field} is missing required marker `{required_id}`"
                ));
            }
        }
        for marker in markers {
            let id = marker
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            if marker.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
                blockers.push(format!("{evidence}: {field} marker `{id}` does not exist"));
            }
            if marker
                .get("source_bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                blockers.push(format!("{evidence}: {field} marker `{id}` is empty"));
            }
            if marker
                .get("missing_tokens")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|tokens| !tokens.is_empty())
            {
                blockers.push(format!(
                    "{evidence}: {field} marker `{id}` has missing tokens"
                ));
            }
            if marker
                .get("unresolved_markers")
                .and_then(serde_json::Value::as_array)
                .is_none_or(|markers| !markers.is_empty())
            {
                blockers.push(format!(
                    "{evidence}: {field} marker `{id}` has unresolved markers"
                ));
            }
        }
    }
    let Some(scan_errors) = value
        .get("hidden_fallback_scan_errors")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!("{evidence}: missing hidden_fallback_scan_errors"));
        return;
    };
    if !scan_errors.is_empty() {
        blockers.push(format!(
            "{evidence}: reports {} hidden fallback scan error(s)",
            scan_errors.len()
        ));
    }
    let Some(findings) = value
        .get("hidden_fallback_findings")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!("{evidence}: missing hidden_fallback_findings"));
        return;
    };
    if !findings.is_empty() {
        blockers.push(format!(
            "{evidence}: reports {} hidden fallback finding(s)",
            findings.len()
        ));
    }
}

fn inspect_release_workload_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let required = value
        .get("required_closed_families")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if required < 12 {
        blockers.push(format!(
            "{evidence}: required_closed_families is {required}; needs at least 12"
        ));
    }
    let matched = value
        .get("matched_required_families")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if matched < 12 {
        blockers.push(format!(
            "{evidence}: matched_required_families is {matched}; needs at least 12"
        ));
    }
    if matched < required {
        blockers.push(format!(
            "{evidence}: matched_required_families {matched} is below required_closed_families {required}"
        ));
    }
    let release_cases = value
        .get("release_suite_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if release_cases < matched {
        blockers.push(format!(
            "{evidence}: release_suite_case_count {release_cases} is below matched_required_families {matched}"
        ));
    }
    let family_count = value
        .get("cpu_sota_100x_family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if family_count < 10 {
        blockers.push(format!(
            "{evidence}: cpu_sota_100x_family_count is {family_count}; needs at least 10"
        ));
    }
    let required_hundred_x = value
        .get("required_cpu_sota_100x_families")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if required_hundred_x < 10 {
        blockers.push(format!(
            "{evidence}: required_cpu_sota_100x_families lists {required_hundred_x} family/families; needs at least 10 release 100x families"
        ));
    }
    inspect_duplicate_array_values(evidence, value, "required_cpu_sota_100x_families", blockers);
    let missing_required_hundred_x = value
        .get("missing_required_cpu_sota_100x_families")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required_hundred_x != 0 {
        blockers.push(format!(
            "{evidence}: missing_required_cpu_sota_100x_families reports {missing_required_hundred_x} missing required family/families"
        ));
    }
    let case_count = value
        .get("cpu_sota_100x_contract_cases")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if case_count < 10 {
        blockers.push(format!(
            "{evidence}: cpu_sota_100x_contract_cases lists {case_count} active case id(s); needs at least 10"
        ));
    }
    inspect_duplicate_array_values(evidence, value, "cpu_sota_100x_contract_cases", blockers);
    let Some(families) = value.get("families").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing workload families array"));
        return;
    };
    inspect_duplicate_workload_family_ids(evidence, value, blockers);
    let mut required_family_count = 0usize;
    let mut covered_family_count = 0usize;
    let mut matched_release_cases = BTreeSet::new();
    let mut hundred_x_family_count = 0usize;
    let mut artifacts = BTreeSet::new();
    let mut workload_numbers = BTreeSet::new();
    for family in families {
        let id = family
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if family.get("required").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        required_family_count += 1;
        let duplicate_matched_cases =
            crate::benchmark_evidence_semantics::duplicate_nonblank_string_array_values(
                family,
                "matched_cases",
            );
        if !duplicate_matched_cases.is_empty() {
            let duplicates = duplicate_matched_cases
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has duplicate matched_cases: {duplicates}"
            ));
        }
        let matched_cases = family
            .get("matched_cases")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if matched_cases == 0 {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no matched_cases"
            ));
        } else {
            covered_family_count += 1;
            if let Some(cases) = family
                .get("matched_cases")
                .and_then(serde_json::Value::as_array)
            {
                matched_release_cases.extend(cases.iter().filter_map(serde_json::Value::as_str));
            }
        }
        let dispatch_policy = family
            .get("dispatch_policy")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if dispatch_policy.is_empty() {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no dispatch_policy"
            ));
        }
        let bench_target_ids = family
            .get("bench_target_ids")
            .and_then(serde_json::Value::as_array)
            .map(|targets| {
                targets
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if bench_target_ids.is_empty()
            || !bench_target_ids
                .iter()
                .all(|target| target.starts_with("release.workload."))
        {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` must list release BENCH_TARGETS.toml target ids"
            ));
        }
        if id == "megakernel-queued-batches" && dispatch_policy != "megakernel" {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` must use megakernel dispatch policy, found `{dispatch_policy}`"
            ));
        }
        if dispatch_policy != "megakernel" {
            let justification = family
                .get("non_megakernel_justification")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if justification.len() < 48 {
                blockers.push(format!(
                    "{evidence}: required workload family `{id}` uses non-megakernel dispatch policy `{dispatch_policy}` without a concrete architectural or measured justification"
                ));
            }
        }
        let cpu_sota_contracts = family
            .get("cpu_sota_contracts")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if cpu_sota_contracts == 0 {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no CPU-SOTA baseline contract"
            ));
        }
        if family
            .get("max_cpu_sota_min_speedup_x")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|speedup| speedup >= 100.0)
        {
            let hundred_x_cases = family
                .get("cpu_sota_100x_cases")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            if hundred_x_cases == 0 {
                blockers.push(format!(
                    "{evidence}: required workload family `{id}` declares a 100x contract but lists no cpu_sota_100x_cases"
                ));
            } else {
                hundred_x_family_count += 1;
            }
        }
        let duplicate_hundred_x_cases =
            crate::benchmark_evidence_semantics::duplicate_nonblank_string_array_values(
                family,
                "cpu_sota_100x_cases",
            );
        if !duplicate_hundred_x_cases.is_empty() {
            let duplicates = duplicate_hundred_x_cases
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has duplicate cpu_sota_100x_cases: {duplicates}"
            ));
        }
        let workload_number = family
            .get("release_plan_workload")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if workload_number == 0 || !workload_numbers.insert(workload_number) {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has invalid or duplicate release_plan_workload `{workload_number}`"
            ));
        }
        let artifact = family
            .get("evidence_artifact")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if artifact.is_empty() {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no evidence_artifact"
            ));
        } else {
            if !artifacts.insert(artifact) {
                blockers.push(format!(
                    "{evidence}: required workload family `{id}` reuses evidence artifact `{artifact}`"
                ));
            }
            if !artifact.starts_with("release/evidence/benchmarks/workload-") {
                blockers.push(format!(
                    "{evidence}: required workload family `{id}` artifact `{artifact}` is not a workload benchmark artifact"
                ));
            }
        }
        let command = family
            .get("benchmark_command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !command.contains("cargo_full") || !command.contains(artifact) {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` benchmark_command must use cargo_full and its evidence_artifact"
            ));
        }
        if family
            .get("fair_cpu_sota_baseline_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no fair CPU-SOTA baseline crate bound to CUDA"
            ));
        }
        if family
            .get("cpu_sota_baseline_names")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len)
            == 0
        {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` has no named CPU-SOTA baseline provenance"
            ));
        }
        if family
            .get("reproducible_cuda_command")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            blockers.push(format!(
                "{evidence}: required workload family `{id}` does not declare a reproducible CUDA benchmark command"
            ));
        }
    }
    if required_family_count < 12 {
        blockers.push(format!(
            "{evidence}: declares {required_family_count} required workload families; needs at least 12"
        ));
    }
    if covered_family_count < 12 {
        blockers.push(format!(
            "{evidence}: covers {covered_family_count} required workload families; needs at least 12"
        ));
    }
    inspect_declared_workload_matrix_count(
        evidence,
        value,
        "required_closed_families",
        required_family_count,
        blockers,
    );
    inspect_declared_workload_matrix_count(
        evidence,
        value,
        "matched_required_families",
        covered_family_count,
        blockers,
    );
    inspect_declared_workload_matrix_count(
        evidence,
        value,
        "release_suite_case_count",
        matched_release_cases.len(),
        blockers,
    );
    inspect_declared_workload_matrix_count(
        evidence,
        value,
        "cpu_sota_100x_family_count",
        hundred_x_family_count,
        blockers,
    );
}

fn inspect_declared_workload_matrix_count(
    evidence: &str,
    value: &serde_json::Value,
    field: &str,
    derived: usize,
    blockers: &mut Vec<String>,
) {
    let Some(declared) = value.get(field).and_then(serde_json::Value::as_u64) else {
        return;
    };
    if declared != derived as u64 {
        blockers.push(format!(
            "{evidence}: {field}={declared}, but derived row evidence has {derived}"
        ));
    }
}

fn inspect_duplicate_workload_family_ids(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            value, "families", "id",
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!(
            "{evidence}: duplicate workload family ids: {duplicates}"
        ));
    }
}

fn inspect_duplicate_array_values(
    evidence: &str,
    value: &serde_json::Value,
    field: &str,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_string_array_values(value, field);
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!("{evidence}: duplicate {field}: {duplicates}"));
    }
}

#[cfg(test)]
mod part10_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_duplicate_matrix_cpu_100x_case_ids() {
        let matrix = serde_json::json!({
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.condition_eval.1m"
            ]
        });
        let mut blockers = Vec::new();

        inspect_duplicate_array_values(
            "release-workload-matrix.json",
            &matrix,
            "cpu_sota_100x_contract_cases",
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate cpu_sota_100x_contract_cases: release.condition_eval.1m"
            )),
            "Fix: completion audit must reject duplicate CPU-SOTA matrix contract case ids; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_matrix_cpu_100x_family_ids() {
        let matrix = serde_json::json!({
            "required_cpu_sota_100x_families": [
                "release.condition-eval",
                "release.condition-eval"
            ]
        });
        let mut blockers = Vec::new();

        inspect_duplicate_array_values(
            "release-workload-matrix.json",
            &matrix,
            "required_cpu_sota_100x_families",
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "duplicate required_cpu_sota_100x_families: release.condition-eval"
            )),
            "Fix: completion audit must reject duplicate CPU-SOTA matrix required family ids; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_workload_matrix_family_ids() {
        let matrix = serde_json::json!({
            "required_closed_families": 12,
            "matched_required_families": 12,
            "release_suite_case_count": 12,
            "cpu_sota_100x_family_count": 10,
            "required_cpu_sota_100x_families": [
                "release.condition-eval",
                "release.entropy-window",
                "release.ifds-witness",
                "release.loop-carried",
                "release.sparse-frontier",
                "release.memory-coalescing",
                "release.bank-conflict",
                "release.vec-pack",
                "release.control-flow",
                "release.dataflow-dse"
            ],
            "missing_required_cpu_sota_100x_families": [],
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.entropy_window.1m",
                "release.ifds_witness.1m",
                "release.loop_carried.1m",
                "release.sparse_frontier.1m",
                "release.memory_coalescing.1m",
                "release.bank_conflict.1m",
                "release.vec_pack.1m",
                "release.control_flow.1m",
                "release.dataflow_dse.1m"
            ],
            "families": [
                {"id": "condition-eval", "required": true},
                {"id": "condition-eval", "required": true}
            ]
        });
        let mut blockers = Vec::new();

        inspect_release_workload_matrix_semantics(
            "release-workload-matrix.json",
            &matrix,
            &mut blockers,
        );

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("duplicate workload family ids: condition-eval")),
            "Fix: completion audit must reject duplicate workload matrix family ids before row counts can prove coverage; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_workload_family_case_rows() {
        let matrix = serde_json::json!({
            "required_closed_families": 12,
            "matched_required_families": 12,
            "release_suite_case_count": 12,
            "cpu_sota_100x_family_count": 10,
            "required_cpu_sota_100x_families": [
                "release.condition-eval",
                "release.string-bitmap-scatter",
                "release.offset-count-aggregation",
                "release.entropy-window",
                "release.quantified-condition-loops",
                "release.alias-reaching-def",
                "release.ifds-witness",
                "release.c-ast-traversal",
                "release.megakernel-queue",
                "release.egraph-saturation"
            ],
            "missing_required_cpu_sota_100x_families": [],
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.string_bitmap_scatter.1m",
                "release.offset_count_aggregation.1m",
                "release.entropy_window.1m",
                "release.quantified_condition_loops.1m",
                "release.alias_reaching_def.1m",
                "release.ifds_witness.1m",
                "release.c_ast_traversal.1m",
                "release.megakernel_queue.1m",
                "release.egraph_saturation.1m"
            ],
            "families": [
                {
                    "id": "condition-eval",
                    "required": true,
                    "matched_cases": [
                        "release.condition_eval.1m",
                        "release.condition_eval.1m"
                    ],
                    "dispatch_policy": "megakernel",
                    "bench_target_ids": ["release.workload.condition_eval"],
                    "cpu_sota_contracts": [{"class": "CpuSota"}],
                    "max_cpu_sota_min_speedup_x": 100.0,
                    "cpu_sota_100x_cases": [
                        "release.condition_eval.1m",
                        "release.condition_eval.1m"
                    ],
                    "release_plan_workload": 1,
                    "evidence_artifact": "release/evidence/benchmarks/workload-01-condition-eval.json",
                    "benchmark_command": "cargo_full run --release -- release/evidence/benchmarks/workload-01-condition-eval.json",
                    "fair_cpu_sota_baseline_count": 1,
                    "cpu_sota_baseline_names": ["CPU baseline"],
                    "reproducible_cuda_command": true
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_release_workload_matrix_semantics(
            "release-workload-matrix.json",
            &matrix,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "required workload family `condition-eval` has duplicate matched_cases: release.condition_eval.1m"
            )),
            "Fix: completion audit must reject duplicate family matched_cases before row counts can prove workload coverage; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "required workload family `condition-eval` has duplicate cpu_sota_100x_cases: release.condition_eval.1m"
            )),
            "Fix: completion audit must reject duplicate family cpu_sota_100x_cases before 100x coverage can be inflated; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_inflated_workload_matrix_counts() {
        let matrix = serde_json::json!({
            "required_closed_families": 12,
            "matched_required_families": 12,
            "release_suite_case_count": 12,
            "cpu_sota_100x_family_count": 10,
            "required_cpu_sota_100x_families": [
                "release.condition-eval",
                "release.string-bitmap-scatter",
                "release.offset-count-aggregation",
                "release.entropy-window",
                "release.quantified-condition-loops",
                "release.alias-reaching-def",
                "release.ifds-witness",
                "release.c-ast-traversal",
                "release.megakernel-queue",
                "release.egraph-saturation"
            ],
            "missing_required_cpu_sota_100x_families": [],
            "cpu_sota_100x_contract_cases": [
                "release.condition_eval.1m",
                "release.string_bitmap_scatter.1m",
                "release.offset_count_aggregation.1m",
                "release.entropy_window.1m",
                "release.quantified_condition_loops.1m",
                "release.alias_reaching_def.1m",
                "release.ifds_witness.1m",
                "release.c_ast_traversal.1m",
                "release.megakernel_queue.1m",
                "release.egraph_saturation.1m"
            ],
            "families": [
                {
                    "id": "condition-eval",
                    "required": true,
                    "matched_cases": ["release.condition_eval.1m"],
                    "dispatch_policy": "megakernel",
                    "bench_target_ids": ["release.workload.condition_eval"],
                    "cpu_sota_contracts": [{"class": "CpuSota"}],
                    "max_cpu_sota_min_speedup_x": 100.0,
                    "cpu_sota_100x_cases": ["release.condition_eval.1m"],
                    "release_plan_workload": 1,
                    "evidence_artifact": "release/evidence/benchmarks/workload-01-condition-eval.json",
                    "benchmark_command": "cargo_full run --release -- release/evidence/benchmarks/workload-01-condition-eval.json",
                    "fair_cpu_sota_baseline_count": 1,
                    "cpu_sota_baseline_names": ["CPU baseline"],
                    "reproducible_cuda_command": true
                }
            ]
        });
        let mut blockers = Vec::new();

        inspect_release_workload_matrix_semantics(
            "release-workload-matrix.json",
            &matrix,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release-workload-matrix.json: required_closed_families=12, but derived row evidence has 1"
            )),
            "Fix: completion audit must reject inflated required workload family counts; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release-workload-matrix.json: release_suite_case_count=12, but derived row evidence has 1"
            )),
            "Fix: completion audit must reject inflated release suite case counts; blockers={blockers:?}"
        );
        assert!(
            blockers.iter().any(|blocker| blocker.contains(
                "release-workload-matrix.json: cpu_sota_100x_family_count=10, but derived row evidence has 1"
            )),
            "Fix: completion audit must reject inflated 100x family counts; blockers={blockers:?}"
        );
    }
}
