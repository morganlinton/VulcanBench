//! End-to-end test: constant propagation in the GPU pipeline.
//!
//! After const-fold + let-dedupe, the const-prop CPU rewrite turns
//! `Var(name)` into `LitU32(value)` whenever `name` was let-bound to
//! a literal in an enclosing scope. Subsequent DCE drops the now-
//! unused let bindings.

#![cfg(test)]

mod common;
#[path = "common/self_optimizer_const_prop_bool.rs"]
mod self_optimizer_const_prop_bool;

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
fn cuda_const_prop_replaces_var_with_literal() {
    // let a = 42
    // store buf 0 (Var a)
    //   ⇒ const-prop should rewrite the store to `store buf 0 42`
    //   ⇒ DCE drops the now-dead `let a = 42`
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(42)),
            Node::store("buf", Expr::u32(0), Expr::var("a")),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(42)),
            "expected LitU32(42) after const-prop; got {value:?}"
        );
    }
    // The `let a = 42` should be dropped by DCE since its only use
    // was rewritten to a literal.
    let has_let_a = body
        .iter()
        .any(|n| matches!(n, Node::Let { name, .. } if name.as_str() == "a"));
    assert!(
        !has_let_a,
        "DCE should drop `let a` after const-prop replaced its only use; \
         body={body:?}"
    );
}

#[test]
fn cuda_const_prop_cascades_through_dedupe() {
    // let a = 5
    // let b = 5     ← CSE rewrites RHS to Var(a)
    // store buf 0 (Var b)
    //   After CSE+let-dedupe: `let b = Var(a)`.
    //   After const-prop: `let b = 5` (since Var(a) → 5), then
    //   store turns into `store buf 0 5`. DCE drops both lets.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(5)),
            Node::let_bind("b", Expr::u32(5)),
            Node::store("buf", Expr::u32(0), Expr::var("b")),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);

    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(5)),
            "expected LitU32(5) after cascading const-prop+dedupe; got {value:?}"
        );
    }
    // Both lets should be dead after the cascading rewrite.
    let any_let = body.iter().any(|n| matches!(n, Node::Let { .. }));
    assert!(
        !any_let,
        "DCE should drop both lets after const-prop cascades; body={body:?}"
    );
}

#[test]
fn cuda_const_prop_folds_i32_arithmetic() {
    // store buf 0 (LitI32(7) - LitI32(10))   →  store buf 0 LitI32(-3)
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::sub(Expr::i32(7), Expr::i32(10)),
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitI32(-3)),
            "expected LitI32(-3) after i32 fold; got {value:?}"
        );
    }
}

#[test]
fn cuda_const_prop_folds_i32_via_var() {
    // let n = LitI32(-5)
    // store buf 0 (Var(n) * LitI32(3))   →  store buf 0 LitI32(-15)
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("n", Expr::i32(-5)),
            Node::store("buf", Expr::u32(0), Expr::mul(Expr::var("n"), Expr::i32(3))),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitI32(-15)),
            "expected LitI32(-15) after i32 fold via Var; got {value:?}"
        );
    }
}

#[test]
fn cuda_select_const_true_collapses_to_arm() {
    // store buf 0 (Select(true, 1, 99)) → store buf 0 1
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::Select {
                cond: Box::new(Expr::bool(true)),
                true_val: Box::new(Expr::u32(1)),
                false_val: Box::new(Expr::u32(99)),
            },
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(1)), "got {value:?}");
    }
}

#[test]
fn cuda_select_const_zero_keeps_false_arm() {
    // store buf 0 (Select(0u32, 1, 7)) → store buf 0 7
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::Select {
                cond: Box::new(Expr::u32(0)),
                true_val: Box::new(Expr::u32(1)),
                false_val: Box::new(Expr::u32(7)),
            },
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(matches!(value, Expr::LitU32(7)), "got {value:?}");
    }
}

#[test]
fn cuda_const_prop_folds_u32_min_max_absdiff() {
    // store buf 0 (Min(20, 7))     → store buf 0 7
    // store buf 0 (Max(20, 7))     → store buf 0 20
    // store buf 0 (AbsDiff(20, 7)) → store buf 0 13
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    for (op, expected) in [
        (BinOp::Min, 7u32),
        (BinOp::Max, 20u32),
        (BinOp::AbsDiff, 13u32),
    ] {
        let p = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![Node::store(
                "buf",
                Expr::u32(0),
                binop(op, Expr::u32(20), Expr::u32(7)),
            )],
        );
        let out = run_pipeline(p);
        let body = body_of(&out);
        let store = body
            .iter()
            .find(|n| matches!(n, Node::Store { .. }))
            .expect("store survives");
        if let Node::Store { value, .. } = store {
            match value {
                Expr::LitU32(v) if *v == expected => {}
                other => panic!("expected LitU32({expected}) after {op:?} fold; got {other:?}"),
            }
        }
    }
}

