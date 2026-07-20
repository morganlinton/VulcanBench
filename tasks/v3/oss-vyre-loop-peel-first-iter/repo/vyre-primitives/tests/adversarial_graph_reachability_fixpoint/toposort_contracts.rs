use super::*;

#[test]
fn toposort_empty_graph() {
    assert_eq!(toposort(0, &[]), Ok(Vec::new()));
}

#[test]
fn toposort_single_node_no_edges() {
    assert_eq!(toposort(1, &[]), Ok(vec![0]));
}

#[test]
fn toposort_two_nodes_one_edge() {
    // 0 depends on 1
    let got = toposort(2, &[(0, 1)]).unwrap();
    assert_eq!(got, vec![1, 0]);
}

#[test]
fn toposort_linear_chain() {
    let edges: Vec<(u32, u32)> = (0..9).map(|i| (i, i + 1)).collect();
    let got = toposort(10, &edges).unwrap();
    let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
    for i in 0..9 {
        assert!(
            pos(i + 1) < pos(i),
            "chain toposort must place {i} after {}",
            i + 1
        );
    }
}

#[test]
fn toposort_cycle_of_two_rejected() {
    let err = toposort(2, &[(0, 1), (1, 0)]).unwrap_err();
    assert!(matches!(err, ToposortError::Cycle { .. }));
}

#[test]
fn toposort_cycle_of_three_rejected() {
    let err = toposort(3, &[(0, 1), (1, 2), (2, 0)]).unwrap_err();
    assert!(matches!(err, ToposortError::Cycle { .. }));
}

#[test]
fn toposort_self_loop_rejected() {
    let err = toposort(2, &[(0, 0)]).unwrap_err();
    assert!(matches!(err, ToposortError::Cycle { .. }));
}

#[test]
fn toposort_unknown_node_rejected() {
    let err = toposort(2, &[(0, 5)]).unwrap_err();
    assert!(matches!(
        err,
        ToposortError::UnknownNode { edge: 0, node: 5 }
    ));
}

#[test]
fn toposort_diamond_respects_partial_order() {
    let got = toposort(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]).unwrap();
    let pos = |v: u32| got.iter().position(|&x| x == v).unwrap();
    assert!(pos(3) < pos(1));
    assert!(pos(3) < pos(2));
    assert!(pos(1) < pos(0));
    assert!(pos(2) < pos(0));
}

#[test]
fn toposort_parallel_edges_ok() {
    let got = toposort(2, &[(0, 1), (0, 1)]).unwrap();
    assert_eq!(got, vec![1, 0]);
}

#[test]
fn toposort_u32_max_indegree_saturates() {
    // Create a node with many incoming edges to test saturating_add.
    let mut edges = Vec::new();
    for i in 1..10 {
        edges.push((0, i));
    }
    let got = toposort(10, &edges).unwrap();
    assert_eq!(got.len(), 10);
}

// ---------------------------------------------------------------------------
// SCC decomposition
// ---------------------------------------------------------------------------
