//! Regression test for VF-LOWER-001: chained-shift fusion with a+b >= 32
//! must emit Expr::u32(0) (correct u32 shift semantics), NOT x << 31
//! (saturating-clamp miscompile).
//!
//! Before the fix:  `(x << 16) << 16`  →  `x << 31`  (WRONG, non-zero for any x with bits <= 30)
//! After the fix:   `(x << 16) << 16`  →  `0`          (correct: all bits shifted out)

use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::strength_reduce::StrengthReduce;

/// Run StrengthReduce on a one-node program and extract the store value.
fn run_and_extract_store_value(program: Program) -> Expr {
    let result = StrengthReduce::transform(program);
    let entry = result.program.entry().to_vec();
    // Unwrap any wrapping Region that Program::wrapped may inject.
    let body = match entry.as_slice() {
        [Node::Region { body, .. }] => body.as_ref().to_vec(),
        other => other.to_vec(),
    };
    for node in &body {
        if let Node::Store { value, .. } = node {
            return value.clone();
        }
    }
    // Recurse one more level for wrapped bodies.
    panic!("No Store node found in result; body: {body:?}");
}

/// (x << 16) << 16 must reduce to 0, not x << 31.
#[test]
fn chained_shl_overflow_produces_zero_not_clamped_shift() {
    let inner_shl = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(16)),
    };
    let outer_shl = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(inner_shl),
        right: Box::new(Expr::u32(16)),
    };
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), outer_shl)],
    );

    let value = run_and_extract_store_value(program);
    assert_eq!(
        value,
        Expr::LitU32(0),
        "Fix: (x << 16) << 16 must fuse to LitU32(0) (all bits shifted out), \
         got {value:?} which would be x << 31 — a miscompile"
    );
}

/// (x << 1) << 31 also overflows: total = 32 > 31 → must emit 0.
#[test]
fn chained_shl_by_one_and_31_produces_zero() {
    let inner = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(1)),
    };
    let outer = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(inner),
        right: Box::new(Expr::u32(31)),
    };
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), outer)],
    );

    let value = run_and_extract_store_value(program);
    assert_eq!(
        value,
        Expr::LitU32(0),
        "Fix: (x << 1) << 31 must fuse to LitU32(0), got {value:?}"
    );
}

/// (x << 15) << 16 = total 31 — exactly in range; must NOT produce 0.
#[test]
fn chained_shl_exactly_31_stays_as_shift() {
    let inner = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(15)),
    };
    let outer = Expr::BinOp {
        op: BinOp::Shl,
        left: Box::new(inner),
        right: Box::new(Expr::u32(16)),
    };
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), outer)],
    );

    let value = run_and_extract_store_value(program);
    // total = 31, must fuse to x << 31, not 0.
    match &value {
        Expr::BinOp {
            op: BinOp::Shl,
            right,
            ..
        } => {
            assert_eq!(
                **right,
                Expr::LitU32(31),
                "Fix: (x << 15) << 16 must fuse to x << 31, right operand was {right:?}"
            );
        }
        other => panic!(
            "Fix: (x << 15) << 16 must remain a Shl BinOp fused to << 31, got {other:?}"
        ),
    }
}

/// (x >> 20) >> 15 = total 35 > 31 → must emit 0 for Shr too.
#[test]
fn chained_shr_overflow_produces_zero() {
    let inner = Expr::BinOp {
        op: BinOp::Shr,
        left: Box::new(Expr::var("x")),
        right: Box::new(Expr::u32(20)),
    };
    let outer = Expr::BinOp {
        op: BinOp::Shr,
        left: Box::new(inner),
        right: Box::new(Expr::u32(15)),
    };
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), outer)],
    );

    let value = run_and_extract_store_value(program);
    assert_eq!(
        value,
        Expr::LitU32(0),
        "Fix: (x >> 20) >> 15 must fuse to LitU32(0), got {value:?}"
    );
}
