use super::*;

#[test]
fn reachable_empty_graph_empty_sources() {
    let got = reachable(0, &[], &[]).unwrap();
    assert!(got.is_empty());
}

#[test]
fn reachable_empty_graph_non_empty_sources() {
    // Sources outside node count are still reported as reachable from themselves
    let got = reachable(0, &[], &[0, 1, 2]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2]));
}

#[test]
fn reachable_single_node_self_loop() {
    let got = reachable(1, &[(0, 0)], &[0]).unwrap();
    assert_eq!(got, hs(&[0]));
}

#[test]
fn reachable_chain_of_five() {
    let edges: Vec<(u32, u32)> = (0..4).map(|i| (i, i + 1)).collect();
    let got = reachable(5, &edges, &[0]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2, 3, 4]));
}

#[test]
fn reachable_fork_join() {
    // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
    let got = reachable(4, &[(0, 1), (0, 2), (1, 3), (2, 3)], &[0]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2, 3]));
}

#[test]
fn reachable_multiple_sources() {
    let got = reachable(4, &[(0, 1), (2, 3)], &[0, 2]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2, 3]));
}

#[test]
fn reachable_cycle_of_three() {
    let got = reachable(3, &[(0, 1), (1, 2), (2, 0)], &[0]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2]));
}

#[test]
fn reachable_cycle_of_three_source_in_middle() {
    let got = reachable(3, &[(0, 1), (1, 2), (2, 0)], &[1]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2]));
}

#[test]
fn reachable_disconnected_component_excluded() {
    let got = reachable(6, &[(0, 1), (1, 2), (3, 4), (4, 5)], &[0]).unwrap();
    assert_eq!(got, hs(&[0, 1, 2]));
    assert!(!got.contains(&3));
    assert!(!got.contains(&4));
    assert!(!got.contains(&5));
}

#[test]
fn reachable_unknown_node_from_edge_is_rejected() {
    let err = reachable(3, &[(0, 1), (5, 1)], &[0]).unwrap_err();
    assert_eq!(err.index, 1);
    assert_eq!(err.node, 5);
    assert_eq!(err.node_count, 3);
}

#[test]
fn reachable_unknown_to_node_from_edge_is_rejected() {
    let err = reachable(3, &[(0, 1), (1, 5)], &[0]).unwrap_err();
    assert_eq!(err.index, 1);
    assert_eq!(err.node, 5);
}

#[test]
fn reachable_program_builder_non_empty() {
    let p = reachable_program(4, 4, "src", "reach", 3);
    assert!(!p.is_explicit_noop());
    assert!(!p.buffers().is_empty());
}

#[test]
fn reachable_program_zero_iters_seeds_only() {
    let p = reachable_program(4, 4, "src", "reach", 0);
    assert!(!p.is_explicit_noop());
}

#[test]
fn reachable_program_declares_scratch_buffers() {
    let p = reachable_program(4, 4, "src", "reach", 2);
    let names: Vec<&str> = p.buffers().iter().map(|b| b.name()).collect();
    assert!(names.contains(&"reach_frontier_a"));
    assert!(names.contains(&"reach_frontier_b"));
}

// ---------------------------------------------------------------------------
// Topological sort
// ---------------------------------------------------------------------------
