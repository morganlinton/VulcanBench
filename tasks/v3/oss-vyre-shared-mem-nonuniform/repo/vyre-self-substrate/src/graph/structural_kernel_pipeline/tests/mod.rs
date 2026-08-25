use super::*;
use vyre_foundation::ir::{Node, Program};
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::graph::{
    knowledge_compile::{AND_NODE, LITERAL_TRUE},
    sum_product_circuit::{KIND_LEAF, KIND_PRODUCT, KIND_SUM},
};

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-8 * (1.0 + a.abs() + b.abs())
}

fn approx_eq_f32(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs() + b.abs())
}

fn program_generator(program: &Program) -> &str {
    let Some(Node::Region { generator, .. }) = program.entry.first() else {
        panic!("Fix: structural graph Program must start with a Region.");
    };
    generator.as_str()
}

#[test]
fn program_builders_emit_expected_structural_primitives() {
    let shape = ProgramGraphShape::new(4, 3);
    assert_eq!(
        program_generator(&dispatch_sum_product(
            "k", "off", "cnt", "ch", "w", "leaf", "out", 3, 2
        )),
        "vyre-primitives::graph::sum_product_evaluate"
    );
    assert_eq!(
        program_generator(&dispatch_matroid_exchange_bfs_step(
            "fin", "adj", "vis", "fout", "changed", 3
        )),
        "vyre-primitives::graph::matroid_exchange_bfs_step"
    );
    assert_eq!(
        program_generator(&dispatch_monoidal_compose("f", "g", "out", 2, 2, 2)),
        "vyre-primitives::graph::monoidal_compose"
    );
    assert_eq!(
        program_generator(&dispatch_tensor_flow_forward(shape, "tin", "tout", 2, 2, 1)),
        "vyre-primitives::graph::tensor_flow_forward"
    );
    assert_eq!(
        program_generator(&dispatch_functor_apply("src", "map", "dst", 3)),
        "vyre-primitives::graph::functor_apply"
    );
    assert_eq!(
        program_generator(&dispatch_persistent_bfs_batch(
            shape, "fin", "fout", "changed", 2, 1, 2
        )),
        "vyre-primitives::graph::persistent_bfs_batch"
    );
    assert_eq!(
        program_generator(&dispatch_dominator_frontier(4, 4, 4, "seed", "out")),
        "vyre-primitives::graph::dominator_frontier"
    );
    assert_eq!(
        program_generator(&dispatch_ddnnf_evaluate(
            "kind", "var", "off", "cnt", "ch", "assign", "out", 3, 2, 2
        )),
        "vyre-primitives::graph::ddnnf_evaluate"
    );
    assert_eq!(
        program_generator(&dispatch_frontier_to_queue(
            "frontier", "queue", "len", 4, 4
        )),
        "vyre-primitives::graph::frontier_to_queue"
    );
    assert_eq!(
        program_generator(&dispatch_csr_queue_forward_traverse(
            "queue", "len", "off", "target", "kind", "out", 4, 3, 4, 1
        )),
        "vyre-primitives::graph::csr_queue_forward_traverse"
    );
    assert_eq!(
        program_generator(&dispatch_csr_backward_traverse(shape, "fin", "fout", 1)),
        "vyre-primitives::graph::csr_backward_traverse"
    );
    assert_eq!(
        program_generator(&dispatch_csr_backward_or_changed_parallel(
            shape, "frontier", "changed", 1
        )),
        "vyre-primitives::graph::csr_backward_or_changed"
    );
    assert_eq!(
        program_generator(&dispatch_chebyshev_filter(
            "l", "x", "c", "y", "scratch", 2, 1
        )),
        "vyre-primitives::graph::chebyshev_filter"
    );
    assert_eq!(
        program_generator(&dispatch_sheaf_diffusion_step("s", "r", "d", "out", 2, 2)),
        "vyre-primitives::graph::sheaf_diffusion_step"
    );
    assert_eq!(
        program_generator(&dispatch_backdoor_descendants_check("z", "d", "v", 4)),
        "vyre-primitives::graph::backdoor_descendants_check"
    );
    assert_eq!(
        program_generator(&dispatch_do_intervention_delete_incoming(
            "a", "m", "out", 2
        )),
        "vyre-primitives::graph::do_intervention_delete_incoming"
    );
    assert_eq!(
        program_generator(&dispatch_do_rule2_reverse_incoming("a", "m", "out", 2)),
        "vyre-primitives::graph::do_rule2_reverse_incoming"
    );
    assert_eq!(
        program_generator(&dispatch_tensor_scc_fixpoint(
            "rows", "seed", "group", "out", 4, 8
        )),
        "vyre-primitives::math::tensor_scc"
    );
}

