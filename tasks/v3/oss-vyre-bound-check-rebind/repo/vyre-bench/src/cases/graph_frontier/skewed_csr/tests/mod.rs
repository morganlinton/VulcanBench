use super::*;

#[test]
fn skewed_csr_fixture_has_variable_degree_and_bitset_frontier() {
    let fixture = build_skewed_csr_fixture(4096).unwrap();

    assert_eq!(fixture.edge_offsets.len(), 4097);
    assert!(fixture.edge_targets.len() > 4096);
    assert_eq!(fixture.stats.max_degree, support::UGLY_HUB_DEGREE);
    assert!(fixture.stats.high_degree_sources > 0);
    assert!(fixture.stats.active_sources > 0);
    assert_eq!(fixture.frontier_in.len(), 128);
    assert_eq!(fixture.frontier_out_seed, vec![0_u32; 128]);
}

#[test]
fn skewed_csr_cpu_oracle_sets_packed_output_words() {
    let fixture = build_skewed_csr_fixture(4096).unwrap();
    let oracle = skewed_csr_cpu_oracle(&fixture);

    assert_eq!(oracle.output.len(), fixture.frontier_out_seed.len());
    assert!(oracle.allowed_edges_from_active > fixture.stats.active_sources);
    assert!(oracle.output_words_set > 0);
    assert!(oracle.output.iter().any(|word| *word != 0));
}

#[test]
fn skewed_csr_prepare_builds_primitive_program_and_oracle() {
    let prepared = prepare_skewed_csr_case(None).unwrap();

    assert_eq!(prepared.program.workgroup_size(), [256, 1, 1]);
    assert_eq!(prepared.stats.node_count, CSR_NODE_COUNT);
    assert_eq!(
        prepared.baseline_output.len(),
        prepared.stats.frontier_words as usize * 4
    );
    assert_eq!(prepared.inputs.len(), 7);
    assert!(prepared.input_bytes_total > u64::from(CSR_NODE_COUNT) * 20);
}

#[test]
fn skewed_csr_queue_inputs_preserve_frontier_and_device_scratch() {
    let fixture = build_skewed_csr_fixture(4096).unwrap();
    let capacity = support::skewed_csr_queue_capacity(fixture.stats.active_sources).unwrap();
    let high_capacity = support::skewed_csr_active_high_degree_sources(
        &fixture,
        queue_materialize::GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
    )
    .unwrap();
    let inputs = support::skewed_csr_queue_inputs(&fixture, capacity, high_capacity).unwrap();

    assert_eq!(inputs.len(), 9);
    assert_eq!(
        inputs[queue_materialize::QUEUE_FRONTIER_IN_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in)
    );
    assert_eq!(
        inputs[queue_materialize::QUEUE_ACTIVE_QUEUE_INDEX].len(),
        capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        vyre_primitives::wire::decode_u32_le_bytes_all(&inputs[queue_materialize::QUEUE_LEN_INDEX]),
        vec![0]
    );
    assert_eq!(
        inputs[queue_materialize::QUEUE_HIGH_QUEUE_INDEX].len(),
        high_capacity as usize * std::mem::size_of::<u32>()
    );
    assert_eq!(
        vyre_primitives::wire::decode_u32_le_bytes_all(
            &inputs[queue_materialize::QUEUE_HIGH_LEN_INDEX]
        ),
        vec![0]
    );

    let undersized = capacity.saturating_sub(1);
    let err = support::skewed_csr_queue_inputs(&fixture, undersized, high_capacity).unwrap_err();
    assert!(
        err.to_string().contains("queue_capacity >= active_sources"),
        "queue capacity errors must name the invariant, got: {err}"
    );
}

