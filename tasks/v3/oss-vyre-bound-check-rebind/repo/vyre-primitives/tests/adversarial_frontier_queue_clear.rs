//! Adversarial tests for packed frontier queue materialization with fused output clear.

#![cfg(all(feature = "graph", feature = "cpu-parity"))]

use proptest::prelude::*;
use vyre_primitives::graph::csr_frontier_queue::{
    frontier_to_queue_cpu, frontier_words_to_queue_clear_out_parallel,
};
use vyre_reference::value::Value;

fn pack_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn unpack_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
        .collect()
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn generated_words(node_count: u32, seed: u32) -> Vec<u32> {
    let words = node_count.div_ceil(32);
    (0..words)
        .map(|word| {
            let mut bits = mix32(seed ^ word.wrapping_mul(0x9e37_79b9));
            if word % 3 == 0 {
                bits |= 1 << 31;
            }
            if word % 5 == 0 {
                bits |= 1;
            }
            bits
        })
        .collect()
}

#[test]
fn word_parallel_frontier_queue_clear_out_matches_queue_and_zeros_output() {
    let node_count = 70;
    let queue_capacity = 8;
    let frontier = [
        (1_u32 << 0) | (1_u32 << 1) | (1_u32 << 31),
        (1_u32 << 0) | (1_u32 << 31),
        (1_u32 << 0) | (1_u32 << 5) | (1_u32 << 31),
    ];
    let frontier_out_seed = [u32::MAX, 0xA5A5_A5A5, 0x8000_0001];
    let (expected_queue, expected_seen) =
        frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
    let program = frontier_words_to_queue_clear_out_parallel(
        "frontier",
        "queue",
        "queue_len",
        "frontier_out",
        node_count,
        queue_capacity,
    );

    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(pack_words(&frontier)),
            Value::from(vec![
                0_u8;
                queue_capacity as usize * std::mem::size_of::<u32>()
            ]),
            Value::from(pack_words(&[0])),
            Value::from(pack_words(&frontier_out_seed)),
        ],
    )
    .expect("word-level clear-out frontier queue materializer should reference-evaluate");

    let mut queue = unpack_words(&outputs[0].to_bytes());
    queue.truncate(expected_queue.len());
    queue.sort_unstable();
    let mut expected_sorted = expected_queue;
    expected_sorted.sort_unstable();

    assert_eq!(unpack_words(&outputs[1].to_bytes()), vec![expected_seen]);
    assert_eq!(queue, expected_sorted);
    assert_eq!(unpack_words(&outputs[2].to_bytes()), vec![0, 0, 0]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn generated_word_parallel_frontier_queue_clear_out_matches_cpu_oracle(
        node_count in 1u32..=257,
        frontier_seed in any::<u32>(),
        out_seed in any::<u32>(),
        capacity_salt in any::<u32>(),
    ) {
        let frontier = generated_words(node_count, frontier_seed);
        let frontier_words = frontier.len();
        let queue_capacity = 1 + capacity_salt % (node_count + 7);
        let frontier_out_seed = generated_words(node_count, out_seed ^ 0xa5a5_5a5a);
        let (all_active, expected_seen) =
            frontier_to_queue_cpu(&frontier, node_count, node_count as usize);
        let program = frontier_words_to_queue_clear_out_parallel(
            "frontier",
            "queue",
            "queue_len",
            "frontier_out",
            node_count,
            queue_capacity,
        );

        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(pack_words(&frontier)),
                Value::from(vec![
                    0_u8;
                    queue_capacity as usize * std::mem::size_of::<u32>()
                ]),
                Value::from(pack_words(&[0])),
                Value::from(pack_words(&frontier_out_seed)),
            ],
        )
        .expect("generated clear-out frontier queue materializer should reference-evaluate");

        let queue_words = unpack_words(&outputs[0].to_bytes());
        let written = expected_seen.min(queue_capacity) as usize;
        let mut actual_queue = queue_words.into_iter().take(written).collect::<Vec<_>>();
        let mut unique_actual = actual_queue.clone();
        unique_actual.sort_unstable();
        unique_actual.dedup();
        let mut sorted_active = all_active.clone();
        sorted_active.sort_unstable();

        prop_assert_eq!(unpack_words(&outputs[1].to_bytes()), vec![expected_seen]);
        prop_assert_eq!(unique_actual.len(), actual_queue.len());
        for node in &actual_queue {
            prop_assert!(
                sorted_active.binary_search(node).is_ok(),
                "queue emitted inactive node {node} for node_count={node_count}"
            );
        }
        if queue_capacity >= expected_seen {
            actual_queue.sort_unstable();
            prop_assert_eq!(actual_queue, sorted_active);
        }
        prop_assert_eq!(
            unpack_words(&outputs[2].to_bytes()),
            vec![0; frontier_words]
        );
    }
}
