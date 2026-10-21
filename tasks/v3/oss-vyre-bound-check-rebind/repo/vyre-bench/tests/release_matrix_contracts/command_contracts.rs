use super::*;

#[test]
fn release_matrix_reports_no_structural_blockers() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    assert!(
        matrix.blockers.is_empty(),
        "Fix: release workload matrix still has structural blockers: {:?}",
        matrix.blockers
    );
}

#[test]
fn release_matrix_links_workloads_to_artifact_commands() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    for family in matrix
        .families
        .iter()
        .filter(|family| !family.matched_cases.is_empty())
    {
        assert!(
            family
                .evidence_artifact
                .starts_with("release/evidence/benchmarks/workload-"),
            "Fix: workload `{}` must point at a release benchmark evidence artifact, got `{}`.",
            family.id,
            family.evidence_artifact
        );
        let command = family.benchmark_command.as_deref().unwrap_or("");
        assert!(
            command.starts_with("cargo_full ")
                && command.contains("vyre-bench")
                && command.contains("--suite release")
                && command.contains("--backend cuda")
                && command.contains("--enforce-budgets")
                && command.contains(&family.evidence_artifact),
            "Fix: workload `{}` must publish a reproducible CUDA release benchmark command, got `{command}`.",
            family.id
        );
        let artifact_path = workspace_root().join(&family.evidence_artifact);
        assert!(
            artifact_path.exists(),
            "Fix: workload `{}` references missing release evidence artifact `{}`.",
            family.id,
            family.evidence_artifact
        );
    }
}

#[test]
fn release_matrix_commands_prefer_canonical_release_workload_cases() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let expected = [
        ("condition-eval", "release.condition_eval.1m"),
        (
            "offset-count-aggregation",
            "release.offset_count_aggregation.1m",
        ),
        ("entropy-window", "release.entropy_window.1m"),
        ("alias-reaching-def", "release.alias_reaching_def.1m"),
        ("ifds-witness", "release.ifds_witness.1m"),
        ("c-ast-traversal", "release.c_ast_traversal.1m"),
        ("egraph-saturation", "release.egraph_saturation.1m"),
        ("sparse-output-compaction", "sparse.compaction.count.1m"),
        (
            "metadata-conditions",
            "metadata.condition.filesize_header.1m",
        ),
    ];

    for (family_id, case_id) in expected {
        let family = matrix
            .families
            .iter()
            .find(|family| family.id == family_id)
            .unwrap_or_else(|| panic!("Fix: release matrix missing family `{family_id}`."));
        let command = family.benchmark_command.as_deref().unwrap_or("");
        assert!(
            command.contains(&format!("--case {case_id} ")),
            "Fix: workload `{family_id}` command must prefer canonical release case `{case_id}`, got `{command}`."
        );
    }
}

#[test]
fn release_matrix_commands_match_bench_target_case_ids() {
    let targets = bench_targets_manifest();
    let target_rows = bench_target_rows(&targets);
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);

    for family in matrix
        .families
        .iter()
        .filter(|family| family.benchmark_command.is_some())
    {
        let command = family.benchmark_command.as_deref().unwrap_or("");
        for target_id in &family.bench_target_ids {
            let Some(target) = target_rows
                .iter()
                .find(|target| target.get("id").and_then(toml::Value::as_str) == Some(*target_id))
            else {
                panic!(
                    "Fix: BENCH_TARGETS.toml is missing target `{target_id}` for release matrix family `{}`.",
                    family.id
                );
            };
            assert_eq!(
                target.get("suite").and_then(toml::Value::as_str),
                Some("release-workload"),
                "Fix: BENCH_TARGETS target `{target_id}` for family `{}` must be suite=release-workload.",
                family.id
            );
            let bench_case_id = target
                .get("bench_case_id")
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "Fix: BENCH_TARGETS target `{target_id}` for family `{}` must declare bench_case_id.",
                        family.id
                    )
                });
            assert!(
                command.contains(&format!("--case {bench_case_id} ")),
                "Fix: BENCH_TARGETS target `{target_id}` bench_case_id `{bench_case_id}` must match release matrix command `{command}`."
            );
        }
    }
}

#[test]
fn release_matrix_bench_targets_reference_active_release_cases() {
    let targets = bench_targets_manifest();
    let target_rows = bench_target_rows(&targets);
    let registry = vyre_bench::registry::collect_all();

    for target in target_rows.iter().filter(|target| {
        target.get("suite").and_then(toml::Value::as_str) == Some("release-workload")
    }) {
        let target_id = target
            .get("id")
            .and_then(toml::Value::as_str)
            .expect("Fix: every release-workload BENCH_TARGETS target needs an id.");
        let bench_case_id = target
            .get("bench_case_id")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "Fix: release-workload BENCH_TARGETS target `{target_id}` must declare bench_case_id."
                )
            });
        let Some(case) = registry
            .iter()
            .find(|case| case.id().0.as_str() == bench_case_id)
        else {
            panic!(
                "Fix: release-workload BENCH_TARGETS target `{target_id}` references missing bench_case_id `{bench_case_id}`."
            );
        };
        assert!(
            case.active_in_suite(SuiteKind::Release),
            "Fix: release-workload BENCH_TARGETS target `{target_id}` bench_case_id `{bench_case_id}` must be active in the release suite."
        );
    }
}

#[test]
fn release_matrix_covers_all_release_workload_bench_targets() {
    let targets = bench_targets_manifest();
    let target_rows = bench_target_rows(&targets);
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let matrix_targets = matrix
        .families
        .iter()
        .flat_map(|family| family.bench_target_ids.iter().copied())
        .collect::<BTreeSet<_>>();

    for target in target_rows.iter().filter(|target| {
        target.get("suite").and_then(toml::Value::as_str) == Some("release-workload")
    }) {
        let target_id = target
            .get("id")
            .and_then(toml::Value::as_str)
            .expect("Fix: every release-workload BENCH_TARGETS target needs an id.");
        assert!(
            matrix_targets.contains(target_id),
            "Fix: release-workload BENCH_TARGETS target `{target_id}` must be linked from a release matrix family."
        );
    }
}

#[test]
fn release_matrix_committed_evidence_matches_generated_matrix() {
    let workspace = workspace_root();
    let expected_path = workspace.join("release/evidence/benchmarks/release-workload-matrix.json");
    let expected = std::fs::read_to_string(&expected_path)
        .expect("Fix: release-workload-matrix.json must be readable.");
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let generated = format!(
        "{}\n",
        serde_json::to_string_pretty(&matrix)
            .expect("Fix: release workload matrix must serialize as JSON.")
    );

    assert_eq!(
        expected,
        generated,
        "Fix: regenerate `{}` from vyre-bench release-matrix after changing release workload source data.",
        expected_path.display()
    );
}
