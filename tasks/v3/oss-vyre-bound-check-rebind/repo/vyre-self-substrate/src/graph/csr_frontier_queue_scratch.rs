//! Shared scratch planning for resident CSR frontier queues.

use vyre_primitives::bitset::frontier::frontier_tail_mask;
use vyre_primitives::graph::csr_frontier_queue::FRONTIER_WORD_SCAN_BLOCK_LANES;
use vyre_primitives::graph::csr_queue_split::{
    csr_queue_split_low_dispatch_grid, csr_queue_split_mixed_logical_lanes,
    CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD,
};
use vyre_primitives::graph::csr_queue_strided::{
    csr_queue_strided_forward_dispatch_grid, CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE,
};

const U32_BYTES: usize = std::mem::size_of::<u32>();

/// Packed-frontier width where resident sparse CSR switches from node scanning
/// to deterministic word-prefix queue materialization.
pub(crate) const WORD_PREFIX_MIN_FRONTIER_WORDS: usize = 256;

/// Active-source queue bucket where deterministic word-prefix queue
/// materialization starts paying for its extra scan/scatter launches.
pub(crate) const WORD_PREFIX_MIN_QUEUE_CAPACITY: u32 = 512;

/// Nonzero packed frontier words needed before word-level atomic reservations
/// are expected to be more expensive than deterministic prefix scatter.
pub(crate) const WORD_PREFIX_MIN_NONZERO_WORDS: usize = 64;

/// Smallest resident multi-source queue Program shape for low-degree sparse
/// traversal. Single-source frontiers stay exact; multi-source frontiers avoid
/// compiling short-lived 2/4/8/16/32-slot Programs on streaming workloads.
pub(crate) const RESIDENT_CSR_QUEUE_MULTI_SOURCE_MIN_CAPACITY: u32 = 64;

/// Maximum word-prefix scan blocks whose offsets are summed inside the scatter
/// pass instead of paying a separate block-offset scan launch.
pub(crate) const WORD_PREFIX_INLINE_BLOCK_OFFSET_MAX_BLOCKS: u32 = 8;

/// CSR rows at or above this degree use the row-strided queue consumer.
pub(crate) const STRIDED_FORWARD_MIN_ROW_DEGREE: u32 = CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD;

/// Mixed split traversal must save at least this much logical lane work versus
/// striding every queued source before paying the extra low-row dispatch.
pub(crate) const MIXED_SPLIT_MAX_STRIDED_LANE_BPS: u64 = 7_500;

/// Queue materializer selected for a resident CSR frontier query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResidentCsrQueueMaterializer {
    /// Packed frontier words are scanned directly and active bits are appended atomically.
    AtomicWordScan,
    /// Packed words are popcount-scanned, then scattered into queue order.
    DeterministicWordPrefix,
}

/// Queue consumer selected for resident CSR traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResidentCsrQueueTraverseKind {
    /// One lane consumes an entire queued source row.
    RowSerial,
    /// A fixed lane team consumes each queued source row.
    RowStrided,
    /// Low-degree rows stay scalar; high-degree rows compact into a bounded
    /// high queue consumed by the row-strided primitive.
    MixedSplit { high_queue_capacity: u32 },
}

/// Scratch dimensions for deterministic word-prefix queue materialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrontierWordPrefixScratch {
    pub(crate) block_count: u32,
    pub(crate) partial_words: usize,
    pub(crate) block_total_words: usize,
}

/// Host-visible statistics for selecting resident CSR queue shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResidentCsrQueueFrontierStats {
    pub(crate) effective_queue_capacity: u32,
    pub(crate) max_active_nodes: u32,
    pub(crate) max_nonzero_words: usize,
}

pub(crate) fn resident_csr_queue_materializer(
    frontier_words: usize,
) -> ResidentCsrQueueMaterializer {
    if frontier_words >= WORD_PREFIX_MIN_FRONTIER_WORDS {
        ResidentCsrQueueMaterializer::DeterministicWordPrefix
    } else {
        ResidentCsrQueueMaterializer::AtomicWordScan
    }
}

