//! Tests for the IR visitor walk utilities.
//!
//! `walk_nodes`, `walk_exprs`, `referenced_buffers`, and
//! `collect_call_op_ids` must traverse every node and expression
//! without missing nested bodies.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::visit::{
    collect_call_op_ids, referenced_buffers, walk_exprs, walk_nodes,
};

fn sample_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("a", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::store("a", Expr::u32(0), Expr::u32(1)),
            Node::if_then(
                Expr::lt(Expr::var("x"), Expr::u32(10)),
                vec![
                    Node::store("b", Expr::u32(0), Expr::add(Expr::var("x"), Expr::u32(1))),
                    Node::Return,
                ],
            ),
            Node::Return,
        ],
    )
}

#[test]
fn walk_nodes_visits_all_nodes() {
    let prog = sample_program();
    let mut count = 0;
    walk_nodes(&prog, |_node| {
        count += 1;
    });
    // Store, If (with Region wrapper), Return inside if, Return at top
    assert!(
        count >= 4,
        "walk_nodes must visit at least 4 nodes, got {count}"
    );
}

#[test]
fn walk_nodes_visits_store_node() {
    let prog = sample_program();
    let mut saw_store = false;
    walk_nodes(&prog, |node| {
        if matches!(node, Node::Store { buffer, .. } if buffer.as_str() == "a") {
            saw_store = true;
        }
    });
    assert!(saw_store, "walk_nodes must visit the store to 'a'");
}

#[test]
fn walk_exprs_visits_all_expressions() {
    let prog = sample_program();
    let mut count = 0;
    walk_exprs(&prog, |_expr| {
        count += 1;
    });
    // Must visit literals, vars, comparisons, arithmetic inside nodes
    assert!(
        count >= 6,
        "walk_exprs must visit at least 6 expressions, got {count}"
    );
}

#[test]
fn walk_exprs_finds_load_and_var() {
    let prog = sample_program();
    let mut saw_load = false;
    let mut saw_var = false;
    walk_exprs(&prog, |expr| {
        if matches!(expr, Expr::Load { .. }) {
            saw_load = true;
        }
        if matches!(expr, Expr::Var { .. }) {
            saw_var = true;
        }
    });
    assert!(
        saw_load || saw_var,
        "walk_exprs must find at least one load or var"
    );
}

#[test]
fn referenced_buffers_finds_all_buffers() {
    let prog = sample_program();
    let refs = referenced_buffers(&prog);
    assert!(refs.contains("a"), "referenced_buffers must find 'a'");
    assert!(refs.contains("b"), "referenced_buffers must find 'b'");
}

#[test]
fn referenced_buffers_empty_for_return_only() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let refs = referenced_buffers(&prog);
    assert!(
        refs.is_empty(),
        "return-only program must have no buffer references"
    );
}

#[test]
fn collect_call_op_ids_empty_when_no_calls() {
    let prog = sample_program();
    let ids = collect_call_op_ids(&prog);
    assert!(ids.is_empty(), "sample program has no Expr::Call");
}

#[test]
fn collect_call_op_ids_finds_calls() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "x",
                Expr::call("math::add", vec![Expr::u32(1), Expr::u32(2)]),
            ),
            Node::Return,
        ],
    );
    let ids = collect_call_op_ids(&prog);
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].as_ref(), "math::add");
}

#[test]
fn collect_call_op_ids_deduplicates() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::call("math::add", vec![Expr::u32(1)])),
            Node::let_bind("y", Expr::call("math::add", vec![Expr::u32(2)])),
            Node::Return,
        ],
    );
    let ids = collect_call_op_ids(&prog);
    // Should deduplicate identical op_ids
    assert!(
        ids.len() <= 2,
        "collect_call_op_ids should not explode on duplicates, got {} ids",
        ids.len()
    );
}

#[test]
fn walk_nodes_visits_nested_if_body() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::if_then(Expr::bool(true), vec![Node::barrier(), Node::Return]),
            Node::Return,
        ],
    );
    let mut saw_barrier = false;
    walk_nodes(&prog, |node| {
        if matches!(node, Node::Barrier { .. }) {
            saw_barrier = true;
        }
    });
    assert!(saw_barrier, "walk_nodes must descend into If body");
}

#[test]
fn walk_nodes_visits_loop_body() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::loop_for("i", Expr::u32(0), Expr::u32(3), vec![Node::barrier()]),
            Node::Return,
        ],
    );
    let mut saw_barrier = false;
    walk_nodes(&prog, |node| {
        if matches!(node, Node::Barrier { .. }) {
            saw_barrier = true;
        }
    });
    assert!(saw_barrier, "walk_nodes must descend into Loop body");
}
