fn inspect_suite_evidence_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let file_count = value
        .get("file_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if file_count == 0 {
        blockers.push(format!("{evidence}: file_count is zero"));
    }
    let vyre_file_count = value
        .get("vyre_file_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let dataflow_consumer_file_count = value
        .get("dataflow_consumer_file_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let vyrec_file_count = value
        .get("vyrec_file_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if vyre_file_count == 0 {
        blockers.push(format!("{evidence}: vyre_file_count is zero"));
    }
    if dataflow_consumer_file_count == 0 {
        blockers.push(format!("{evidence}: dataflow_consumer_file_count is zero"));
    }
    if vyrec_file_count == 0 {
        blockers.push(format!("{evidence}: vyrec_file_count is zero"));
    }
    let Some(files) = value.get("files").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing files array"));
        return;
    };
    if files.is_empty() {
        blockers.push(format!("{evidence}: files array is empty"));
        return;
    }
    let active_files = files
        .iter()
        .filter(|file| {
            file.get("has_test_entrypoint")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || file
                    .get("assertion_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    > 0
                || file
                    .get("layers")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|layers| {
                        layers
                            .iter()
                            .any(|layer| layer.as_str() == Some("benchmark"))
                    })
        })
        .count();
    if active_files == 0 {
        blockers.push(format!(
            "{evidence}: suite has no assertion-bearing, entrypoint-bearing, or benchmark file"
        ));
    }
}

fn is_before_after_benchmark_evidence(evidence: &str) -> bool {
    [
        "lower-rewrite-impact-before-after.json",
        "optimizer-impact-cuda.json",
        "pass-family-benchmarks.json",
        "egraph-before-after.json",
        "alias-aware-before-after.json",
    ]
    .iter()
    .any(|suffix| evidence.ends_with(suffix))
}

fn inspect_before_after_benchmark_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value
        .get("selected_backend")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|backend| backend != "cuda")
    {
        blockers.push(format!("{evidence}: selected_backend must be cuda"));
    }
    if evidence.ends_with("cpu-only-100x-proof.json") {
        if value
            .get("source_fingerprint")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        {
            blockers.push(format!(
                "{evidence}: aggregate proof must preserve source_fingerprint"
            ));
        }
        if value.get("git").is_none_or(serde_json::Value::is_null) {
            blockers.push(format!(
                "{evidence}: aggregate proof must preserve git provenance object"
            ));
        }
        let contract_case_count = value
            .get("cpu_sota_100x_contract_case_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if contract_case_count < 10 {
            blockers.push(format!(
                "{evidence}: cpu_sota_100x_contract_case_count is {contract_case_count}; needs at least 10"
            ));
        }
        let passing_case_count = value
            .get("cpu_sota_100x_passing_case_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if passing_case_count < 10 {
            blockers.push(format!(
                "{evidence}: cpu_sota_100x_passing_case_count is {passing_case_count}; needs at least 10"
            ));
        }
        let min_wall_samples = value
            .get("min_wall_samples")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if min_wall_samples < 30 {
            blockers.push(format!(
                "{evidence}: min_wall_samples is {min_wall_samples}; needs at least 30"
            ));
        }
        let min_baseline_wall_samples = value
            .get("min_baseline_wall_samples")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if min_baseline_wall_samples < 30 {
            blockers.push(format!(
                "{evidence}: min_baseline_wall_samples is {min_baseline_wall_samples}; needs at least 30"
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
            if value
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                blockers.push(format!(
                    "{evidence}: aggregate proof has non-positive `{field}`"
                ));
            }
        }
    }
    let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing cases array"));
        return;
    };
    if cases.is_empty() {
        blockers.push(format!("{evidence}: cases array is empty"));
        return;
    }
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
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
        let egraph_quality_win = evidence.ends_with("egraph-before-after.json")
            && metrics
                .and_then(|metrics| {
                    metric_p50(metrics.get("egraph_output_ops"))
                        .zip(metric_p50(metrics.get("egraph_baseline_ops_after")))
                })
                .is_some_and(|(output, baseline)| output < baseline)
            && metrics
                .and_then(|metrics| metric_p50(metrics.get("egraph_applied_rewrites")))
                .is_some_and(|rewrites| rewrites > 0.0)
            && metrics
                .and_then(|metrics| metric_p50(metrics.get("egraph_bitwise_case_count")))
                .is_some_and(|cases| cases >= 192.0)
            && metrics
                .and_then(|metrics| metric_p50(metrics.get("egraph_boolean_case_count")))
                .is_some_and(|cases| cases >= 128.0);
        if evidence.ends_with("alias-aware-before-after.json") {
            for metric in [
                "alias_pass_wins",
                "alias_fact_count",
                "alias_cross_binding_fact_count",
                "reaching_def_fact_count",
            ] {
                if !metrics
                    .and_then(|metrics| metric_p50(metrics.get(metric)))
                    .is_some_and(|value| value > 0.0)
                {
                    blockers.push(format!(
                        "{evidence}: case `{id}` must include positive p50 `{metric}`"
                    ));
                }
            }
        }
        match (wall, baseline) {
            (Some(wall), Some(baseline)) if wall < baseline => {}
            (Some(_), Some(_)) if egraph_quality_win => {}
            (Some(_), Some(_)) if before_after_semantic_win(id, metrics) => {}
            (Some(wall), Some(baseline)) => blockers.push(format!(
                "{evidence}: case `{id}` did not improve p50 wall time: wall={wall:.2}, baseline={baseline:.2}"
            )),
            _ => blockers.push(format!(
                "{evidence}: case `{id}` must include p50 wall_ns and baseline_wall_ns"
            )),
        }
    }
}