#[test]
fn skewed_csr_queue_prepare_builds_sparse_resident_sequence() {
    let prepared = queue_materialize::prepare_skewed_csr_queue_materialize_step(None).unwrap();

    assert_eq!(prepared.reset_program.workgroup_size(), [1, 1, 1]);
    assert_eq!(queue_materialize::QUEUE_RESET_GRID, [1, 1, 1]);
    assert_eq!(prepared.queue_program.workgroup_size(), [256, 1, 1]);
    assert!(prepared.row_strided_traverse);
    assert!(prepared.split_high_degree_traverse);
    assert_eq!(prepared.high_degree_queue_capacity, 256);
    assert_eq!(
        prepared.traverse_grid,
        [prepared.queue_capacity.div_ceil(256).max(1), 1, 1]
    );
    assert_eq!(prepared.high_traverse_grid, [32, 1, 1]);
    assert_eq!(
        prepared.traverse_logical_lanes,
        u64::from(prepared.queue_capacity)
            + u64::from(prepared.high_degree_queue_capacity)
                * u64::from(
                    vyre_primitives::graph::csr_queue_strided::CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE
                )
    );
    assert_eq!(
        prepared.queue_program.buffers()[3].name.as_ref(),
        "frontier_out"
    );
    assert_eq!(
        prepared.queue_program.buffers()[3].count,
        prepared.stats.frontier_words
    );
    assert_eq!(prepared.inputs.len(), 9);
    assert_eq!(prepared.stats.node_count, CSR_NODE_COUNT);
    assert_eq!(
        u64::from(prepared.queue_capacity),
        prepared.stats.active_sources
    );
    assert!(
        prepared.queue_capacity < prepared.stats.node_count / 32,
        "queue capacity should stay sparse relative to the full node-grid launch"
    );
    assert!(
        prepared.traverse_logical_lanes < u64::from(prepared.stats.node_count) / 4,
        "mixed queue traversal should keep launched logical lanes well below graph-sized traversal"
    );
    assert_ne!(
        queue_materialize::graph_queue_materialize_sequence_fingerprint(&prepared),
        prepared.traverse_program.fingerprint()
    );
}

#[test]
fn skewed_csr_graph_row_striding_requires_wide_rows() {
    let lanes =
        vyre_primitives::graph::csr_queue_strided::CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE;
    assert_eq!(
        queue_materialize::GRAPH_QUEUE_ROW_STRIDED_MIN_DEGREE,
        lanes.saturating_mul(lanes)
    );
    assert!(
        !queue_materialize::graph_queue_should_use_row_strided(96),
        "96-degree rows are not wide enough to justify a 32-lane team for every queued graph source"
    );
    assert!(queue_materialize::graph_queue_should_use_row_strided(
        support::UGLY_HUB_DEGREE
    ));
    assert_eq!(
        queue_materialize::GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
        lanes * 2
    );
    assert!(
        queue_materialize::graph_queue_should_use_split_high_degree(1_000, 256),
        "graph hubs should use the mixed split path when most queued rows stay low-degree"
    );
    assert!(queue_materialize::graph_queue_should_use_row_strided(
        queue_materialize::GRAPH_QUEUE_ROW_STRIDED_MIN_DEGREE
    ));
}

#[test]
fn generated_skewed_csr_queue_capacity_covers_active_sources_without_node_grid() {
    const CASES: u32 = 10_000;

    let mut total_nodes = 0_u64;
    let mut total_queue_capacity = 0_u64;
    let mut total_high_capacity = 0_u64;
    let mut all_row_strided_candidates = 0_u32;
    let mut split_cases = 0_u32;

    for case in 0..CASES {
        let node_count = 32_u32 << (case % 8);
        let fixture = build_skewed_csr_fixture(node_count).unwrap_or_else(|error| {
            panic!("generated skewed CSR fixture case {case} failed: {error}")
        });
        let capacity = support::skewed_csr_queue_capacity(fixture.stats.active_sources)
            .unwrap_or_else(|error| panic!("generated queue capacity case {case} failed: {error}"));
        let high_capacity = support::skewed_csr_active_high_degree_sources(
            &fixture,
            queue_materialize::GRAPH_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
        )
        .unwrap_or_else(|error| {
            panic!("generated high-degree capacity case {case} failed: {error}")
        });
        let inputs = support::skewed_csr_queue_inputs(&fixture, capacity, high_capacity)
            .unwrap_or_else(|error| panic!("generated queue inputs case {case} failed: {error}"));

        assert_eq!(
            u64::from(capacity),
            fixture.stats.active_sources,
            "queue capacity should exactly cover active sources case {case}"
        );
        assert_eq!(
            inputs[queue_materialize::QUEUE_ACTIVE_QUEUE_INDEX].len(),
            capacity as usize * std::mem::size_of::<u32>(),
            "active queue byte length case {case}"
        );
        assert_eq!(
            inputs[queue_materialize::QUEUE_HIGH_QUEUE_INDEX].len(),
            high_capacity as usize * std::mem::size_of::<u32>(),
            "high queue byte length case {case}"
        );
        assert!(
            support::skewed_csr_queue_inputs(&fixture, capacity.saturating_sub(1), high_capacity)
                .is_err(),
            "undersized queue should fail case {case}"
        );
        all_row_strided_candidates += u32::from(
            queue_materialize::graph_queue_should_use_row_strided(fixture.stats.max_degree),
        );
        split_cases += u32::from(queue_materialize::graph_queue_should_use_split_high_degree(
            capacity,
            high_capacity,
        ));
        total_nodes += u64::from(node_count);
        total_queue_capacity += u64::from(capacity);
        total_high_capacity += u64::from(high_capacity);
    }

    assert_eq!(all_row_strided_candidates, CASES);
    assert!(split_cases > 0);
    assert!(total_high_capacity > 0);
    assert!(
        total_queue_capacity * 8 < total_nodes,
        "generated sparse frontiers should avoid graph-sized queue traversal"
    );
}

