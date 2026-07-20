use super::*;
use vyre_foundation::ir::{Node, Program};
use vyre_primitives::graph::program_graph::ProgramGraphShape;

fn region_generator(node: &Node) -> &str {
    let Node::Region { generator, .. } = node else {
        panic!("Fix: graph traversal child helper must emit a Region.");
    };
    generator.as_str()
}

fn program_generator(program: &Program) -> &str {
    let Some(Node::Region { generator, .. }) = program.entry.first() else {
        panic!("Fix: graph traversal Program must start with a Region.");
    };
    generator.as_str()
}

fn dense_adj(edges: &[(u32, u32)], node_count: u32) -> Vec<u32> {
    let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let mut rows = vec![0; node_count as usize * words];
    for &(src, dst) in edges {
        rows[dst as usize * words + src as usize / 32] |= 1 << (src % 32);
    }
    rows
}

#[test]
fn dispatch_programs_emit_expected_graph_primitives() {
    let shape = ProgramGraphShape::new(4, 4);
    assert_eq!(
        program_generator(&dispatch_adaptive_dense_step("fin", "fout", "adj", 4)),
        "vyre-primitives::graph::adaptive_traverse_dense"
    );
    assert_eq!(
        program_generator(&dispatch_csr_forward_parallel(
            shape, "frontier", "changed", 1
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        program_generator(&dispatch_csr_forward_batch(
            shape, "frontier", "changed", 1, 2
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        program_generator(&dispatch_csr_forward_batch_global(
            shape, "frontier", "changed", 1, 2
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        program_generator(&dispatch_csr_forward_batch_global_slot(
            shape, "frontier", "changed", 1, 2, 0, 1
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        program_generator(&dispatch_frontier_degree_sum(shape)),
        "vyre-primitives::graph::csr_frontier_degree_sum"
    );
    assert_eq!(
        program_generator(&dispatch_persistent_bfs_step(
            shape, "frontier", "changed", 1
        )),
        "vyre-primitives::graph::persistent_bfs_step"
    );
}

#[test]
fn child_regions_preserve_parent_context() {
    let shape = ProgramGraphShape::new(4, 4);
    let parent = "vyre-self-substrate::graph::traversal_dispatch_pipeline";
    assert_eq!(
        region_generator(&child_csr_forward_stage(
            parent, shape, "frontier", "changed", 1
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        region_generator(&prefixed_child_csr_forward_stage(
            parent, shape, "frontier", "changed", 1, "csr"
        )),
        "vyre-primitives::graph::csr_forward_or_changed"
    );
    assert_eq!(
        region_generator(&child_persistent_step(
            parent, shape, "frontier", "changed", "scratch", 1
        )),
        "vyre-primitives::graph::persistent_bfs_step"
    );
    assert_eq!(
        region_generator(&prefixed_child_persistent_step(
            parent, shape, "frontier", "changed", "scratch", 1, "step"
        )),
        "vyre-primitives::graph::persistent_bfs_step"
    );
    assert_eq!(
        region_generator(&active_child_persistent_step(
            parent,
            shape,
            "frontier",
            "changed",
            "scratch",
            "active",
            1,
            "active_step"
        )),
        "vyre-primitives::graph::persistent_bfs_step"
    );
}

#[test]
fn body_builders_emit_composable_ir() {
    let shape = ProgramGraphShape::new(4, 4);
    assert_ne!(csr_forward_body(shape, "frontier", "changed", 1).len(), 0);
    assert_ne!(
        prefixed_csr_forward_body(shape, "frontier", "changed", 1, "csr").len(),
        0
    );
    assert_ne!(
        persistent_step_body(shape, "frontier", "changed", "scratch", 1).len(),
        0
    );
    assert_ne!(
        prefixed_persistent_step_body(shape, "frontier", "changed", "scratch", 1, "step").len(),
        0
    );
}

#[test]
fn checked_batch_builders_reject_invalid_dimensions() {
    let shape = ProgramGraphShape::new(4, 4);
    assert!(matches!(
        dispatch_csr_forward_batch_checked(shape, "frontier", "changed", 1, 0),
        Err(_)
    ));
    assert!(dispatch_csr_forward_batch_global_slot_checked(
        shape, "frontier", "changed", 1, 1, 2, 2
    )
    .is_err());
}

#[test]
fn cpu_reference_wrappers_match_traversal_contracts() {
    assert!(!select_dense_traversal(&[0x7f], 32));
    assert!(select_dense_traversal(&[0xff], 32));

    let dense = dense_adj(&[(0, 1), (2, 3)], 8);
    assert_eq!(reference_dense_step(&[1], &dense, 8), vec![0b10]);

    let frontier = [0b101];
    let edge_offsets = [0, 3, 7, 9, 9, 12];
    assert_eq!(
        reference_frontier_degree_sum(&frontier, &edge_offsets, 5),
        5
    );
    assert_eq!(
        try_reference_frontier_degree_sum(&frontier, &edge_offsets, 5).unwrap(),
        5
    );

    let mut current = Vec::new();
    let mut next = Vec::new();
    reference_csr_closure_into(
        3,
        &[0, 1, 2, 2],
        &[1, 2],
        &[1, 1],
        &[0b001],
        1,
        4,
        &mut current,
        &mut next,
    );
    assert_eq!(current, vec![0b111]);
}