fn before_after_semantic_win(
    case_id: &str,
    metrics: Option<&serde_json::Map<String, serde_json::Value>>,
) -> bool {
    let Some(metrics) = metrics else {
        return false;
    };
    match case_id {
        "lower.rewrites.impact.corpus" => {
            metric_p50(metrics.get("lower_ops_eliminated")).is_some_and(|value| value > 0.0)
                || metric_p50(metrics.get("lower_optimized_issue_score"))
                    .zip(metric_p50(metrics.get("lower_baseline_issue_score")))
                    .is_some_and(|(optimized, baseline)| optimized < baseline)
        }
        "foundation.optimizer.impact" => {
            metric_p50(metrics.get("optimizer_nodes_eliminated")).is_some_and(|value| value > 0.0)
        }
        "lower.egraph_saturation" => {
            metric_p50(metrics.get("egraph_applied_rewrites")).is_some_and(|value| value > 0.0)
                && metric_p50(metrics.get("egraph_output_ops"))
                    .zip(metric_p50(metrics.get("egraph_baseline_ops_after")))
                    .is_some_and(|(output, baseline)| output < baseline)
        }
        "lower.alias_aware_optimizations" => {
            metric_p50(metrics.get("alias_pass_wins")).is_some_and(|value| value >= 5.0)
        }
        _ => false,
    }
}

