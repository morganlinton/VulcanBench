//! Adversarial tests for `Program::canonicalized()` commutative reordering.
//!
//! Canonicalization must normalize `Add(literal, var)` to `Add(var, literal)`
//! so that structurally-equivalent programs have identical wire bytes.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

// Helper: extract the BinOp from the first Store node inside the top-level Region.
fn extract_binop(prog: &Program) -> (BinOp, Expr, Expr) {
    let body = match prog.entry().first() {
        Some(Node::Region { body, .. }) => body.as_ref(),
        other => panic!("expected Region, got: {:?}", other),
    };
    match body.first() {
        Some(Node::Store { value, .. }) => match value {
            Expr::BinOp { op, left, right } => {
                (op.clone(), left.as_ref().clone(), right.as_ref().clone())
            }
            other => panic!("expected BinOp, got: {:?}", other),
        },
        other => panic!("expected Store, got: {:?}", other),
    }
}

fn prog_with_expr(expr: Expr) -> Program {
    Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), expr), Node::Return],
    )
}

#[test]
fn canonical_add_literal_var_becomes_var_literal() {
    let prog = prog_with_expr(Expr::add(Expr::u32(5), Expr::var("x")));
    let canon = prog.canonicalized();
    let (op, left, right) = extract_binop(&canon);
    assert_eq!(op, BinOp::Add);
    assert!(matches!(left, Expr::Var { .. }));
    assert!(matches!(right, Expr::LitU32(5)));
}

#[test]
fn canonical_mul_literal_var_becomes_var_literal() {
    let prog = prog_with_expr(Expr::mul(Expr::u32(7), Expr::var("y")));
    let canon = prog.canonicalized();
    let (op, left, right) = extract_binop(&canon);
    assert_eq!(op, BinOp::Mul);
    assert!(matches!(left, Expr::Var { .. }));
    assert!(matches!(right, Expr::LitU32(7)));
}

#[test]
fn canonical_bitand_literal_var_becomes_var_literal() {
    let prog = prog_with_expr(Expr::bitand(Expr::u32(0xFF), Expr::var("mask")));
    let canon = prog.canonicalized();
    let (op, left, right) = extract_binop(&canon);
    assert_eq!(op, BinOp::BitAnd);
    assert!(matches!(left, Expr::Var { .. }));
    assert!(matches!(right, Expr::LitU32(0xFF)));
}

#[test]
fn canonical_var_literal_is_unchanged() {
    // Already in canonical order: var on left, literal on right.
    let prog = prog_with_expr(Expr::add(Expr::var("x"), Expr::u32(5)));
    let canon = prog.canonicalized();
    let (op, left, right) = extract_binop(&canon);
    assert_eq!(op, BinOp::Add);
    assert!(matches!(left, Expr::Var { .. }));
    assert!(matches!(right, Expr::LitU32(5)));
}

#[test]
fn canonical_non_commutative_op_is_unchanged() {
    // Sub is not commutative; order must be preserved.
    let prog = prog_with_expr(Expr::sub(Expr::u32(5), Expr::var("x")));
    let canon = prog.canonicalized();
    let (op, left, right) = extract_binop(&canon);
    assert_eq!(op, BinOp::Sub);
    assert!(matches!(left, Expr::LitU32(5)));
    assert!(matches!(right, Expr::Var { .. }));
}

#[test]
fn canonical_buffer_order_is_normalized() {
    // Buffers declared in one order should be sorted canonically.
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("z", 2, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("m", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let canon = prog.canonicalized();
    let names: Vec<_> = canon.buffers().iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["a", "m", "z"]);
}

#[test]
fn canonical_wire_bytes_are_deterministic() {
    // Two programs that differ only in buffer order must have identical
    // canonical wire bytes.
    let prog_a = Program::wrapped(
        vec![
            BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let prog_b = Program::wrapped(
        vec![
            BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let bytes_a = prog_a.canonical_wire_bytes().expect("must encode");
    let bytes_b = prog_b.canonical_wire_bytes().expect("must encode");
    assert_eq!(bytes_a, bytes_b);
}

#[test]
fn canonical_hash_matches_for_equivalent_programs() {
    let prog_a = prog_with_expr(Expr::add(Expr::u32(3), Expr::var("x")));
    let prog_b = prog_with_expr(Expr::add(Expr::var("x"), Expr::u32(3)));
    let hash_a = prog_a.canonical_wire_hash().expect("must hash");
    let hash_b = prog_b.canonical_wire_hash().expect("must hash");
    assert_eq!(hash_a, hash_b);
}