#[test]
fn composed_programs_and_bodies_are_non_empty() {
    let reach = dispatch_reachable_program(4, 3, "sources", "reach", 2);
    assert!(!reach.buffers().is_empty());
    assert!(!reach.entry().is_empty());

    let ifds = dispatch_build_ifds_csr(1, 2, 2, 1, 0, 1, 0, 4);
    assert_eq!(
        program_generator(&ifds),
        "vyre-primitives::graph::exploded_build_ifds_csr"
    );

    let batched = dispatch_batched_path_reconstruct(3, 4);
    assert_eq!(
        program_generator(&batched),
        "vyre-primitives::graph::batched_path_reconstruct"
    );

    assert!(!emit_find_root_body("parent", "id", "root", "scratch", 4).is_empty());
    assert!(!emit_union_roots_body("parent", "a", "b", "edge", 4).is_empty());
    assert_eq!(
        program_generator(&dispatch_union_find_program("parent", "a", "b", 4, 2)),
        "vyre-primitives::graph::union_find"
    );
}

#[test]
fn checked_builders_reject_bad_shapes_without_panicking() {
    assert!(
        dispatch_sum_product_checked("k", "o", "c", "ch", "w", "l", "out", 0, 0)
            .unwrap_err()
            .contains("n_nodes > 0")
    );
    assert!(
        dispatch_matroid_exchange_bfs_step_checked("f", "a", "v", "o", "c", 0)
            .unwrap_err()
            .contains("n > 0")
    );
    assert!(dispatch_monoidal_compose_checked("f", "g", "o", 0, 1, 1)
        .unwrap_err()
        .contains("a, b, c > 0"));
    assert!(dispatch_persistent_bfs_batch_checked(
        ProgramGraphShape::new(u32::MAX, 3),
        "i",
        "o",
        "c",
        u32::MAX,
        1,
        1,
    )
    .unwrap_err()
    .contains("frontier words overflow"));
    assert!(
        dispatch_ddnnf_evaluate_checked("k", "v", "o", "c", "ch", "a", "out", 0, 0, 1)
            .unwrap_err()
            .contains("n_nodes > 0")
    );
    assert!(
        dispatch_chebyshev_filter_checked("l", "x", "c", "y", "s", 0, 1)
            .unwrap_err()
            .contains("n > 0")
    );
    assert!(
        dispatch_sheaf_diffusion_step_checked("s", "r", "d", "o", 0, 1)
            .unwrap_err()
            .contains("n_nodes > 0")
    );
    assert!(
        dispatch_do_intervention_delete_incoming_checked("a", "m", "o", 0)
            .unwrap_err()
            .contains("n > 0")
    );
    assert!(dispatch_do_rule2_reverse_incoming_checked("a", "m", "o", 0)
        .unwrap_err()
        .contains("n > 0"));
}

#[test]
fn cpu_references_cover_logic_and_category_contracts() {
    let sp = reference_sum_product_evaluate(
        &[KIND_LEAF, KIND_LEAF, KIND_SUM, KIND_PRODUCT],
        &[0, 0, 0, 0],
        &[0, 0, 2, 2],
        &[0, 1, 0, 1],
        &[0.25, 0.75, 0.0, 0.0],
        &[0.6, 0.4, 0.0, 0.0],
        &[0, 1, 2, 3],
    );
    assert!(approx_eq(sp[2], 0.45));
    assert!(approx_eq(sp[3], 0.24));

    assert_eq!(
        reference_matroid_exchange_bfs_step(
            &[1, 0, 0],
            &[0, 1, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0],
            3
        ),
        (vec![0, 1, 0], true)
    );
    assert_eq!(
        reference_monoidal_compose(&[1.0, 2.0, 3.0, 4.0], &[1.0, 0.0, 0.0, 1.0], 2, 2, 2),
        vec![1.0, 2.0, 3.0, 4.0]
    );
    assert_eq!(
        reference_functor_apply(&[10, 20, 30], &[2, 0, 1], 3),
        vec![20, 30, 10]
    );
    assert_eq!(
        reference_ddnnf_evaluate(
            &[(LITERAL_TRUE, 0, 0), (LITERAL_TRUE, 0, 0), (AND_NODE, 0, 2)],
            &[0, 1, 0],
            &[0, 1],
            &[u32::MAX, u32::MAX],
            &[0, 1, 2],
        )[2],
        1
    );
    assert_eq!(
        reference_try_ddnnf_evaluate_cpu(&[(LITERAL_TRUE, 0, 0)], &[0], &[], &[1], &[0]).unwrap(),
        vec![1]
    );
}

