//! Shared graph-frontier bitset utilities for dispatch wrappers.
//!
//! Graph wrapper modules use these helpers for host-side wave bookkeeping only.
//! Primitive traversal semantics stay in `vyre-primitives`.

#[cfg(test)]
use crate::optimizer::dispatcher::DispatchError;
#[cfg(test)]
use vyre_primitives::bitset::bitset_words;
#[cfg(test)]
use vyre_primitives::bitset::frontier as primitive_frontier;

#[cfg(test)]
pub(crate) use primitive_frontier::{frontier_tail_mask, mask_frontier_tail_bits};

/// Count active frontier bits through the primitive frontier authority.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when the primitive count overflows the
/// compact u32 frontier count representation.
#[cfg(test)]
pub(crate) fn frontier_popcount(frontier: &[u32]) -> Result<u32, DispatchError> {
    primitive_frontier::checked_frontier_popcount(frontier).map_err(|err| {
        DispatchError::BadInputs(format!(
            "Fix: graph frontier primitive popcount rejected input: {err}"
        ))
    })
}

/// Merge a neighbor frontier into a visited set and materialize only new bits.
///
/// `visited`, `neighbors`, and `next_wave` use the canonical packed bitset
/// shape from `bitset_words(node_count)`. Bits outside `node_count` are ignored
/// in the final word.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when the input slices are not shaped
/// for `node_count`.
#[cfg(test)]
pub(crate) fn absorb_new_frontier_bits(
    node_count: u32,
    visited: &mut [u32],
    neighbors: &[u32],
    next_wave: &mut Vec<u32>,
) -> Result<bool, DispatchError> {
    primitive_frontier::absorb_new_frontier_bits(node_count, visited, neighbors, next_wave)
        .map(|summary| summary.added_any)
        .map_err(|err| {
            DispatchError::BadInputs(format!(
                "Fix: graph frontier closure primitive absorption rejected input for {node_count} nodes: {err}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_mask_matches_node_domain_width() {
        assert_eq!(frontier_tail_mask(0), u32::MAX);
        assert_eq!(frontier_tail_mask(1), 0b1);
        assert_eq!(frontier_tail_mask(31), 0x7FFF_FFFF);
        assert_eq!(frontier_tail_mask(32), u32::MAX);
        assert_eq!(frontier_tail_mask(35), 0b111);
    }

    #[test]
    fn popcount_delegates_to_primitive_authority() {
        assert_eq!(
            frontier_popcount(&[0b1011, 0x8000_0000])
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - fixed frontier bitset is valid"),
            4
        );
    }

    #[test]
    fn generated_absorb_new_frontier_bits_matches_scalar_reference() {
        let patterns = [
            0,
            u32::MAX,
            0x5555_5555,
            0xAAAA_AAAA,
            0x8000_0001,
            0x7FFF_FFFE,
            0x1357_9BDF,
            0x2468_ACE0,
        ];

        for node_count in 0..=512 {
            let words = bitset_words(node_count) as usize;
            for (case_index, pattern) in patterns.into_iter().enumerate() {
                let mut visited = (0..words)
                    .map(|word_index| {
                        pattern.rotate_left((word_index as u32 + case_index as u32) % u32::BITS)
                    })
                    .collect::<Vec<_>>();
                let neighbors = (0..words)
                    .map(|word_index| {
                        (!pattern).rotate_right((word_index as u32).wrapping_mul(5) % u32::BITS)
                    })
                    .collect::<Vec<_>>();
                mask_frontier_tail_bits(node_count, &mut visited);
                let before = visited.clone();
                let mut next_wave = Vec::new();

                let had_new_nodes =
                    absorb_new_frontier_bits(node_count, &mut visited, &neighbors, &mut next_wave)
                        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated frontier shapes are valid");

                let tail_index = words.saturating_sub(1);
                let tail_mask = frontier_tail_mask(node_count);
                let expected_next = before
                    .iter()
                    .zip(neighbors.iter())
                    .enumerate()
                    .map(|(word_index, (&visited_word, &neighbor_word))| {
                        let in_domain_neighbors = if word_index == tail_index {
                            neighbor_word & tail_mask
                        } else {
                            neighbor_word
                        };
                        in_domain_neighbors & !visited_word
                    })
                    .collect::<Vec<_>>();
                let expected_visited = before
                    .iter()
                    .zip(expected_next.iter())
                    .map(|(&visited_word, &new_bits)| visited_word | new_bits)
                    .collect::<Vec<_>>();

                assert_eq!(
                    next_wave, expected_next,
                    "node_count={node_count} case_index={case_index}"
                );
                assert_eq!(
                    visited, expected_visited,
                    "node_count={node_count} case_index={case_index}"
                );
                assert_eq!(
                    had_new_nodes,
                    expected_next.iter().any(|&word| word != 0),
                    "node_count={node_count} case_index={case_index}"
                );
            }
        }
    }

    #[test]
    fn generated_absorb_new_frontier_bits_rejects_shape_mismatches() {
        for node_count in 0..=256 {
            let words = bitset_words(node_count) as usize;
            let mut next_wave = Vec::new();

            let mut long_visited = vec![0; words + 1];
            let neighbors = vec![0; words];
            assert!(matches!(
                absorb_new_frontier_bits(node_count, &mut long_visited, &neighbors, &mut next_wave,),
                Err(_)
            ));

            let mut visited = vec![0; words];
            let long_neighbors = vec![0; words + 1];
            assert!(matches!(
                absorb_new_frontier_bits(node_count, &mut visited, &long_neighbors, &mut next_wave,),
                Err(_)
            ));
        }
    }
}
