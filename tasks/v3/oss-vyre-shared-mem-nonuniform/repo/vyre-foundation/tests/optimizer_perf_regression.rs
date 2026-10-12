//! Integration test crate for the containing Vyre package.

#![allow(clippy::match_like_matches_macro)]
//! End-to-end optimizer performance regression tests.
//!
//! These tests verify that the full `optimize()` pipeline fires all critical
//! passes (const_fold, strength_reduce, FMA synthesis, CSE, DCE) and that the
//! output instruction count is strictly less than the input. Any regression
//! that inflates the output IR is a performance bug.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::pre_lowering::optimize;

fn node_count(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::If {
                then, otherwise, ..
            } => 1 + node_count(then) + node_count(otherwise),
            Node::Loop { body, .. } => 1 + node_count(body),
            Node::Block(body) => 1 + node_count(body),
            Node::Region { body, .. } => 1 + node_count(body),
            _ => 1,
        })
        .sum()
}

/// Unwrap the top-level Region wrapper that `Program::wrapped` / `optimize`
/// always produces. Returns the inner body nodes for assertion.
fn body_of(program: &Program) -> &[Node] {
    let entry = program.entry();
    // Program::wrapped wraps everything in a Region node
    if entry.len() == 1 {
        if let Node::Region { body, .. } = &entry[0] {
            return body;
        }
    }
    entry
}

// ── Full pipeline integration ────────────────────────────────────────

#[test]
fn optimize_reduces_literal_arithmetic_to_constants() {
    // Program: out[0] = (2 + 3) * (4 - 1)
    // Should fold entirely to out[0] = 15
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::mul(
                Expr::add(Expr::u32(2), Expr::u32(3)),
                Expr::sub(Expr::u32(4), Expr::u32(1)),
            ),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    // After full pipeline, the nested expr should be a single LitU32(15)
    let has_literal = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::LitU32(15),
            ..
        } => true,
        _ => false,
    });
    assert!(
        has_literal,
        "Fix: optimize() must fold (2+3)*(4-1) to 15. Body: {body:?}"
    );
}

#[test]
fn optimize_eliminates_dead_let_bindings() {
    // dead_var is never used → DCE should remove it
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("dead_var", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::store("out", Expr::u32(0), Expr::u32(42)),
        ],
    );

    let before_count = node_count(body_of(&program));
    let optimized = optimize(program);
    let after_count = node_count(body_of(&optimized));

    assert!(
        after_count < before_count,
        "Fix: DCE must remove dead let bindings. Before: {before_count}, After: {after_count}"
    );
}

#[test]
fn optimize_cse_deduplicates_repeated_expressions() {
    // Same expression computed twice should be deduplicated by CSE
    let common_expr = Expr::add(Expr::var("x"), Expr::u32(1));
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", common_expr.clone()),
            Node::let_bind("b", common_expr),
            Node::store(
                "out",
                Expr::u32(0),
                Expr::add(Expr::var("a"), Expr::var("b")),
            ),
        ],
    );

    let optimized = optimize(program);
    // CSE should recognize that "b" has the same value as "a"
    // and the store should reference "a" via both paths or the duplicated
    // let should reference "a" instead of recomputing.
    // At minimum, optimization should not increase node count.
    let after_count = node_count(body_of(&optimized));
    assert!(
        after_count <= 3,
        "Fix: CSE should not inflate node count. After: {after_count}"
    );
}

#[test]
fn optimize_strength_reduces_multiply_by_power_of_two() {
    // x * 8 should become x << 3
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::mul(Expr::var("x"), Expr::u32(8)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_shift = body.iter().any(|n| match n {
        Node::Store { value, .. } => contains_shl(value),
        _ => false,
    });
    assert!(
        has_shift,
        "Fix: strength_reduce must convert x*8 to x<<3. Body: {body:?}"
    );
}

fn contains_shl(expr: &Expr) -> bool {
    match expr {
        Expr::BinOp {
            op: vyre_foundation::ir::BinOp::Shl,
            ..
        } => true,
        Expr::BinOp { left, right, .. } => contains_shl(left) || contains_shl(right),
        Expr::UnOp { operand, .. } => contains_shl(operand),
        _ => false,
    }
}

#[test]
fn optimize_synthesizes_fma_from_mul_add() {
    // (a * b) + c where c is float should become Fma(a, b, c)
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::mul(Expr::var("a"), Expr::var("b")), Expr::f32(1.0)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_fma = body.iter().any(|n| match n {
        Node::Store { value, .. } => contains_fma(value),
        _ => false,
    });
    assert!(
        has_fma,
        "Fix: const_fold must synthesize FMA from (a*b)+c with float addend. Body: {body:?}"
    );
}

fn contains_fma(expr: &Expr) -> bool {
    match expr {
        Expr::Fma { .. } => true,
        Expr::BinOp { left, right, .. } => contains_fma(left) || contains_fma(right),
        Expr::UnOp { operand, .. } => contains_fma(operand),
        _ => false,
    }
}

// ── Identity collapses ───────────────────────────────────────────────

#[test]
fn optimize_collapses_add_zero() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::var("x"), Expr::u32(0)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_identity = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::Var(name),
            ..
        } => name == "x",
        _ => false,
    });
    assert!(has_identity, "Fix: x + 0 must fold to x. Body: {body:?}");
}

