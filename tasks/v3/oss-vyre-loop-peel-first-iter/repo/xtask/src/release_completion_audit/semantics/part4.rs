fn inspect_release_evidence_run_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    const REQUIRED_GENERATORS: &[(&str, &[&str])] = &[
        (
            "version-matrix",
            &["version-matrix.json", "release-tag-plan.json"],
        ),
        ("backend-matrix", &["backend-matrix.json"]),
        ("conformance-matrix", &["conformance-matrix.json"]),
        ("release-workload-matrix", &["release-workload-matrix.json"]),
        (
            "hygiene-matrix",
            &[
                "hygiene-matrix.json",
                "no-stubs-scan.json",
                "no-hidden-fallback-scan.json",
                "resource-bound-scan.json",
                "error-surface-scan.json",
                "cargo-wrapper-scan.json",
                "audit-location-scan.json",
                "public-doc-scan.json",
                "test-hygiene-scan.json",
            ],
        ),
        (
            "test-matrix",
            &[
                "test-matrix.json",
                "modularization-map.json",
                "oversized-test-closure.json",
                "release-surface-suite-coverage.json",
                "unit-suite.json",
                "adversarial-suite.json",
                "property-suite.json",
                "conformance-suite.json",
                "corpus-suite.json",
                "benchmark-suite.json",
                "gap-suite.json",
                "fuzz-suite.json",
            ],
        ),
        (
            "docs-matrix",
            &[
                "docs-matrix.json",
                "vyre-readme-contracts.json",
                "release-notes-version-story.md",
                "cuda-release-path.md",
                "wgpu-fallback-proof.md",
                "megakernel-default-proof.md",
                "optimization-proof.md",
                "egraph-saturation.md",
                "c-parser-linux-proof.md",
                "distributed-parser-coherence.md",
                "weir-integration.md",
                "test-architecture.md",
                "vyre-readme-proof.md",
                "weir-readme-proof.md",
                "parser-doc-proof.md",
                "benchmark-doc-proof.md",
                "conformance-doc-proof.md",
                "release-notes.md",
                "crate-metadata-proof.md",
                "release-hygiene-proof.md",
                "cpu-only-100x-proof.md",
            ],
        ),
        ("metadata-matrix", &["metadata-matrix.json"]),
        ("feature-matrix", &["feature-matrix.json"]),
        (
            "optimization-corpus",
            &[
                "optimization-corpus.json",
                "optimization-corpus-contracts.json",
                "optimization-family-manifest.json",
                "optimization-analysis-fixtures.json",
                "optimization-case-manifest.json",
            ],
        ),
        (
            "optimization-matrix",
            &[
                "optimization-integration-matrix.json",
                "alias-aware-dse.json",
                "alias-aware-stlf.json",
                "alias-aware-licm.json",
                "alias-aware-fusion-fission.json",
                "weir-facts-pass-firing.json",
                "egraph-saturation-matrix.json",
                "egraph-semantic-contracts.json",
            ],
        ),
        (
            "parser-coherence",
            &[
                "distributed-parser-map.json",
                "vyre-frontend-c-contracts.json",
                "vyrec-cli-contracts.json",
                "weir-contracts.json",
                "surgec-contracts.json",
                "surgec-grammar-gen-contracts.json",
            ],
        ),
        (
            "weir-matrix",
            &[
                "weir-analysis-api-matrix.json",
                "weir-vyre-integration-tests.json",
                "weir-readme-contracts.json",
            ],
        ),
    ];

    let command_count = value
        .get("command_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_commands = value
        .get("total_commands")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let schema_version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let successful_commands = value
        .get("successful_commands")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let command_failures = value
        .get("command_failures")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let artifact_failures = value
        .get("artifact_failures")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let blocker_count = value
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    let required_count = value
        .get("required_command_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if schema_version < 2 || command_count < 13 || total_commands < 13 || required_count < 13 {
        blockers.push(format!(
            "{evidence}: schema_version={schema_version}, command_count={command_count}, total_commands={total_commands}, required_command_count={required_count}; structural release evidence must cover every generator with schema>=2"
        ));
    }
    if successful_commands != total_commands || command_failures != 0 || artifact_failures != 0 {
        blockers.push(format!(
            "{evidence}: successful_commands={successful_commands}, total_commands={total_commands}, command_failures={command_failures}, artifact_failures={artifact_failures}; release evidence run must be clean"
        ));
    }
    if blocker_count != 0 {
        blockers.push(format!(
            "{evidence}: release-evidence-run recorded {blocker_count} blocker(s)"
        ));
    }
    let Some(commands) = value.get("commands").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing commands array"));
        return;
    };
    for (required, expected_artifacts) in REQUIRED_GENERATORS {
        let Some(command) = commands.iter().find(|command| {
            command
                .get("args")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|args| {
                    args.first().and_then(serde_json::Value::as_str) == Some(*required)
                })
                && command.get("required").and_then(serde_json::Value::as_bool) == Some(true)
        }) else {
            blockers.push(format!(
                "{evidence}: release-evidence run is missing required generator `{required}` with expected artifacts"
            ));
            continue;
        };
        if command.get("status").and_then(serde_json::Value::as_str) != Some("success") {
            blockers.push(format!(
                "{evidence}: release-evidence generator `{required}` did not report success"
            ));
        }
        let artifacts = command
            .get("expected_artifacts")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let statuses = command
            .get("artifact_statuses")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for expected in *expected_artifacts {
            if !artifacts.iter().any(|artifact| {
                artifact
                    .as_str()
                    .is_some_and(|artifact| artifact.ends_with(expected))
            }) {
                blockers.push(format!(
                    "{evidence}: release-evidence generator `{required}` does not declare expected artifact `{expected}`"
                ));
            }
            let Some(status) = statuses.iter().find(|status| {
                status
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|path| path.ends_with(expected))
            }) else {
                blockers.push(format!(
                    "{evidence}: release-evidence generator `{required}` has no artifact status for `{expected}`"
                ));
                continue;
            };
            let exists = status
                .get("exists")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let bytes = status
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let read_error = status.get("read_error");
            let read_error_is_clean = read_error.is_some_and(serde_json::Value::is_null);
            if !exists || bytes == 0 || !read_error_is_clean {
                blockers.push(format!(
                    "{evidence}: release-evidence generator `{required}` artifact `{expected}` exists={exists} bytes={bytes} read_error={}",
                    read_error
                        .map(serde_json::Value::to_string)
                        .unwrap_or_else(|| "<missing>".to_string())
                ));
            }
        }
    }
}

