use super::super::fixture::{
    ifds_skewed_closure_oracle, IfdsSkewedFixture, IfdsSkewedStats, IFDS_REACH_MASK,
};
use super::super::queue::{
    ifds_queue_closure_inputs, ifds_queue_should_use_row_strided, ifds_skewed_queue_closure_oracle,
    QUEUE_CLOSURE_QUEUE_A_INDEX, QUEUE_CLOSURE_QUEUE_B_INDEX, QUEUE_CLOSURE_SEED_LEN_INDEX,
    QUEUE_CLOSURE_SEED_QUEUE_INDEX,
};
use proptest::prelude::*;

fn generated_ifds_fixture(
    node_count: u32,
    seeds: &[u32],
    edges: &[(u32, u32, bool)],
) -> IfdsSkewedFixture {
    let frontier_words = node_count.div_ceil(32);
    let mut frontier_in = vec![0_u32; frontier_words as usize];
    for &seed in seeds {
        let node = seed % node_count;
        frontier_in[(node / 32) as usize] |= 1_u32 << (node % 32);
    }
    let active_sources = frontier_in
        .iter()
        .map(|word| u64::from(word.count_ones()))
        .sum::<u64>();
    let mut by_source = vec![Vec::<(u32, u32)>::new(); node_count as usize];
    for &(src, dst, allowed) in edges {
        let kind_mask = if allowed { IFDS_REACH_MASK } else { 0 };
        by_source[(src % node_count) as usize].push((dst % node_count, kind_mask));
    }

    let mut edge_offsets = Vec::with_capacity(node_count as usize + 1);
    let mut edge_targets = Vec::new();
    let mut edge_kind_mask = Vec::new();
    let mut allowed_edges_from_active = 0_u64;
    let mut filtered_edges_from_active = 0_u64;
    let mut max_degree = 0_u32;
    let mut high_degree_sources = 0_u64;
    edge_offsets.push(0);
    for src in 0..node_count {
        let src_edges = &by_source[src as usize];
        let degree = src_edges.len() as u32;
        max_degree = max_degree.max(degree);
        high_degree_sources += u64::from(degree >= 24);
        let src_word = (src / 32) as usize;
        let src_bit = 1_u32 << (src % 32);
        let src_active = frontier_in[src_word] & src_bit != 0;
        for &(dst, kind_mask) in src_edges {
            if src_active && kind_mask & IFDS_REACH_MASK != 0 {
                allowed_edges_from_active += 1;
            } else if src_active {
                filtered_edges_from_active += 1;
            }
            edge_targets.push(dst);
            edge_kind_mask.push(kind_mask);
        }
        edge_offsets.push(edge_targets.len() as u32);
    }

    IfdsSkewedFixture {
        nodes: vec![0; node_count as usize],
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_tags: vec![0; node_count as usize],
        frontier_in,
        frontier_out_seed: vec![0; frontier_words as usize],
        stats: IfdsSkewedStats {
            nodes: node_count,
            edges: edges.len() as u32,
            frontier_words,
            active_sources,
            allowed_edges_from_active,
            filtered_edges_from_active,
            output_words_set: 0,
            max_degree,
            high_degree_sources,
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4_096,
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::Off
        )),
        ..ProptestConfig::default()
    })]

    #[test]
    fn ifds_queue_closure_compact_capacity_matches_full_capacity_for_generated_graphs(
        node_count in 1_u32..=128,
        seeds in proptest::collection::vec(any::<u32>(), 0..=24),
        edges in proptest::collection::vec((any::<u32>(), any::<u32>(), any::<bool>()), 0..=256),
    ) {
        let fixture = generated_ifds_fixture(node_count, &seeds, &edges);
        let max_iters = node_count.saturating_add(1);
        let full = ifds_skewed_queue_closure_oracle(&fixture, max_iters, fixture.stats.nodes)?;
        let bitset = ifds_skewed_closure_oracle(&fixture, max_iters);
        let compact_capacity = full
            .max_wave_queue_len
            .max(fixture.stats.active_sources as u32)
            .max(1);
        let compact = ifds_skewed_queue_closure_oracle(&fixture, max_iters, compact_capacity)?;
        let compact_inputs = ifds_queue_closure_inputs(&fixture, compact_capacity)?;

        prop_assert_eq!(&full.output, &bitset.output);
        prop_assert_eq!(&full.output, &compact.output);
        prop_assert_eq!(full.iterations, compact.iterations);
        prop_assert_eq!(full.changed, compact.changed);
        prop_assert_eq!(full.total_queue_pops, compact.total_queue_pops);
        prop_assert_eq!(full.max_wave_queue_len, compact.max_wave_queue_len);
        prop_assert_eq!(&full.wave_queue_lengths, &compact.wave_queue_lengths);
        prop_assert_eq!(
            compact
                .wave_queue_lengths
                .iter()
                .map(|&len| u64::from(len))
                .sum::<u64>(),
            compact.total_queue_pops
        );
        prop_assert_eq!(
            compact.wave_queue_lengths.iter().copied().max().unwrap_or(0),
            compact.max_wave_queue_len
        );
        prop_assert_eq!(
            compact_inputs[QUEUE_CLOSURE_QUEUE_A_INDEX].len(),
            compact_capacity as usize * std::mem::size_of::<u32>()
        );
        prop_assert_eq!(
            compact_inputs[QUEUE_CLOSURE_QUEUE_B_INDEX].len(),
            compact_capacity as usize * std::mem::size_of::<u32>()
        );

        if fixture.stats.active_sources > 0 || full.max_wave_queue_len > 0 {
            prop_assert!(
                ifds_skewed_queue_closure_oracle(
                    &fixture,
                    max_iters,
                    compact_capacity.saturating_sub(1)
                )
                .is_err()
            );
        }
    }
}