fn inspect_release_tag_plan_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
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
        if value.get(field).and_then(serde_json::Value::as_str) != Some(expected) {
            blockers.push(format!("{evidence}: {field} must be `{expected}`"));
        }
    }
    let order = value
        .get("tag_creation_order")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in [
        "vyre-v0.6.3-rc.1",
        "weir-v0.1.0-rc.1",
        "vyre-0.6.3-weir-0.1.0-rc.1",
        "vyre-v0.6.3",
        "weir-v0.1.0",
        "vyre-0.6.3-weir-0.1.0",
    ] {
        if !order.iter().any(|entry| entry.as_str() == Some(required)) {
            blockers.push(format!(
                "{evidence}: tag_creation_order is missing `{required}`"
            ));
        }
    }
    let ordered_tags = order
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    for (rc, final_tag) in [
        ("vyre-v0.6.3-rc.1", "vyre-v0.6.3"),
        ("weir-v0.1.0-rc.1", "weir-v0.1.0"),
        ("vyre-0.6.3-weir-0.1.0-rc.1", "vyre-0.6.3-weir-0.1.0"),
    ] {
        let rc_index = ordered_tags.iter().position(|tag| *tag == rc);
        let final_index = ordered_tags.iter().position(|tag| *tag == final_tag);
        if !matches!((rc_index, final_index), (Some(left), Some(right)) if left < right) {
            blockers.push(format!(
                "{evidence}: tag_creation_order must list `{rc}` before `{final_tag}`"
            ));
        }
    }
    if !value
        .get("required_gate_before_rc_tag")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|command| {
            command.contains("version-matrix")
                && command.contains("release-completion-audit")
                && command.contains("vyre-release-gate")
                && command.contains("scripts/apply-branch-protection.sh")
                && command.contains("cargo_full")
        })
    {
        blockers.push(format!(
            "{evidence}: required_gate_before_rc_tag must include version matrix, completion audit, release gate, branch-protection application, and cargo_full"
        ));
    }
    if !value
        .get("required_gate_before_tag")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|command| {
            command.contains("version-matrix")
                && command.contains("release-completion-audit")
                && command.contains("vyre-release-gate")
                && command.contains("scripts/apply-branch-protection.sh")
                && command.contains("cargo_full")
        })
    {
        blockers.push(format!(
            "{evidence}: required_gate_before_tag must include version matrix, completion audit, release gate, branch-protection application, and cargo_full"
        ));
    }
    let version_blockers = value
        .get("version_matrix_blocker_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    if version_blockers != 0 {
        blockers.push(format!(
            "{evidence}: version_matrix_blocker_count is {version_blockers}, expected zero"
        ));
    }
}

fn inspect_feature_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let missing_required = value
        .get("missing_required_release_packages")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required != 0 {
        blockers.push(format!(
            "{evidence}: missing_required_release_packages has {missing_required} entrie(s), expected zero"
        ));
    }
    let Some(packages) = value.get("packages").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing packages array"));
        return;
    };
    if !packages
        .iter()
        .any(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some("vyrec"))
    {
        blockers.push(format!("{evidence}: missing package `vyrec`"));
    }
    for package in ["vyre", "vyre-driver-cuda", "vyre-driver-wgpu"] {
        let Some(entry) = packages
            .iter()
            .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(package))
        else {
            blockers.push(format!("{evidence}: missing package `{package}`"));
            continue;
        };
        if entry
            .get("default_feature_members")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|members| !members.is_empty())
        {
            blockers.push(format!(
                "{evidence}: package `{package}` default feature set must be empty"
            ));
        }
    }
    for (package, required_features) in [
        ("vyre-driver-cuda", &["cuda"][..]),
        ("vyre-driver-wgpu", &["wgpu"][..]),
        ("weir", &["default", "serde"][..]),
    ] {
        let Some(entry) = packages
            .iter()
            .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some(package))
        else {
            blockers.push(format!("{evidence}: missing package `{package}`"));
            continue;
        };
        let features = entry
            .get("features")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        for required in required_features {
            if !features
                .iter()
                .any(|feature| feature.as_str() == Some(*required))
            {
                blockers.push(format!(
                    "{evidence}: package `{package}` missing feature `{required}`"
                ));
            }
        }
    }
    let Some(vyre) = packages
        .iter()
        .find(|entry| entry.get("name").and_then(serde_json::Value::as_str) == Some("vyre"))
    else {
        return;
    };
    let features = vyre
        .get("features")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in ["cuda", "wgpu"] {
        if !features
            .iter()
            .any(|feature| feature.as_str() == Some(required))
        {
            blockers.push(format!(
                "{evidence}: top-level vyre crate missing feature `{required}`"
            ));
        }
    }
}