#[test]
fn cuda_const_prop_folds_saturating_arithmetic() {
    // SaturatingAdd(MAX-3, 10) → MAX (clamps).
    // SaturatingSub(5, 8)      → 0.
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            binop(BinOp::SaturatingAdd, Expr::u32(u32::MAX - 3), Expr::u32(10)),
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        match value {
            Expr::LitU32(v) if *v == u32::MAX => {}
            other => panic!("expected LitU32(MAX) after SaturatingAdd fold; got {other:?}"),
        }
    }

    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            binop(BinOp::SaturatingSub, Expr::u32(5), Expr::u32(8)),
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::LitU32(0)),
            "expected LitU32(0) after SaturatingSub fold; got {value:?}"
        );
    }
}

#[test]
fn cuda_const_prop_folds_unop_literals() {
    use vyre::ir::model::types::UnOp;
    fn unop(op: UnOp, operand: Expr) -> Expr {
        Expr::UnOp {
            op,
            operand: Box::new(operand),
        }
    }
    // BitNot(0xF0F0F0F0) → 0x0F0F0F0F
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            unop(UnOp::BitNot, Expr::u32(0xF0F0_F0F0)),
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        match value {
            Expr::LitU32(v) if *v == 0x0F0F_0F0F => {}
            other => panic!("expected LitU32(0x0F0F0F0F) after BitNot fold; got {other:?}"),
        }
    }

    // Popcount(0xFF) → 8
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            unop(UnOp::Popcount, Expr::u32(0xFF)),
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(8)),
            "expected LitU32(8) after Popcount fold; got {value:?}"
        );
    }

    // LogicalNot(true) → false
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("b", Expr::bool(true)),
            Node::if_then_else(
                unop(UnOp::LogicalNot, Expr::var("b")),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(99))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(99)),
            "LogicalNot(true) → false should pick the else arm; got {value:?}"
        );
    }
}

#[test]
fn cuda_const_prop_folds_bool_binops() {
    // (true && false) → false; gates the else branch.
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::if_then_else(
            binop(BinOp::And, Expr::bool(true), Expr::bool(false)),
            vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            vec![Node::store("buf", Expr::u32(0), Expr::u32(7))],
        )],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    if let Some(Node::Store { value, .. }) = body.iter().find(|n| matches!(n, Node::Store { .. })) {
        assert!(
            matches!(value, Expr::LitU32(7)),
            "(true && false) → false should pick else; got {value:?}"
        );
    }
}

#[test]
fn cuda_const_prop_simplifies_bool_eq_with_literal() {
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    // (b == true) collapses to Var(b); the If picks the then arm
    // when b is true at runtime, else the else arm. Without folding
    // the cond stays a BinOp; with folding it becomes Var(b).
    use vyre::ir::{BufferAccess, BufferDecl, DataType};
    let p = Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("buf", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind(
                "b",
                Expr::eq(Expr::load("input", Expr::u32(0)), Expr::u32(7)),
            ),
            Node::if_then_else(
                binop(BinOp::Eq, Expr::var("b"), Expr::bool(true)),
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
                vec![Node::store("buf", Expr::u32(0), Expr::u32(2))],
            ),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let if_node = body.iter().find(|n| matches!(n, Node::If { .. }));
    if let Some(Node::If { cond, .. }) = if_node {
        assert!(
            !matches!(cond, Expr::BinOp { op: BinOp::Eq, .. }),
            "(b == true) must simplify to Var(b); got cond={cond:?}"
        );
    }
}

#[test]
fn cuda_const_prop_preserves_non_literal_var() {
    // let a = Load(buf, 0)   ← NOT a literal; const-prop must skip
    // store buf 0 (Var a)
    //   The store keeps its `Var(a)` reference.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::load("buf", Expr::u32(0))),
            Node::store("buf", Expr::u32(0), Expr::var("a")),
        ],
    );
    let out = run_pipeline(p);
    let body = body_of(&out);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        assert!(
            matches!(value, Expr::Var(n) if n.as_str() == "a"),
            "Var(a) must survive when `a` is not let-bound to a literal; got {value:?}"
        );
    }
}
