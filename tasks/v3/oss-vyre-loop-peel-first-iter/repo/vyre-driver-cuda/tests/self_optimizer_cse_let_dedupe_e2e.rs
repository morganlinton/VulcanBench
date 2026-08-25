//! End-to-end test: GPU CSE + let-level dedupe rewrite.
//!
//! Programs with structurally-identical let RHSes get the duplicates
//! rewritten as `Var` references to the original. Verified on real
//! CUDA hardware.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::cse_via_encoded::{apply_cse_let_dedupe, gpu_cse_canonicals};

fn body_of(out: &Program) -> Vec<Node> {
    match out.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    }
}

#[test]
fn cuda_let_dedupe_collapses_duplicate_literal_let_pairs() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // Two lets with the same literal RHS:
    //   let a = 5
    //   let b = 5    // duplicate; rewrite to `let b = a`
    //   let c = 7
    //   store buf 0 (a + b + c)   // ensure a, b, c are all live
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(5)),
            Node::let_bind("b", Expr::u32(5)),
            Node::let_bind("c", Expr::u32(7)),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::add(Expr::add(Expr::var("a"), Expr::var("b")), Expr::var("c")),
            ),
        ],
    );

    let (arena, canonical) =
        gpu_cse_canonicals(&p, &dispatcher).expect("gpu_cse_canonicals must succeed");
    let rewritten = apply_cse_let_dedupe(&p, &arena, &canonical);

    let body = body_of(&rewritten);
    // Find the `let b = ...` Node and assert its value is `Var("a")`.
    let let_b = body
        .iter()
        .find(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "b"))
        .expect("`let b` survives");
    if let Node::Let { value, .. } = let_b {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "a"),
            "`let b` should rewrite to `Var(a)`; got {value:?}"
        );
    }
    // `let a = 5` and `let c = 7` are unchanged.
    let let_a = body
        .iter()
        .find(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "a"))
        .expect("`let a` survives");
    if let Node::Let { value, .. } = let_a {
        assert!(matches!(value, Expr::LitU32(5)));
    }
    let let_c = body
        .iter()
        .find(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "c"))
        .expect("`let c` survives");
    if let Node::Let { value, .. } = let_c {
        assert!(matches!(value, Expr::LitU32(7)));
    }
}

#[test]
fn cuda_let_dedupe_collapses_duplicate_binop_let_pairs() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // Two lets with `1 + 2`; the second is rewritten.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("y", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::add(Expr::var("x"), Expr::var("y")),
            ),
        ],
    );

    let (arena, canonical) =
        gpu_cse_canonicals(&p, &dispatcher).expect("gpu_cse_canonicals must succeed");
    let rewritten = apply_cse_let_dedupe(&p, &arena, &canonical);

    let body = body_of(&rewritten);
    let let_y = body
        .iter()
        .find(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "y"))
        .expect("`let y` survives");
    if let Node::Let { value, .. } = let_y {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "x"),
            "`let y` should rewrite to `Var(x)`; got {value:?}"
        );
    }
}

#[test]
fn cuda_let_dedupe_no_change_for_distinct_lets() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // No duplicates  -  every let has a different value. Rewrite must
    // leave each let's RHS untouched.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(1)),
            Node::let_bind("b", Expr::u32(2)),
            Node::let_bind("c", Expr::u32(3)),
            Node::store(
                "buf",
                Expr::u32(0),
                Expr::add(Expr::add(Expr::var("a"), Expr::var("b")), Expr::var("c")),
            ),
        ],
    );

    let (arena, canonical) =
        gpu_cse_canonicals(&p, &dispatcher).expect("gpu_cse_canonicals must succeed");
    let rewritten = apply_cse_let_dedupe(&p, &arena, &canonical);

    let body = body_of(&rewritten);
    for (name, expected) in [("a", 1u32), ("b", 2), ("c", 3)] {
        let n = body
            .iter()
            .find(|n| matches!(n, Node::Let { name: nm, .. } if nm.as_str() == name))
            .unwrap_or_else(|| panic!("`let {name}` survives"));
        if let Node::Let { value, .. } = n {
            assert!(
                matches!(value, Expr::LitU32(v) if *v == expected),
                "`let {name}` should be untouched (expected LitU32({expected})); got {value:?}"
            );
        }
    }
}
