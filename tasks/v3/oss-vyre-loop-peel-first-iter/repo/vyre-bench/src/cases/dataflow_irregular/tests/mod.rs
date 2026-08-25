use super::fixture::{
    ifds_active_high_degree_sources, ifds_active_queue_inputs, ifds_queue_inputs,
};
use super::queue::{
    ifds_queue_closure_delta_lanes_per_source, ifds_queue_closure_inputs,
    ifds_queue_closure_reset_program, ifds_queue_materialize_sequence_fingerprint,
    ifds_queue_should_use_row_strided, ifds_queue_traverse_logical_lanes,
    ifds_sparse_queue_capacity, prepare_ifds_skewed_active_queue_step,
    prepare_ifds_skewed_queue_closure, prepare_ifds_skewed_queue_materialize_step,
    ACTIVE_QUEUE_ACTIVE_QUEUE_INDEX, ACTIVE_QUEUE_EDGE_KIND_INDEX, ACTIVE_QUEUE_EDGE_OFFSETS_INDEX,
    ACTIVE_QUEUE_EDGE_TARGETS_INDEX, ACTIVE_QUEUE_FRONTIER_OUT_INDEX, ACTIVE_QUEUE_LEN_INDEX,
    QUEUE_ACTIVE_QUEUE_INDEX, QUEUE_CLOSURE_ACCUMULATOR_INDEX, QUEUE_CLOSURE_EDGE_KIND_INDEX,
    QUEUE_CLOSURE_EDGE_OFFSETS_INDEX, QUEUE_CLOSURE_EDGE_TARGETS_INDEX, QUEUE_CLOSURE_LEN_A_INDEX,
    QUEUE_CLOSURE_LEN_B_INDEX, QUEUE_CLOSURE_QUEUE_A_INDEX, QUEUE_CLOSURE_QUEUE_B_INDEX,
    QUEUE_CLOSURE_SEED_FRONTIER_INDEX, QUEUE_CLOSURE_SEED_LEN_INDEX,
    QUEUE_CLOSURE_SEED_QUEUE_INDEX, QUEUE_FRONTIER_IN_INDEX, QUEUE_FRONTIER_OUT_INDEX,
    QUEUE_HIGH_LEN_INDEX, QUEUE_HIGH_QUEUE_INDEX, QUEUE_LEN_INDEX, QUEUE_RESET_GRID,
};
use super::*;
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_low_dispatch_grid, csr_queue_split_mixed_logical_lanes,
    CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
};

mod queue_closure_contracts;
mod queue_closure_generated;
mod queue_generated;

#[test]
fn ifds_skewed_fixture_has_filtered_edges_and_bitset_frontier() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let oracle = ifds_skewed_cpu_oracle(&fixture);

    assert_eq!(fixture.edge_offsets.len(), 4097);
    assert!(fixture.edge_targets.len() > 4096);
    assert_eq!(fixture.stats.max_degree, UGLY_HUB_DEGREE);
    assert!(fixture.stats.high_degree_sources > 0);
    assert!(ifds_queue_should_use_row_strided(fixture.stats.max_degree));
    assert!(fixture.stats.active_sources > 0);
    assert!(oracle.allowed_edges_from_active > 0);
    assert!(oracle.filtered_edges_from_active > 0);
    assert_eq!(fixture.frontier_in.len(), 128);
}

#[test]
fn ifds_skewed_cpu_oracle_sets_packed_output_words() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let oracle = ifds_skewed_cpu_oracle(&fixture);

    assert_eq!(oracle.output.len(), fixture.frontier_out_seed.len());
    assert!(oracle.output_words_set > 0);
    assert!(oracle.output.iter().any(|word| *word != 0));
}