pub(crate) const fn resident_csr_queue_materializer_for_stats(
    frontier_words: usize,
    queue_capacity: u32,
    max_nonzero_words: usize,
) -> ResidentCsrQueueMaterializer {
    if frontier_words >= WORD_PREFIX_MIN_FRONTIER_WORDS
        && queue_capacity >= WORD_PREFIX_MIN_QUEUE_CAPACITY
        && max_nonzero_words >= WORD_PREFIX_MIN_NONZERO_WORDS
    {
        ResidentCsrQueueMaterializer::DeterministicWordPrefix
    } else {
        ResidentCsrQueueMaterializer::AtomicWordScan
    }
}

#[cfg(test)]
pub(crate) const fn resident_csr_queue_traverse_kind(
    max_row_degree: u32,
) -> ResidentCsrQueueTraverseKind {
    if max_row_degree >= STRIDED_FORWARD_MIN_ROW_DEGREE {
        ResidentCsrQueueTraverseKind::RowStrided
    } else {
        ResidentCsrQueueTraverseKind::RowSerial
    }
}

pub(crate) fn resident_csr_queue_traverse_kind_for_graph_stats(
    node_count: u32,
    max_row_degree: u32,
    high_degree_source_count: u32,
    queue_capacity: u32,
) -> ResidentCsrQueueTraverseKind {
    if max_row_degree < STRIDED_FORWARD_MIN_ROW_DEGREE || queue_capacity <= 1 || node_count == 0 {
        return ResidentCsrQueueTraverseKind::RowSerial;
    }

    let high_queue_capacity =
        resident_csr_queue_high_degree_capacity_bound(high_degree_source_count, queue_capacity);
    if high_queue_capacity == 0 {
        return ResidentCsrQueueTraverseKind::RowSerial;
    }

    let all_strided_lanes =
        u64::from(queue_capacity) * u64::from(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);
    let mixed_lanes = csr_queue_split_mixed_logical_lanes(queue_capacity, high_queue_capacity);
    if mixed_lanes.saturating_mul(10_000)
        <= all_strided_lanes.saturating_mul(MIXED_SPLIT_MAX_STRIDED_LANE_BPS)
    {
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        }
    } else {
        ResidentCsrQueueTraverseKind::RowStrided
    }
}

pub(crate) fn resident_csr_queue_high_degree_source_count(edge_offsets: &[u32]) -> u32 {
    edge_offsets.windows(2).fold(0_u32, |count, pair| {
        count.saturating_add(u32::from(
            pair[1].saturating_sub(pair[0]) >= STRIDED_FORWARD_MIN_ROW_DEGREE,
        ))
    })
}

pub(crate) const fn resident_csr_queue_high_degree_capacity_bound(
    high_degree_source_count: u32,
    queue_capacity: u32,
) -> u32 {
    if high_degree_source_count < queue_capacity {
        high_degree_source_count
    } else {
        queue_capacity
    }
}

pub(crate) const fn resident_csr_queue_traverse_grid(
    queue_capacity: u32,
    kind: ResidentCsrQueueTraverseKind,
) -> [u32; 3] {
    match kind {
        ResidentCsrQueueTraverseKind::RowSerial => {
            let blocks = queue_capacity.div_ceil(256);
            [if blocks == 0 { 1 } else { blocks }, 1, 1]
        }
        ResidentCsrQueueTraverseKind::RowStrided => {
            csr_queue_strided_forward_dispatch_grid(queue_capacity)
        }
        ResidentCsrQueueTraverseKind::MixedSplit {
            high_queue_capacity,
        } => csr_queue_strided_forward_dispatch_grid(high_queue_capacity),
    }
}

pub(crate) const fn resident_csr_queue_split_low_grid(queue_capacity: u32) -> [u32; 3] {
    csr_queue_split_low_dispatch_grid(queue_capacity)
}

