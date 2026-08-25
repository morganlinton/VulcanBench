//! Failure-oriented tests for graph-IR invariants.
//!
//! Covers dangling edges, cycles, orphan Phi nodes, and error-message
//! contracts that must never silently swallow structural violations.

use vyre_foundation::graph_view::{
    from_graph, DataEdge, DataflowKind, EdgeKind, GraphNode, GraphValidateError, NodeGraph,
};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

#[test]
fn dangling_edge_is_rejected() {
    let g = NodeGraph::new(
        vec![GraphNode::new(0, DataflowKind::Barrier)],
        vec![DataEdge::new(0, 1, EdgeKind::Ordering)],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::DanglingEdge { from: 0, to: 1 }),
        "dangling edge must be rejected, got {err:?}"
    );
}

#[test]
fn self_loop_cycle_is_rejected() {
    let g = NodeGraph::new(
        vec![GraphNode::new(0, DataflowKind::Barrier)],
        vec![DataEdge::new(0, 0, EdgeKind::Ordering)],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::Cycle { ref path } if path == &[0]),
        "self-loop must be rejected as a cycle, got {err:?}"
    );
}

#[test]
fn two_node_cycle_is_rejected() {
    let g = NodeGraph::new(
        vec![
            GraphNode::new(0, DataflowKind::Barrier),
            GraphNode::new(1, DataflowKind::Barrier),
        ],
        vec![
            DataEdge::new(0, 1, EdgeKind::Ordering),
            DataEdge::new(1, 0, EdgeKind::Ordering),
        ],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::Cycle { .. }),
        "two-node cycle must be rejected, got {err:?}"
    );
}

#[test]
fn three_node_cycle_is_rejected() {
    let g = NodeGraph::new(
        vec![
            GraphNode::new(0, DataflowKind::Barrier),
            GraphNode::new(1, DataflowKind::Barrier),
            GraphNode::new(2, DataflowKind::Barrier),
        ],
        vec![
            DataEdge::new(0, 1, EdgeKind::Ordering),
            DataEdge::new(1, 2, EdgeKind::Ordering),
            DataEdge::new(2, 0, EdgeKind::Ordering),
        ],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::Cycle { .. }),
        "three-node cycle must be rejected, got {err:?}"
    );
}

#[test]
fn orphan_phi_empty_predecessors_is_rejected() {
    let g = NodeGraph::new(
        vec![
            GraphNode::new(0, DataflowKind::Barrier),
            GraphNode::new(1, DataflowKind::Phi(vec![])),
        ],
        vec![],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::OrphanPhi { node_id: 1 }),
        "empty-phi must be rejected, got {err:?}"
    );
}

#[test]
fn phi_with_out_of_range_predecessor_is_rejected() {
    let g = NodeGraph::new(
        vec![
            GraphNode::new(0, DataflowKind::Barrier),
            GraphNode::new(1, DataflowKind::Phi(vec![0, 2])),
        ],
        vec![],
    );
    let err = from_graph(g).unwrap_err();
    assert!(
        matches!(err, GraphValidateError::OrphanPhi { node_id: 1 }),
        "phi with out-of-range predecessor must be rejected, got {err:?}"
    );
}

#[test]
fn valid_phi_round_trips_after_lowering() {
    let mut g = NodeGraph::default();
    g.workgroup_size = [1, 1, 1];
    g.buffers
        .push(BufferDecl::read_write("out", 0, DataType::U32).with_count(1));
    g.nodes.push(GraphNode::new(
        0,
        DataflowKind::Statement(Node::store("out", Expr::u32(0), Expr::u32(1))),
    ));
    g.nodes.push(GraphNode::new(1, DataflowKind::Phi(vec![0])));
    let p = from_graph(g).unwrap();
    assert_eq!(
        p.entry().len(),
        1,
        "Phi must be dropped, leaving only the statement"
    );
}

#[test]
fn graph_validate_error_display_is_actionable() {
    let cycle_err = GraphValidateError::Cycle {
        path: vec![0, 1, 2],
    };
    let msg = cycle_err.to_string();
    assert!(
        msg.contains("cycle"),
        "cycle error must mention 'cycle': {msg}"
    );
    assert!(
        msg.contains("Fix:"),
        "cycle error must carry a Fix: hint: {msg}"
    );

    let dangling_err = GraphValidateError::DanglingEdge { from: 0, to: 5 };
    let msg = dangling_err.to_string();
    assert!(
        msg.contains("non-existent node"),
        "dangling edge must mention non-existent node: {msg}"
    );
    assert!(
        msg.contains("Fix:"),
        "dangling edge must carry a Fix: hint: {msg}"
    );

    let orphan_err = GraphValidateError::OrphanPhi { node_id: 3 };
    let msg = orphan_err.to_string();
    assert!(
        msg.contains("Phi node 3 has no valid predecessors"),
        "orphan phi must name the node: {msg}"
    );
    assert!(
        msg.contains("Fix:"),
        "orphan phi must carry a Fix: hint: {msg}"
    );
}

#[test]
fn graph_validate_error_implements_std_error() {
    let err = GraphValidateError::Cycle { path: vec![0, 1] };
    let dyn_err: &(dyn std::error::Error) = &err;
    assert!(
        dyn_err.source().is_none(),
        "GraphValidateError must have no source chain"
    );
}