#[test]
fn ifds_skewed_prepare_builds_vyre_program_and_oracle() {
    let prepared = prepare_ifds_skewed_step(None).unwrap();

    assert_eq!(prepared.program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.stats.nodes, NODE_COUNT);
    assert_eq!(prepared.baseline_output.len(), FRONTIER_WORDS * 4);
    assert_eq!(prepared.inputs.len(), 7);
    assert!(prepared.stats.filtered_edges_from_active > 0);
    assert!(prepared.input_bytes_total > u64::from(NODE_COUNT) * 20);
}

#[test]
fn ifds_queue_inputs_preserve_sparse_frontier_and_device_scratch() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let capacity = ifds_sparse_queue_capacity(fixture.stats.active_sources).unwrap();
    let high_capacity =
        ifds_active_high_degree_sources(&fixture, CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD).unwrap();
    let inputs = ifds_queue_inputs(&fixture, capacity, high_capacity).unwrap();

    assert_eq!(inputs.len(), 9);
    assert_eq!(
        inputs[QUEUE_FRONTIER_IN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in)
    );
    assert_eq!(
        inputs[QUEUE_ACTIVE_QUEUE_INDEX].len(),
        capacity as usize * std::mem::size_of::<u32>()
    );
    assert!(inputs[QUEUE_ACTIVE_QUEUE_INDEX]
        .iter()
        .all(|byte| *byte == 0));
    assert_eq!(
        inputs[QUEUE_LEN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[0])
    );
    assert_eq!(
        inputs[QUEUE_FRONTIER_OUT_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_out_seed)
    );
    assert_eq!(
        inputs[QUEUE_HIGH_QUEUE_INDEX].len(),
        high_capacity as usize * std::mem::size_of::<u32>()
    );
    assert!(inputs[QUEUE_HIGH_QUEUE_INDEX].iter().all(|byte| *byte == 0));
    assert_eq!(
        inputs[QUEUE_HIGH_LEN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[0])
    );
}

#[test]
fn ifds_queue_inputs_reject_capacity_below_active_sources() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let undersized = fixture.stats.active_sources.saturating_sub(1) as u32;
    let high_capacity =
        ifds_active_high_degree_sources(&fixture, CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD).unwrap();

    let err = ifds_queue_inputs(&fixture, undersized, high_capacity).unwrap_err();

    assert!(
        err.to_string().contains("queue_capacity >= active_sources"),
        "queue fixture errors must name the capacity invariant, got: {err}"
    );

    let high_err =
        ifds_queue_inputs(&fixture, fixture.stats.active_sources as u32, u32::MAX).unwrap_err();
    assert!(
        high_err
            .to_string()
            .contains("high_degree_queue_capacity <= queue_capacity"),
        "high queue capacity errors must name the split invariant, got: {high_err}"
    );
}

#[test]
fn ifds_active_queue_inputs_materialize_frontier_queue_once() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let capacity = ifds_sparse_queue_capacity(fixture.stats.active_sources).unwrap();
    let inputs = ifds_active_queue_inputs(&fixture, capacity).unwrap();
    let mut active_queue = Vec::new();
    vyre_primitives::wire::unpack_u32_slice_into(
        &inputs[ACTIVE_QUEUE_ACTIVE_QUEUE_INDEX],
        capacity as usize,
        "active queue test",
        &mut active_queue,
    )
    .unwrap();
    let mut queue_len = Vec::new();
    vyre_primitives::wire::unpack_u32_slice_into(
        &inputs[ACTIVE_QUEUE_LEN_INDEX],
        1,
        "active queue len test",
        &mut queue_len,
    )
    .unwrap();

    assert_eq!(inputs.len(), 6);
    assert_eq!(queue_len, vec![fixture.stats.active_sources as u32]);
    assert_eq!(
        active_queue.len(),
        capacity as usize,
        "active queue buffer should be capacity-padded for stable resident dispatch"
    );
    assert_eq!(active_queue[0], 0);
    assert!(
        active_queue[..fixture.stats.active_sources as usize]
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "pre-materialized active queue should preserve source order"
    );
    assert_eq!(
        inputs[ACTIVE_QUEUE_FRONTIER_OUT_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_out_seed)
    );
    assert_eq!(
        inputs[ACTIVE_QUEUE_EDGE_OFFSETS_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets)
    );
    assert_eq!(
        inputs[ACTIVE_QUEUE_EDGE_TARGETS_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets)
    );
    assert_eq!(
        inputs[ACTIVE_QUEUE_EDGE_KIND_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask)
    );
}

