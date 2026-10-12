pub(crate) fn check_hygiene_release_surface_coverage(
    requirement_id: &str,
    matrix: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let Some(coverage) = matrix.get("release_surface_coverage") else {
        failures.push(format!(
            "requirement `{requirement_id}` hygiene matrix is missing release_surface_coverage"
        ));
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
            failures.push(format!(
                "requirement `{requirement_id}` hygiene release_surface_coverage.{field} must be true"
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
                failures.push(format!(
                    "requirement `{requirement_id}` hygiene release_surface_coverage.{field} is missing `{required_value}`"
                ));
            }
        }
    }
}
pub(crate) fn check_release_surface_coverage(
    requirement: &Requirement,
    matrix: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    let Some(surfaces) = matrix
        .get("surface_coverages")
        .and_then(serde_json::Value::as_array)
    else {
        failures.push(format!(
            "requirement `{}` test matrix is missing release surface coverage",
            requirement.id
        ));
        return;
    };
    if surfaces.len() != 3 {
        failures.push(format!(
            "requirement `{}` test matrix must report exactly Vyre, Weir, and tools/vyrec surface coverage",
            requirement.id
        ));
    }
    for surface_id in ["vyre", "weir", "vyrec"] {
        let Some(surface) = surfaces.iter().find(|surface| {
            surface.get("surface").and_then(serde_json::Value::as_str) == Some(surface_id)
        }) else {
            failures.push(format!(
                "requirement `{}` test matrix is missing `{surface_id}` surface coverage",
                requirement.id
            ));
            continue;
        };
        for (field, label) in [
            ("file_count", "test files"),
            ("assertion_count", "assertions"),
            ("entrypoint_count", "test entrypoints"),
        ] {
            if surface
                .get(field)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
                == 0
            {
                failures.push(format!(
                    "requirement `{}` `{surface_id}` release surface has zero {label}",
                    requirement.id
                ));
            }
        }
        let missing_layers = surface
            .get("missing_layers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if missing_layers != 0 {
            failures.push(format!(
                "requirement `{}` `{surface_id}` release surface reports {missing_layers} missing test layer(s)",
                requirement.id
            ));
        }
        let blockers = surface
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if blockers != 0 {
            failures.push(format!(
                "requirement `{}` `{surface_id}` release surface reports {blockers} blocker(s)",
                requirement.id
            ));
        }
    }
}
pub(crate) fn check_release_evidence_run(
    requirement: &Requirement,
    run: &serde_json::Value,
    failures: &mut Vec<String>,
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

    let total = run
        .get("total_commands")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let schema_version = run
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let successful = run
        .get("successful_commands")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let required = run
        .get("required_command_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let command_failures = run
        .get("command_failures")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let artifact_failures = run
        .get("artifact_failures")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX);
    let blockers = run
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if schema_version < 2
        || total < 13
        || required < 13
        || successful != total
        || command_failures != 0
        || artifact_failures != 0
        || blockers != 0
    {
        failures.push(format!(
            "requirement `{}` release-evidence-run must be schema>=2 and clean: schema_version={schema_version}, total={total}, required={required}, successful={successful}, command_failures={command_failures}, artifact_failures={artifact_failures}, blockers={blockers}",
            requirement.id
        ));
    }

    let commands = run
        .get("commands")
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    for (generator, expected_artifacts) in REQUIRED_GENERATORS {
        let Some(command) = commands.iter().find(|command| {
            command
                .get("args")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|args| {
                    args.iter()
                        .any(|arg| arg.as_str().is_some_and(|arg| arg == *generator))
                })
        }) else {
            failures.push(format!(
                "requirement `{}` release-evidence-run is missing generator `{generator}`",
                requirement.id
            ));
            continue;
        };

        let status = command
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if status != "success" {
            failures.push(format!(
                "requirement `{}` release-evidence-run generator `{generator}` status is `{status}`, expected `success`",
                requirement.id
            ));
        }

        let artifacts = command
            .get("expected_artifacts")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        let artifact_statuses = command
            .get("artifact_statuses")
            .and_then(serde_json::Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for expected in *expected_artifacts {
            if !artifacts.iter().any(|artifact| {
                artifact
                    .as_str()
                    .is_some_and(|artifact| artifact.ends_with(expected))
            }) {
                failures.push(format!(
                    "requirement `{}` release-evidence-run generator `{generator}` does not declare expected artifact `{expected}`",
                    requirement.id
                ));
            }
            let Some(status) = artifact_statuses.iter().find(|status| {
                status
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|path| path.ends_with(expected))
            }) else {
                failures.push(format!(
                    "requirement `{}` release-evidence-run generator `{generator}` has no artifact status for `{expected}`",
                    requirement.id
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
                failures.push(format!(
                    "requirement `{}` release-evidence-run generator `{generator}` artifact `{expected}` exists={exists} bytes={bytes} read_error={}",
                    requirement.id,
                    read_error
                        .map(serde_json::Value::to_string)
                        .unwrap_or_else(|| "<missing>".to_string())
                ));
            }
        }
    }
}
