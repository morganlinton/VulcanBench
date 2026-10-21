use super::*;

fn pack_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn unpack_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
        .collect()
}

fn prefix_frontier_queue_reference(
    frontier: &[u32],
    node_count: u32,
    queue_capacity: u32,
) -> (Vec<u32>, u32) {
    let pass_a =
        frontier_word_counts_scan_pass_a("frontier", "word_partials", "block_totals", node_count);
    let pass_a_outputs =
        vyre_reference::reference_eval(&pass_a, &[Value::from(pack_words(frontier))])
            .expect("prefix frontier queue pass A should reference-evaluate");
    let scatter = frontier_word_block_prefix_to_queue_parallel(
        "frontier",
        "word_partials",
        "block_totals",
        "queue",
        "queue_len",
        node_count,
        queue_capacity,
    );
    let scatter_outputs = vyre_reference::reference_eval(
        &scatter,
        &[
            Value::from(pack_words(frontier)),
            pass_a_outputs[0].clone(),
            pass_a_outputs[1].clone(),
            Value::from(vec![
                0_u8;
                queue_capacity as usize * std::mem::size_of::<u32>()
            ]),
            Value::from(pack_words(&[0])),
        ],
    )
    .expect("prefix frontier queue scatter should reference-evaluate");
    let queue = unpack_words(&scatter_outputs[0].to_bytes());
    let len = unpack_words(&scatter_outputs[1].to_bytes())
        .into_iter()
        .next()
        .unwrap_or(0);
    (queue, len)
}

fn prefix_frontier_queue_cpu_model(
    frontier: &[u32],
    node_count: u32,
    queue_capacity: u32,
) -> (Vec<u32>, u32) {
    let words = bitset_words(node_count) as usize;
    let scan_lanes = 1024usize;
    let blocks = words.div_ceil(scan_lanes).max(1);
    let mut word_partials = vec![0u32; blocks * scan_lanes];
    let mut block_totals = vec![0u32; blocks];
    for block in 0..blocks {
        let mut acc = 0u32;
        for lane in 0..scan_lanes {
            let word_index = block * scan_lanes + lane;
            let count = if word_index < words {
                masked_frontier_word(frontier[word_index], word_index, words, node_count)
                    .count_ones()
            } else {
                0
            };
            acc = acc.wrapping_add(count);
            if word_index < words {
                word_partials[word_index] = acc;
            }
        }
        block_totals[block] = acc;
    }
    let mut block_totals_scanned = Vec::with_capacity(blocks);
    let mut block_acc = 0u32;
    for total in block_totals {
        block_acc = block_acc.wrapping_add(total);
        block_totals_scanned.push(block_acc);
    }

    let mut queue = vec![0u32; queue_capacity as usize];
    for word_index in 0..words {
        let block = word_index / scan_lanes;
        let word = masked_frontier_word(frontier[word_index], word_index, words, node_count);
        let active_bits = word.count_ones();
        let end = word_partials[word_index].wrapping_add(if block == 0 {
            0
        } else {
            block_totals_scanned[block - 1]
        });
        let start = end.wrapping_sub(active_bits);
        let mut remaining = word;
        for rank in 0..active_bits {
            let bit = remaining.trailing_zeros();
            let src = (word_index as u32) * 32 + bit;
            let slot = start + rank;
            if slot < queue_capacity && src < node_count {
                queue[slot as usize] = src;
            }
            remaining &= remaining - 1;
        }
    }
    (queue, block_totals_scanned.last().copied().unwrap_or(0))
}

fn masked_frontier_word(word: u32, word_index: usize, words: usize, node_count: u32) -> u32 {
    let tail_bits = node_count % 32;
    if tail_bits == 0 || word_index + 1 != words {
        return word;
    }
    word & ((1u32 << tail_bits) - 1)
}