#[test]
fn ifds_queue_materialize_prepare_builds_parallel_sparse_sequence() {
    let prepared = prepare_ifds_skewed_queue_materialize_step(None).unwrap();

    assert_eq!(prepared.reset_program.workgroup_size(), [1, 1, 1]);
    assert_eq!(QUEUE_RESET_GRID, [1, 1, 1]);
    assert_eq!(prepared.queue_program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.traverse_program.workgroup_size(), [256, 1, 1]);
    assert!(prepared.row_strided_traverse);
    assert!(prepared.split_high_degree_traverse);
    assert!(prepared.high_traverse_program.is_some());
    assert_eq!(prepared.high_degree_queue_capacity, 256);
    assert_eq!(
        prepared.traverse_grid,
        csr_queue_split_low_dispatch_grid(prepared.queue_capacity)
    );
    assert_eq!(prepared.high_traverse_grid, [32, 1, 1]);
    assert_eq!(
        prepared.traverse_logical_lanes,
        csr_queue_split_mixed_logical_lanes(
            prepared.queue_capacity,
            prepared.high_degree_queue_capacity,
        )
    );
    assert_eq!(
        prepared.queue_program.buffers()[0].name.as_ref(),
        "frontier_in"
    );
    assert_eq!(
        prepared.queue_program.buffers()[0].count as usize,
        FRONTIER_WORDS
    );
    assert_eq!(
        prepared.queue_program.buffers()[3].name.as_ref(),
        "frontier_out"
    );
    assert_eq!(
        prepared.queue_program.buffers()[3].count as usize,
        FRONTIER_WORDS
    );
    assert_eq!(prepared.stats.nodes, NODE_COUNT);
    assert_eq!(prepared.inputs.len(), 9);
    assert_eq!(
        prepared.inputs[QUEUE_FRONTIER_IN_INDEX].len(),
        FRONTIER_WORDS * 4
    );
    assert_eq!(
        prepared.inputs[QUEUE_ACTIVE_QUEUE_INDEX].len(),
        prepared.queue_capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        prepared.inputs[QUEUE_HIGH_QUEUE_INDEX].len(),
        prepared.high_degree_queue_capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(prepared.baseline_output.len(), FRONTIER_WORDS * 4);
    assert!(u64::from(prepared.queue_capacity) >= prepared.stats.active_sources);
    assert!(
        prepared.queue_capacity < prepared.stats.nodes / 32,
        "queue capacity should stay sparse relative to the full node-grid launch"
    );
    assert!(
        prepared.traverse_logical_lanes
            < ifds_queue_traverse_logical_lanes(prepared.queue_capacity, true) / 16,
        "split IFDS traversal should avoid assigning a row-strided team to every active source"
    );
    assert!(prepared.stats.allowed_edges_from_active > 0);
    assert!(prepared.input_bytes_total > u64::from(NODE_COUNT) * 12);
    assert_ne!(
        ifds_queue_materialize_sequence_fingerprint(&prepared),
        prepared.traverse_program.fingerprint()
    );
}

#[test]
fn ifds_active_queue_prepare_builds_sparse_traversal_program() {
    let prepared = prepare_ifds_skewed_active_queue_step(None).unwrap();

    assert_eq!(prepared.traverse_program.workgroup_size(), [256, 1, 1]);
    assert!(prepared.row_strided_traverse);
    assert_eq!(
        prepared.traverse_grid,
        vyre_primitives::graph::csr_queue_strided::csr_queue_strided_forward_dispatch_grid(
            prepared.queue_capacity
        )
    );
    assert_eq!(
        prepared.traverse_logical_lanes,
        ifds_queue_traverse_logical_lanes(prepared.queue_capacity, prepared.row_strided_traverse)
    );
    assert_eq!(prepared.stats.nodes, NODE_COUNT);
    assert_eq!(prepared.inputs.len(), 6);
    assert_eq!(
        prepared.inputs[ACTIVE_QUEUE_ACTIVE_QUEUE_INDEX].len(),
        prepared.queue_capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        prepared.inputs[ACTIVE_QUEUE_LEN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[prepared.stats.active_sources as u32])
    );
    assert_eq!(prepared.baseline_output.len(), FRONTIER_WORDS * 4);
    assert!(u64::from(prepared.queue_capacity) >= prepared.stats.active_sources);
    assert!(prepared.queue_capacity < prepared.stats.nodes / 32);
    assert!(prepared.stats.allowed_edges_from_active > 0);
}

#[test]
fn ifds_queue_reset_only_clears_len_before_fused_queue_build() {
    let program = vyre_primitives::graph::csr_frontier_queue::frontier_queue_len_init("queue_len");

    assert_eq!(program.workgroup_size(), [1, 1, 1]);
    assert_eq!(program.buffers().len(), 1);
    assert_eq!(program.buffers()[0].name.as_ref(), "queue_len");
    assert_eq!(program.buffers()[0].binding, 0);
    assert_eq!(program.buffers()[0].count, 1);
}

#[test]
fn ifds_skewed_closure_oracle_expands_seed_frontier() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let oracle = ifds_skewed_closure_oracle(&fixture, closure::CLOSURE_MAX_ITERS);

    assert_eq!(oracle.output.len(), fixture.frontier_in.len());
    assert_eq!(oracle.changed, 1);
    assert!(oracle.iterations > 0);
    assert!(oracle.iterations <= closure::CLOSURE_MAX_ITERS);
    assert!(
        oracle.output_words_set
            >= fixture
                .frontier_in
                .iter()
                .filter(|word| **word != 0)
                .count() as u64
    );
    let launch_waves = ifds_skewed_launch_wave_iterations(&fixture, closure::CLOSURE_MAX_ITERS);
    assert!(launch_waves >= oracle.iterations);
    assert!(launch_waves <= closure::CLOSURE_MAX_ITERS);
}

