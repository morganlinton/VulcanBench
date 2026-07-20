#[test]
fn release_matrix_does_not_attach_condition_eval_to_specialized_workloads() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    for family_id in [
        "metadata-conditions",
        "offset-count-aggregation",
        "entropy-window",
    ] {
        let family = matrix
            .families
            .iter()
            .find(|family| family.id == family_id)
            .unwrap_or_else(|| panic!("Fix: release matrix missing family `{family_id}`."));
        assert!(
            !family
                .matched_cases
                .iter()
                .any(|case| case == "conditions.yara_like.eval.1m"),
            "Fix: workload `{family_id}` must not inherit the generic condition-eval release case."
        );
        assert!(
            !family
                .cpu_sota_100x_cases
                .iter()
                .any(|case| case == "conditions.yara_like.eval.1m"),
            "Fix: workload `{family_id}` must not count generic condition-eval as its CPU-SOTA 100x proof case."
        );
    }
}

#[test]
fn release_matrix_does_not_attach_parser_pipeline_to_c_ast_workload() {
    let registry = vyre_bench::registry::collect_all();
    let matrix = vyre_bench::release_matrix::build_release_matrix(&registry);
    let family = matrix
        .families
        .iter()
        .find(|family| family.id == "c-ast-traversal")
        .expect("Fix: release matrix missing C AST traversal family.");

    assert!(
        !family
            .matched_cases
            .iter()
            .any(|case| case == "frontend.c.parser.linux_driver_pipeline"),
        "Fix: C AST traversal workload must not inherit the broad parser pipeline benchmark."
    );
    assert!(
        !family
            .cpu_sota_100x_cases
            .iter()
            .any(|case| case == "frontend.c.parser.linux_driver_pipeline"),
        "Fix: C AST traversal workload must not count the broad parser pipeline as its CPU-SOTA 100x proof case."
    );
    assert_eq!(
        family.max_cpu_sota_min_speedup_x,
        Some(100.0),
        "Fix: C AST traversal workload max CPU-SOTA speedup must come from the canonical release case, not parser pipeline evidence."
    );
}
