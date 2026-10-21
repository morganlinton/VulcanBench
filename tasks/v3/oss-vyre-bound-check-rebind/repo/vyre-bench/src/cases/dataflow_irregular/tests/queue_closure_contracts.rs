use super::*;

#[test]
fn ifds_queue_closure_inputs_allocate_ping_pong_queues_and_seed_accumulator() {
    let fixture = build_ifds_skewed_fixture(4096).unwrap();
    let inputs = ifds_queue_closure_inputs(&fixture, fixture.stats.nodes).unwrap();

    assert_eq!(inputs.len(), 11);
    assert_eq!(
        inputs[QUEUE_CLOSURE_SEED_FRONTIER_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in)
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_SEED_LEN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[fixture.stats.active_sources as u32])
    );
    let mut seed_queue = Vec::new();
    vyre_primitives::wire::unpack_u32_slice_into(
        &inputs[QUEUE_CLOSURE_SEED_QUEUE_INDEX],
        fixture.stats.active_sources as usize,
        "queue closure seed queue test",
        &mut seed_queue,
    )
    .unwrap();
    assert_eq!(seed_queue.len(), fixture.stats.active_sources as usize);
    assert_eq!(seed_queue[0], 0);
    assert!(
        seed_queue.windows(2).all(|pair| pair[0] < pair[1]),
        "pre-materialized queue closure seed queue should preserve source order"
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_ACCUMULATOR_INDEX],
        inputs[QUEUE_CLOSURE_SEED_FRONTIER_INDEX]
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_QUEUE_A_INDEX].len(),
        fixture.stats.nodes as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_QUEUE_B_INDEX].len(),
        fixture.stats.nodes as usize * std::mem::size_of::<u32>()
    );
    assert!(inputs[QUEUE_CLOSURE_QUEUE_A_INDEX]
        .iter()
        .all(|byte| *byte == 0));
    assert!(inputs[QUEUE_CLOSURE_QUEUE_B_INDEX]
        .iter()
        .all(|byte| *byte == 0));
    assert_eq!(
        inputs[QUEUE_CLOSURE_LEN_A_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[0])
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_LEN_B_INDEX],
        vyre_primitives::wire::pack_u32_slice(&[0])
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_EDGE_OFFSETS_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_offsets)
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_EDGE_TARGETS_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_targets)
    );
    assert_eq!(
        inputs[QUEUE_CLOSURE_EDGE_KIND_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.edge_kind_mask)
    );
}

#[test]
fn ifds_queue_closure_reset_program_restores_accumulator_and_clears_lengths() {
    let program = ifds_queue_closure_reset_program(128, 7, 256);

    assert_eq!(program.workgroup_size(), [256, 1, 1]);
    assert_eq!(program.buffers().len(), 7);
    assert_eq!(program.buffers()[0].name.as_ref(), "frontier_seed");
    assert_eq!(program.buffers()[1].name.as_ref(), "seed_queue");
    assert_eq!(program.buffers()[2].name.as_ref(), "seed_len");
    assert_eq!(program.buffers()[3].name.as_ref(), "active_queue");
    assert_eq!(program.buffers()[4].name.as_ref(), "accumulator");
    assert_eq!(program.buffers()[5].name.as_ref(), "queue_a_len");
    assert_eq!(program.buffers()[6].name.as_ref(), "queue_b_len");
    assert_eq!(program.buffers()[1].count, 7);
    assert_eq!(program.buffers()[3].count, 256);
    assert_eq!(program.buffers()[4].count, 128);
}

#[test]
fn ifds_queue_closure_prepare_builds_delta_fixpoint_sequence() {
    let prepared = prepare_ifds_skewed_queue_closure(None).unwrap();

    assert_eq!(prepared.reset_program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.clear_len_program.workgroup_size(), [1, 1, 1]);
    assert_eq!(prepared.delta_program.workgroup_size(), [256, 1, 1]);
    assert!(prepared.row_strided_delta);
    assert_eq!(
        prepared.delta_grid,
        vyre_primitives::graph::csr_queue_delta::csr_queue_delta_strided_dispatch_grid(
            prepared.queue_capacity
        )
    );
    assert_eq!(
        prepared.delta_program.buffers()[0].name.as_ref(),
        "active_queue"
    );
    assert_eq!(
        prepared.delta_program.buffers()[6].name.as_ref(),
        "next_queue"
    );
    assert_eq!(
        prepared.reset_program.buffers()[0].name.as_ref(),
        "frontier_seed"
    );
    assert_eq!(
        prepared.reset_program.buffers()[0].count as usize,
        FRONTIER_WORDS
    );
    assert_eq!(prepared.stats.nodes, NODE_COUNT);
    assert_eq!(
        prepared.seed_queue_len,
        prepared.stats.active_sources as u32
    );
    assert_eq!(prepared.queue_capacity, prepared.max_wave_queue_len);
    assert!(prepared.queue_capacity < NODE_COUNT);
    assert_eq!(prepared.inputs.len(), 11);
    assert_eq!(
        prepared.inputs[QUEUE_CLOSURE_SEED_QUEUE_INDEX].len(),
        prepared.seed_queue_len as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        prepared.inputs[QUEUE_CLOSURE_QUEUE_A_INDEX].len(),
        prepared.queue_capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(prepared.baseline_output.len(), FRONTIER_WORDS * 4);
    assert_eq!(prepared.closure_changed, 1);
    assert!(prepared.closure_iterations > 0);
    assert!(prepared.closure_iterations <= closure::CLOSURE_MAX_ITERS);
    assert!(prepared.total_queue_pops >= prepared.stats.active_sources);
    assert!(prepared.max_wave_queue_len >= prepared.stats.active_sources as u32);
    assert_eq!(
        prepared.wave_queue_lengths.len(),
        prepared.closure_iterations as usize
    );
    assert_eq!(
        prepared
            .wave_queue_lengths
            .iter()
            .map(|&len| u64::from(len))
            .sum::<u64>(),
        prepared.total_queue_pops
    );
    assert_eq!(
        prepared
            .wave_queue_lengths
            .iter()
            .copied()
            .max()
            .unwrap_or(0),
        prepared.max_wave_queue_len
    );
    let launch_lanes = crate::cases::queue_closure_profile::queue_closure_launch_lanes_per_wave(
        prepared.delta_grid,
        prepared.delta_program.workgroup_size(),
    );
    let lane_profile =
        crate::cases::queue_closure_profile::QueueClosureLaneProfile::from_wave_lengths_with_launch_lanes(
            prepared.queue_capacity,
            &prepared.wave_queue_lengths,
            ifds_queue_closure_delta_lanes_per_source(prepared.row_strided_delta),
            launch_lanes,
        );
    assert_eq!(
        lane_profile.profiled_delta_source_slots,
        prepared.total_queue_pops
    );
    assert!(lane_profile.elided_delta_lanes > 0);
    assert!(lane_profile.launch_elided_delta_lanes > 0);
    assert!(lane_profile.launch_lane_elision_x1000 > 800);
}
