use super::super::*;
use crate::graph::program_graph::ProgramGraphShape;

#[test]
fn dynamic_changed_slot_program_carries_slot_input_buffer() {
    let program = try_csr_forward_or_changed_parallel_batch_global_dynamic_slot(
        ProgramGraphShape::new(8, 8),
        "frontier",
        "changed",
        "changed_slot",
        0xFF,
        1,
        4,
    )
    .expect("Fix: dynamic changed-slot program must build");

    assert!(
        program
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "changed_slot"),
        "Fix: dynamic changed-slot program must expose a read-only slot selector input."
    );
    let rendered = format!("{:?}", program.entry);
    assert!(
        rendered.contains("changed_slot"),
        "Fix: dynamic changed-slot program must load the slot and use it for the changed write."
    );
}

#[test]
fn dynamic_changed_slot_rejects_zero_changed_slots() {
    let err = try_csr_forward_or_changed_parallel_batch_global_dynamic_slot(
        ProgramGraphShape::new(8, 8),
        "frontier",
        "changed",
        "changed_slot",
        0xFF,
        1,
        0,
    )
    .expect_err("zero changed slots must be rejected");
    assert!(err.contains("at least one changed slot"));
}
