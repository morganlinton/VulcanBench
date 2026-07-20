fn inspect_optimization_family_manifest_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some(families) = value.get("families").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing families array"));
        return;
    };
    if families.len() < 14 {
        blockers.push(format!(
            "{evidence}: lists {} optimization families; needs at least 14 required release families",
            families.len()
        ));
    }
    let declared_required = value
        .get("required_family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if declared_required < 14 {
        blockers.push(format!(
            "{evidence}: declares {declared_required} required optimization families; needs all 14 release families"
        ));
    }
    let missing_required = value
        .get("missing_required_families")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if missing_required != 0 {
        blockers.push(format!(
            "{evidence}: missing_required_families reports {missing_required} missing required optimization family/families"
        ));
    }
    inspect_duplicate_optimization_family_rows(evidence, value, blockers);
    for family in families {
        let name = family
            .get("family")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if family
            .get("cases")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: optimization family `{name}` has zero generated cases"
            ));
        }
    }
    for required in [
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
    ] {
        let required_cases = families
            .iter()
            .find(|family| {
                family.get("family").and_then(serde_json::Value::as_str) == Some(required)
            })
            .and_then(|family| family.get("cases").and_then(serde_json::Value::as_u64))
            .unwrap_or(0);
        if required_cases < 128 {
            blockers.push(format!(
                "{evidence}: required optimization family `{required}` has {required_cases} generated case(s), needs at least 128"
            ));
        }
    }
}

fn inspect_duplicate_optimization_family_rows(
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
            "{evidence}: duplicate optimization family rows: {duplicates}"
        ));
    }
}

fn inspect_optimization_case_manifest_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let pass_instances = value
        .get("pass_instance_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let generated_cases = value
        .get("generated_cases")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let unique_case_ids = value
        .get("unique_case_ids")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if pass_instances < 4_096 {
        blockers.push(format!(
            "{evidence}: pass_instance_count {pass_instances} is below release floor 4096"
        ));
    }
    if generated_cases != pass_instances {
        blockers.push(format!(
            "{evidence}: generated_cases {generated_cases} does not match pass_instance_count {pass_instances}"
        ));
    }
    if unique_case_ids != pass_instances {
        blockers.push(format!(
            "{evidence}: unique_case_ids {unique_case_ids} does not match pass_instance_count {pass_instances}"
        ));
    }
    let duplicate_case_ids = value
        .get("duplicate_case_ids")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if duplicate_case_ids != 0 {
        blockers.push(format!(
            "{evidence}: duplicate_case_ids contains {duplicate_case_ids} duplicate id(s)"
        ));
    }
    let family_count = value
        .get("family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let required_family_count = value
        .get("required_family_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(12);
    if family_count < required_family_count || family_count < 12 {
        blockers.push(format!(
            "{evidence}: family_count {family_count} is below required family count {required_family_count}"
        ));
    }
    let Some(entries) = value.get("entries").and_then(serde_json::Value::as_array) else {
        blockers.push(format!("{evidence}: missing entries array"));
        return;
    };
    if entries.len() as u64 != pass_instances {
        blockers.push(format!(
            "{evidence}: entries array has {} entries, pass_instance_count is {pass_instances}",
            entries.len()
        ));
    }
    inspect_duplicate_optimization_case_entry_ids(evidence, value, blockers);
    for field in [
        "cases_with_child_bodies",
        "cases_with_bindings",
        "cases_with_literals",
    ] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!("{evidence}: `{field}` must be nonzero"));
        }
    }
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unknown>");
        if entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
            || entry
                .get("family")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
        {
            blockers.push(format!(
                "{evidence}: case manifest entry `{id}` is missing id or family"
            ));
        }
        if entry
            .get("total_ops")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: case manifest entry `{id}` has zero total_ops"
            ));
        }
    }
}

