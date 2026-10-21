use super::*;
use crate::bitset::bitset_words;
use crate::graph::csr_frontier_queue::try_csr_queue_forward_traverse_cpu_into;
use crate::graph::csr_queue_strided::{
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE, CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE,
};

#[test]
fn split_low_program_has_stable_buffer_shape() {
    let program = csr_queue_split_low_forward_traverse(
        "active_queue",
        "queue_len",
        "edge_offsets",
        "edge_targets",
        "edge_kind_mask",
        "frontier_out",
        "high_queue",
        "high_len",
        64,
        12,
        8,
        3,
        CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
        1,
    );

    assert_eq!(
        program.workgroup_size(),
        CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE
    );
    assert_eq!(program.buffers().len(), 8);
    assert_eq!(program.buffers()[5].name.as_ref(), "frontier_out");
    assert_eq!(program.buffers()[6].name.as_ref(), "high_queue");
    assert_eq!(program.buffers()[7].name.as_ref(), "high_len");
}

#[test]
fn split_low_rejects_offset_count_overflow_without_panic() {
    let result = std::panic::catch_unwind(|| {
        csr_queue_split_low_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            "high_queue",
            "high_len",
            u32::MAX,
            0,
            1,
            1,
            CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
            1,
        )
    });

    assert!(
        result.is_ok(),
        "CSR queue split builder must reject offset-count overflow without panicking"
    );
    let program = result.unwrap();
    assert!(program.stats().trap());
    let entry = format!("{:?}", program.entry());
    assert!(
        entry.contains("node_count + 1 overflows u32"),
        "Fix: trap must retain the CSR offset-count overflow diagnostic, got: {entry}"
    );
}

#[test]
fn mixed_logical_lanes_charge_low_rows_once_and_high_rows_as_lane_teams() {
    assert_eq!(csr_queue_split_low_dispatch_grid(0), [1, 1, 1]);
    assert_eq!(csr_queue_split_low_dispatch_grid(1), [1, 1, 1]);
    assert_eq!(csr_queue_split_low_dispatch_grid(256), [1, 1, 1]);
    assert_eq!(csr_queue_split_low_dispatch_grid(257), [2, 1, 1]);
    assert_eq!(csr_queue_split_mixed_logical_lanes(12_057, 256), 20_249);
    assert!(
        csr_queue_split_mixed_logical_lanes(12_057, 256)
            < 12_057 * u64::from(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE)
    );
    assert_eq!(CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE, [256, 1, 1]);
}

#[test]
fn generated_split_low_plus_high_queue_matches_scalar_traversal() {
    const CASES: u32 = 10_000;
    const ALLOW: u32 = 1;
    const THRESHOLD: u32 = 16;

    let mut overflow_cases = 0_u32;
    let mut lane_wins = 0_u32;
    for case in 0..CASES {
        let node_count = 33 + (mix32(case ^ 0x7A11_51E5) % 191);
        let (edge_offsets, edge_targets, edge_kind_mask) = generated_graph(node_count, case);
        let active_queue = generated_active_queue(node_count, case);
        let queue_len = active_queue.len() as u32;
        let high_active = active_queue
            .iter()
            .filter(|&&src| {
                let start = edge_offsets[src as usize];
                let end = edge_offsets[src as usize + 1];
                end - start >= THRESHOLD
            })
            .count();
        let high_capacity = high_active.saturating_sub((case as usize) & 1);
        overflow_cases += u32::from(high_capacity < high_active);
        let seed = vec![0_u32; bitset_words(node_count) as usize];
        let split = try_csr_queue_split_low_forward_traverse_cpu(
            &active_queue,
            queue_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &seed,
            node_count,
            high_capacity,
            THRESHOLD,
            ALLOW,
        )
        .unwrap_or_else(|err| panic!("generated split case {case} failed: {err}"));

        let mut mixed_out = split.frontier_out;
        for &src in &split.high_queue {
            let start = edge_offsets[src as usize] as usize;
            let end = edge_offsets[src as usize + 1] as usize;
            emit_scalar_row_cpu(
                start,
                end,
                &edge_targets,
                &edge_kind_mask,
                node_count,
                ALLOW,
                &mut mixed_out,
            );
        }

        let mut scalar_out = seed;
        try_csr_queue_forward_traverse_cpu_into(
            &active_queue,
            queue_len,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            node_count,
            ALLOW,
            &mut scalar_out,
        )
        .unwrap_or_else(|err| panic!("generated scalar case {case} failed: {err}"));

        assert_eq!(mixed_out, scalar_out, "case {case}");
        assert_eq!(split.high_len as usize, high_active, "case {case}");
        assert_eq!(split.high_queue.len(), high_capacity, "case {case}");
        lane_wins += u32::from(
            csr_queue_split_mixed_logical_lanes(queue_len, split.high_queue.len() as u32)
                < u64::from(queue_len) * u64::from(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE),
        );
    }

    assert!(overflow_cases > CASES / 4);
    assert!(lane_wins > CASES * 9 / 10);
}

fn generated_active_queue(node_count: u32, case: u32) -> Vec<u32> {
    let mut active = Vec::new();
    for src in 0..node_count {
        if src % 17 == 0 || src % 31 == case % 31 || (mix32(src ^ case) & 63) == 0 {
            active.push(src);
        }
    }
    if active.is_empty() {
        active.push(case % node_count);
    }
    active
}

fn generated_graph(node_count: u32, case: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut offsets = Vec::with_capacity(node_count as usize + 1);
    let mut targets = Vec::new();
    let mut kinds = Vec::new();
    offsets.push(0);
    for src in 0..node_count {
        let degree = if src % 31 == case % 31 {
            16 + (mix32(src ^ case ^ 0xA511_0DD5) % 17)
        } else if src % 7 == 0 {
            5
        } else {
            1 + (mix32(src ^ case ^ 0xC001_BA5E) % 3)
        };
        for edge in 0..degree {
            targets.push(mix32(src ^ case ^ edge.wrapping_mul(0x9E37_79B9)) % node_count);
            kinds.push(if (edge + src + case) % 5 == 0 { 2 } else { 1 });
        }
        offsets.push(targets.len() as u32);
    }
    (offsets, targets, kinds)
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
