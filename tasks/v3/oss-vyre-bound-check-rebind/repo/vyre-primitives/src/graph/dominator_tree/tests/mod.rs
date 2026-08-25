use super::*;

#[test]
fn program_builds_without_panic() {
    let p = dominator_tree_program(4, 4, 4, "idom");
    assert_eq!(p.workgroup_size, [1, 1, 1]);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert!(names.contains(&"idom"));
    assert!(names.contains(&"dt_depth"));
}

#[test]
fn program_exposes_chk_phases_as_child_regions() {
    use vyre_foundation::ir::Node;

    fn collect(nodes: &[Node], generators: &mut Vec<String>) {
        for node in nodes {
            match node {
                Node::Region {
                    generator,
                    source_region,
                    body,
                } => {
                    if source_region.is_some() {
                        generators.push(generator.as_str().to_string());
                    }
                    collect(body.as_ref(), generators);
                }
                Node::Block(children) => collect(children, generators),
                Node::If {
                    then, otherwise, ..
                } => {
                    collect(then, generators);
                    collect(otherwise, generators);
                }
                Node::Loop { body, .. } => collect(body, generators),
                _ => {}
            }
        }
    }

    let p = dominator_tree_program(4, 4, 4, "idom");
    let mut generators = Vec::new();
    collect(p.entry(), &mut generators);

    assert!(
        generators.iter().any(|g| g == "vyre-primitives::graph::dominator_tree::init_state")
            && generators
                .iter()
                .any(|g| g == "vyre-primitives::graph::dominator_tree::recompute_depth")
            && generators.iter().any(|g| {
                g == "vyre-primitives::graph::dominator_tree::intersect_predecessors"
            }),
        "Fix: dominator_tree must expose CHK initialization, depth recompute, and predecessor intersection as child regions so LegoGate and print-composition can see real phase boundaries: {generators:?}"
    );
}

#[test]
fn checked_builder_rejects_u32_max_node_count() {
    let err = try_dominator_tree_program(u32::MAX, 0, 0, "idom").unwrap_err();
    assert!(err.contains("u32::MAX collides with IDOM_NONE"));
}

#[test]
fn legacy_builder_returns_inert_trap_on_u32_max() {
    let p = dominator_tree_program(u32::MAX, 0, 0, "idom");
    assert_eq!(p.workgroup_size, [1, 1, 1]);
    assert_eq!(p.buffers.len(), 6);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert!(names.contains(&"idom"));
    use vyre_foundation::ir::Node;
    assert!(
            matches!(
                p.entry.first(),
                Some(Node::Region { body, .. }) if body.len() == 1
            ),
            "Fix: invalid dominator_tree shape must compile to an inert early-return trap, not a full kernel."
        );
}

#[test]
fn empty_graph_returns_empty() {
    let idoms = cpu_ref(0, 0, &[]);
    assert!(idoms.is_empty());
}

#[test]
fn single_node_self_idom() {
    let idoms = cpu_ref(1, 0, &[]);
    assert_eq!(idoms, vec![Some(0)]);
}

#[test]
fn linear_chain_idoms() {
    // 0 -> 1 -> 2 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (1, 2), (2, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(2));
}

#[test]
fn diamond_idoms() {
    // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(0));
    assert_eq!(idoms[3], Some(0));
}

#[test]
fn while_loop_idoms() {
    // 0 -> 1, 1 -> 2, 2 -> 1, 1 -> 3
    let idoms = cpu_ref(4, 0, &[(0, 1), (1, 2), (2, 1), (1, 3)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], Some(1));
    assert_eq!(idoms[3], Some(1));
}

#[test]
fn unreachable_nodes_are_none() {
    // 0 -> 1. 2 and 3 are disconnected.
    let idoms = cpu_ref(4, 0, &[(0, 1)]);
    assert_eq!(idoms[0], Some(0));
    assert_eq!(idoms[1], Some(0));
    assert_eq!(idoms[2], None);
    assert_eq!(idoms[3], None);
}

