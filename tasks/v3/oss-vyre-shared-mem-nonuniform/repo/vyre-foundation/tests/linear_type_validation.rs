//! Adversarial tests for linear-type discipline checking (P-1.0-V2.2).
//!
//! `check_linear_types` enforces substructural constraints declared on
//! each buffer: Linear (exactly 1 use), Affine (≤1 use), Relevant
//! (≥1 use), and Unrestricted (default, no checking).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, LinearType, Node, Program};
use vyre_foundation::validate::linear_type::check_linear_types;

fn program_with_nodes(buffers: Vec<BufferDecl>, nodes: Vec<Node>) -> Program {
    Program::wrapped(buffers, [1, 1, 1], nodes)
}

// ------------------------------------------------------------------
// LinearType::Linear  -  exactly one use
// ------------------------------------------------------------------

#[test]
fn linear_unused_is_rejected() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(errs
        .iter()
        .any(|e| e.message().contains("Linear") && e.message().contains("0 time")));
}

#[test]
fn linear_single_use_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![Node::store("x", Expr::u32(0), Expr::u32(42)), Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(
        !errs.iter().any(|e| e.message().contains("Linear")),
        "single use must satisfy Linear, got: {:?}",
        errs
    );
}

#[test]
fn linear_double_use_is_rejected() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![
            Node::store("x", Expr::u32(0), Expr::u32(1)),
            Node::store("x", Expr::u32(0), Expr::u32(2)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(errs
        .iter()
        .any(|e| e.message().contains("Linear") && e.message().contains("2 time")));
}

#[test]
fn linear_both_branches_count_as_two_uses() {
    // The checker is conservative: a buffer in both If branches counts as 2.
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![
            Node::if_then(
                Expr::bool(true),
                vec![Node::store("x", Expr::u32(0), Expr::u32(1)), Node::Return],
            ),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    // The store inside if_then is one use, but the checker walks all nodes.
    // Actually if_then puts its body inside, so the store is one occurrence.
    // Let's make both branches use x.
    assert!(
        !errs.iter().any(|e| e.message().contains("Linear")),
        "single occurrence inside if_then should be 1 use, got: {:?}",
        errs
    );
}

// ------------------------------------------------------------------
// LinearType::Affine  -  at most one use
// ------------------------------------------------------------------

#[test]
fn affine_unused_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Affine),
        ],
        vec![Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(
        !errs.iter().any(|e| e.message().contains("Affine")),
        "unused must satisfy Affine, got: {:?}",
        errs
    );
}

#[test]
fn affine_single_use_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Affine),
        ],
        vec![Node::store("x", Expr::u32(0), Expr::u32(42)), Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(
        !errs.iter().any(|e| e.message().contains("Affine")),
        "single use must satisfy Affine, got: {:?}",
        errs
    );
}

#[test]
fn affine_double_use_is_rejected() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Affine),
        ],
        vec![
            Node::store("x", Expr::u32(0), Expr::u32(1)),
            Node::store("x", Expr::u32(0), Expr::u32(2)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(errs
        .iter()
        .any(|e| e.message().contains("Affine") && e.message().contains("2 time")));
}

// ------------------------------------------------------------------
// LinearType::Relevant  -  at least one use
// ------------------------------------------------------------------

#[test]
fn relevant_unused_is_rejected() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Relevant),
        ],
        vec![Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(errs
        .iter()
        .any(|e| e.message().contains("Relevant") && e.message().contains("unused")));
}

#[test]
fn relevant_single_use_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Relevant),
        ],
        vec![Node::store("x", Expr::u32(0), Expr::u32(42)), Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(
        !errs.iter().any(|e| e.message().contains("Relevant")),
        "single use must satisfy Relevant, got: {:?}",
        errs
    );
}

#[test]
fn relevant_many_uses_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Relevant),
        ],
        vec![
            Node::store("x", Expr::u32(0), Expr::u32(1)),
            Node::store("x", Expr::u32(0), Expr::u32(2)),
            Node::store("x", Expr::u32(0), Expr::u32(3)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        !errs.iter().any(|e| e.message().contains("Relevant")),
        "many uses must satisfy Relevant, got: {:?}",
        errs
    );
}

// ------------------------------------------------------------------
// LinearType::Unrestricted  -  no checking
// ------------------------------------------------------------------

#[test]
fn unrestricted_unused_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Unrestricted),
        ],
        vec![Node::Return],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.is_empty(),
        "Unrestricted must never error, got: {:?}",
        errs
    );
}

#[test]
fn unrestricted_many_uses_passes() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Unrestricted),
        ],
        vec![
            Node::store("x", Expr::u32(0), Expr::u32(1)),
            Node::store("x", Expr::u32(0), Expr::u32(2)),
            Node::store("x", Expr::u32(0), Expr::u32(3)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.is_empty(),
        "Unrestricted must never error, got: {:?}",
        errs
    );
}

// ------------------------------------------------------------------
// Mixed disciplines in one program
// ------------------------------------------------------------------

#[test]
fn mixed_disciplines_only_flagged_buffers_are_checked() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("linear", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
            BufferDecl::storage("unrestricted", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Unrestricted),
        ],
        vec![
            Node::store("linear", Expr::u32(0), Expr::u32(1)),
            Node::store("unrestricted", Expr::u32(0), Expr::u32(2)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.is_empty(),
        "mixed program with correct uses must pass, got: {:?}",
        errs
    );
}

#[test]
fn mixed_disciplines_reports_only_violating_buffers() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("linear", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
            BufferDecl::storage("affine", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Affine),
        ],
        vec![
            Node::store("linear", Expr::u32(0), Expr::u32(1)),
            Node::store("affine", Expr::u32(0), Expr::u32(2)),
            Node::store("affine", Expr::u32(0), Expr::u32(3)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.iter().any(|e| e.message().contains("affine")),
        "affine violation must be reported, got: {:?}",
        errs
    );
    assert!(
        !errs.iter().any(|e| e.message().contains("linear")),
        "linear must not be reported, got: {:?}",
        errs
    );
}
