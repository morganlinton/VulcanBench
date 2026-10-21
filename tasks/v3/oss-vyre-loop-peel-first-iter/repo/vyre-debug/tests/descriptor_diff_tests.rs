//! Test: descriptor diff tests.
use vyre_debug::descriptor_diff::{bisect_rewrites, diff_descriptors};
use vyre_debug::fixtures::loop_carry_smoke;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

fn minimal_program() -> Program {
    let buffer =
        BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16);
    Program::wrapped(
        vec![buffer],
        [64, 1, 1],
        vec![Node::Store {
            buffer: Ident::from("out"),
            index: Expr::InvocationId { axis: 0 },
            value: Expr::LitU32(7),
        }],
    )
}

#[test]
fn diff_descriptors_identical_returns_empty_diff() {
    let p = minimal_program();
    let desc1 = vyre_lower::lower(&p).unwrap();
    let desc2 = vyre_lower::lower(&p).unwrap();
    let diff = diff_descriptors(&desc1, &desc2);
    assert!(diff.bindings_dropped.is_empty());
    assert!(diff.bindings_added.is_empty());
    assert!(diff.op_count_delta.is_empty());
    assert!(!diff.root_shape_changed);
}

#[test]
fn diff_descriptors_after_descriptor_dce_removes_ops() {
    let p = minimal_program();
    let mut desc_before = vyre_lower::lower(&p).unwrap();
    // Add a dead op manually
    desc_before.body.ops.push(vyre_lower::KernelOp {
        result: Some(999),
        kind: vyre_lower::KernelOpKind::Literal,
        operands: vec![0],
    });
    let mut desc_after = desc_before.clone();
    desc_after.body.ops.pop(); // Remove the op to create a difference

    let diff = diff_descriptors(&desc_before, &desc_after);
    // op_count_delta should have a negative entry for the root path []
    let delta = diff.op_count_delta.get(&vec![]).copied().unwrap_or(0);
    assert!(delta < 0, "Expected negative delta, got {}", delta);
}

#[test]
fn bisect_rewrites_clean_program_no_failure() {
    let p = loop_carry_smoke();
    let res = bisect_rewrites(&p).unwrap();
    assert!(res.first_failing_rewrite.is_none());
    assert_eq!(res.rewrite_history.len(), 18);
}