#[test]
fn skewed_csr_queue_closure_inputs_materialize_seed_queue_once() {
    let fixture = build_skewed_csr_fixture(4096).unwrap();
    let capacity = support::skewed_csr_queue_capacity(fixture.stats.active_sources).unwrap();
    let inputs = support::skewed_csr_queue_closure_inputs(&fixture, capacity).unwrap();

    assert_eq!(inputs.len(), 11);
    assert_eq!(
        inputs[queue_closure::QUEUE_CLOSURE_SEED_FRONTIER_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in)
    );
    assert_eq!(
        inputs[queue_closure::QUEUE_CLOSURE_ACCUMULATOR_INDEX],
        vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in)
    );

    let seed_queue = vyre_primitives::wire::decode_u32_le_bytes_all(
        &inputs[queue_closure::QUEUE_CLOSURE_SEED_QUEUE_INDEX],
    );
    assert_eq!(seed_queue.len(), capacity as usize);
    assert!(seed_queue.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        vyre_primitives::wire::decode_u32_le_bytes_all(
            &inputs[queue_closure::QUEUE_CLOSURE_SEED_LEN_INDEX]
        ),
        vec![capacity]
    );
    assert_eq!(
        inputs[queue_closure::QUEUE_CLOSURE_QUEUE_A_INDEX].len(),
        capacity as usize * std::mem::size_of::<u32>()
    );
    assert!(inputs[queue_closure::QUEUE_CLOSURE_QUEUE_A_INDEX]
        .iter()
        .all(|byte| *byte == 0));
    assert!(inputs[queue_closure::QUEUE_CLOSURE_QUEUE_B_INDEX]
        .iter()
        .all(|byte| *byte == 0));
    assert_eq!(
        vyre_primitives::wire::decode_u32_le_bytes_all(
            &inputs[queue_closure::QUEUE_CLOSURE_LEN_A_INDEX]
        ),
        vec![0]
    );
    assert_eq!(
        vyre_primitives::wire::decode_u32_le_bytes_all(
            &inputs[queue_closure::QUEUE_CLOSURE_LEN_B_INDEX]
        ),
        vec![0]
    );
}