#[test]
fn lt_matches_chk_on_diamond() {
    let lt = lengauer_tarjan_idoms(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    let chk = cooper_harvey_kennedy_idoms(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(lt, chk);
}

#[test]
fn lt_matches_chk_on_while_loop() {
    let edges = &[(0, 1), (1, 2), (2, 1), (1, 3)];
    let lt = lengauer_tarjan_idoms(4, 0, edges);
    let chk = cooper_harvey_kennedy_idoms(4, 0, edges);
    assert_eq!(lt, chk);
}

#[test]
fn generated_try_lt_matches_chk_on_small_graphs() {
    for case in 0..16384usize {
        let n = 1 + case % 10;
        let mut edges = Vec::new();
        for src in 0..n {
            for dst in 0..n {
                if src != dst && ((src * 17 + dst * 31 + case) % 11) < 3 {
                    edges.push((src as u32, dst as u32));
                }
            }
        }
        let lt = try_lengauer_tarjan_idoms(n as u32, 0, &edges)
                .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated dominator LT oracle should reserve and evaluate");
        let chk = cooper_harvey_kennedy_idoms(n as u32, 0, &edges);

        assert_eq!(lt, chk, "case {case}: LT and CHK idoms diverged");
    }
}

#[test]
fn try_cpu_ref_into_reuses_output_and_workspace() {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&[Some(99); 12]);
    let mut scratch = DominatorTreeCpuScratch::new();
    scratch.reserve_outer_for_test(16, 17);
    let out_capacity = out.capacity();
    let outer_caps = scratch.outer_capacities();

    try_cpu_ref_into(
        4,
        0,
        &[(0, 1), (0, 2), (1, 3), (2, 3)],
        &mut out,
        &mut scratch,
    )
    .expect("Fix: diamond dominator graph must evaluate.");

    assert_eq!(out, vec![Some(0), Some(0), Some(0), Some(0)]);
    assert_eq!(out.capacity(), out_capacity);
    assert_eq!(scratch.outer_capacities(), outer_caps);

    let succ_row_zero_capacity = scratch.test_succ_row_capacity(0);
    let pred_row_one_capacity = scratch.test_pred_row_capacity(1);

    try_cpu_ref_into(2, 0, &[(0, 1)], &mut out, &mut scratch)
        .expect("Fix: second dominator graph must reuse workspace.");

    assert_eq!(out, vec![Some(0), Some(0)]);
    assert_eq!(out.capacity(), out_capacity);
    assert_eq!(
        scratch.test_succ_row(0),
        &[1][..],
        "Fix: workspace reuse must clear stale successor edges from the previous graph."
    );
    assert!(
        scratch.test_pred_row(1).contains(&0),
        "Fix: workspace reuse must rebuild predecessor rows for the second graph."
    );
    assert_eq!(scratch.test_succ_row_capacity(0), succ_row_zero_capacity);
    assert_eq!(scratch.test_pred_row_capacity(1), pred_row_one_capacity);

    try_cpu_ref_into(3, 5, &[(0, 1)], &mut out, &mut scratch)
        .expect("Fix: out-of-range entry should produce all-None idoms.");
    assert_eq!(out, vec![None, None, None]);
    assert_eq!(out.capacity(), out_capacity);
}

#[test]
fn generated_idom_set_conversion_is_sorted_and_includes_self() {
    for case in 0..8192usize {
        let n = 1 + case % 32;
        let edges: Vec<(u32, u32)> = (1..n)
            .map(|node| ((node - 1) as u32, node as u32))
            .collect();
        let idoms = try_cpu_ref(n as u32, 0, &edges)
                .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated dominator CPU oracle should reserve and evaluate");
        let sets = try_idoms_to_dominator_sets(&idoms, n as u32)
                .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated dominator set conversion should reserve and evaluate");

        assert_eq!(sets.len(), n, "case {case}: one set per node");
        for (node, set) in sets.iter().enumerate() {
            assert!(
                set.windows(2).all(|pair| pair[0] < pair[1]),
                "case {case} node {node}: dominator set must be sorted and unique"
            );
            assert!(
                set.contains(&(node as u32)),
                "case {case} node {node}: dominator set must contain the node itself"
            );
        }
    }
}

#[test]
fn validation_rejects_bad_offsets() {
    let err = validate_dominator_tree_inputs(2, &[0, 1], &[0], &[0, 0, 0], &[]).unwrap_err();
    assert!(matches!(err, DominatorTreeError::BadOffsets { .. }));
}

#[test]
fn validation_rejects_oob_target() {
    let err = validate_dominator_tree_inputs(2, &[0, 1, 1], &[5], &[0, 0, 0], &[]).unwrap_err();
    assert!(matches!(
        err,
        DominatorTreeError::TargetOutOfRange { target: 5, .. }
    ));
}

#[test]
fn validation_rejects_non_monotonic_offsets() {
    let err = validate_dominator_tree_inputs(2, &[0, 2, 1], &[0, 0], &[0, 0, 0], &[]).unwrap_err();
    assert!(matches!(
        err,
        DominatorTreeError::NonMonotonicOffsets { .. }
    ));
}

#[test]
fn validation_returns_layout() {
    let layout =
        validate_dominator_tree_inputs(3, &[0, 1, 2, 2], &[1, 2], &[0, 0, 0, 0], &[]).unwrap();
    assert_eq!(layout.node_count, 3);
    assert_eq!(layout.edge_count, 2);
    assert_eq!(layout.pred_edge_count, 0);
}

#[test]
fn dominator_cpu_source_exposes_fallible_oracle_storage() {
    let full_cpu_source = concat!(
        include_str!("../lengauer_tarjan.rs"),
        include_str!("../cooper_harvey_kennedy.rs"),
        include_str!("../cpu_ref.rs"),
    );
    let lt_source = full_cpu_source
        .split("/// Cooper–Harvey–Kennedy iterative immediate dominators")
        .next()
        .expect("Fix: dominator LT source must precede CHK oracle");

    assert!(
        full_cpu_source.contains("pub fn try_cpu_ref(")
            && full_cpu_source.contains("try_idoms_to_dominator_sets")
            && full_cpu_source.contains("crate::graph::scratch::reserve_graph_items")
            && lt_source.contains("pub fn try_lengauer_tarjan_idoms(")
            && !lt_source.contains("fn reserve_dominator_vec")
            && !lt_source.contains("vec![Vec::new(); n]")
            && !lt_source.contains("vec![None; n]"),
        "Fix: dominator CPU oracle must expose fallible allocation paths for large graph parity."
    );
}