#[test]
fn ifds_skewed_closure_resident_inputs_keep_immutable_seed() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let inputs = super::fixture::ifds_closure_resident_inputs(&fixture);

    assert_eq!(inputs.len(), 8);
    assert_eq!(inputs[5].len(), fixture.frontier_in.len() * 4);
    assert_eq!(inputs[5], inputs[6]);
    assert_eq!(inputs[7], vyre_primitives::wire::pack_u32_slice(&[0]));
}

#[test]
fn ifds_skewed_closure_prepare_builds_resident_fixpoint_program() {
    let prepared = closure::prepare_ifds_skewed_closure(None).unwrap();

    assert_eq!(prepared.program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.reset_program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.stats.nodes, NODE_COUNT);
    assert_eq!(prepared.inputs.len(), 7);
    assert_eq!(prepared.inputs[5].len(), FRONTIER_WORDS * 4);
    assert_eq!(prepared.baseline_outputs.len(), 2);
    assert_eq!(prepared.baseline_outputs[0].len(), FRONTIER_WORDS * 4);
    assert_eq!(prepared.baseline_outputs[1].len(), 4);
    assert_eq!(prepared.closure_changed, 1);
    assert!(prepared.closure_iterations > 0);
    assert!(prepared.dispatch_iterations >= prepared.closure_iterations);
    assert!(prepared.dispatch_iterations < closure::CLOSURE_MAX_ITERS);
    assert!(prepared.input_bytes_total > u64::from(NODE_COUNT) * 20);
}
