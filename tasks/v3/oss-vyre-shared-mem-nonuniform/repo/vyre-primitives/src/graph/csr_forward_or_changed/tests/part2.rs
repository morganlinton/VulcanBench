use super::super::*;
use crate::graph::program_graph::ProgramGraphShape;

#[test]
fn parallel_batch_global_program_uses_one_changed_flag() {
    let program = csr_forward_or_changed_parallel_batch_global(
        ProgramGraphShape::new(65, 4),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        3,
    );
    assert_eq!(
        program.workgroup_size,
        CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE
    );
    let frontier = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "frontiers")
        .expect("Fix: frontiers buffer must exist");
    let changed = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "changed")
        .expect("Fix: changed buffer must exist");
    assert_eq!(frontier.count(), 9);
    assert_eq!(changed.count(), 1);
}

#[test]
fn parallel_batch_global_slot_program_uses_changed_history_buffer() {
    let program = csr_forward_or_changed_parallel_batch_global_slot(
        ProgramGraphShape::new(65, 4),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        3,
        5,
        8,
    );
    assert_eq!(
        program.workgroup_size,
        CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE
    );
    let frontier = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "frontiers")
        .expect("Fix: frontiers buffer must exist");
    let changed = program
        .buffers
        .iter()
        .find(|buffer| buffer.name() == "changed")
        .expect("Fix: changed buffer must exist");
    assert_eq!(frontier.count(), 9);
    assert_eq!(changed.count(), 8);
}

#[test]
fn checked_parallel_batch_rejects_zero_queries() {
    let error = try_csr_forward_or_changed_parallel_batch(
        ProgramGraphShape::new(65, 4),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        0,
    )
    .expect_err("checked CSR batch builder must reject empty query batches");

    assert!(
        error.contains("at least one query frontier"),
        "error should describe the invalid batch shape: {error}"
    );
}

#[test]
fn checked_parallel_batch_rejects_flat_frontier_overflow() {
    let error = try_csr_forward_or_changed_parallel_batch(
        ProgramGraphShape::new(u32::MAX, 0),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        33,
    )
    .expect_err("checked CSR batch builder must reject flat frontier overflow");

    assert!(
        error.contains("frontier words overflow u32"),
        "error should describe the flat frontier overflow: {error}"
    );
}

#[test]
fn legacy_parallel_batch_fails_fast_on_flat_frontier_overflow() {
    let panic = std::panic::catch_unwind(|| {
        let _ = csr_forward_or_changed_parallel_batch(
            ProgramGraphShape::new(u32::MAX, 0),
            "frontiers",
            "changed",
            0xFFFF_FFFF,
            33,
        );
    })
    .expect_err("legacy CSR batch builder must fail fast on flat frontier overflow");

    let message = panic_payload_message(panic);
    assert!(
        message.contains("frontier words overflow u32"),
        "error should describe the flat frontier overflow: {message}"
    );
}

#[test]
fn checked_parallel_global_slot_rejects_invalid_changed_slot() {
    let error = try_csr_forward_or_changed_parallel_batch_global_slot(
        ProgramGraphShape::new(65, 4),
        "frontiers",
        "changed",
        0xFFFF_FFFF,
        3,
        8,
        8,
    )
    .expect_err("checked CSR global-slot builder must reject out-of-range changed slot");

    assert!(
        error.contains("changed_slot must be inside"),
        "error should describe the invalid changed slot: {error}"
    );
}

#[test]
fn legacy_parallel_global_slot_fails_fast_on_invalid_changed_slot() {
    let panic = std::panic::catch_unwind(|| {
        let _ = csr_forward_or_changed_parallel_batch_global_slot(
            ProgramGraphShape::new(65, 4),
            "frontiers",
            "changed",
            0xFFFF_FFFF,
            3,
            8,
            8,
        );
    })
    .expect_err("legacy CSR global-slot builder must fail fast on invalid changed slot");

    let message = panic_payload_message(panic);
    assert!(
        message.contains("changed_slot must be inside"),
        "error should describe the invalid changed slot: {message}"
    );
}

#[test]
fn csr_forward_or_changed_batch_source_has_checked_api_without_panics() {
    let source = concat!(
        include_str!("../program_parallel_batch.rs"),
        include_str!("../program_parallel_batch_global.rs")
    );
    let batch_source = source
        .split("/// Parallel in-place expansion for several frontier accumulators at once.")
        .nth(1)
        .expect("Fix: CSR batch builder source must be present")
        .split("/// CPU reference for one in-place expansion pass.")
        .next()
        .expect("Fix: CSR batch builder source must precede CPU oracle");

    assert!(
            batch_source.contains("pub fn try_csr_forward_or_changed_parallel_batch(")
                && batch_source
                    .contains("pub fn try_csr_forward_or_changed_parallel_batch_global_slot(")
                && !batch_source.contains("inert_")
                && !batch_source.contains("Err(_) =>")
                && !batch_source.contains("Node::return_()"),
            "Fix: batched CSR forward-or-changed builders must expose checked release APIs and must not compile inert no-op kernels."
        );
}

fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        format!("{payload:?}")
    }
}