fn inspect_duplicate_optimization_case_entry_ids(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let duplicates =
        crate::benchmark_evidence_semantics::duplicate_nonblank_object_array_field_values(
            value, "entries", "id",
        );
    if !duplicates.is_empty() {
        let duplicates = duplicates.into_iter().collect::<Vec<_>>().join(", ");
        blockers.push(format!(
            "{evidence}: duplicate optimization case entry ids: {duplicates}"
        ));
    }
}

fn is_test_suite_evidence(evidence: &str) -> bool {
    [
        "unit-suite.json",
        "adversarial-suite.json",
        "property-suite.json",
        "conformance-suite.json",
        "corpus-suite.json",
        "benchmark-suite.json",
        "gap-suite.json",
        "fuzz-suite.json",
    ]
    .iter()
    .any(|suffix| evidence.ends_with(suffix))
}

#[cfg(test)]
mod part6_tests {
    use super::*;

    #[test]
    fn completion_audit_rejects_duplicate_optimization_family_rows() {
        let family_manifest = serde_json::json!({
            "families": [
                {"family": "algebraic", "cases": 128},
                {"family": "algebraic", "cases": 128}
            ]
        });
        let mut blockers = Vec::new();

        inspect_optimization_family_manifest_semantics(
            "optimization-family-manifest.json",
            &family_manifest,
            &mut blockers,
        );

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("duplicate optimization family rows: algebraic")),
            "Fix: completion audit must reject duplicate optimization family manifest rows; blockers={blockers:?}"
        );
    }

    #[test]
    fn completion_audit_rejects_duplicate_optimization_case_entry_ids() {
        let case_manifest = serde_json::json!({
            "pass_instance_count": 4096,
            "generated_cases": 4096,
            "unique_case_ids": 4096,
            "duplicate_case_ids": [],
            "family_count": 14,
            "required_family_count": 14,
            "entries": [
                {
                    "id": "optimization.algebraic.0001",
                    "family": "algebraic",
                    "total_ops": 3
                },
                {
                    "id": "optimization.algebraic.0001",
                    "family": "predicate",
                    "total_ops": 5
                }
            ],
            "cases_with_child_bodies": 1,
            "cases_with_bindings": 1,
            "cases_with_literals": 1
        });
        let mut blockers = Vec::new();

        inspect_optimization_case_manifest_semantics(
            "optimization-case-manifest.json",
            &case_manifest,
            &mut blockers,
        );

        assert!(
            blockers.iter().any(|blocker| {
                blocker.contains("duplicate optimization case entry ids: optimization.algebraic.0001")
            }),
            "Fix: completion audit must reject duplicate optimization case manifest entry ids even when unique_case_ids and duplicate_case_ids claim clean evidence; blockers={blockers:?}"
        );
    }
}

fn inspect_oversized_test_closure_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    if value.get("closed").and_then(serde_json::Value::as_bool) != Some(true) {
        blockers.push(format!("{evidence}: oversized test closure must be closed"));
    }
    if value
        .get("total_oversized_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX)
        != 0
    {
        blockers.push(format!("{evidence}: total_oversized_files must be zero"));
    }
    if value
        .get("total_god_test_candidates")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX)
        != 0
    {
        blockers.push(format!(
            "{evidence}: total_god_test_candidates must be zero"
        ));
    }
    if value
        .get("required_split_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::MAX)
        != 0
    {
        blockers.push(format!("{evidence}: required_split_count must be zero"));
    }
    if !value
        .get("oversized_files")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        blockers.push(format!(
            "{evidence}: oversized_files must be an empty array"
        ));
    }
    if !value
        .get("god_test_candidates")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        blockers.push(format!(
            "{evidence}: god_test_candidates must be an empty array"
        ));
    }
}

