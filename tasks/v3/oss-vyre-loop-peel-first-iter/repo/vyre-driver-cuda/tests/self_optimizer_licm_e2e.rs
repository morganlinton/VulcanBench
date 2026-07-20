//! End-to-end test: loop-invariant code motion in the GPU pipeline.

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

/// Make a Program where `seed` is bound to a non-literal Load so it
/// survives const-prop, then a Loop body uses Var(seed) in an
/// invariant Let. LICM should hoist the invariant Let above the
/// Loop.
#[test]
fn cuda_licm_hoists_invariant_let_above_loop() {
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(16),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("seed", Expr::load("input", Expr::u32(0))),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    // Invariant: doesn't reference Var(i).
                    Node::let_bind("inv", Expr::add(Expr::var("seed"), Expr::u32(7))),
                    // Variant: references Var(i).
                    Node::store(
                        "buf",
                        Expr::var("i"),
                        Expr::add(Expr::var("inv"), Expr::var("i")),
                    ),
                ],
            ),
        ],
    );

    let out = run_pipeline(p);
    let body = body_of(&out);

    // The hoisted `let inv = …` should appear at the TOP-LEVEL,
    // not inside the Loop body.
    let top_level_inv_count = body
        .iter()
        .filter(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "inv"))
        .count();
    assert_eq!(
        top_level_inv_count, 1,
        "`let inv` must be hoisted to the top-level scope; body={body:?}"
    );

    // The Loop body should NO LONGER contain `let inv`.
    let loop_node = body
        .iter()
        .find(|n| matches!(n, Node::Loop { .. }))
        .expect("Loop survives");
    if let Node::Loop { body: lbody, .. } = loop_node {
        let inner_inv = lbody
            .iter()
            .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "inv"));
        assert!(
            !inner_inv,
            "`let inv` must NOT remain inside the Loop body; lbody={lbody:?}"
        );
    }
}

#[test]
fn cuda_licm_keeps_iter_dependent_let_inside_loop() {
    // This Let references Var(i) and so cannot be hoisted.
    let p = Program::wrapped(
        vec![BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16)],
        [1, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("dep", Expr::add(Expr::var("i"), Expr::u32(3))),
                Node::store("buf", Expr::var("i"), Expr::var("dep")),
            ],
        )],
    );

    let out = run_pipeline(p);
    let body = body_of(&out);

    // `let dep` must stay inside the Loop body  -  it depends on `i`.
    let loop_node = body
        .iter()
        .find(|n| matches!(n, Node::Loop { .. }))
        .expect("Loop survives");
    if let Node::Loop { body: lbody, .. } = loop_node {
        let inner_dep = lbody
            .iter()
            .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "dep"));
        assert!(
            inner_dep,
            "iter-dependent `let dep` must stay inside the Loop body"
        );
    }
    // The top-level scope should NOT have a hoisted `let dep`.
    let top_dep = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "dep"));
    assert!(
        !top_dep,
        "iter-dependent `let dep` must NOT be hoisted to top level"
    );
}

#[test]
fn cuda_licm_does_not_hoist_let_depending_on_kept_local() {
    // `let a = Load(rw, 0)` is RW, so LICM keeps it inside the loop.
    // `let b = Var(a) + 1` depends on `a`. LICM must NOT hoist `b`,
    // because that would put `b` above the loop where `a` is undef.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("rw", 0, BufferAccess::ReadWrite, DataType::U32).with_count(8),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("a", Expr::load("rw", Expr::u32(0))),
                Node::let_bind("b", Expr::add(Expr::var("a"), Expr::u32(1))),
                Node::store("buf", Expr::var("i"), Expr::var("b")),
            ],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // `b` must NOT appear at top level.
    let top_b = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "b"));
    assert!(
        !top_b,
        "`let b` (depends on kept-local `a`) must not be hoisted; body={body:?}"
    );
}

#[test]
fn cuda_licm_hoists_read_only_load_above_loop() {
    // Loads from ReadOnly buffers are loop-invariant: the substrate
    // forbids writes to ReadOnly buffers, so the value never changes
    // mid-loop. LICM should hoist `let val = Load(input, 0)` even
    // though it contains a Load.
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(4),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                // Loop-invariant Load from ReadOnly buffer.
                Node::let_bind("val", Expr::load("input", Expr::u32(0))),
                Node::store("buf", Expr::var("i"), Expr::var("val")),
            ],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    // `let val` should appear at top level (above the Loop).
    let top_val = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "val"));
    assert!(
        top_val,
        "ReadOnly Load `let val` must be hoisted above the Loop; body={body:?}"
    );
}
