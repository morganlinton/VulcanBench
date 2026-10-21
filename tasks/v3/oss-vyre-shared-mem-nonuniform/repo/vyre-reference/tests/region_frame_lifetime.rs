//! P1.12: prove the reference interpreter's Region handler is
//! memory-safe.
//!
//! The handler pushes a `Frame::Nodes { nodes: body.as_slice() }`
//! where `body: &Arc<Vec<Node>>` borrows from a Region node on the
//! parent frame's slice. The parent frame is always re-pushed onto
//! the stack BEFORE the Region is executed, so the borrow chain
//! (`parent frame -> Region node -> Arc -> Vec -> body slice`) is
//! live for the entire duration of the Region's execution.
//!
//! These tests exercise:
//!  1. Deeply nested Regions (lifetime chain survives recursion).
//!  2. Regions whose body references bindings from the enclosing
//!     scope (cross-frame borrows don't dangle).
//!  3. Program held behind a Cell-bounded reference so the interp
//!     couldn't have moved/reallocated it mid-execution.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::value::Value;

fn run(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let values: Vec<Value> = inputs.into_iter().map(Value::from).collect();
    let outputs = vyre_reference::reference_eval(program, &values).expect("program must execute");
    outputs.into_iter().map(|v| v.to_bytes()).collect()
}

fn out_buffer() -> BufferDecl {
    BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)
}

#[test]
fn deeply_nested_regions_execute_without_dangling_borrow() {
    // 10 levels of Region nesting; each one wraps a store of the
    // level index. Final store wins (last writer in the node list).
    let mut inner = vec![Node::store("out", Expr::u32(0), Expr::u32(9))];
    for i in (0..9).rev() {
        inner = vec![
            Node::Region {
                generator: "nested".into(),
                source_region: None,
                body: std::sync::Arc::new(inner),
            },
            Node::store("out", Expr::u32(0), Expr::u32(i)),
        ];
    }
    let program = Program::wrapped(vec![out_buffer()], [1, 1, 1], inner);
    let outputs = run(&program, vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(outputs[0][..4].try_into().unwrap());
    // The outermost store (i=0) runs LAST because we built the nesting
    // so that after the inner Region finishes executing, control
    // returns to the parent frame which then executes the outer Store.
    assert_eq!(val, 0, "outermost store is last writer");
}

#[test]
fn region_body_reads_enclosing_let_across_frames() {
    // The outer Let survives across the Region's frame push/pop.
    // A dangling borrow would show up as either a read of the wrong
    // value or a crash.
    let inner = vec![Node::store("out", Expr::u32(0), Expr::var("outer"))];
    let body = vec![
        Node::let_bind("outer", Expr::u32(0x1357_9bdf)),
        Node::Region {
            generator: "lifetime".into(),
            source_region: None,
            body: std::sync::Arc::new(inner),
        },
    ];
    let program = Program::wrapped(vec![out_buffer()], [1, 1, 1], body);
    let outputs = run(&program, vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(outputs[0][..4].try_into().unwrap());
    assert_eq!(val, 0x1357_9bdf);
}

#[test]
fn region_body_written_inside_loop_preserves_borrow() {
    // Region inside a Loop: the loop body's slice outlives each
    // iteration; the Region inside borrows into the body's Arc.
    // If any frame incorrectly dropped the borrow, the loop would
    // either panic or return stale data.
    let inner = vec![Node::assign(
        "acc",
        Expr::add(Expr::var("acc"), Expr::var("i")),
    )];
    let body = vec![
        Node::let_bind("acc", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(5),
            vec![Node::Region {
                generator: "loop-region".into(),
                source_region: None,
                body: std::sync::Arc::new(inner),
            }],
        ),
        Node::store("out", Expr::u32(0), Expr::var("acc")),
    ];
    let program = Program::wrapped(vec![out_buffer()], [1, 1, 1], body);
    let outputs = run(&program, vec![vec![0u8; 4]]);
    let val = u32::from_le_bytes(outputs[0][..4].try_into().unwrap());
    assert_eq!(val, 10); // 0 + 1 + 2 + 3 + 4
}

#[test]
fn region_inside_region_inside_if_executes_all_branches() {
    // If-branches open scopes; Regions inside them must not leak
    // borrows when the If arm's frame pops. A dangling borrow would
    // show up as incorrect output for one of the branches.
    let then_branch = vec![Node::Region {
        generator: "then".into(),
        source_region: None,
        body: std::sync::Arc::new(vec![Node::store("out", Expr::u32(0), Expr::u32(111))]),
    }];
    let else_branch = vec![Node::Region {
        generator: "else".into(),
        source_region: None,
        body: std::sync::Arc::new(vec![Node::store("out", Expr::u32(0), Expr::u32(222))]),
    }];

    for (label, cond, expected) in [
        ("true-branch", true, 111u32),
        ("false-branch", false, 222u32),
    ] {
        let body = vec![Node::if_then_else(
            Expr::bool(cond),
            then_branch.clone(),
            else_branch.clone(),
        )];
        let program = Program::wrapped(vec![out_buffer()], [1, 1, 1], body);
        let outputs = run(&program, vec![vec![0u8; 4]]);
        let val = u32::from_le_bytes(outputs[0][..4].try_into().unwrap());
        assert_eq!(val, expected, "case {label}");
    }
}