fn inspect_test_matrix_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let test_files = value
        .get("test_files")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if test_files == 0 {
        blockers.push(format!("{evidence}: test_files is zero"));
    }
    for (field, label) in [
        ("vyre_test_files", "Vyre"),
        ("weir_test_files", "Weir"),
        ("vyrec_test_files", "tools/vyrec"),
    ] {
        if value
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: {label} release-surface test file count is zero"
            ));
        }
    }
    let layers = value
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for required in [
        "unit",
        "integration",
        "property",
        "adversarial",
        "corpus",
        "benchmark",
        "conformance",
        "gap",
        "fuzz",
    ] {
        if !layers.iter().any(|layer| layer.as_str() == Some(required)) {
            blockers.push(format!("{evidence}: missing `{required}` test layer"));
        }
    }
    if value
        .get("oversized_files")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|files| !files.is_empty())
    {
        blockers.push(format!(
            "{evidence}: oversized_files must exist and be empty"
        ));
    }
    if value
        .get("god_test_candidates")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|files| !files.is_empty())
    {
        blockers.push(format!(
            "{evidence}: god_test_candidates must exist and be empty"
        ));
    }
    inspect_surface_entries(evidence, value.get("surface_coverages"), blockers);
}

fn inspect_surface_coverage_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    inspect_top_level_blockers(evidence, value, blockers);
    inspect_surface_entries(evidence, value.get("surfaces"), blockers);
}

fn inspect_top_level_blockers(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let blocker_count = value
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .map_or(usize::MAX, Vec::len);
    if blocker_count != 0 {
        blockers.push(format!("{evidence}: reports {blocker_count} blocker(s)"));
    }
}

fn inspect_surface_entries(
    evidence: &str,
    maybe_surfaces: Option<&serde_json::Value>,
    blockers: &mut Vec<String>,
) {
    let Some(surfaces) = maybe_surfaces.and_then(serde_json::Value::as_array) else {
        blockers.push(format!(
            "{evidence}: missing release surface coverage array"
        ));
        return;
    };
    if surfaces.len() != 3 {
        blockers.push(format!(
            "{evidence}: release surface coverage must contain exactly Vyre, Weir, and tools/vyrec"
        ));
    }
    for required_surface in ["vyre", "weir", "vyrec"] {
        let Some(surface) = surfaces.iter().find(|surface| {
            surface.get("surface").and_then(serde_json::Value::as_str) == Some(required_surface)
        }) else {
            blockers.push(format!(
                "{evidence}: missing `{required_surface}` release surface coverage"
            ));
            continue;
        };
        if surface
            .get("file_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: `{required_surface}` release surface has zero test files"
            ));
        }
        if surface
            .get("assertion_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: `{required_surface}` release surface has zero assertions"
            ));
        }
        if surface
            .get("entrypoint_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            == 0
        {
            blockers.push(format!(
                "{evidence}: `{required_surface}` release surface has zero executable test entrypoints"
            ));
        }
        let missing_layers = surface
            .get("missing_layers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if missing_layers != 0 {
            blockers.push(format!(
                "{evidence}: `{required_surface}` release surface reports {missing_layers} missing test layer(s)"
            ));
        }
        let blockers_count = surface
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .map_or(usize::MAX, Vec::len);
        if blockers_count != 0 {
            blockers.push(format!(
                "{evidence}: `{required_surface}` release surface reports {blockers_count} blocker(s)"
            ));
        }
    }
}

fn inspect_modularization_map_semantics(
    evidence: &str,
    value: &serde_json::Value,
    blockers: &mut Vec<String>,
) {
    let Some(directories) = value
        .get("directories")
        .and_then(serde_json::Value::as_array)
    else {
        blockers.push(format!("{evidence}: missing directories array"));
        return;
    };
    if directories.is_empty() {
        blockers.push(format!("{evidence}: directories array is empty"));
    }
    for required_surface in ["vyre", "weir", "vyrec"] {
        if !directories.iter().any(|directory| {
            directory.get("surface").and_then(serde_json::Value::as_str) == Some(required_surface)
        }) {
            blockers.push(format!(
                "{evidence}: modularization map is missing `{required_surface}` surface directories"
            ));
        }
    }
    for directory in directories {
        if directory.get("exists").and_then(serde_json::Value::as_bool) != Some(true) {
            let path = directory
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>");
            blockers.push(format!("{evidence}: modular directory `{path}` is missing"));
        }
    }
}