#[test]
fn ifds_queue_closure_generated_ugly_hubs_match_bitset_with_exact_capacity() {
    const CASES: u32 = 10_000;

    let mut compact_cases = 0_u32;
    let mut exact_pressure_cases = 0_u32;
    let mut filtered_active_cases = 0_u32;
    let mut row_strided_cases = 0_u32;

    for case in 0..CASES {
        let node_count = 32 + (mix32(case ^ 0xA11C_E001) % 225);
        let hub = mix32(case ^ 0xA11C_E002) % node_count;
        let seeds = generated_seeds(node_count, hub, case);
        let edges = generated_ugly_hub_edges(node_count, hub, case);
        let fixture = generated_ifds_fixture(node_count, &seeds, &edges);
        let max_iters = node_count.saturating_add(1);

        row_strided_cases += u32::from(ifds_queue_should_use_row_strided(fixture.stats.max_degree));
        filtered_active_cases += u32::from(fixture.stats.filtered_edges_from_active > 0);

        let full = ifds_skewed_queue_closure_oracle(&fixture, max_iters, fixture.stats.nodes)
            .unwrap_or_else(|error| {
                panic!("generated IFDS queue closure full-capacity case {case} failed: {error}")
            });
        let bitset = ifds_skewed_closure_oracle(&fixture, max_iters);
        let compact_capacity = full
            .max_wave_queue_len
            .max(fixture.stats.active_sources as u32)
            .max(1);
        let compact = ifds_skewed_queue_closure_oracle(&fixture, max_iters, compact_capacity)
            .unwrap_or_else(|error| {
                panic!("generated IFDS queue closure compact-capacity case {case} failed: {error}")
            });
        let compact_inputs =
            ifds_queue_closure_inputs(&fixture, compact_capacity).unwrap_or_else(|error| {
                panic!("generated IFDS queue closure inputs case {case} failed: {error}")
            });

        assert_eq!(full.output, bitset.output, "bitset oracle case {case}");
        assert_eq!(
            full.output, compact.output,
            "compact queue output case {case}"
        );
        assert_eq!(
            full.iterations, compact.iterations,
            "iterations case {case}"
        );
        assert_eq!(full.changed, compact.changed, "changed flag case {case}");
        assert_eq!(
            full.total_queue_pops, compact.total_queue_pops,
            "queue pop count case {case}"
        );
        assert_eq!(
            full.max_wave_queue_len, compact.max_wave_queue_len,
            "max wave case {case}"
        );
        assert_eq!(
            full.wave_queue_lengths, compact.wave_queue_lengths,
            "wave profile case {case}"
        );
        assert_eq!(
            compact
                .wave_queue_lengths
                .iter()
                .map(|&len| u64::from(len))
                .sum::<u64>(),
            compact.total_queue_pops,
            "wave profile sum case {case}"
        );
        assert_eq!(
            compact
                .wave_queue_lengths
                .iter()
                .copied()
                .max()
                .unwrap_or(0),
            compact.max_wave_queue_len,
            "wave profile max case {case}"
        );
        assert_eq!(
            compact_inputs[QUEUE_CLOSURE_SEED_LEN_INDEX],
            vyre_primitives::wire::pack_u32_slice(&[fixture.stats.active_sources as u32]),
            "seed length input case {case}"
        );
        assert_eq!(
            compact_inputs[QUEUE_CLOSURE_QUEUE_A_INDEX].len(),
            compact_capacity as usize * std::mem::size_of::<u32>(),
            "queue A byte length case {case}"
        );
        assert_eq!(
            compact_inputs[QUEUE_CLOSURE_QUEUE_B_INDEX].len(),
            compact_capacity as usize * std::mem::size_of::<u32>(),
            "queue B byte length case {case}"
        );

        let mut seed_queue = Vec::new();
        vyre_primitives::wire::unpack_u32_slice_into(
            &compact_inputs[QUEUE_CLOSURE_SEED_QUEUE_INDEX],
            fixture.stats.active_sources as usize,
            "generated IFDS queue closure seed queue",
            &mut seed_queue,
        )
        .unwrap_or_else(|error| panic!("seed queue unpack case {case} failed: {error}"));
        assert!(
            seed_queue.windows(2).all(|pair| pair[0] < pair[1]),
            "seed queue order case {case}"
        );

        if compact_capacity < fixture.stats.nodes {
            compact_cases += 1;
        }
        if compact_capacity == full.max_wave_queue_len {
            exact_pressure_cases += 1;
        }
        assert!(
            ifds_skewed_queue_closure_oracle(
                &fixture,
                max_iters,
                compact_capacity.saturating_sub(1)
            )
            .is_err(),
            "case {case} should fail below exact queue capacity"
        );
    }

    assert_eq!(row_strided_cases, CASES);
    assert_eq!(filtered_active_cases, CASES);
    assert!(
        compact_cases > CASES * 9 / 10,
        "generated hub closures should usually avoid graph-sized ping-pong queues"
    );
    assert!(
        exact_pressure_cases > CASES / 2,
        "generated hub closures should frequently exercise exact queue pressure"
    );
}

