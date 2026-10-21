use super::super::*;
use crate::graph::program_graph::ProgramGraphShape;

#[test]
fn cpu_ref_expands_in_place_frontier_pass() {
    let (frontier, changed) = cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[1, 1, 1, 1],
        &[0b0001],
        1,
    );
    assert_eq!(frontier, vec![0b1111]);
    assert_eq!(changed, 1);
}

#[test]
fn cpu_ref_closure_reaches_fixpoint() {
    let closure = cpu_ref_closure(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        10,
    );
    assert_eq!(closure, vec![0b1111]);
}

#[test]
fn cpu_ref_closure_into_reuses_buffers() {
    let mut current = Vec::with_capacity(8);
    let mut next = Vec::with_capacity(8);
    cpu_ref_closure_into(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0b0001],
        0xFFFF_FFFF,
        10,
        &mut current,
        &mut next,
    );
    let current_capacity = current.capacity();
    let next_capacity = next.capacity();
    assert_eq!(current, vec![0b1111]);

    cpu_ref_closure_into(
        4,
        &[0, 1, 2, 3, 3],
        &[1, 2, 3],
        &[1, 1, 1],
        &[0],
        0xFFFF_FFFF,
        10,
        &mut current,
        &mut next,
    );
    assert_eq!(current.capacity(), current_capacity);
    assert_eq!(next.capacity(), next_capacity);
    assert_eq!(current, vec![0]);
}

#[test]
fn validate_csr_inputs_rejects_mismatched_and_nonmonotonic_csr() {
    let err = validate_csr_inputs(2, &[0, 1, 1], &[1], &[]).unwrap_err();
    assert!(err.contains("edge_targets.len() == edge_kind_mask.len()"));

    let err = validate_csr_inputs(2, &[0, 2, 1], &[1, 0], &[1, 1]).unwrap_err();
    assert!(err.contains("offsets must be monotonic"));
}

#[test]
fn cpu_ref_into_rejects_malformed_csr_before_touching_output_storage() {
    let mut out = vec![0xDEAD_BEEFu32, 0xABCD_EF01];
    let ptr = out.as_ptr();
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cpu_ref_into(2, &[0, 2, 1], &[1, 0], &[1, 1], &[0b01], 1, &mut out);
    }));
    std::panic::set_hook(previous_hook);

    assert!(err.is_err(), "malformed CSR must be rejected");
    assert_eq!(
        out,
        vec![0xDEAD_BEEFu32, 0xABCD_EF01],
        "Fix: malformed CSR must not clear or resize caller output before validation."
    );
    assert_eq!(out.as_ptr(), ptr);
}

#[test]
fn empty_offsets_shorthand_is_empty_edge_set_only() {
    assert_eq!(
        validate_csr_inputs(64, &[], &[], &[]).expect("Fix: empty CSR shorthand is valid"),
        CsrForwardOrChangedLayout {
            node_count: 64,
            node_words: 64,
            edge_offset_words: 65,
            edge_storage_words: 1,
            shape_edge_count: 0,
            frontier_words: 2,
        }
    );

    let err = validate_csr_inputs(64, &[], &[1], &[]).unwrap_err();
    assert!(err.contains("empty edge_offsets may only encode an empty edge set"));

    let mut out = Vec::new();
    let changed = cpu_ref_into(64, &[], &[], &[], &[0b101], 0xFFFF_FFFF, &mut out);
    assert_eq!(changed, 0);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], 0b101);
    assert_eq!(out[1], 0);
}

#[test]
fn dispatch_plan_selects_changed_history_and_pins_buffer_shape() {
    let edge_offsets = vec![0u32; 66];
    let plan = plan_csr_forward_or_changed_dispatch(65, &edge_offsets, &[], &[], 0xFFFF_FFFF, 8)
        .expect("Fix: bounded CSR forward-or-changed plan should validate");

    assert_eq!(plan.layout().node_count, 65);
    assert_eq!(plan.frontier_words(), 3);
    assert_eq!(plan.node_words(), 65);
    assert_eq!(plan.edge_storage_words(), 1);
    assert_eq!(plan.changed_words(), 8);
    assert!(plan.uses_changed_history());
    assert_eq!(plan.changed_slot_value(3), Some(3));
    assert_eq!(plan.changed_read_index(3).unwrap(), 3);
    assert!(
        plan.changed_read_index(8).is_err(),
        "Fix: changed-history readback index must reject iterations outside the buffer"
    );
    assert_eq!(plan.dispatch_grid(), [1, 1, 1]);
    assert_eq!(
        plan.program().workgroup_size,
        CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE
    );
    assert!(
        plan.program()
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "changed_slot"),
        "Fix: changed-history fast path must expose the primitive slot selector"
    );
}

#[test]
fn dispatch_plan_uses_single_changed_word_for_unbounded_or_zero_iteration_cases() {
    let plan = plan_csr_forward_or_changed_dispatch(0, &[], &[], &[], 0xFFFF_FFFF, 0)
        .expect("Fix: zero-node zero-iteration plan should validate");
    assert_eq!(plan.frontier_words(), 1);
    assert_eq!(plan.changed_words(), 1);
    assert!(!plan.uses_changed_history());
    assert_eq!(plan.changed_slot_value(0), None);
    assert_eq!(plan.changed_read_index(99).unwrap(), 0);
    assert_eq!(plan.dispatch_grid(), [1, 1, 1]);

    let long_plan = plan_csr_forward_or_changed_dispatch(1, &[0, 0], &[], &[], 0xFFFF_FFFF, 65)
        .expect("Fix: long-running plan should validate without changed history");
    assert_eq!(long_plan.changed_words(), 1);
    assert!(!long_plan.uses_changed_history());
    assert!(
        !long_plan
            .program()
            .buffers()
            .iter()
            .any(|buffer| buffer.name() == "changed_slot"),
        "Fix: unbounded path must not carry the changed-history slot input"
    );
}

#[test]
fn parallel_dispatch_grid_packs_source_lanes_into_blocks() {
    assert_eq!(csr_forward_or_changed_parallel_grid(0), [1, 1, 1]);
    assert_eq!(csr_forward_or_changed_parallel_grid(65), [1, 1, 1]);
    assert_eq!(csr_forward_or_changed_parallel_grid(256), [1, 1, 1]);
    assert_eq!(csr_forward_or_changed_parallel_grid(257), [2, 1, 1]);
    assert_eq!(
        csr_forward_or_changed_parallel_batch_grid(513, 3),
        [3, 3, 1]
    );
}

#[test]
fn parallel_program_keeps_frontier_and_changed_resident() {
    let program = csr_forward_or_changed_parallel(
        ProgramGraphShape::new(65, 4),
        "frontier",
        "changed",
        0xFFFF_FFFF,
    );
    assert_eq!(
        program.workgroup_size,
        CSR_FORWARD_OR_CHANGED_PARALLEL_WORKGROUP_SIZE
    );
    let names: Vec<&str> = program.buffers.iter().map(|buffer| buffer.name()).collect();
    assert!(names.contains(&"frontier"));
    assert!(names.contains(&"changed"));
    assert!(
        names.iter().any(|name| name.starts_with("pg_")),
        "parallel CSR expansion must keep ProgramGraph buffers resident"
    );
}

#[test]
fn parallel_batch_program_packs_query_frontiers() {
    let program = csr_forward_or_changed_parallel_batch(
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
    assert_eq!(changed.count(), 3);
}