#[cfg(test)]
pub(crate) fn resident_csr_queue_effective_capacity(
    node_count: u32,
    frontiers: &[&[u32]],
    requested_capacity: u32,
) -> Result<u32, String> {
    resident_csr_queue_frontier_stats(node_count, frontiers, requested_capacity)
        .map(|stats| stats.effective_queue_capacity)
}

pub(crate) fn resident_csr_queue_frontier_stats(
    node_count: u32,
    frontiers: &[&[u32]],
    requested_capacity: u32,
) -> Result<ResidentCsrQueueFrontierStats, String> {
    if node_count == 0 {
        return Err("Fix: resident CSR queue frontier stats require node_count > 0.".to_string());
    }
    if frontiers.is_empty() {
        return Err(
            "Fix: resident CSR queue frontier stats require at least one frontier.".to_string(),
        );
    }
    if requested_capacity == 0 {
        return Err(
            "Fix: resident CSR queue frontier stats require requested_capacity > 0.".to_string(),
        );
    }

    let expected_words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    let final_word_mask = frontier_tail_mask(node_count);
    let mut max_active = 0u32;
    let mut max_nonzero_words = 0usize;
    for (query_index, frontier) in frontiers.iter().enumerate() {
        let stats = frontier_query_stats(expected_words, final_word_mask, frontier, query_index)?;
        max_active = max_active.max(stats.active_nodes);
        max_nonzero_words = max_nonzero_words.max(stats.nonzero_words);
    }

    let capped_active = max_active.min(requested_capacity);
    let active_floor = capped_active.max(1);
    let bucketed_active = active_floor.checked_next_power_of_two().unwrap_or(u32::MAX);
    Ok(ResidentCsrQueueFrontierStats {
        effective_queue_capacity: requested_capacity.min(bucketed_active).max(1),
        max_active_nodes: max_active,
        max_nonzero_words,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrontierQueryStats {
    active_nodes: u32,
    nonzero_words: usize,
}

fn frontier_query_stats(
    expected_words: usize,
    final_word_mask: u32,
    frontier: &[u32],
    query_index: usize,
) -> Result<FrontierQueryStats, String> {
    if frontier.len() < expected_words {
        return Err(format!(
            "Fix: resident CSR queue query {query_index} frontier has {} word(s), expected at least {expected_words}.",
            frontier.len()
        ));
    }

    let mut active = 0u32;
    let mut nonzero_words = 0usize;
    for (word_index, &word) in frontier.iter().take(expected_words).enumerate() {
        let in_domain_word = if word_index + 1 == expected_words {
            word & final_word_mask
        } else {
            word
        };
        if in_domain_word != 0 {
            nonzero_words += 1;
        }
        active = active
            .checked_add(in_domain_word.count_ones())
            .ok_or_else(|| {
                format!(
                    "Fix: resident CSR queue query {query_index} frontier popcount overflowed u32 while sizing the active queue."
                )
            })?;
    }
    Ok(FrontierQueryStats {
        active_nodes: active,
        nonzero_words,
    })
}

#[cfg(test)]
fn capped_frontier_popcount(
    node_count: u32,
    final_word_mask: u32,
    frontier: &[u32],
    requested_capacity: u32,
    query_index: usize,
) -> Result<u32, String> {
    let expected_words = vyre_primitives::bitset::bitset_words(node_count) as usize;
    frontier_query_stats(expected_words, final_word_mask, frontier, query_index)
        .map(|stats| stats.active_nodes.min(requested_capacity))
}

pub(crate) fn frontier_word_prefix_scratch(
    frontier_words: usize,
) -> Result<FrontierWordPrefixScratch, String> {
    let lanes = FRONTIER_WORD_SCAN_BLOCK_LANES as usize;
    let padded = frontier_words.checked_add(lanes - 1).ok_or_else(|| {
        format!(
            "Fix: resident CSR queue frontier_words={frontier_words} overflows word-prefix block rounding."
        )
    })?;
    let block_total_words = (padded / lanes).max(1);
    let partial_words = block_total_words.checked_mul(lanes).ok_or_else(|| {
        format!(
            "Fix: resident CSR queue word-prefix scratch overflows partial word count for frontier_words={frontier_words}."
        )
    })?;
    let block_count = u32::try_from(block_total_words).map_err(|_| {
        format!(
            "Fix: resident CSR queue word-prefix block count {block_total_words} exceeds u32 launch space."
        )
    })?;
    Ok(FrontierWordPrefixScratch {
        block_count,
        partial_words,
        block_total_words,
    })
}

pub(crate) fn frontier_word_prefix_uses_precomputed_offsets(block_count: u32) -> bool {
    block_count > WORD_PREFIX_INLINE_BLOCK_OFFSET_MAX_BLOCKS
}

pub(crate) fn frontier_word_dispatch_grid(frontier_words: usize) -> Result<[u32; 3], String> {
    let words = u32::try_from(frontier_words).map_err(|_| {
        format!(
            "Fix: resident CSR queue frontier word count {frontier_words} exceeds u32 launch space."
        )
    })?;
    Ok([words.div_ceil(256).max(1), 1, 1])
}

pub(crate) fn resident_csr_queue_scratch_bytes_per_query(
    frontier_words: usize,
    queue_capacity: u32,
) -> Result<usize, String> {
    resident_csr_queue_scratch_bytes_per_query_for_materializer(
        frontier_words,
        queue_capacity,
        resident_csr_queue_materializer(frontier_words),
    )
}

pub(crate) fn resident_csr_queue_scratch_bytes_per_query_for_materializer(
    frontier_words: usize,
    queue_capacity: u32,
    materializer: ResidentCsrQueueMaterializer,
) -> Result<usize, String> {
    resident_csr_queue_scratch_bytes_per_query_for_materializer_and_traverse(
        frontier_words,
        queue_capacity,
        materializer,
        ResidentCsrQueueTraverseKind::RowSerial,
    )
}

pub(crate) fn resident_csr_queue_scratch_bytes_per_query_for_materializer_and_traverse(
    frontier_words: usize,
    queue_capacity: u32,
    materializer: ResidentCsrQueueMaterializer,
    traverse_kind: ResidentCsrQueueTraverseKind,
) -> Result<usize, String> {
    let frontier_bytes = words_to_bytes(frontier_words, "frontier")?;
    let queue_bytes = words_to_bytes(queue_capacity as usize, "active_queue")?;
    let mut bytes = frontier_bytes;
    bytes = checked_add(bytes, queue_bytes, "active_queue")?;
    bytes = checked_add(bytes, U32_BYTES, "queue_len")?;
    bytes = checked_add(bytes, frontier_bytes, "frontier_out")?;
    if materializer == ResidentCsrQueueMaterializer::DeterministicWordPrefix {
        let word_prefix = frontier_word_prefix_scratch(frontier_words)?;
        bytes = checked_add(
            bytes,
            words_to_bytes(word_prefix.partial_words, "word_partials")?,
            "word_partials",
        )?;
        bytes = checked_add(
            bytes,
            words_to_bytes(word_prefix.block_total_words, "block_totals")?,
            "block_totals",
        )?;
    }
    if let ResidentCsrQueueTraverseKind::MixedSplit {
        high_queue_capacity,
    } = traverse_kind
    {
        bytes = checked_add(
            bytes,
            words_to_bytes(high_queue_capacity as usize, "high_queue")?,
            "high_queue",
        )?;
        bytes = checked_add(bytes, U32_BYTES, "high_len")?;
    }
    Ok(bytes)
}

fn words_to_bytes(words: usize, label: &str) -> Result<usize, String> {
    words.checked_mul(U32_BYTES).ok_or_else(|| {
        format!("Fix: resident CSR queue {label} word count {words} overflows byte count.")
    })
}

fn checked_add(base: usize, extra: usize, label: &str) -> Result<usize, String> {
    base.checked_add(extra).ok_or_else(|| {
        format!("Fix: resident CSR queue scratch byte count overflowed while adding {label}.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materializer_switches_at_word_prefix_threshold() {
        assert_eq!(
            resident_csr_queue_materializer(WORD_PREFIX_MIN_FRONTIER_WORDS - 1),
            ResidentCsrQueueMaterializer::AtomicWordScan
        );
        assert_eq!(
            resident_csr_queue_materializer(WORD_PREFIX_MIN_FRONTIER_WORDS),
            ResidentCsrQueueMaterializer::DeterministicWordPrefix
        );
    }

    #[test]
    fn active_frontier_stats_select_word_prefix_only_when_it_can_pay_off() {
        assert_eq!(
            resident_csr_queue_materializer_for_stats(
                WORD_PREFIX_MIN_FRONTIER_WORDS,
                WORD_PREFIX_MIN_QUEUE_CAPACITY - 1,
                WORD_PREFIX_MIN_NONZERO_WORDS,
            ),
            ResidentCsrQueueMaterializer::AtomicWordScan
        );
        assert_eq!(
            resident_csr_queue_materializer_for_stats(
                WORD_PREFIX_MIN_FRONTIER_WORDS,
                WORD_PREFIX_MIN_QUEUE_CAPACITY,
                WORD_PREFIX_MIN_NONZERO_WORDS - 1,
            ),
            ResidentCsrQueueMaterializer::AtomicWordScan
        );
        assert_eq!(
            resident_csr_queue_materializer_for_stats(
                WORD_PREFIX_MIN_FRONTIER_WORDS,
                WORD_PREFIX_MIN_QUEUE_CAPACITY,
                WORD_PREFIX_MIN_NONZERO_WORDS,
            ),
            ResidentCsrQueueMaterializer::DeterministicWordPrefix
        );
    }

    #[test]
    fn generated_word_prefix_scratch_covers_threshold_and_block_edges() {
        for words in 0..4096usize {
            let scratch = frontier_word_prefix_scratch(words)
                .expect("Fix: generated word-prefix scratch should fit");
            assert!(scratch.block_count >= 1);
            assert!(scratch.partial_words >= FRONTIER_WORD_SCAN_BLOCK_LANES as usize);
            assert_eq!(
                scratch.partial_words,
                scratch.block_total_words * FRONTIER_WORD_SCAN_BLOCK_LANES as usize
            );
            assert!(
                scratch.partial_words >= words,
                "partial scratch must cover every packed frontier word"
            );
        }
    }

    #[test]
    fn small_word_prefix_block_counts_inline_offsets() {
        for block_count in 1..=WORD_PREFIX_INLINE_BLOCK_OFFSET_MAX_BLOCKS {
            assert!(
                !frontier_word_prefix_uses_precomputed_offsets(block_count),
                "block_count={block_count} should use in-scatter offsets"
            );
        }
        assert!(frontier_word_prefix_uses_precomputed_offsets(
            WORD_PREFIX_INLINE_BLOCK_OFFSET_MAX_BLOCKS + 1
        ));
    }

    #[test]
    fn high_degree_rows_select_strided_queue_consumer() {
        assert_eq!(
            resident_csr_queue_traverse_kind(STRIDED_FORWARD_MIN_ROW_DEGREE - 1),
            ResidentCsrQueueTraverseKind::RowSerial
        );
        assert_eq!(
            resident_csr_queue_traverse_kind(STRIDED_FORWARD_MIN_ROW_DEGREE),
            ResidentCsrQueueTraverseKind::RowStrided
        );
        assert_eq!(
            resident_csr_queue_traverse_grid(9, ResidentCsrQueueTraverseKind::RowSerial),
            [1, 1, 1]
        );
        assert_eq!(
            resident_csr_queue_traverse_grid(9, ResidentCsrQueueTraverseKind::RowStrided),
            [2, 1, 1]
        );
    }

    #[test]
    fn skewed_high_degree_graphs_select_mixed_split_when_lane_savings_are_material() {
        let queue_capacity = 128;
        assert_eq!(
            resident_csr_queue_high_degree_capacity_bound(1, queue_capacity),
            1
        );
        assert_eq!(
            resident_csr_queue_traverse_kind_for_graph_stats(
                4096,
                STRIDED_FORWARD_MIN_ROW_DEGREE,
                1,
                queue_capacity,
            ),
            ResidentCsrQueueTraverseKind::MixedSplit {
                high_queue_capacity: 1
            }
        );
        assert_eq!(resident_csr_queue_split_low_grid(queue_capacity), [1, 1, 1]);
        assert_eq!(
            resident_csr_queue_traverse_grid(
                queue_capacity,
                ResidentCsrQueueTraverseKind::MixedSplit {
                    high_queue_capacity: 1
                },
            ),
            csr_queue_strided_forward_dispatch_grid(1)
        );
    }

    #[test]
    fn uniformly_high_degree_graphs_keep_global_strided_consumer() {
        let queue_capacity = 128;
        assert_eq!(
            resident_csr_queue_high_degree_capacity_bound(queue_capacity, queue_capacity),
            queue_capacity
        );
        assert_eq!(
            resident_csr_queue_traverse_kind_for_graph_stats(
                4096,
                STRIDED_FORWARD_MIN_ROW_DEGREE,
                queue_capacity,
                queue_capacity,
            ),
            ResidentCsrQueueTraverseKind::RowStrided
        );
    }

    #[test]
    fn generated_exact_high_degree_counts_avoid_superhub_overallocation() {
        for seed in 0..10_000u32 {
            let queue_capacity = 2 + (mix32(seed) % 2_047);
            let high_degree_sources = 1 + (mix32(seed ^ 0x9e37_79b9) % queue_capacity);
            let extra_edges_per_high_row = mix32(seed ^ 0xa5a5_51ce) % 257;
            let edge_count = high_degree_sources.saturating_mul(
                STRIDED_FORWARD_MIN_ROW_DEGREE.saturating_add(extra_edges_per_high_row),
            );
            let edge_bound_capacity = resident_csr_queue_high_degree_capacity_bound(
                edge_count / STRIDED_FORWARD_MIN_ROW_DEGREE,
                queue_capacity,
            );
            let exact_capacity =
                resident_csr_queue_high_degree_capacity_bound(high_degree_sources, queue_capacity);
            let exact_kind = resident_csr_queue_traverse_kind_for_graph_stats(
                4096,
                STRIDED_FORWARD_MIN_ROW_DEGREE + extra_edges_per_high_row,
                high_degree_sources,
                queue_capacity,
            );

            assert!(
                exact_capacity <= edge_bound_capacity,
                "exact high-row metadata must never allocate more high_queue slots than the edge-count bound"
            );
            assert_eq!(exact_capacity, high_degree_sources.min(queue_capacity));
            if let ResidentCsrQueueTraverseKind::MixedSplit {
                high_queue_capacity,
            } = exact_kind
            {
                assert_eq!(
                    high_queue_capacity, exact_capacity,
                    "mixed split must carry the exact bounded high-row count"
                );
            }
        }
    }

    #[test]
    fn effective_queue_capacity_buckets_active_frontiers_and_ignores_tail_bits() {
        let first = [0b11_u32, u32::MAX & !0b111_u32];
        let second = [0_u32, 0b101_u32];
        let frontiers: [&[u32]; 2] = [&first, &second];

        let stats = resident_csr_queue_frontier_stats(35, &frontiers, 1_024)
            .expect("Fix: valid resident CSR queue frontiers should produce stats");
        assert_eq!(stats.max_active_nodes, 2);
        assert_eq!(stats.max_nonzero_words, 1);
        assert_eq!(
            resident_csr_queue_effective_capacity(35, &frontiers, 1_024)
                .expect("Fix: valid resident CSR queue frontiers should size"),
            2,
            "tail bits outside node_count must not inflate resident queue capacity"
        );
        let mut single = vec![0u32; vyre_primitives::bitset::bitset_words(1_000) as usize];
        single[0] = 1;
        assert_eq!(
            resident_csr_queue_effective_capacity(1_000, &[&single], 1_024)
                .expect("Fix: single active source should size"),
            1
        );
        let dense = [u32::MAX; 9];
        assert_eq!(
            resident_csr_queue_effective_capacity(288, &[&dense], 257)
                .expect("Fix: requested capacity remains a hard traversal cap"),
            257
        );
    }

    #[test]
    fn effective_queue_capacity_caps_dense_frontiers_to_requested_capacity() {
        let node_count = 1_000_000_u32;
        let frontier = vec![u32::MAX; vyre_primitives::bitset::bitset_words(node_count) as usize];

        assert_eq!(
            resident_csr_queue_effective_capacity(node_count, &[&frontier], 17)
                .expect("Fix: dense resident CSR frontier should size to requested cap"),
            17
        );

        let mut overpadded = vec![0u32; vyre_primitives::bitset::bitset_words(33) as usize + 128];
        overpadded[0] = 1;
        overpadded[2..].fill(u32::MAX);
        assert_eq!(
            resident_csr_queue_effective_capacity(33, &[&overpadded], 1_024)
                .expect("Fix: resident CSR frontier sizing should ignore out-of-domain padding"),
            1,
            "out-of-domain padding must not inflate resident queue capacity"
        );
    }

    #[test]
    fn capped_frontier_popcount_masks_tail_and_saturates_at_capacity() {
        let node_count = 65_u32;
        let final_word_mask = frontier_tail_mask(node_count);
        let frontier = [u32::MAX, u32::MAX, u32::MAX];

        assert_eq!(
            capped_frontier_popcount(node_count, final_word_mask, &frontier, 40, 0)
                .expect("Fix: capped popcount should stop at requested capacity"),
            40
        );
        assert_eq!(
            capped_frontier_popcount(node_count, final_word_mask, &frontier, 100, 0)
                .expect("Fix: capped popcount should mask bits past node_count"),
            65
        );
    }

    #[test]
    fn generated_effective_queue_capacity_bounds_overlaunch() {
        for seed in 0..10_000u32 {
            let node_count = 1 + (mix32(seed) % 4_096);
            let words = vyre_primitives::bitset::bitset_words(node_count) as usize;
            let mut first = vec![0u32; words];
            let mut second = vec![0u32; words];
            for word_index in 0..words {
                first[word_index] = mix32(seed ^ word_index as u32);
                second[word_index] = mix32(seed.rotate_left(7) ^ word_index as u32);
            }
            let frontiers: [&[u32]; 2] = [&first, &second];
            let requested_capacity = 1 + (mix32(seed ^ 0x7a5a_51ce_u32) % 8_192);
            let effective =
                resident_csr_queue_effective_capacity(node_count, &frontiers, requested_capacity)
                    .expect("Fix: generated resident CSR queue frontiers should size");
            let max_active = frontiers
                .iter()
                .map(|frontier| in_domain_popcount(node_count, frontier))
                .max()
                .unwrap_or(0);

            assert!(effective >= 1);
            assert!(effective <= requested_capacity);
            if max_active == 0 {
                assert_eq!(effective, 1);
            } else if max_active > requested_capacity {
                assert_eq!(effective, requested_capacity);
            } else {
                assert!(effective >= max_active);
                assert!(
                    effective <= max_active.next_power_of_two(),
                    "capacity should only round active_count={max_active} to its bucket, got {effective}"
                );
                if max_active <= requested_capacity / 2 {
                    assert!(
                        effective <= max_active * 2,
                        "uncapped sparse frontier should not overlaunch by more than one bucket: active={max_active} effective={effective}"
                    );
                }
            }
        }
    }

    fn in_domain_popcount(node_count: u32, frontier: &[u32]) -> u32 {
        let final_word_mask = frontier_tail_mask(node_count);
        frontier
            .iter()
            .enumerate()
            .map(|(index, &word)| {
                if index + 1 == frontier.len() {
                    word & final_word_mask
                } else {
                    word
                }
                .count_ones()
            })
            .sum()
    }

    fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7feb_352d);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846c_a68b);
        value ^ (value >> 16)
    }
}