fn generated_seeds(node_count: u32, hub: u32, case: u32) -> [u32; 5] {
    [
        hub,
        0,
        node_count - 1,
        mix32(case ^ 0x51CE_0001) % node_count,
        mix32(case ^ 0x51CE_0002) % node_count,
    ]
}

fn generated_ugly_hub_edges(node_count: u32, hub: u32, case: u32) -> Vec<(u32, u32, bool)> {
    let hub_degree = 1_024 + (mix32(case ^ 0xD37A_0001) % 1_025);
    let dst_pool = 8 + (mix32(case ^ 0xD37A_0002) % node_count.min(24));
    let mut edges = Vec::with_capacity((node_count as usize * 3) + hub_degree as usize + 16);

    for edge in 0..hub_degree {
        let dst = match edge % 7 {
            0 => hub,
            1 => hub.wrapping_add((edge.wrapping_mul(3) + 1) % dst_pool) % node_count,
            2 => hub.wrapping_add((edge.wrapping_mul(5) + 7) % dst_pool) % node_count,
            3 => {
                hub.wrapping_add(mix32(case ^ edge.wrapping_mul(0x9E37_79B9)) % dst_pool)
                    % node_count
            }
            4 => 0,
            5 => node_count - 1,
            _ => hub.wrapping_add((edge / 2) % dst_pool) % node_count,
        };
        let allowed = edge % 5 != 0;
        edges.push((hub, dst, allowed));
        if edge % 7 == 0 {
            edges.push((hub, dst, true));
        }
    }

    let chain_len = node_count.min(17);
    for step in 0..chain_len {
        let src = hub.wrapping_add(step) % node_count;
        let dst = hub.wrapping_add(step + 1) % node_count;
        edges.push((src, dst, true));
    }

    for src in 0..node_count {
        let fanout = mix32(case ^ src.wrapping_mul(0x45D9_F3B)) % 4;
        for edge in 0..fanout {
            let dst = src
                .wrapping_add(1)
                .wrapping_add(mix32(case ^ src ^ edge.wrapping_mul(0x85EB_CA6B)) % node_count)
                % node_count;
            let allowed = (src ^ edge ^ case) & 3 != 0;
            edges.push((src, dst, allowed));
        }
    }

    edges
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
