//! Invariant tests for `Program` constructors and rewrite methods.
//!
//! `Program::empty`, `Program::new`, `Program::wrapped`, and the
//! `with_rewritten_*` family must preserve or correctly update metadata.

use vyre::ir::{BufferDecl, DataType, Node, Program};

#[test]
fn empty_program_has_default_workgroup_size() {
    let prog = Program::empty();
    assert_eq!(prog.workgroup_size(), [1, 1, 1]);
}

#[test]
fn empty_program_has_no_buffers() {
    let prog = Program::empty();
    assert!(prog.buffers().is_empty());
}

#[test]
fn empty_program_has_single_region_entry() {
    let prog = Program::empty();
    // Program::empty() calls wrapped() which wraps in a Region.
    assert_eq!(prog.entry().len(), 1);
}

#[test]
fn empty_program_is_explicit_noop() {
    let prog = Program::empty();
    assert!(prog.is_explicit_noop());
}

#[test]
#[allow(deprecated)]
fn new_program_preserves_workgroup_size() {
    let prog = Program::new(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [8, 4, 2],
        vec![Node::Return],
    );
    assert_eq!(prog.workgroup_size(), [8, 4, 2]);
}

#[test]
fn wrapped_program_preserves_workgroup_size() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [16, 1, 1],
        vec![Node::Return],
    );
    assert_eq!(prog.workgroup_size(), [16, 1, 1]);
}

#[test]
fn with_rewritten_entry_changes_entry() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let rewritten = prog.with_rewritten_entry(vec![Node::barrier(), Node::Return]);
    // Both have a single Region wrapper; structural_eq looks inside.
    assert!(!rewritten.structural_eq(&prog));
}

#[test]
fn with_rewritten_buffers_changes_buffers() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let rewritten = prog.with_rewritten_buffers(vec![
        BufferDecl::output("out", 0, DataType::U32).with_count(2)
    ]);
    assert_eq!(rewritten.buffer("out").unwrap().count(), 2);
    assert!(!rewritten.structural_eq(&prog));
}

#[test]
fn with_rewritten_entry_preserves_buffers() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let rewritten = prog.with_rewritten_entry(vec![Node::barrier(), Node::Return]);
    assert_eq!(rewritten.buffers().len(), 1);
    assert_eq!(rewritten.buffer("out").unwrap().count(), 1);
}

#[test]
fn with_rewritten_buffers_preserves_entry() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let rewritten = prog.with_rewritten_buffers(vec![
        BufferDecl::output("out", 0, DataType::U32).with_count(2)
    ]);
    assert_eq!(rewritten.entry(), prog.entry());
}

#[test]
fn entry_op_id_roundtrips() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_entry_op_id("my::op");
    assert_eq!(prog.entry_op_id(), Some("my::op"));
}

#[test]
fn entry_op_id_none_by_default() {
    let prog = Program::empty();
    assert_eq!(prog.entry_op_id(), None);
}

#[test]
fn into_entry_vec_consumes_program() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let nodes = prog.into_entry_vec();
    assert_eq!(nodes.len(), 1);
}

#[test]
fn clone_preserves_structural_eq() {
    let prog = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::Return],
    );
    let cloned = prog.clone();
    assert!(prog.structural_eq(&cloned));
}
