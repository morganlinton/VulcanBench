use super::*;

#[test]
fn ast_to_pg_nodes_has_zero_ulp_tolerance() {
    assert_eq!(
        vyre_harness::OpEntry::tolerance_for_id(entry().id),
        0,
        "Fix: integer PG layout should not allow ULP drift"
    );
}

#[test]
fn ast_to_pg_nodes_adversarial_fixtures_cpu_parity() {
    for (case_idx, vast) in adversarial_vast_cases().iter().enumerate() {
        let node_count = node_count_from_vast(vast);
        let expected = reference_ast_to_pg_nodes(vast);
        let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(node_count), "out_pg_nodes");
        let actual = run_reference_eval(&program, std::slice::from_ref(vast));
        assert_eq!(
            actual,
            vec![expected],
            "Fix: adversarial fixture {case_idx} must match reference"
        );
    }
}

#[test]
fn ast_to_pg_nodes_gpu_dispatch_matches_cpu_for_witness() {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    let backend =
        BACKEND.get_or_init(|| WgpuBackend::acquire().expect("Fix: GPU backend must be available"));

    for (case_idx, vast) in adversarial_vast_cases().into_iter().take(4).enumerate() {
        let node_count = node_count_from_vast(&vast);
        let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(node_count), "out_pg_nodes");
        let optimized = optimize(program.clone());
        let expected = run_reference_eval(&program, std::slice::from_ref(&vast));
        let actual = backend
            .dispatch(&optimized, &[vast], &DispatchConfig::default())
            .unwrap_or_else(|error| {
                panic!("Fix: case {case_idx} GPU dispatch must succeed as {error}")
            });

        assert_eq!(
            actual, expected,
            "Fix: case {case_idx} must match CPU reference"
        );
    }
}

fn bounded_node_strategy() -> impl Strategy<Value = (u32, u32, u32, u32, u32, u32)> {
    prop_oneof![
        (
            Just(node_kind::VARIABLE),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
        (
            Just(node_kind::CALL),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
        (
            Just(node_kind::IMPORT),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
        (
            Just(node_kind::LITERAL),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
        (
            Just(node_kind::SSA),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
        (
            Just(node_kind::BASIC_BLOCK),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>()
        ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn ast_to_pg_nodes_proptest_cpu_reference_matches(row_nodes in prop::collection::vec(bounded_node_strategy(), 1..64)) {
        let vast = build_vast(&row_nodes.iter().map(|(kind, parent, span_start, span_len, attr_off, attr_len)| {
            build_vast_node(*kind, *parent, *span_start, *span_len, *attr_off, *attr_len)
        }).collect::<Vec<_>>());

        let expected = reference_ast_to_pg_nodes(&vast);
        let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(node_count_from_vast(&vast)), "out_pg_nodes");
        let actual = run_reference_eval(&program, std::slice::from_ref(&vast));
        prop_assert_eq!(actual, vec![expected]);
    }
}
