//! End-to-end test: dead-branch elimination in the GPU pipeline.
//!
//! When const-prop turns an `If`'s cond into a constant literal,
//! the dead-branch pass splices the surviving body into the parent
//! scope, eliminating both the `If` wrapper and the dead branch.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{Expr, Node, Program};
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
fn cuda_dead_branch_const_true_keeps_then_drops_otherwise() {
    // if true { store buf 0 (LitU32 1) } else { store buf 0 (LitU32 99) }
    //   ⇒ store buf 0 (LitU32 1)  (the `else` is dead)
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::if_then_else(
            Expr::bool(true),
            vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    // Body should contain ONLY the surviving Store(LitU32(1)).
    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "no `If` node should survive const-true cond; body={body:?}"
    );
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(1)), "got {value:?}");
    }
    // The 99-branch must NOT survive.
    let has_99 = body.iter().any(|n| {
        matches!(
            n,
            Node::Store {
                value: Expr::LitU32(99),
                ..
            }
        )
    });
    assert!(
        !has_99,
        "the LitU32(99) store should be eliminated; body={body:?}"
    );
}

#[test]
fn cuda_dead_branch_const_zero_keeps_otherwise() {
    // if 0u32 { store buf 0 1 } else { store buf 0 7 }
    //   ⇒ store buf 0 7
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::if_then_else(
            Expr::u32(0),
            vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    assert!(body.iter().all(|n| !matches!(n, Node::If { .. })));
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(7)), "got {value:?}");
    }
}

#[test]
fn cuda_dead_branch_via_const_prop_cascade() {
    // let cond = false  ← const-prop turns `Var(cond)` into LitBool(false)
    // if Var(cond) { store buf 0 1 } else { store buf 0 42 }
    //   After const-prop: if false { … } else { store buf 0 42 }
    //   After dead-branch: store buf 0 42 only.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("cond", Expr::bool(false)),
            Node::if_then_else(
                Expr::var("cond"),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(42))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "if must collapse after const-prop+dead-branch; body={body:?}"
    );
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(42)), "got {value:?}");
    }
    // `let cond` should be dropped by DCE.
    assert!(!body.iter().any(|n| matches!(n, Node::Let { .. })));
}

#[test]
fn cuda_empty_loop_eliminated() {
    // for i in 0..0 { store buf 0 99 }
    //   ⇒ the entire loop drops; no Store survives from the body.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(0),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            ),
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    assert!(
        body.iter().all(|n| !matches!(n, Node::Loop { .. })),
        "empty loop must be eliminated; body={body:?}"
    );
    // The 99 store from inside the loop must not survive.
    let has_99 = body.iter().any(|n| {
        matches!(
            n,
            Node::Store {
                value: Expr::LitU32(99),
                ..
            }
        )
    });
    assert!(!has_99, "loop body store must not survive empty-loop elim");
    // The trailing `store buf 0 7` is kept.
    let has_7 = body.iter().any(|n| {
        matches!(
            n,
            Node::Store {
                value: Expr::LitU32(7),
                ..
            }
        )
    });
    assert!(has_7, "trailing post-loop store must survive");
}

#[test]
fn cuda_comparison_fold_unlocks_dead_branch() {
    // let n = 5
    // if (Var(n) == 5) { store buf 0 1 } else { store buf 0 0 }
    //   After const-prop: n→5, BinOp(Eq, 5, 5) → LitBool(true).
    //   After dead-branch: store buf 0 1 only.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("n", Expr::u32(5)),
            Node::if_then_else(
                Expr::eq(Expr::var("n"), Expr::u32(5)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(0))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "comparison fold + dead-branch should collapse the If; body={body:?}"
    );
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(1)), "got {value:?}");
    }
}

#[test]
fn cuda_empty_block_eliminated() {
    // Block(empty) followed by a Store; the Block must drop.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::Block(Vec::new()),
            Node::store("buf", Expr::u32(0), Expr::u32(11)),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::Block(_))),
        "empty block must be eliminated; body={body:?}"
    );
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(11)), "got {value:?}");
    }
}

#[test]
fn cuda_if_both_branches_empty_dropped() {
    // if Var(x) { } else { }   where cond is a Var (atomic-free).
    //   Both branches are empty so the entire If can drop because
    //   evaluating an atomic-free cond has no observable effect.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("x", Expr::u32(7)),
            Node::if_then_else(Expr::var("x"), vec![], vec![]),
            Node::store("buf", Expr::u32(0), Expr::u32(11)),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "If with both empty branches must be eliminated; body={body:?}"
    );
}

#[test]
fn cuda_if_with_equal_arms_collapses() {
    // if cond { store buf 0 7 } else { store buf 0 7 } → store buf 0 7
    use vyre::ir::{BufferAccess, BufferDecl, DataType};
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "c",
                Expr::eq(Expr::load("input", Expr::u32(0)), Expr::u32(0)),
            ),
            Node::if_then_else(
                Expr::var("c"),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::If { .. })),
        "If with equal arms must drop; body={body:?}"
    );
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(7)),
            "spliced body must keep the single Store(LitU32(7)); got {value:?}"
        );
    }
}

#[test]
fn cuda_loop_with_var_equal_bounds_eliminated() {
    // for i in n..n { ... }  -  both bounds reference the same Var, so
    // the half-open range is empty regardless of n's runtime value.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("n", Expr::u32(7)),
            Node::loop_for(
                "i",
                Expr::var("n"),
                Expr::var("n"),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            ),
            Node::store("buf", Expr::u32(0), Expr::u32(11)),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    assert!(
        body.iter().all(|n| !matches!(n, Node::Loop { .. })),
        "var-equal-bounds loop must be eliminated; body={body:?}"
    );
    let has_99 = body.iter().any(|n| {
        matches!(
            n,
            Node::Store {
                value: Expr::LitU32(99),
                ..
            }
        )
    });
    assert!(!has_99, "loop body must not survive the empty range");
}

#[test]
fn cuda_noop_body_loop_eliminated() {
    // for i in 0..n { let _x = 5; }   ← loop body has only a Let
    //   that DCE will drop. After DCE leaves the body empty, the
    //   entire Loop should drop too because `from`/`to` are
    //   atomic-free (literal-or-Var).
    use vyre::ir::{BufferAccess, BufferDecl, DataType};
    let p = Program::wrapped(
        vec![BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(10),
                vec![Node::let_bind("x", Expr::u32(5))],
            ),
            Node::store("buf", Expr::u32(0), Expr::u32(42)),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    assert!(
        body.iter().all(|n| !matches!(n, Node::Loop { .. })),
        "no-op body loop must be eliminated; body={body:?}"
    );
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("trailing store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(42)), "got {value:?}");
    }
}

#[test]
fn cuda_dead_branch_preserves_non_constant_if() {
    // if Load(input, 0) { store buf 0 1 } else { store buf 0 2 }
    //   The cond is non-constant; the If must survive.
    use vyre::ir::{BufferAccess, BufferDecl, DataType};
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then_else(
            Expr::load("input", Expr::u32(0)),
            vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            vec![Node::store("buf", Expr::u32(0), Expr::u32(2))],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    let has_if = body.iter().any(|n| matches!(n, Node::If { .. }));
    assert!(
        has_if,
        "non-constant cond must keep the If wrapper; body={body:?}"
    );
}
