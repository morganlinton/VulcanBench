use super::layout::{PersistentBfsBatchLayout, PersistentBfsFrontierLayout, PersistentBfsLayout};
use super::program::bitset_words;

/// Validate a persistent-BFS CSR graph layout.
///
/// # Errors
///
/// Returns an actionable diagnostic when offsets are malformed, masks and
/// targets diverge, the edge count exceeds u32 indexing, or an edge target is
/// outside `0..node_count`.
pub fn validate_persistent_bfs_graph_layout(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<PersistentBfsLayout, String> {
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!("Fix: persistent_bfs node_count + 1 overflows usize for node_count={node_count}.")
    })?;
    if edge_offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: persistent_bfs expected {expected_offsets} CSR offsets for {node_count} nodes, got {}.",
            edge_offsets.len()
        ));
    }
    if edge_targets.len() != edge_kind_mask.len() {
        return Err(format!(
            "Fix: persistent_bfs requires edge_targets.len() == edge_kind_mask.len(), got {} vs {}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    let edge_count = u32::try_from(edge_targets.len()).map_err(|_| {
        format!(
            "Fix: persistent_bfs edge count {} exceeds u32 index space.",
            edge_targets.len()
        )
    })?;
    let final_offset = edge_offsets[expected_offsets - 1] as usize;
    if final_offset != edge_targets.len() {
        return Err(format!(
            "Fix: persistent_bfs final CSR offset {final_offset} must equal edge_count {}.",
            edge_targets.len()
        ));
    }
    for (row, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(format!(
                "Fix: persistent_bfs CSR offsets are non-monotonic at row {row}: {} > {}.",
                pair[0], pair[1]
            ));
        }
    }
    for (idx, &target) in edge_targets.iter().enumerate() {
        if target >= node_count {
            return Err(format!(
                "Fix: persistent_bfs CSR target[{idx}]={target} is outside node_count {node_count}."
            ));
        }
    }
    let words_u32 = bitset_words(node_count);
    Ok(PersistentBfsLayout {
        node_count,
        edge_count,
        words: words_u32 as usize,
        words_u32,
        node_words: node_count as usize,
        edge_storage_words: edge_targets.len().max(1),
    })
}

/// Validate the full non-resident persistent-BFS dispatch/input contract.
///
/// # Errors
///
/// Returns an actionable diagnostic when the graph layout is malformed or the
/// seed frontier length does not match the graph.
pub fn validate_persistent_bfs_inputs(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_in: &[u32],
) -> Result<PersistentBfsLayout, String> {
    let layout = validate_persistent_bfs_graph_layout(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    )?;
    if frontier_in.len() != layout.words {
        return Err(format!(
            "Fix: persistent_bfs expected frontier length {} words for {node_count} nodes, got {}.",
            layout.words,
            frontier_in.len()
        ));
    }
    Ok(layout)
}

/// Validate flat-frontier batch shape for persistent BFS.
///
/// The frontier buffer is laid out as `[query][word]`, where
/// `words_per_query` is derived from the already-validated graph layout.
///
/// # Errors
///
/// Returns an actionable diagnostic when query count cannot be represented by
/// GPU grid dimensions, the flat word count overflows, or the supplied
/// frontier buffer length does not match `words_per_query * query_count`.
pub fn validate_persistent_bfs_batch_frontiers(
    words_per_query: usize,
    frontier_inputs: &[u32],
    query_count: usize,
) -> Result<PersistentBfsBatchLayout, String> {
    let query_count_u32 = u32::try_from(query_count).map_err(|_| {
        format!(
            "Fix: persistent_bfs_batch query_count {query_count} exceeds u32::MAX; shard the BFS query batch before GPU dispatch."
        )
    })?;
    let total_words = words_per_query.checked_mul(query_count).ok_or_else(|| {
        format!(
            "Fix: persistent_bfs_batch word count overflows usize for {words_per_query} words/query and {query_count} queries; shard the BFS query batch before GPU dispatch."
        )
    })?;
    if frontier_inputs.len() != total_words {
        return Err(format!(
            "Fix: persistent_bfs_batch expected {total_words} frontier word(s), got {}.",
            frontier_inputs.len()
        ));
    }
    Ok(PersistentBfsBatchLayout {
        query_count: query_count_u32,
        total_words,
    })
}

/// Validate a single persistent-BFS frontier against an already-validated graph layout.
///
/// # Errors
///
/// Returns an actionable diagnostic when the graph frontier width cannot be
/// represented by primitive metadata, or when the supplied frontier length
/// does not match the graph frontier width.
pub fn validate_persistent_bfs_frontier(
    words_per_query: usize,
    frontier_in: &[u32],
) -> Result<PersistentBfsFrontierLayout, String> {
    let words_u32 = u32::try_from(words_per_query).map_err(|_| {
        format!(
            "Fix: persistent_bfs frontier word count {words_per_query} exceeds u32::MAX; shard the graph before GPU dispatch."
        )
    })?;
    if frontier_in.len() != words_per_query {
        return Err(format!(
            "Fix: persistent_bfs expected frontier length {words_per_query} word(s), got {}.",
            frontier_in.len()
        ));
    }
    Ok(PersistentBfsFrontierLayout {
        words: words_per_query,
        words_u32,
    })
}
