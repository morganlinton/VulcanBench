//! Coverage for the new const-fold ops added in this session:
//! Shl, Shr, Div (with safe-zero guard), Mod (with safe-zero guard).

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

fn extract_store_value(p: &Program) -> Expr {
    let body = body_of(p);
    let store = body
        .iter()
        .find(|n| matches!(n, Node::Store { .. }))
        .expect("store survives");
    if let Node::Store { value, .. } = store {
        value.clone()
    } else {
        unreachable!()
    }
}

#[test]
fn cuda_const_fold_shl() {
    // 5u32 << 3u32 = 40u32
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::shl(Expr::u32(5), Expr::u32(3)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    assert!(
        matches!(value, Expr::LitU32(40)),
        "expected LitU32(40); got {value:?}"
    );
}

#[test]
fn cuda_const_fold_shr() {
    // 80u32 >> 2u32 = 20u32
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::shr(Expr::u32(80), Expr::u32(2)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    assert!(
        matches!(value, Expr::LitU32(20)),
        "expected LitU32(20); got {value:?}"
    );
}

#[test]
fn cuda_const_fold_div_nonzero() {
    // 100u32 / 4u32 = 25u32
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::div(Expr::u32(100), Expr::u32(4)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    assert!(
        matches!(value, Expr::LitU32(25)),
        "expected LitU32(25); got {value:?}"
    );
}

#[test]
fn cuda_const_fold_div_by_zero_skipped() {
    // 100u32 / 0u32: must NOT fold (divide by zero would crash the
    // compiler at emit time). The original Div Expr survives.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::div(Expr::u32(100), Expr::u32(0)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    // Should still be a BinOp(Div, 100, 0)  -  not folded.
    match value {
        Expr::BinOp { left, right, .. } => {
            assert!(matches!(left.as_ref(), Expr::LitU32(100)));
            assert!(matches!(right.as_ref(), Expr::LitU32(0)));
        }
        other => panic!("expected BinOp(Div, 100, 0); got {other:?}"),
    }
}

#[test]
fn cuda_const_fold_rem_nonzero() {
    // 17u32 % 5u32 = 2u32
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::rem(Expr::u32(17), Expr::u32(5)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    assert!(
        matches!(value, Expr::LitU32(2)),
        "expected LitU32(2); got {value:?}"
    );
}

#[test]
fn cuda_const_fold_rem_by_zero_skipped() {
    // 17u32 % 0u32: must NOT fold.
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::rem(Expr::u32(17), Expr::u32(0)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    match value {
        Expr::BinOp { left, right, .. } => {
            assert!(matches!(left.as_ref(), Expr::LitU32(17)));
            assert!(matches!(right.as_ref(), Expr::LitU32(0)));
        }
        other => panic!("expected BinOp(Rem, 17, 0); got {other:?}"),
    }
}

#[test]
fn cuda_const_fold_chained_shifts() {
    // (1u32 << 4u32) >> 2u32 = 16 >> 2 = 4
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            Expr::shr(Expr::shl(Expr::u32(1), Expr::u32(4)), Expr::u32(2)),
        )],
    );
    let out = run_pipeline(p);
    let value = extract_store_value(&out);
    assert!(
        matches!(value, Expr::LitU32(4)),
        "expected LitU32(4); got {value:?}"
    );
}

#[test]
fn cuda_const_fold_saturating_mul_gpu() {
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    // Non-overflowing: 7 * 9 = 63
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            binop(BinOp::SaturatingMul, Expr::u32(7), Expr::u32(9)),
        )],
    );
    let value = extract_store_value(&run_pipeline(p));
    assert!(
        matches!(value, Expr::LitU32(63)),
        "expected LitU32(63); got {value:?}"
    );

    // Overflowing: 0xFFFFFFFE * 2 = 0xFFFFFFFFFFFFFFFC, saturates to MAX
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            binop(BinOp::SaturatingMul, Expr::u32(0xFFFF_FFFE), Expr::u32(2)),
        )],
    );
    let value = extract_store_value(&run_pipeline(p));
    match value {
        Expr::LitU32(v) if v == u32::MAX => {}
        other => panic!("overflowing SaturatingMul should saturate; got {other:?}"),
    }

    // Zero left operand: result is 0 regardless of right
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::store(
            "buf",
            Expr::u32(0),
            binop(BinOp::SaturatingMul, Expr::u32(0), Expr::u32(99)),
        )],
    );
    let value = extract_store_value(&run_pipeline(p));
    assert!(
        matches!(value, Expr::LitU32(0)),
        "0 * x must fold to 0; got {value:?}"
    );
}

#[test]
fn cuda_const_fold_eq_lt_gt_le_ge_ne_gpu() {
    // GPU const-fold should evaluate every comparison op on literal
    // operands. The kernel writes 0/1 into the value buffer; the
    // decoder reconstructs LitU32(0|1).
    use vyre::ir::BinOp;
    fn binop(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::BinOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        }
    }
    for (op, expected) in [
        (BinOp::Eq, 0u32),
        (BinOp::Ne, 1),
        (BinOp::Lt, 1),
        (BinOp::Gt, 0),
        (BinOp::Le, 1),
        (BinOp::Ge, 0),
    ] {
        let p = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![Node::store(
                "buf",
                Expr::u32(0),
                binop(op, Expr::u32(3), Expr::u32(7)),
            )],
        );
        let value = extract_store_value(&run_pipeline(p));
        match value {
            Expr::LitU32(v) if v == expected => {}
            Expr::LitBool(b) if (b as u32) == expected => {}
            other => panic!("{op:?}(3, 7) expected {expected}; got {other:?}"),
        }
    }
}