fn generated_hostile_frontier(case: u32, node_count: u32) -> Vec<u32> {
    let words = bitset_words(node_count) as usize;
    let mut state = 0xA5A5_5A5Au32 ^ case.wrapping_mul(0x27D4_EB2D);
    let mut frontier = vec![0u32; words];
    for (word_index, word) in frontier.iter_mut().enumerate() {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *word = match (case + word_index as u32) % 6 {
            0 => 0,
            1 => u32::MAX,
            2 => state & 0x0101_0101,
            3 => state & 0x1111_1111,
            4 => state | 0x8000_0001,
            _ => state,
        };
    }
    let tail_bits = node_count % 32;
    if tail_bits != 0 {
        let in_range_mask = (1_u32 << tail_bits) - 1;
        if case % 5 == 0 {
            frontier[words - 1] &= !in_range_mask;
            frontier[words - 1] |= !in_range_mask;
        } else if case % 3 == 0 {
            frontier[words - 1] |= (!in_range_mask) & 0xD6DB_6DB6;
        }
    }
    frontier
}

#[test]
fn word_parallel_frontier_queue_matches_cpu_and_ignores_tail_bits() {
    let node_count = 70;
    let queue_capacity = 8;
    let frontier = [
        (1_u32 << 0) | (1_u32 << 1) | (1_u32 << 31),
        (1_u32 << 0) | (1_u32 << 31),
        (1_u32 << 0) | (1_u32 << 5) | (1_u32 << 31),
    ];
    let (expected_queue, expected_seen) =
        frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
    let program = frontier_words_to_queue_parallel(
        "frontier",
        "queue",
        "queue_len",
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
        ],
    )
    .expect("word-level frontier queue materializer should reference-evaluate");

    let mut queue = unpack_words(&outputs[0].to_bytes());
    queue.truncate(expected_queue.len());
    queue.sort_unstable();
    let mut expected_sorted = expected_queue;
    expected_sorted.sort_unstable();

    assert_eq!(unpack_words(&outputs[1].to_bytes()), vec![expected_seen]);
    assert_eq!(
        queue, expected_sorted,
        "word-level materializer must enqueue exactly in-range active nodes"
    );
    assert!(
        !queue.contains(&95),
        "tail bits beyond node_count must not enter the active queue"
    );
}

#[test]
fn word_parallel_frontier_queue_matches_cpu_len_across_2048_generated_frontiers() {
    for case in 0u32..2048 {
        let node_count = 1 + case.wrapping_mul(17) % 127;
        let queue_capacity = 1 + case.wrapping_mul(13) % 48;
        let words = bitset_words(node_count) as usize;
        let mut state = 0x9E37_79B9u32 ^ case.wrapping_mul(0x85EB_CA6B);
        let mut frontier = vec![0u32; words];

        for word in &mut frontier {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *word = match case % 5 {
                0 => 0,
                1 => state & 0x0101_0101,
                2 => state & 0x1111_1111,
                3 => state,
                _ => state | 0x8000_0001,
            };
        }

        let tail_bits = node_count % 32;
        if tail_bits != 0 {
            let in_range_mask = (1u32 << tail_bits) - 1;
            let tail_mask = !in_range_mask;
            if case % 7 == 0 {
                frontier[words - 1] &= !in_range_mask;
                frontier[words - 1] |= tail_mask;
            } else if case % 3 == 0 {
                frontier[words - 1] |= tail_mask & 0xA5A5_A5A5;
            }
        }

        let (expected_queue, expected_seen) =
            frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
        let active_nodes: Vec<u32> = (0..node_count)
            .filter(|&node| {
                let word = frontier[node as usize / 32];
                (word & (1u32 << (node % 32))) != 0
            })
            .collect();
        let program = frontier_words_to_queue_parallel(
            "frontier",
            "queue",
            "queue_len",
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
            ],
        )
        .unwrap_or_else(|error| {
            panic!(
                "case {case}: word-level frontier queue materializer failed reference_eval: {error}"
            )
        });

        let mut queue = unpack_words(&outputs[0].to_bytes());
        queue.truncate(expected_queue.len());
        queue.sort_unstable();
        let mut expected_sorted = expected_queue;
        expected_sorted.sort_unstable();

        assert_eq!(
            unpack_words(&outputs[1].to_bytes()),
            vec![expected_seen],
            "case {case}: queue_len must report in-range active nodes"
        );
        if expected_seen as usize <= queue_capacity as usize {
            assert_eq!(
                queue, expected_sorted,
                "case {case}: queue contents must match CPU oracle without overflow"
            );
        } else {
            assert_eq!(
                queue.len(),
                queue_capacity as usize,
                "case {case}: overflow must fill the bounded queue"
            );
            assert!(
                queue.windows(2).all(|pair| pair[0] != pair[1]),
                "case {case}: overflow queue must not duplicate an active node"
            );
            assert!(
                queue
                    .iter()
                    .all(|node| active_nodes.binary_search(node).is_ok()),
                "case {case}: overflow queue must contain only active in-range nodes"
            );
        }
        assert!(
            queue.iter().all(|&node| node < node_count),
            "case {case}: tail bit escaped into queue for node_count={node_count}"
        );
    }
}

