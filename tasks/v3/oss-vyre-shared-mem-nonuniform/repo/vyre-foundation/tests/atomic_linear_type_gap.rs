//! Regression tests for atomic expressions in linear-type checking.
//!
//! `Expr::Atomic` is a real buffer use and must contribute to Linear,
//! Affine, and Relevant discipline counts exactly like loads, stores, and
//! buffer-length reads.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, LinearType, Node, Program};
use vyre_foundation::validate::linear_type::check_linear_types;

fn program_with_nodes(buffers: Vec<BufferDecl>, nodes: Vec<Node>) -> Program {
    Program::wrapped(buffers, [1, 1, 1], nodes)
}

/// A `Linear` buffer used only via `atomic_add` counts as exactly 1 use.
#[test]
fn linear_buffer_used_only_via_atomic_counts_as_one_use() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![
            Node::let_bind("_", Expr::atomic_add("x", Expr::u32(0), Expr::u32(1))),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.is_empty(),
        "atomic_add must count as a use for LinearType::Linear"
    );
}

/// A `Relevant` buffer used only via `atomic_add` counts as at least 1 use.
#[test]
fn relevant_buffer_used_only_via_atomic_counts_as_one_use() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Relevant),
        ],
        vec![
            Node::let_bind("_", Expr::atomic_add("x", Expr::u32(0), Expr::u32(1))),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(
        errs.is_empty(),
        "atomic_add must count as a use for LinearType::Relevant"
    );
}

/// An `Affine` buffer used only via `atomic_add` counts as exactly 1 use.
#[test]
fn affine_buffer_used_only_via_atomic_counts_as_one_use() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Affine),
        ],
        vec![
            Node::let_bind("_", Expr::atomic_add("x", Expr::u32(0), Expr::u32(1))),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert!(errs.is_empty());
}

/// When the buffer is used via both `atomic_add` and `store`, Linear fails
/// because the real total is 2 uses.
#[test]
fn linear_with_atomic_and_store_fails_as_two_uses() {
    let program = program_with_nodes(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_linear_type(LinearType::Linear),
        ],
        vec![
            Node::let_bind("_", Expr::atomic_add("x", Expr::u32(0), Expr::u32(1))),
            Node::store("x", Expr::u32(0), Expr::u32(2)),
            Node::Return,
        ],
    );
    let errs = check_linear_types(&program);
    assert_eq!(errs.len(), 1, "atomic + store must count as two uses");
    assert!(
        errs[0].message.contains("used 2 time(s)"),
        "linear-type violation should report the real atomic + store use count"
    );
}