#[test]
fn cpu_references_cover_resident_traversal_and_numeric_graphs() {
    assert_eq!(
        reference_frontier_to_queue(&[0b10111], 5, 3),
        (vec![0, 1, 2], 4)
    );
    assert!(
        reference_try_frontier_to_queue(&[0b10111], 64, 3)
            .unwrap_err()
            .contains("frontier_in.len() == bitset_words(node_count)"),
        "structural frontier-to-queue wrapper must preserve primitive width diagnostics"
    );
    assert_eq!(
        reference_csr_queue_forward_traverse(
            &[0, 1],
            2,
            &[0, 2, 3, 3, 3],
            &[1, 2, 3],
            &[1, 2, 1],
            4,
            1,
        ),
        vec![0b1010]
    );
    assert!(
        reference_try_csr_queue_forward_traverse(&[0], 1, &[0, 1, 1], &[4], &[1], 2, 1)
            .unwrap_err()
            .contains("outside node_count"),
        "structural queue traversal wrapper must preserve primitive CSR diagnostics"
    );

    let mut paths = Vec::new();
    let mut lens = Vec::new();
    reference_cpu_ref_batched(&[0, 0, 1, 2], &[3, 0, 2], 4, &mut paths, &mut lens);
    assert_eq!(lens, vec![4, 1, 3]);
    assert_eq!(&paths[0..4], &[3, 2, 1, 0]);

    let mut out = Vec::new();
    let mut t_prev = Vec::new();
    let mut t_curr = Vec::new();
    let mut t_next = Vec::new();
    reference_chebyshev_filter_into(
        &[0.5, 0.0, 0.0, 0.5],
        &[1.0, 1.0],
        &[0.0, 0.0, 1.0],
        2,
        2,
        &mut out,
        &mut t_prev,
        &mut t_curr,
        &mut t_next,
    );
    assert!(approx_eq_f32(out[0], -0.5));
    assert!(approx_eq_f32(out[1], -0.5));

    assert!(reference_backdoor_descendants_check(&[0, 1], &[0, 1]));
    assert_eq!(
        reference_tensor_scc_fixpoint(&[0b0010, 0b0100, 0b0001], 0b0001, 0b0111, 8),
        0b0111
    );
}

#[test]
fn validation_helpers_cover_ifds_dominance_toposort_and_causal_contracts() {
    assert_eq!(reference_ifds_node_count_checked(2, 3, 4), Some(24));
    assert_eq!(reference_max_ifds_col_count(2, 1, 1, 4), Some(14));
    let layout = reference_validate_ifds_csr_layout(1, 2, 2, 1, 0, 1).unwrap();
    assert_eq!(layout.total_nodes, 4);
    assert_eq!(reference_decode_node((1 << 20) | (2 << 10) | 3), (1, 2, 3));

    assert_eq!(
        reference_validate_csr_shape("test", 3, &[0, 1, 1, 1], &[1]).unwrap(),
        1
    );
    assert_eq!(
        reference_toposort_csr(3, &[0, 1, 2, 2], &[1, 2]).unwrap(),
        vec![0, 1, 2]
    );

    assert_eq!(
        reference_do_intervention_delete_incoming(&[1, 2, 3, 4], &[1, 0], 2),
        vec![0, 2, 0, 4]
    );
    assert!(
        reference_try_do_intervention_delete_incoming(&[1], &[1], 2)
            .unwrap_err()
            .contains("adjacency.len() == n*n"),
        "structural intervention wrapper must expose primitive shape errors"
    );
    assert_eq!(
        reference_do_rule2_reverse_incoming(&[0, 1, 0, 0], &[0, 1], 2),
        vec![0, 0, 1, 0]
    );
    assert!(
        reference_try_do_rule2_reverse_incoming(&[1], &[1], 2)
            .unwrap_err()
            .contains("adjacency.len() == n*n"),
        "structural rule2 wrapper must expose primitive shape errors"
    );
    assert_eq!(
        reference_do_rule3_subgraph(&[0, 1, 1, 0], &[1, 0], 2),
        (vec![0], vec![0])
    );
    assert!(
        reference_try_do_rule3_subgraph(&[1], &[1, 0], 2)
            .unwrap_err()
            .contains("adjacency.len() == n*n"),
        "structural rule3 wrapper must expose primitive shape errors"
    );
}
