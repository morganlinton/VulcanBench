//! Structural contracts for algebraic math kernels.
//!
//! These tests protect performance-critical IR shape. Boolean semiring matrix
//! multiplication is used as a GraphBLAS-style substrate for reachability and
//! parser closure, so its inner loop must remain branchless on SIMT backends.

#![cfg(feature = "math-algebra")]

use vyre::ir::Node;

fn bool_mm_loop_body(nodes: &[Node]) -> Option<&[Node]> {
    for node in nodes {
        match node {
            Node::Loop { var, body, .. } if var.as_str() == "bool_mm_k" => {
                return Some(body);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                if let Some(found) = bool_mm_loop_body(body) {
                    return Some(found);
                }
            }
            Node::Region { body, .. } => {
                if let Some(found) = bool_mm_loop_body(body) {
                    return Some(found);
                }
            }
            Node::If {
                then, otherwise, ..
            } => {
                if let Some(found) = bool_mm_loop_body(then) {
                    return Some(found);
                }
                if let Some(found) = bool_mm_loop_body(otherwise) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn contains_branch(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::If { .. } => true,
        Node::Loop { body, .. } | Node::Block(body) => contains_branch(body),
        Node::Region { body, .. } => contains_branch(body),
        _ => false,
    })
}

#[test]
fn bool_semiring_inner_loop_is_branchless() {
    let program = vyre_libs::math::bool_semiring_matmul("a", "b", "out", 2, 3, 2);
    let body = bool_mm_loop_body(program.entry()).expect("bool_mm_k loop must exist");
    assert!(
        !contains_branch(body),
        "bool-semiring matmul must accumulate with select/bitor instead of divergent per-k branches"
    );
}
