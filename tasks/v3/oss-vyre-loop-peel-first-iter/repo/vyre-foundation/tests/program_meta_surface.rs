//! Surface tests for `Program` metadata and query methods.
//!
//! Exercises `buffer()`, `structural_eq()`, `stats()`, `workgroup_size()`,
//! `output_buffer_indices()`, and related accessors.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn buffer_lookup_finds_existing() {
    let prog = Program::wrapped(
        vec![BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert!(prog.buffer("x").is_some());
    assert_eq!(prog.buffer("x").unwrap().count(), 4);
}

#[test]
fn buffer_lookup_returns_none_for_missing() {
    let prog = Program::wrapped(
        vec![BufferDecl::storage("x", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert!(prog.buffer("y").is_none());
}

#[test]
fn structural_eq_is_reflexive() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert!(prog.structural_eq(&prog));
}

#[test]
fn structural_eq_detects_different_buffers() {
    let a = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let b = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(2)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert!(!a.structural_eq(&b));
}

#[test]
fn structural_eq_detects_different_nodes() {
    let a = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let b = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::barrier(), Node::Return],
    );
    assert!(!a.structural_eq(&b));
}

#[test]
fn workgroup_size_roundtrips() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [8, 4, 2],
        vec![Node::Return],
    );
    assert_eq!(prog.workgroup_size(), [8, 4, 2]);
}

#[test]
fn output_buffer_indices_find_outputs() {
    let prog = Program::wrapped(
        vec![
            BufferDecl::read("in", 0, DataType::U32).with_count(1),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
            BufferDecl::storage(
                "write_only",
                2,
                vyre::ir::BufferAccess::WriteOnly,
                DataType::U32,
            )
            .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    let indices = prog.output_buffer_indices();
    assert_eq!(indices, &[1, 2]);
}

#[test]
fn output_buffer_indices_empty_when_no_outputs() {
    let prog = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let indices = prog.output_buffer_indices();
    assert!(indices.is_empty());
}

#[test]
fn stats_on_empty_program() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let stats = prog.stats();
    assert!(!stats.subgroup_ops());
    assert!(!stats.f16());
    assert!(!stats.bf16());
    assert!(!stats.f64());
    assert!(!stats.async_dispatch());
    assert!(!stats.indirect_dispatch());
    assert!(!stats.tensor_ops());
    assert!(!stats.trap());
}

#[test]
fn stats_detects_trap() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::trap(Expr::u32(0), "test"), Node::Return],
    );
    assert!(prog.stats().trap());
}

#[test]
fn has_indirect_dispatch_false_by_default() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert!(!prog.has_indirect_dispatch());
}

#[test]
fn is_explicit_noop_true_for_empty_region() {
    let prog = Program::wrapped(vec![], [1, 1, 1], vec![]);
    assert!(prog.is_explicit_noop());
}

#[test]
fn is_explicit_noop_false_for_store() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
    );
    assert!(!prog.is_explicit_noop());
}

#[test]
fn fingerprint_is_deterministic() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert_eq!(prog.fingerprint(), prog.fingerprint());
}

#[test]
fn vsa_fingerprint_is_deterministic() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert_eq!(prog.vsa_fingerprint(), prog.vsa_fingerprint());
}

#[test]
fn buffers_returns_all_declarations() {
    let prog = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(2),
            BufferDecl::output("c", 2, DataType::U32).with_count(3),
        ],
        [1, 1, 1],
        vec![Node::Return],
    );
    assert_eq!(prog.buffers().len(), 3);
}

#[test]
fn entry_returns_node_slice() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    // Program::wrapped wraps nodes in a single top-level Region.
    assert_eq!(prog.entry().len(), 1);
}
