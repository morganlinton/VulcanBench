use super::super::fixture::{ifds_active_high_degree_sources, ifds_queue_inputs};
use super::super::queue::{
    ifds_queue_should_use_split_high_degree, ifds_queue_traverse_logical_lanes,
    ifds_sparse_queue_capacity, QUEUE_ACTIVE_QUEUE_INDEX, QUEUE_HIGH_QUEUE_INDEX,
};
use super::*;
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_mixed_logical_lanes, CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
};

#[test]
fn generated_ifds_queue_split_targets_only_active_hub_rows() {
    const CASES: u32 = 10_000;

    let mut split_cases = 0_u32;
    let mut total_row_strided_lanes = 0_u128;
    let mut total_split_lanes = 0_u128;
    let mut total_high_capacity = 0_u64;

    for case in 0..CASES {
        let node_count = 32_u32 << (case % 8);
        let fixture = build_ifds_skewed_fixture(node_count).unwrap_or_else(|error| {
            panic!("generated IFDS split fixture case {case} failed: {error}")
        });
        let queue_capacity = ifds_sparse_queue_capacity(fixture.stats.active_sources)
            .unwrap_or_else(|error| panic!("generated IFDS queue capacity case {case}: {error}"));
        let high_capacity =
            ifds_active_high_degree_sources(&fixture, CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD)
                .unwrap_or_else(|error| {
                    panic!("generated IFDS high capacity case {case}: {error}")
                });
        let inputs = ifds_queue_inputs(&fixture, queue_capacity, high_capacity)
            .unwrap_or_else(|error| panic!("generated IFDS queue inputs case {case}: {error}"));
        let row_strided_lanes = ifds_queue_traverse_logical_lanes(queue_capacity, true);
        let split_lanes = csr_queue_split_mixed_logical_lanes(queue_capacity, high_capacity);

        assert_eq!(
            inputs[QUEUE_ACTIVE_QUEUE_INDEX].len(),
            queue_capacity as usize * std::mem::size_of::<u32>(),
            "active queue bytes case {case}"
        );
        assert_eq!(
            inputs[QUEUE_HIGH_QUEUE_INDEX].len(),
            high_capacity as usize * std::mem::size_of::<u32>(),
            "high queue bytes case {case}"
        );
        let uses_split = ifds_queue_should_use_split_high_degree(queue_capacity, high_capacity);
        if uses_split {
            assert!(
                split_lanes < row_strided_lanes,
                "split lanes should beat all-row striding case {case}"
            );
        }
        split_cases += u32::from(uses_split);
        total_row_strided_lanes += u128::from(row_strided_lanes);
        total_split_lanes += u128::from(if uses_split {
            split_lanes
        } else {
            row_strided_lanes
        });
        total_high_capacity += u64::from(high_capacity);
    }

    assert!(split_cases > CASES / 2);
    assert!(total_high_capacity > 0);
    assert!(
        total_split_lanes * 8 < total_row_strided_lanes,
        "mixed IFDS traversal should keep generated lane pressure far below all-row striding"
    );
}

#[test]
fn generated_word_queue_materializer_launches_frontier_words_not_nodes() {
    const CASES: u32 = 10_000;

    let mut old_lanes = 0_u128;
    let mut new_lanes = 0_u128;
    for case in 0..CASES {
        let node_count = 65_536 + (mix32(case ^ 0xF901_DA7A) % 983_041);
        let frontier_words = node_count.div_ceil(32);
        let queue_capacity = 1 + (mix32(case ^ 0x51E5_4A11) % frontier_words.max(1));
        let program = vyre_primitives::graph::csr_frontier_queue::frontier_words_to_queue_parallel(
            "frontier_in",
            "active_queue",
            "queue_len",
            node_count,
            queue_capacity,
        );
        let inputs = vec![
            vec![0_u8; frontier_words as usize * std::mem::size_of::<u32>()],
            vec![0_u8; queue_capacity as usize * std::mem::size_of::<u32>()],
            vyre_primitives::wire::pack_u32_slice(&[0]),
        ];
        let grid = vyre_driver::infer_dispatch_grid(
            &program,
            &inputs,
            &vyre_driver::DispatchConfig::default(),
        )
        .unwrap_or_else(|error| {
            panic!("generated word queue materializer case {case} failed: {error}")
        });
        let word_grid_x = frontier_words.div_ceil(256).max(1);
        let node_grid_x = node_count.div_ceil(256).max(1);

        assert_eq!(grid, [word_grid_x, 1, 1], "case {case}");
        old_lanes += u128::from(node_grid_x) * 256;
        new_lanes += u128::from(word_grid_x) * 256;
    }

    assert!(
        old_lanes >= new_lanes * 31,
        "packed-word queue materialization should cut generated launch lanes by about 32x"
    );
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