#[test]
fn skewed_csr_queue_closure_prepare_builds_resident_delta_sequence() {
    let prepared = queue_closure::prepare_skewed_csr_queue_closure(None).unwrap();

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
    assert_eq!(prepared.inputs.len(), 11);
    assert_eq!(prepared.stats.node_count, CSR_NODE_COUNT);
    assert_eq!(
        prepared.baseline_output.len(),
        prepared.stats.frontier_words as usize * std::mem::size_of::<u32>()
    );
    assert!(prepared.closure_iterations > 0);
    assert!(prepared.closure_iterations <= queue_closure::GRAPH_QUEUE_CLOSURE_MAX_ITERS);
    assert_eq!(prepared.closure_changed, 1);
    assert!(prepared.queue_capacity >= prepared.seed_queue_len);
    assert!(prepared.queue_capacity >= prepared.max_wave_queue_len);
    assert!(prepared.queue_capacity <= prepared.stats.node_count);
    assert!(prepared.total_queue_pops >= u64::from(prepared.seed_queue_len));
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
            queue_closure::graph_queue_closure_delta_lanes_per_source(prepared.row_strided_delta),
            launch_lanes,
        );
    assert_eq!(
        lane_profile.profiled_delta_source_slots,
        prepared.total_queue_pops
    );
    assert!(lane_profile.elided_delta_lanes > 0);
    assert!(lane_profile.delta_lane_elision_x1000 > 500);
    if prepared.queue_capacity
        > vyre_primitives::graph::csr_queue_delta::CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY
    {
        assert!(lane_profile.launched_delta_lanes < lane_profile.fixed_delta_lanes);
        assert!(lane_profile.launch_elided_delta_lanes > 0);
        assert!(lane_profile.launch_lane_elision_x1000 > 0);
    } else {
        assert!(lane_profile.launched_delta_lanes >= lane_profile.fixed_delta_lanes);
        assert!(
            lane_profile.launched_delta_lanes - lane_profile.fixed_delta_lanes
                < prepared.wave_queue_lengths.len() as u64
                    * u64::from(prepared.delta_program.workgroup_size()[0])
        );
        assert_eq!(lane_profile.launch_elided_delta_lanes, 0);
        assert_eq!(lane_profile.launch_lane_elision_x1000, 0);
    }
}

#[test]
fn generated_skewed_csr_queue_closure_capacity_covers_every_wave() {
    const CASES: u32 = 10_000;

    let mut total_iterations = 0_u64;
    let mut changed_cases = 0_u32;
    let mut total_queue_capacity = 0_u64;

    for case in 0..CASES {
        let node_count = 32_u32 << (case % 8);
        let fixture = build_skewed_csr_fixture(node_count).unwrap_or_else(|error| {
            panic!("generated skewed CSR closure fixture case {case} failed: {error}")
        });
        let oracle = support::skewed_csr_queue_closure_oracle(&fixture, node_count, node_count)
            .unwrap_or_else(|error| {
                panic!("generated skewed CSR closure oracle case {case} failed: {error}")
            });
        let capacity = oracle
            .max_wave_queue_len
            .max(fixture.stats.active_sources as u32)
            .max(1);
        let inputs = support::skewed_csr_queue_closure_inputs(&fixture, capacity)
            .unwrap_or_else(|error| panic!("generated closure inputs case {case} failed: {error}"));

        assert_eq!(oracle.output.len(), fixture.frontier_out_seed.len());
        assert!(oracle.iterations <= node_count, "case {case}");
        assert!(capacity <= node_count, "capacity case {case}");
        assert!(
            oracle.total_queue_pops >= fixture.stats.active_sources,
            "queue pops case {case}"
        );
        assert_eq!(
            oracle.wave_queue_lengths.len(),
            oracle.iterations as usize,
            "wave profile length case {case}"
        );
        assert_eq!(
            oracle
                .wave_queue_lengths
                .iter()
                .map(|&len| u64::from(len))
                .sum::<u64>(),
            oracle.total_queue_pops,
            "wave profile sum case {case}"
        );
        assert_eq!(
            oracle.wave_queue_lengths.iter().copied().max().unwrap_or(0),
            oracle.max_wave_queue_len,
            "wave profile max case {case}"
        );
        assert_eq!(
            vyre_primitives::wire::decode_u32_le_bytes_all(
                &inputs[queue_closure::QUEUE_CLOSURE_SEED_LEN_INDEX]
            ),
            vec![fixture.stats.active_sources as u32],
            "seed length case {case}"
        );
        assert_eq!(
            inputs[queue_closure::QUEUE_CLOSURE_ACCUMULATOR_INDEX],
            vyre_primitives::wire::pack_u32_slice(&fixture.frontier_in),
            "seed accumulator case {case}"
        );
        assert!(
            support::skewed_csr_queue_closure_inputs(
                &fixture,
                (fixture.stats.active_sources as u32).saturating_sub(1),
            )
            .is_err(),
            "undersized queue should fail case {case}"
        );

        total_iterations += u64::from(oracle.iterations);
        changed_cases += oracle.changed;
        total_queue_capacity += u64::from(capacity);
    }

    assert!(changed_cases > CASES / 2);
    assert!(total_iterations > u64::from(CASES));
    assert!(total_queue_capacity > u64::from(CASES));
}