fn inspect_pass_family_benchmark_manifest_semantics(
    evidence: &str,
    path: &Path,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value.get("backend").and_then(serde_json::Value::as_str) != Some("cuda") {
        blockers.push(format!(
            "{evidence}: backend must be cuda for the release path"
        ));
    }
    let required = value
        .get("required_case_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing cases array"));
        return;
    };
    if cases.len() < required as usize || cases.len() < 4 {
        blockers.push(format!(
            "{evidence}: lists {} optimization benchmark case(s), needs at least {required} and never below 4",
            cases.len()
        ));
    }
    for required_family in REQUIRED_BENCHMARKED_OPTIMIZATION_FAMILIES {
        let covered = value
            .get("covered_pass_families")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|families| {
                families
                    .iter()
                    .any(|family| family.as_str() == Some(required_family))
            });
        if !covered {
            blockers.push(format!(
                "{evidence}: pass-family benchmark manifest does not cover `{required_family}`"
            ));
        }
    }
    if value
        .get("uncovered_pass_families")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|families| !families.is_empty())
    {
        blockers.push(format!(
            "{evidence}: uncovered_pass_families must exist and be empty"
        ));
    }
    for required_case in [
        "lower.rewrites.impact.corpus",
        "foundation.optimizer.impact",
        "lower.egraph_saturation",
        "lower.alias_aware_optimizations",
    ] {
        if !cases.iter().any(|case| {
            case.get("case_id").and_then(serde_json::Value::as_str) == Some(required_case)
                && case.get("exists").and_then(serde_json::Value::as_bool) == Some(true)
                && case
                    .get("read_error")
                    .is_some_and(serde_json::Value::is_null)
                && case
                    .get("required_custom_metrics")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|metrics| !metrics.is_empty())
                && case
                    .get("required_positive_metrics")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|metrics| !metrics.is_empty())
        }) {
            blockers.push(format!(
                "{evidence}: missing complete benchmark manifest entry for `{required_case}`"
            ));
        }
    }
    for case in cases {
        let Some(artifact) = case.get("artifact").and_then(serde_json::Value::as_str) else {
            blockers.push(format!("{evidence}: manifest case is missing artifact"));
            continue;
        };
        if case
            .get("covered_pass_families")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|families| families.is_empty())
        {
            blockers.push(format!(
                "{evidence}: manifest case is missing covered_pass_families"
            ));
        }
        for field in [
            "missing_custom_metrics",
            "non_positive_required_metrics",
            "non_winning_cases",
            "blockers",
        ] {
            if case
                .get(field)
                .and_then(serde_json::Value::as_array)
                .is_none_or(|items| !items.is_empty())
            {
                blockers.push(format!(
                    "{evidence}: manifest case `{}` has non-empty `{field}`",
                    case.get("case_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<unknown>")
                ));
            }
        }
        let read_error = case.get("read_error");
        if !read_error.is_some_and(serde_json::Value::is_null) {
            blockers.push(format!(
                "{evidence}: manifest case `{}` read_error={}",
                case.get("case_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<unknown>"),
                read_error
                    .map(serde_json::Value::to_string)
                    .unwrap_or_else(|| "<missing>".to_string())
            ));
        }
        for field in ["min_wall_samples", "min_baseline_wall_samples"] {
            if case
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                < 30
            {
                blockers.push(format!(
                    "{evidence}: manifest case `{}` has `{field}` below 30",
                    case.get("case_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<unknown>")
                ));
            }
        }
        for field in [
            "min_wall_p50",
            "min_wall_p95",
            "min_wall_p99",
            "min_baseline_wall_p50",
            "min_baseline_wall_p95",
            "min_baseline_wall_p99",
        ] {
            if case
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                blockers.push(format!(
                    "{evidence}: manifest case `{}` has non-positive `{field}`",
                    case.get("case_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<unknown>")
                ));
            }
        }
        let has_speed_win = case
            .get("min_wall_speedup_x1000")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            > 1_000;
        let has_semantic_win = case
            .get("non_winning_cases")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.is_empty());
        if !has_speed_win && !has_semantic_win {
            blockers.push(format!(
                "{evidence}: manifest case `{}` does not prove optimized wall_ns p50 beats baseline_wall_ns p50",
                case.get("case_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<unknown>")
            ));
        }
        let Some(report) = read_referenced_release_json(path, artifact, blockers) else {
            continue;
        };
        let suffix = Path::new(artifact)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(artifact);
        let metrics = case
            .get("required_custom_metrics")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if metrics.is_empty() {
            blockers.push(format!(
                "{evidence}: manifest case `{}` lists no required_custom_metrics",
                case.get("case_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<unknown>")
            ));
        }
        for metric in metrics.iter().filter_map(serde_json::Value::as_str) {
            if !benchmark_report_has_metric(&report, metric) {
                blockers.push(format!(
                    "{evidence}: referenced benchmark `{suffix}` is missing metric `{metric}`"
                ));
            }
        }
        let positive_metrics = case
            .get("required_positive_metrics")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        if positive_metrics.is_empty() {
            blockers.push(format!(
                "{evidence}: manifest case `{}` lists no required_positive_metrics",
                case.get("case_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("<unknown>")
            ));
        }
        for metric in positive_metrics
            .iter()
            .filter_map(serde_json::Value::as_str)
        {
            if !benchmark_report_has_positive_metric(&report, metric) {
                blockers.push(format!(
                    "{evidence}: referenced benchmark `{suffix}` has no positive p50 metric `{metric}`"
                ));
            }
        }
    }
}

const REQUIRED_BENCHMARKED_OPTIMIZATION_FAMILIES: &[&str] = &[
    "algebraic",
    "predicate",
    "egraph",
    "memory-layout",
    "control-flow",
    "vector-layout",
    "A13-coalesce-fixture",
    "A14-shared-mem-promote-fixture",
    "A15-bank-conflict-fixture",
    "A16-vec-pack-fixture",
    "weir-dataflow-dse",
    "weir-dataflow-loop-fusion",
    "weir-dataflow-loop-fission",
    "weir-dataflow-licm",
];
