//! Integration test for the CUDA backend.

mod common;
use common::u32_bytes;
use vyre_driver::binding::{BindingPlan, BindingRole};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn binding_plan_orders_by_binding_and_tracks_roles() {
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 2, DataType::U32).with_count(4),
            BufferDecl::read("input", 0, DataType::U32).with_count(4),
            BufferDecl::read_write("state", 1, DataType::U32).with_count(4),
        ],
        [64, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::load("input", Expr::gid_x()),
        )],
    );

    let plan = BindingPlan::from_program(
        &program,
        &[u32_bytes(&[1, 2, 3, 4]), u32_bytes(&[0, 0, 0, 0])],
    )
    .expect("Fix: valid CUDA binding plan should build.");

    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| binding.binding)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "Fix: CUDA binding descriptors must be sorted by VYRE binding number."
    );
    assert_eq!(plan.input_indices, vec![1, 2]);
    assert_eq!(plan.output_indices, vec![0, 2]);
    assert_eq!(plan.bindings[0].role, BindingRole::Input);
    assert_eq!(plan.bindings[1].role, BindingRole::InputOutput);
    assert_eq!(plan.bindings[2].role, BindingRole::Output);
}

#[test]
fn binding_plan_keeps_logical_input_and_output_slots_when_bindings_are_reordered() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("declared_first_high_binding", 9, DataType::U32),
            BufferDecl::output("declared_output_first_high_binding", 8, DataType::U32)
                .with_count(1),
            BufferDecl::read("declared_second_low_binding", 0, DataType::U32),
            BufferDecl::output("declared_output_second_low_binding", 1, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        Vec::new(),
    );
    let inputs = [u32_bytes(&[1, 2, 3]), u32_bytes(&[4, 5])];

    let plan = BindingPlan::from_program(&program, &inputs).expect(
        "Fix: CUDA binding plan must accept logical input order independent of binding number.",
    );

    assert_eq!(
        plan.bindings
            .iter()
            .map(|binding| binding.binding)
            .collect::<Vec<_>>(),
        vec![0, 1, 8, 9],
        "Fix: CUDA descriptor order must remain sorted by binding number."
    );
    assert_eq!(
        plan.input_indices,
        vec![0, 2],
        "Fix: CUDA caller input slots must follow Program::buffers logical order, not binding order."
    );
    assert_eq!(
        plan.output_indices,
        vec![1, 3],
        "Fix: CUDA output vector slots must follow Program::buffers logical order, not binding order."
    );

    let high_input = plan
        .bindings
        .iter()
        .find(|binding| binding.name.as_ref() == "declared_first_high_binding")
        .expect("Fix: high-binding input descriptor must be present.");
    assert_eq!(high_input.input_index, Some(0));
    assert_eq!(
        high_input.element_count, 3,
        "Fix: dynamic high-binding input length must be read from logical input slot 0."
    );

    let low_input = plan
        .bindings
        .iter()
        .find(|binding| binding.name.as_ref() == "declared_second_low_binding")
        .expect("Fix: low-binding input descriptor must be present.");
    assert_eq!(low_input.input_index, Some(1));
    assert_eq!(
        low_input.element_count, 2,
        "Fix: dynamic low-binding input length must be read from logical input slot 1."
    );

    let high_output = plan
        .bindings
        .iter()
        .find(|binding| binding.name.as_ref() == "declared_output_first_high_binding")
        .expect("Fix: high-binding output descriptor must be present.");
    assert_eq!(high_output.output_index, Some(0));

    let low_output = plan
        .bindings
        .iter()
        .find(|binding| binding.name.as_ref() == "declared_output_second_low_binding")
        .expect("Fix: low-binding output descriptor must be present.");
    assert_eq!(low_output.output_index, Some(1));
}

#[test]
fn binding_plan_rejects_wrong_input_count() {
    let program = Program::wrapped(
        vec![BufferDecl::read("input", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        Vec::new(),
    );

    let err = BindingPlan::from_program(&program, &[])
        .expect_err("Fix: missing CUDA input buffer must be rejected.");
    assert!(
        err.to_string().contains("expected 1 input buffer"),
        "Fix: CUDA input-count errors must be actionable, got: {err}"
    );
}

#[test]
fn binding_plan_rejects_unaligned_input_bytes() {
    let program = Program::wrapped(
        vec![BufferDecl::read("input", 0, DataType::U32)],
        [1, 1, 1],
        Vec::new(),
    );

    let err = BindingPlan::from_program(&program, &[vec![1, 2, 3]])
        .expect_err("Fix: unaligned CUDA input buffer must be rejected.");
    assert!(
        err.to_string().contains("not aligned"),
        "Fix: CUDA alignment errors must name the alignment failure, got: {err}"
    );
}

#[test]
fn binding_plan_rejects_static_byte_length_mismatch() {
    let program = Program::wrapped(
        vec![BufferDecl::read("input", 0, DataType::U32).with_count(2)],
        [1, 1, 1],
        Vec::new(),
    );

    let err = BindingPlan::from_program(&program, &[u32_bytes(&[1])])
        .expect_err("Fix: static CUDA buffer byte mismatch must be rejected.");
    assert!(
        err.to_string().contains("expected 8 bytes"),
        "Fix: CUDA static-size errors must report expected bytes, got: {err}"
    );
}

#[test]
fn binding_plan_classifies_uniform_shared_and_persistent_buffers() {
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("uniforms", 0, BufferAccess::Uniform, DataType::U32).with_count(1),
            BufferDecl::workgroup("scratch", 16, DataType::U32),
            BufferDecl::storage("persist", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_kind(vyre_foundation::ir::MemoryKind::Persistent),
        ],
        [64, 1, 1],
        Vec::new(),
    );

    let plan = BindingPlan::from_program(&program, &[u32_bytes(&[7])])
        .expect("Fix: uniform/shared/persistent CUDA binding roles should build.");

    assert_eq!(plan.bindings[0].role, BindingRole::Uniform);
    assert_eq!(plan.bindings[1].role, BindingRole::Shared);
    assert_eq!(plan.bindings[2].role, BindingRole::Persistent);
    assert_eq!(plan.input_indices, vec![0]);
    assert_eq!(plan.shared_indices, vec![1]);
}

#[test]
fn binding_plan_classifies_plain_read_write_as_input_output() {
    let program = Program::wrapped(
        vec![BufferDecl::storage("state", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)],
        [4, 1, 1],
        vec![Node::store(
            "state",
            Expr::gid_x(),
            Expr::add(Expr::load("state", Expr::gid_x()), Expr::u32(1)),
        )],
    );

    let plan = BindingPlan::from_program(&program, &[u32_bytes(&[1, 2, 3, 4])])
        .expect("Fix: plain read-write state must remain a valid input/output binding.");

    assert_eq!(plan.bindings[0].role, BindingRole::InputOutput);
    assert_eq!(plan.input_indices, vec![0]);
    assert_eq!(plan.output_indices, vec![0]);
}