#[test]
fn prefix_frontier_queue_cpu_model_preserves_order_across_4096_generated_frontiers() {
    for case in 0u32..4096 {
        let node_count = 1 + case.wrapping_mul(73) % 8192;
        let words = bitset_words(node_count) as usize;
        let queue_capacity = words as u32 + 1 + case.wrapping_mul(11) % 31;
        let frontier = generated_hostile_frontier(case, node_count);

        let (expected_queue, expected_seen) =
            frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
        let (queue, seen) = prefix_frontier_queue_cpu_model(&frontier, node_count, queue_capacity);

        assert_eq!(
            seen, expected_seen,
            "case {case}: prefix CPU model queue_len must count all in-range active nodes"
        );
        assert_eq!(
            &queue[..expected_queue.len()],
            expected_queue.as_slice(),
            "case {case}: prefix CPU model must preserve CPU source order"
        );
    }
}

#[test]
fn prefix_frontier_queue_reference_preserves_cpu_order_across_128_generated_frontiers() {
    for case in 0u32..128 {
        let node_count = 1 + case.wrapping_mul(73) % 4096;
        let words = bitset_words(node_count) as usize;
        let queue_capacity = words as u32 + 1 + case.wrapping_mul(11) % 31;
        let frontier = generated_hostile_frontier(case, node_count);

        let (expected_queue, expected_seen) =
            frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
        let (queue, seen) = prefix_frontier_queue_reference(&frontier, node_count, queue_capacity);

        assert_eq!(
            seen, expected_seen,
            "case {case}: prefix materializer queue_len must count all in-range active nodes"
        );
        assert_eq!(
            &queue[..expected_queue.len()],
            expected_queue.as_slice(),
            "case {case}: prefix materializer must preserve CPU source order"
        );
        assert!(
            queue[..expected_queue.len()]
                .iter()
                .all(|&node| node < node_count),
            "case {case}: prefix materializer let a tail bit escape into the queue"
        );
    }
}

#[test]
fn prefix_frontier_queue_handles_multi_block_scan_and_overflow() {
    let node_count = 32 * 1024 + 70;
    let words = bitset_words(node_count) as usize;
    let queue_capacity = words as u32 + 17;
    let mut frontier = vec![0u32; words];
    for node in 0..node_count {
        if node % 7 == 0 || node % 257 == 3 {
            frontier[node as usize / 32] |= 1_u32 << (node % 32);
        }
    }
    frontier[words - 1] |= 0xFFFF_0000;

    let (expected_queue, expected_seen) =
        frontier_to_queue_cpu(&frontier, node_count, queue_capacity as usize);
    let (queue, seen) = prefix_frontier_queue_reference(&frontier, node_count, queue_capacity);

    assert!(
        words > 1024,
        "test fixture must cross the local scan block boundary"
    );
    assert!(
        expected_seen as usize > queue_capacity as usize,
        "test fixture must exercise bounded-queue overflow"
    );
    assert_eq!(seen, expected_seen);
    assert_eq!(
        &queue[..queue_capacity as usize],
        expected_queue.as_slice(),
        "multi-block prefix materializer must keep the first capacity active nodes in source order"
    );
    assert!(queue.iter().all(|&node| node < node_count));
}
