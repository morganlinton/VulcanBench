use super::layout::CsrForwardOrChangedLayout;

/// Validate and copy a seed frontier into caller-owned frontier storage.
///
/// The reservation happens before mutation, so allocator failure cannot clobber
/// a reusable frontier buffer.
///
/// # Errors
///
/// Returns the caller's error type for bad seed width or reservation failure.
pub fn copy_csr_forward_seed_frontier_into<E>(
    seed: &[u32],
    frontier_words: usize,
    frontier: &mut Vec<u32>,
    reserve: impl FnOnce(&mut Vec<u32>, usize, &'static str) -> Result<(), E>,
    map_bad_input: impl FnOnce(String) -> E,
) -> Result<(), E> {
    if seed.len() != frontier_words {
        return Err(map_bad_input(format!(
            "Fix: csr_forward_or_changed expected seed frontier length {frontier_words} word(s), got {}. Pass a bitset sized by the primitive launch plan.",
            seed.len()
        )));
    }
    reserve(
        frontier,
        frontier_words,
        "csr_forward_or_changed frontier seed",
    )?;
    frontier.clear();
    frontier.extend_from_slice(seed);
    Ok(())
}

/// Validate that a changed readback word is the primitive's 0/1 flag.
///
/// # Errors
///
/// Returns an actionable diagnostic when a backend writes a non-boolean flag.
pub fn validate_csr_forward_or_changed_flag(changed: u32) -> Result<(), String> {
    if changed <= 1 {
        return Ok(());
    }
    Err(format!(
        "Fix: csr_forward_or_changed backend returned non-boolean changed flag {changed}; expected 0 or 1."
    ))
}

/// Validate the CSR inputs used by the forward-or-changed primitive.
///
/// # Errors
///
/// Returns an actionable diagnostic when offsets are missing, non-monotonic,
/// inconsistent with edge arrays, or when targets/kind masks have mismatched
/// lengths.
pub(crate) fn validate_csr_inputs(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<CsrForwardOrChangedLayout, String> {
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!(
            "Fix: csr_forward_or_changed node_count + 1 overflows usize for node_count={node_count}."
        )
    })?;
    let frontier_words = (crate::bitset::bitset_words(node_count) as usize).max(1);
    if edge_offsets.is_empty() {
        if edge_targets.is_empty() && edge_kind_mask.is_empty() {
            return Ok(CsrForwardOrChangedLayout {
                node_count,
                node_words: (node_count as usize).max(1),
                edge_offset_words: expected_offsets,
                edge_storage_words: 1,
                shape_edge_count: 0,
                frontier_words,
            });
        }
        return Err(format!(
            "Fix: csr_forward_or_changed empty edge_offsets may only encode an empty edge set, got targets_len={} kind_mask_len={}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    if edge_offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: csr_forward_or_changed requires edge_offsets.len() == node_count + 1, got len={}, node_count={node_count}.",
            edge_offsets.len()
        ));
    }
    if edge_targets.len() != edge_kind_mask.len() {
        return Err(format!(
            "Fix: csr_forward_or_changed requires edge_targets.len() == edge_kind_mask.len(), got {} vs {}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    let shape_edge_count = u32::try_from(edge_kind_mask.len()).map_err(|_| {
        format!(
            "Fix: csr_forward_or_changed edge count {} exceeds u32 index space.",
            edge_kind_mask.len()
        )
    })?;
    for (index, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(format!(
                "Fix: csr_forward_or_changed offsets must be monotonic; offsets[{index}]={} > offsets[{}]={}.",
                pair[0],
                index + 1,
                pair[1]
            ));
        }
    }
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    if edge_targets.len() < edge_count {
        return Err(format!(
            "Fix: csr_forward_or_changed final offset declares edge_count={edge_count}, but targets_len={} and kind_mask_len={}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    Ok(CsrForwardOrChangedLayout {
        node_count,
        node_words: (node_count as usize).max(1),
        edge_offset_words: expected_offsets,
        edge_storage_words: edge_kind_mask.len().max(1),
        shape_edge_count,
        frontier_words,
    })
}