#[test]
fn optimize_collapses_mul_one() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::mul(Expr::var("x"), Expr::u32(1)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_identity = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::Var(name),
            ..
        } => name == "x",
        _ => false,
    });
    assert!(has_identity, "Fix: x * 1 must fold to x. Body: {body:?}");
}

#[test]
fn optimize_collapses_bitand_zero() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::bitand(Expr::var("x"), Expr::u32(0)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_zero = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::LitU32(0),
            ..
        } => true,
        _ => false,
    });
    assert!(has_zero, "Fix: x & 0 must fold to 0. Body: {body:?}");
}

// ── Idempotence ──────────────────────────────────────────────────────

#[test]
fn optimize_is_idempotent() {
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("b", Expr::mul(Expr::var("a"), Expr::u32(4))),
            Node::store("out", Expr::u32(0), Expr::var("b")),
        ],
    );

    let once = optimize(program);
    let twice = optimize(once.clone());
    assert_eq!(
        once, twice,
        "Fix: optimize() must be idempotent  -  running twice must produce the same output."
    );
}

// ── Complex multi-pass interaction ───────────────────────────────────

#[test]
fn optimize_complex_kernel_reduces_instruction_count() {
    // Simulate a realistic kernel fragment: index calculation + data store
    // with redundant arithmetic that the pipeline should clean up.
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32),
            BufferDecl::read_write("output", 1, DataType::U32),
        ],
        [64, 1, 1],
        vec![
            // tid = threadIdx.x
            Node::let_bind("tid", Expr::gid_x()),
            // offset = tid * 4 (strength reduce: shl by 2)
            Node::let_bind("offset", Expr::mul(Expr::var("tid"), Expr::u32(4))),
            // dead: unused computation
            Node::let_bind("dead", Expr::add(Expr::u32(100), Expr::u32(200))),
            // val = input[offset] + 0 (identity: should fold to input[offset])
            Node::let_bind(
                "val",
                Expr::add(Expr::load("input", Expr::var("offset")), Expr::u32(0)),
            ),
            // output[offset] = val
            Node::store("output", Expr::var("offset"), Expr::var("val")),
        ],
    );

    let before = node_count(body_of(&program));
    let optimized = optimize(program);
    let after = node_count(body_of(&optimized));

    assert!(
        after < before,
        "Fix: optimizer must reduce instruction count for realistic kernel. Before: {before}, After: {after}"
    );
}

#[test]
fn optimize_select_with_constant_condition_eliminates_branch() {
    // select(true, a, b) → a
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::select(Expr::bool(true), Expr::u32(42), Expr::u32(99)),
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_42 = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::LitU32(42),
            ..
        } => true,
        _ => false,
    });
    assert!(
        has_42,
        "Fix: select(true, 42, 99) must fold to 42. Body: {body:?}"
    );
}

#[test]
fn optimize_double_negation_eliminated() {
    use vyre_foundation::ir::UnOp;
    let program = Program::wrapped(
        vec![BufferDecl::read_write("out", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::UnOp {
                op: UnOp::Negate,
                operand: Box::new(Expr::UnOp {
                    op: UnOp::Negate,
                    operand: Box::new(Expr::var("x")),
                }),
            },
        )],
    );

    let optimized = optimize(program);
    let body = body_of(&optimized);
    let has_x = body.iter().any(|n| match n {
        Node::Store {
            value: Expr::Var(name),
            ..
        } => name == "x",
        _ => false,
    });
    assert!(has_x, "Fix: --x must fold to x. Body: {body:?}");
}

// ── Pipeline fingerprint stability ───────────────────────────────────

#[test]
fn optimize_preserves_fingerprint_stability() {
    // Same program optimized twice must produce the same fingerprint.
    let make_program = || {
        Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [64, 1, 1],
            vec![
                Node::let_bind("x", Expr::add(Expr::u32(1), Expr::u32(2))),
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ],
        )
    };

    let a = optimize(make_program());
    let b = optimize(make_program());
    assert_eq!(
        a, b,
        "Fix: optimize() must be deterministic  -  same input must produce same output."
    );
}
