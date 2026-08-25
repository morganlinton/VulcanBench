//! Regression tests for F-IR-22 and F-IR-23.
//!
//! F-IR-22: cross-arm buffer aliasing when a read-only arm is fused with
//! an atomic-write arm on the same buffer name.
//!
//! F-IR-23: self-composition of parser programs that share workgroup-local
//! scratch buffers.

use vyre_foundation::execution_plan::fusion::{
    fuse_programs, FusionError, FusionSelfAliasingError,
};
use vyre_foundation::ir::{AtomicOp, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

fn entry_body(program: &Program) -> &[Node] {
    match program.entry() {
        [Node::Region { body, .. }] => body.as_ref(),
        entry => entry,
    }
}

// ------------------------------------------------------------------
// F-IR-22: barrier insertion for read-then-atomic
// ------------------------------------------------------------------

#[test]
fn barrier_inserted_when_read_arm_precedes_atomic_arm() {
    let reader = Program::wrapped(
        vec![BufferDecl::read("state", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("snap", Expr::load("state", Expr::u32(0)))],
    );
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind(
            "old",
            Expr::Atomic {
                op: AtomicOp::Add,
                buffer: "state".into(),
                index: Box::new(Expr::u32(0)),
                expected: None,
                value: Box::new(Expr::u32(1)),
                ordering: MemoryOrdering::SeqCst,
            },
        )],
    );

    let fused = fuse_programs(&[reader, writer]).expect("Fix: fusion must succeed with barrier");

    let body = entry_body(&fused);

    let barrier_positions: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, Node::Barrier { .. }))
        .map(|(i, _)| i)
        .collect();

    assert_eq!(
        barrier_positions.len(),
        1,
        "Fix: fusion must insert exactly one SeqCst barrier between read and atomic arms"
    );
}

#[test]
fn barrier_inserted_when_read_arm_precedes_readwrite_store_arm() {
    // Adversarial: RO arm reads buf_a, second arm does a plain store
    // (read-write access) to buf_a. A barrier must still be inserted
    // because the second arm writes a buffer the first arm read.
    let reader = Program::wrapped(
        vec![BufferDecl::read("buf_a", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::load("buf_a", Expr::u32(0)))],
    );
    let writer = Program::wrapped(
        vec![BufferDecl::read_write("buf_a", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("buf_a", Expr::u32(0), Expr::u32(42))],
    );

    let fused = fuse_programs(&[reader, writer]).expect("Fix: fusion must succeed");
    let body = entry_body(&fused);

    assert!(
        body.iter().any(|n| matches!(n, Node::Barrier { .. })),
        "Fix: RO-then-RW on the same buffer must insert a Barrier"
    );
}

#[test]
fn no_barrier_when_arms_are_independent() {
    let a = Program::wrapped(
        vec![BufferDecl::read("a", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::load("a", Expr::u32(0)))],
    );
    let b = Program::wrapped(
        vec![BufferDecl::read_write("b", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("b", Expr::u32(0), Expr::u32(1))],
    );

    let fused = fuse_programs(&[a, b]).unwrap();
    let body = entry_body(&fused);
    assert!(
        !body.iter().any(|n| matches!(n, Node::Barrier { .. })),
        "Fix: independent arms must not get a spurious barrier"
    );
}

#[test]
fn no_barrier_when_both_arms_are_readonly_on_same_buffer() {
    // Adversarial: two RO arms touching the same buffer. RO-RO is safe
    // without a barrier; inserting one would be a pessimisation.
    let a = Program::wrapped(
        vec![BufferDecl::read("shared", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::load("shared", Expr::u32(0)))],
    );
    let b = Program::wrapped(
        vec![BufferDecl::read("shared", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("y", Expr::load("shared", Expr::u32(0)))],
    );

    let fused = fuse_programs(&[a, b]).unwrap();
    let body = entry_body(&fused);
    assert!(
        !body.iter().any(|n| matches!(n, Node::Barrier { .. })),
        "Fix: RO-RO on the same buffer must NOT insert a barrier"
    );
}

// ------------------------------------------------------------------
// F-IR-23: self-composition rejection for parsers
// ------------------------------------------------------------------

#[test]
fn delimiter_parser_self_fusion_rejected() {
    let parser = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_entry_op_id("vyre-libs::parsing::core_delimiter_match")
    .with_non_composable_with_self(true);

    let result = fuse_programs(&[parser.clone(), parser]);
    match result {
        Err(FusionError::SelfAliasing(FusionSelfAliasingError { op_id, fix })) => {
            assert_eq!(op_id, "vyre-libs::parsing::core_delimiter_match");
            assert!(
                fix.contains("rename") || fix.contains("split"),
                "Fix hint must be actionable"
            );
        }
        other => panic!(
            "Fix: fusing two copies of a non-composable parser must fail with SelfAliasing, got {other:?}"
        ),
    }
}

#[test]
fn composable_programs_with_same_op_id_allowed() {
    let adder = Program::wrapped(
        vec![BufferDecl::read_write("x", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("x", Expr::u32(0), Expr::u32(1))],
    )
    .with_entry_op_id("vyre-libs::math::add_one")
    .with_non_composable_with_self(false);

    let fused = fuse_programs(&[adder.clone(), adder]).unwrap();
    assert_eq!(fused.buffers().len(), 1);
}

#[test]
fn non_composable_without_entry_op_id_self_fusion_rejected() {
    // Adversarial: entry_op_id is omitted, so the gate must fall back to a
    // derived key (buffer names + workgroup size + entry count). Two clones
    // share that key and both have non_composable_with_self=true → reject.
    let parser = Program::wrapped(
        vec![BufferDecl::read("in", 0, DataType::U32)],
        [1, 1, 1],
        vec![Node::Return],
    )
    .with_non_composable_with_self(true);
    // Deliberately omit with_entry_op_id

    let result = fuse_programs(&[parser.clone(), parser]);
    match result {
        Err(FusionError::SelfAliasing(FusionSelfAliasingError { op_id, fix })) => {
            assert!(
                !op_id.is_empty(),
                "Fix: self-aliasing error must report the offending key"
            );
            assert!(
                fix.contains("rename") || fix.contains("split"),
                "Fix hint must be actionable"
            );
        }
        other => panic!(
            "Fix: fusing two copies of a non-composable parser without entry_op_id must fail with SelfAliasing, got {other:?}"
        ),
    }
}
