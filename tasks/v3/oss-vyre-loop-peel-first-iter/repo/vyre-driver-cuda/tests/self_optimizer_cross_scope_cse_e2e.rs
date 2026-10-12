//! End-to-end tests: cross-scope expression CSE in the GPU pipeline.
//!
//! Verifies that when the same complex Expr appears as the value of
//! 2+ Stores in the same scope, the optimizer hoists it to a shared
//! `let __cse_N = E;` and replaces the duplicate occurrences with
//! `Var(__cse_N)`.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::pipeline_resident::gpu_pipeline_resident;

fn run_pipeline(p: Program) -> Program {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);
    gpu_pipeline_resident(p, &dispatcher).expect("pipeline must succeed")
}

fn body_of(out: &Program) -> Vec<Node> {
    match out.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    }
}

#[test]
fn cuda_cross_scope_cse_hoists_shared_store_value() {
    // store buf 0 (Var(x) + 7); store buf 1 (Var(x) + 7);
    //   ⇒ both stores share the same canonical Expr. The pass
    //     should hoist `let __cse_0 = (x + 7);` and rewrite both
    //     stores to `Var(__cse_0)`.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::add(Expr::var("x"), Expr::u32(7))),
            Node::store("buf", Expr::u32(1), Expr::add(Expr::var("x"), Expr::u32(7))),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // Count BinOps surviving as Store values. Without the CSE pass
    // both stores keep their own `(x + 7)` BinOp  -  total 2. With
    // the pass, only the hoisted let RHS holds the BinOp; both
    // stores' values are `Var(__cse_0)`.
    let bin_op_store_values = body
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::Store {
                    value: Expr::BinOp { .. },
                    ..
                }
            )
        })
        .count();
    assert!(
        bin_op_store_values <= 1,
        "after cross-scope CSE at most one Store should keep a BinOp value; got {bin_op_store_values}; body={body:?}"
    );
    // At least one Store value should now be `Var(__cse_*)`.
    let any_cse_var = body.iter().any(|n| {
        if let Node::Store {
            value: Expr::Var(name),
            ..
        } = n
        {
            name.as_str().starts_with("__cse_")
        } else {
            false
        }
    });
    assert!(
        any_cse_var,
        "expected at least one Store value to reference a __cse_* let; body={body:?}"
    );
}

#[test]
fn cuda_cross_scope_cse_hoists_shared_if_cond() {
    // Two Ifs in the same scope sharing an identical cond Expr.
    // The pass should hoist the cond to a shared let.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::if_then_else(
                Expr::eq(Expr::var("x"), Expr::u32(7)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(2))],
            ),
            Node::if_then_else(
                Expr::eq(Expr::var("x"), Expr::u32(7)),
                vec![Node::store("buf", Expr::u32(1), Expr::u32(3))],
                vec![Node::store("buf", Expr::u32(1), Expr::u32(4))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // After hoisting: a `let __cse_0 = (x == 7);` should appear
    // before the Ifs, and both Ifs should now have `Var(__cse_0)`
    // as their cond.
    let has_cse_let = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str().starts_with("__cse_")));
    let bin_op_conds = body
        .iter()
        .filter(|n| {
            matches!(
                n,
                Node::If {
                    cond: Expr::BinOp { .. },
                    ..
                }
            )
        })
        .count();
    assert!(
        has_cse_let,
        "shared If cond must be hoisted to a __cse_* let; body={body:?}"
    );
    assert!(
        bin_op_conds <= 1,
        "after hoisting at most one If should keep a BinOp cond; got {bin_op_conds}"
    );
}

#[test]
fn cuda_cross_scope_cse_does_not_hoist_when_only_once() {
    // Single Store with a complex Expr  -  no duplicates, no hoist.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::add(Expr::var("x"), Expr::u32(7))),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let has_cse_let = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str().starts_with("__cse_")));
    assert!(
        !has_cse_let,
        "single occurrence must not trigger hoisting; body={body:?}"
    );
}

#[test]
fn cuda_cross_scope_cse_hoists_in_nested_scope_independently() {
    // Within an If's `then` branch we have two stores sharing a
    // value Expr  -  they should hoist locally inside the branch.
    // The outer scope (which has no duplicate) should not gain a
    // hoisted let.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::load("input", Expr::u32(0))),
            Node::if_then_else(
                Expr::eq(Expr::var("x"), Expr::u32(0)),
                vec![
                    Node::store(
                        "buf",
                        Expr::u32(0),
                        Expr::add(Expr::var("x"), Expr::u32(11)),
                    ),
                    Node::store(
                        "buf",
                        Expr::u32(1),
                        Expr::add(Expr::var("x"), Expr::u32(11)),
                    ),
                ],
                vec![Node::store("buf", Expr::u32(2), Expr::u32(0))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // Walk into the surviving If's then-branch to check for the
    // hoisted let inside.
    let if_node = body.iter().find(|n| matches!(n, Node::If { .. }));
    if let Some(Node::If { then, .. }) = if_node {
        let then_has_cse = then
            .iter()
            .any(|n| matches!(n, Node::Let { name, .. } if name.as_str().starts_with("__cse_")));
        assert!(
            then_has_cse,
            "branch-local duplicates must hoist inside the branch; then={then:?}"
        );
    }
}
