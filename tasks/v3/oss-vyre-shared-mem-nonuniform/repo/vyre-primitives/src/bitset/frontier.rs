//! Packed-frontier bitset utilities and fused frontier absorption.

use core::fmt;
use std::sync::Arc;

use crate::bitset::bitset_words;
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

/// Canonical op id for fused frontier absorption.
pub const ABSORB_NEW_BITS_OP_ID: &str = "vyre-primitives::bitset::frontier_absorb_new_bits";
/// Canonical op id for fused frontier absorption when the caller does not need
/// per-word popcounts.
pub const ABSORB_NEW_BITS_NO_COUNTS_OP_ID: &str =
    "vyre-primitives::bitset::frontier_absorb_new_bits_no_counts";

/// Error returned by packed-frontier helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FrontierError {
    /// A frontier slice length does not match `bitset_words(node_count)`.
    BadShape {
        /// Human-readable slice role.
        name: &'static str,
        /// Declared graph/node domain width.
        node_count: u32,
        /// Expected number of u32 words.
        expected_words: usize,
        /// Actual number of u32 words.
        actual_words: usize,
    },
    /// A popcount exceeded the compact u32 count representation.
    PopcountOverflow {
        /// Number of frontier words counted before overflow.
        frontier_words: usize,
    },
    /// Caller-owned output frontier could not reserve enough storage.
    Allocation {
        /// Human-readable output buffer role.
        name: &'static str,
        /// Requested u32 words.
        requested_words: usize,
        /// Allocator detail.
        source: String,
    },
    /// A compact frontier queue cannot hold all active in-domain bits.
    QueueCapacity {
        /// Declared graph/node domain width.
        node_count: u32,
        /// Queue slots available to the caller.
        capacity: usize,
        /// In-domain active bits that must be materialized.
        required: u32,
    },
    /// Caller-provided active-bit count disagrees with the frontier bits.
    QueueCountMismatch {
        /// Declared graph/node domain width.
        node_count: u32,
        /// Active in-domain bits promised by the caller.
        expected: u32,
        /// Active in-domain bits observed before mismatch was proven.
        observed: u32,
    },
}

impl fmt::Display for FrontierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadShape {
                name,
                node_count,
                expected_words,
                actual_words,
            } => write!(
                f,
                "{name} frontier for {node_count} nodes requires {expected_words} u32 words, got {actual_words}."
            ),
            Self::PopcountOverflow { frontier_words } => write!(
                f,
                "frontier popcount exceeds u32::MAX for {frontier_words} frontier words."
            ),
            Self::Allocation {
                name,
                requested_words,
                source,
            } => write!(
                f,
                "{name} frontier could not reserve {requested_words} u32 words: {source}."
            ),
            Self::QueueCapacity {
                node_count,
                capacity,
                required,
            } => write!(
                f,
                "frontier queue for {node_count} nodes requires {required} slots, got capacity {capacity}."
            ),
            Self::QueueCountMismatch {
                node_count,
                expected,
                observed,
            } => write!(
                f,
                "frontier queue for {node_count} nodes expected {expected} active bits, observed {observed}."
            ),
        }
    }
}

impl std::error::Error for FrontierError {}

/// Summary from absorbing a neighbor frontier into a visited frontier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrontierAbsorbSummary {
    /// True when at least one previously-unvisited node was added.
    pub added_any: bool,
    /// Number of newly-added in-domain nodes.
    pub added_popcount: u32,
}

/// Number of u32 words expected for a frontier over `node_count` nodes.
#[must_use]
pub const fn frontier_words(node_count: u32) -> usize {
    bitset_words(node_count) as usize
}

/// Mask for valid bits in the final frontier word.
#[must_use]
pub const fn frontier_tail_mask(node_count: u32) -> u32 {
    let tail_bits = node_count % u32::BITS;
    if tail_bits == 0 {
        u32::MAX
    } else {
        (1u32 << tail_bits) - 1
    }
}

/// Build a fused GPU program for one frontier-closure absorption step.
#[must_use]
pub fn frontier_absorb_new_bits_program(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    added_counts: &str,
    words: u32,
    final_word_mask: u32,
) -> Program {
    let body = frontier_absorb_new_bits_body_prefixed(
        visited,
        neighbors,
        next_wave,
        Some(added_counts),
        words,
        final_word_mask,
        "frontier_absorb",
    );

    Program::wrapped(
        vec![
            BufferDecl::storage(visited, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(neighbors, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(next_wave, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(added_counts, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(ABSORB_NEW_BITS_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build one frontier-absorption body.
///
/// `next_wave[w] = (neighbors[w] & domain_mask) & !visited[w]`
/// and `visited[w] |= next_wave[w]`. When `added_counts` is supplied, the body
/// also writes `popcount(next_wave[w])`.
#[must_use]
pub(crate) fn frontier_absorb_new_bits_body_prefixed(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    added_counts: Option<&str>,
    words: u32,
    final_word_mask: u32,
    local_prefix: &str,
) -> Vec<Node> {
    frontier_absorb_new_bits_body_prefixed_with_flag(
        visited,
        neighbors,
        next_wave,
        added_counts,
        None,
        words,
        final_word_mask,
        local_prefix,
    )
}

/// Build one frontier-absorption body and optionally set a scalar flag when
/// any new bit is added.
#[must_use]
pub(crate) fn frontier_absorb_new_bits_body_prefixed_with_flag(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    added_counts: Option<&str>,
    added_any_flag: Option<(&str, Expr)>,
    words: u32,
    final_word_mask: u32,
    local_prefix: &str,
) -> Vec<Node> {
    let local = |name: &str| -> String {
        if local_prefix.is_empty() {
            name.to_string()
        } else {
            format!("{local_prefix}_{name}")
        }
    };
    let t = Expr::InvocationId { axis: 0 };
    let final_word = words.saturating_sub(1);
    let old_visited = local("old_visited");
    let neighbor_word = local("neighbors");
    let domain_mask = local("domain_mask");
    let in_domain_neighbors = local("in_domain_neighbors");
    let new_bits = local("new_bits");
    let changed_old = local("changed_old");

    let mut body = vec![
        Node::let_bind(old_visited.as_str(), Expr::load(visited, t.clone())),
        Node::let_bind(neighbor_word.as_str(), Expr::load(neighbors, t.clone())),
        Node::let_bind(
            domain_mask.as_str(),
            Expr::select(
                Expr::eq(t.clone(), Expr::u32(final_word)),
                Expr::u32(final_word_mask),
                Expr::u32(u32::MAX),
            ),
        ),
        Node::let_bind(
            in_domain_neighbors.as_str(),
            Expr::bitand(
                Expr::var(neighbor_word.as_str()),
                Expr::var(domain_mask.as_str()),
            ),
        ),
        Node::let_bind(
            new_bits.as_str(),
            Expr::bitand(
                Expr::var(in_domain_neighbors.as_str()),
                Expr::bitnot(Expr::var(old_visited.as_str())),
            ),
        ),
        Node::store(next_wave, t.clone(), Expr::var(new_bits.as_str())),
        Node::store(
            visited,
            t.clone(),
            Expr::bitor(
                Expr::var(old_visited.as_str()),
                Expr::var(new_bits.as_str()),
            ),
        ),
    ];
    if let Some(added_counts) = added_counts {
        body.push(Node::store(
            added_counts,
            t.clone(),
            Expr::UnOp {
                op: UnOp::Popcount,
                operand: Box::new(Expr::var(new_bits.as_str())),
            },
        ));
    }
    if let Some((flag_buffer, flag_index)) = added_any_flag {
        body.push(Node::if_then(
            Expr::ne(Expr::var(new_bits.as_str()), Expr::u32(0)),
            vec![Node::let_bind(
                changed_old.as_str(),
                Expr::atomic_or(flag_buffer, flag_index, Expr::u32(1)),
            )],
        ));
    }

    vec![Node::if_then(Expr::lt(t, Expr::u32(words)), body)]
}

/// Build a fused GPU program for one frontier-closure absorption step without
/// per-word popcount output.
#[must_use]
pub fn frontier_absorb_new_bits_no_counts_program(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    words: u32,
    final_word_mask: u32,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(visited, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(neighbors, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(next_wave, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(ABSORB_NEW_BITS_NO_COUNTS_OP_ID),
            source_region: None,
            body: Arc::new(frontier_absorb_new_bits_body_prefixed(
                visited,
                neighbors,
                next_wave,
                None,
                words,
                final_word_mask,
                "frontier_absorb_no_counts",
            )),
        }],
    )
}

/// Build a fused frontier-absorption GPU program from the logical node count.
#[must_use]
pub fn frontier_absorb_new_bits_for_node_count_program(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    added_counts: &str,
    node_count: u32,
) -> Program {
    frontier_absorb_new_bits_program(
        visited,
        neighbors,
        next_wave,
        added_counts,
        bitset_words(node_count),
        frontier_tail_mask(node_count),
    )
}

/// Build a no-count frontier-absorption GPU program from the logical node
/// count.
#[must_use]
pub fn frontier_absorb_new_bits_no_counts_for_node_count_program(
    visited: &str,
    neighbors: &str,
    next_wave: &str,
    node_count: u32,
) -> Program {
    frontier_absorb_new_bits_no_counts_program(
        visited,
        neighbors,
        next_wave,
        bitset_words(node_count),
        frontier_tail_mask(node_count),
    )
}

/// Validate that `frontier` has the canonical packed shape for `node_count`.
pub fn validate_frontier_shape(
    node_count: u32,
    frontier: &[u32],
    name: &'static str,
) -> Result<usize, FrontierError> {
    let expected_words = frontier_words(node_count);
    if frontier.len() != expected_words {
        return Err(FrontierError::BadShape {
            name,
            node_count,
            expected_words,
            actual_words: frontier.len(),
        });
    }
    Ok(expected_words)
}

/// Count set bits in a packed u32 frontier with checked overflow reporting.
pub fn checked_frontier_popcount(frontier: &[u32]) -> Result<u32, FrontierError> {
    let mut popcount = 0u32;
    for &word in frontier {
        popcount =
            popcount
                .checked_add(word.count_ones())
                .ok_or(FrontierError::PopcountOverflow {
                    frontier_words: frontier.len(),
                })?;
    }
    Ok(popcount)
}

/// Count only in-domain set bits in a frontier with the canonical shape.
pub fn checked_frontier_domain_popcount(
    node_count: u32,
    frontier: &[u32],
) -> Result<u32, FrontierError> {
    let expected_words = validate_frontier_shape(node_count, frontier, "input")?;
    let final_word_index = expected_words.saturating_sub(1);
    let final_word_mask = frontier_tail_mask(node_count);
    let mut popcount = 0u32;
    for (word_index, &word) in frontier.iter().enumerate() {
        let in_domain_word = if word_index == final_word_index {
            word & final_word_mask
        } else {
            word
        };
        popcount = popcount.checked_add(in_domain_word.count_ones()).ok_or(
            FrontierError::PopcountOverflow {
                frontier_words: expected_words,
            },
        )?;
    }
    Ok(popcount)
}

/// Materialize active node ids from a packed frontier into queue order.
///
/// The output queue is exact-length, not padded to `queue_capacity`. Callers
/// that need fixed-size resident buffers can resize after the active prefix is
/// written.
pub fn materialize_frontier_queue_into(
    node_count: u32,
    frontier: &[u32],
    queue_capacity: usize,
    queue: &mut Vec<u32>,
) -> Result<u32, FrontierError> {
    let required = checked_frontier_domain_popcount(node_count, frontier)?;
    materialize_frontier_queue_exact_count_into(
        node_count,
        frontier,
        required,
        queue_capacity,
        queue,
    )
}

/// Materialize the queue prefix that fits while returning the full active count.
///
/// Device-side frontier compaction exposes overflow pressure by allowing the
/// observed queue length to exceed the queue storage capacity, while clamping
/// stores to the resident queue. This host helper mirrors that behavior and
/// still scans set bits word-by-word instead of walking every node id.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn materialize_frontier_queue_prefix_into(
    node_count: u32,
    frontier: &[u32],
    queue_capacity: usize,
    queue: &mut Vec<u32>,
) -> Result<u32, FrontierError> {
    let expected_words = validate_frontier_shape(node_count, frontier, "input")?;
    let reserve_words = queue_capacity.min(node_count as usize);
    if reserve_words > queue.capacity() {
        queue
            .try_reserve_exact(reserve_words - queue.capacity())
            .map_err(|source| FrontierError::Allocation {
                name: "frontier_queue",
                requested_words: reserve_words,
                source: source.to_string(),
            })?;
    }

    queue.clear();
    let final_word_index = expected_words.saturating_sub(1);
    let final_word_mask = frontier_tail_mask(node_count);
    let mut observed = 0u32;
    for (word_index, &word) in frontier.iter().enumerate() {
        let mut bits = if word_index == final_word_index {
            word & final_word_mask
        } else {
            word
        };
        while bits != 0 {
            let bit = bits.trailing_zeros();
            if queue.len() < queue_capacity {
                queue.push((word_index as u32 * u32::BITS) + bit);
            }
            observed = observed
                .checked_add(1)
                .ok_or(FrontierError::PopcountOverflow {
                    frontier_words: expected_words,
                })?;
            bits &= bits - 1;
        }
    }
    Ok(observed)
}

/// Materialize active node ids when the caller already knows the active count.
///
/// This is the preferred host path for benchmark/dataflow fixtures that counted
/// active sources while constructing the frontier. It preserves the same
/// ordered queue and tail-masking behavior as [`materialize_frontier_queue_into`]
/// without a second full popcount pass.
pub fn materialize_frontier_queue_exact_count_into(
    node_count: u32,
    frontier: &[u32],
    active_count: u32,
    queue_capacity: usize,
    queue: &mut Vec<u32>,
) -> Result<u32, FrontierError> {
    let expected_words = validate_frontier_shape(node_count, frontier, "input")?;
    if active_count as usize > queue_capacity {
        return Err(FrontierError::QueueCapacity {
            node_count,
            capacity: queue_capacity,
            required: active_count,
        });
    }
    let required_usize = active_count as usize;
    if required_usize > queue.capacity() {
        queue
            .try_reserve_exact(required_usize - queue.capacity())
            .map_err(|source| FrontierError::Allocation {
                name: "frontier_queue",
                requested_words: required_usize,
                source: source.to_string(),
            })?;
    }

    queue.clear();
    let final_word_index = expected_words.saturating_sub(1);
    let final_word_mask = frontier_tail_mask(node_count);
    let mut observed = 0u32;
    for (word_index, &word) in frontier.iter().enumerate() {
        let mut bits = if word_index == final_word_index {
            word & final_word_mask
        } else {
            word
        };
        while bits != 0 {
            observed = observed
                .checked_add(1)
                .ok_or(FrontierError::PopcountOverflow {
                    frontier_words: expected_words,
                })?;
            if observed > active_count {
                return Err(FrontierError::QueueCountMismatch {
                    node_count,
                    expected: active_count,
                    observed,
                });
            }
            let bit = bits.trailing_zeros();
            queue.push((word_index as u32 * u32::BITS) + bit);
            bits &= bits - 1;
        }
    }
    if observed != active_count {
        return Err(FrontierError::QueueCountMismatch {
            node_count,
            expected: active_count,
            observed,
        });
    }
    Ok(observed)
}

/// Clear out-of-domain bits in the final frontier word.
pub fn mask_frontier_tail_bits(node_count: u32, frontier: &mut [u32]) {
    if let Some(last_word) = frontier.last_mut() {
        *last_word &= frontier_tail_mask(node_count);
    }
}

/// Merge a neighbor frontier into a visited set and materialize only new bits.
pub fn absorb_new_frontier_bits(
    node_count: u32,
    visited: &mut [u32],
    neighbors: &[u32],
    next_wave: &mut Vec<u32>,
) -> Result<FrontierAbsorbSummary, FrontierError> {
    let expected_words = validate_frontier_shape(node_count, visited, "visited")?;
    validate_frontier_shape(node_count, neighbors, "neighbors")?;
    if expected_words > next_wave.capacity() {
        next_wave
            .try_reserve_exact(expected_words - next_wave.capacity())
            .map_err(|source| FrontierError::Allocation {
                name: "next_wave",
                requested_words: expected_words,
                source: source.to_string(),
            })?;
    }
    next_wave.clear();
    next_wave.resize(expected_words, 0);
    let mut summary = FrontierAbsorbSummary::default();
    let last_word_index = expected_words.saturating_sub(1);
    let tail_mask = frontier_tail_mask(node_count);
    for (word_index, (visited_word, neighbor_word)) in visited
        .iter_mut()
        .zip(neighbors.iter().copied())
        .enumerate()
    {
        let in_domain_neighbors = if word_index == last_word_index {
            neighbor_word & tail_mask
        } else {
            neighbor_word
        };
        let new_bits = in_domain_neighbors & !*visited_word;
        next_wave[word_index] = new_bits;
        *visited_word |= new_bits;
        summary.added_any |= new_bits != 0;
        summary.added_popcount = summary
            .added_popcount
            .checked_add(new_bits.count_ones())
            .ok_or(FrontierError::PopcountOverflow {
                frontier_words: expected_words,
            })?;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorb_masks_tail_and_reports_added_popcount() {
        let mut visited = vec![0b0001, 0b0001];
        let neighbors = vec![0b0111, 0b1000_0111];
        let mut next_wave = Vec::new();
        let summary = absorb_new_frontier_bits(35, &mut visited, &neighbors, &mut next_wave)
            .expect("Fix: valid frontier");
        assert_eq!(summary.added_popcount, 4);
        assert_eq!(next_wave, vec![0b0110, 0b0110]);
        assert_eq!(visited, vec![0b0111, 0b0111]);
    }

    #[test]
    fn no_count_absorb_program_keeps_only_frontier_outputs() {
        let program = frontier_absorb_new_bits_no_counts_for_node_count_program(
            "visited",
            "neighbors",
            "next",
            65,
        );
        let names = program
            .buffers()
            .iter()
            .map(|buffer| buffer.name())
            .collect::<Vec<_>>();

        assert_eq!(program.workgroup_size(), [256, 1, 1]);
        assert_eq!(names, vec!["visited", "neighbors", "next"]);
        assert_eq!(program.buffers()[0].count, 3);
        assert_eq!(program.buffers()[1].count, 3);
        assert_eq!(program.buffers()[2].count, 3);
        assert!(!names.contains(&"added_counts"));
    }

    #[test]
    fn frontier_queue_materializes_set_bits_in_order_and_masks_tail() {
        let frontier = [0b1010_u32, u32::MAX, u32::MAX];
        let mut queue = Vec::new();

        let len = materialize_frontier_queue_into(65, &frontier, 100, &mut queue)
            .expect("Fix: frontier queue should fit");

        assert_eq!(len, 35);
        assert_eq!(queue[0..4], [1, 3, 32, 33]);
        assert_eq!(*queue.last().unwrap(), 64);
        assert!(
            queue.iter().all(|node| *node < 65),
            "tail bits outside node_count must not enter the frontier queue"
        );
    }

    #[test]
    fn frontier_queue_rejects_under_capacity_without_mutating_output() {
        let frontier = [0b1111_u32];
        let mut queue = vec![99, 100];

        let err = materialize_frontier_queue_into(4, &frontier, 3, &mut queue)
            .expect_err("under-capacity queue must fail");

        assert!(matches!(
            err,
            FrontierError::QueueCapacity {
                node_count: 4,
                capacity: 3,
                required: 4,
            }
        ));
        assert_eq!(queue, vec![99, 100]);
    }

    #[test]
    fn exact_count_frontier_queue_materializes_ordered_tail_masked_bits() {
        let frontier = [0b1010_u32, u32::MAX, u32::MAX];
        let mut queue = Vec::new();

        let len = materialize_frontier_queue_exact_count_into(65, &frontier, 35, 35, &mut queue)
            .expect("Fix: exact-count frontier queue should fit");

        assert_eq!(len, 35);
        assert_eq!(queue[0..4], [1, 3, 32, 33]);
        assert_eq!(*queue.last().unwrap(), 64);
        assert_eq!(queue.len(), 35);
    }

    #[test]
    fn exact_count_frontier_queue_rejects_stale_low_or_high_counts() {
        let frontier = [0b1111_u32];
        let mut queue = Vec::new();

        let low = materialize_frontier_queue_exact_count_into(4, &frontier, 3, 4, &mut queue)
            .expect_err("stale low count must fail");
        assert!(matches!(
            low,
            FrontierError::QueueCountMismatch {
                node_count: 4,
                expected: 3,
                observed: 4,
            }
        ));

        let high = materialize_frontier_queue_exact_count_into(4, &frontier, 5, 5, &mut queue)
            .expect_err("stale high count must fail");
        assert!(matches!(
            high,
            FrontierError::QueueCountMismatch {
                node_count: 4,
                expected: 5,
                observed: 4,
            }
        ));
    }

    #[test]
    fn prefix_frontier_queue_clamps_capacity_and_returns_full_count() {
        let frontier = [0b1010_u32, u32::MAX, u32::MAX];
        let mut queue = Vec::new();

        let len = materialize_frontier_queue_prefix_into(65, &frontier, 4, &mut queue)
            .expect("Fix: prefix frontier queue should materialize");

        assert_eq!(len, 35);
        assert_eq!(queue, vec![1, 3, 32, 33]);
        assert!(
            queue.iter().all(|node| *node < 65),
            "tail bits outside node_count must not enter the prefix queue"
        );
    }

    #[test]
    fn prefix_frontier_queue_zero_capacity_still_counts_active_bits() {
        let frontier = [u32::MAX, u32::MAX];
        let mut queue = vec![99, 100];

        let len = materialize_frontier_queue_prefix_into(33, &frontier, 0, &mut queue)
            .expect("Fix: zero-capacity prefix queue should still report pressure");

        assert_eq!(len, 33);
        assert!(queue.is_empty());
    }

    #[test]
    fn prefix_frontier_queue_rejects_bad_shape_without_mutating_output() {
        let frontier = [0b1010_u32];
        let mut queue = vec![99, 100];

        let err = materialize_frontier_queue_prefix_into(64, &frontier, 8, &mut queue)
            .expect_err("bad prefix frontier shape must fail");

        assert!(matches!(err, FrontierError::BadShape { name: "input", .. }));
        assert_eq!(queue, vec![99, 100]);
    }

    #[test]
    fn generated_frontier_queue_matches_scalar_scan_across_10000_shapes() {
        for seed in 0..10_000_u32 {
            let node_count = 1 + (mix32(seed) % 8_192);
            let words = frontier_words(node_count);
            let mut frontier = (0..words)
                .map(|word| mix32(seed ^ (word as u32).wrapping_mul(0x9E37_79B9)))
                .collect::<Vec<_>>();
            if seed & 7 == 0 {
                frontier.fill(0);
                let node = mix32(seed ^ 0x5150_ACE5) % node_count;
                frontier[(node / 32) as usize] |= 1_u32 << (node % 32);
            }
            let expected = scalar_frontier_queue(node_count, &frontier);
            let mut queue = Vec::new();

            let len =
                materialize_frontier_queue_into(node_count, &frontier, expected.len(), &mut queue)
                    .expect("Fix: generated frontier queue should fit exactly");

            assert_eq!(len as usize, expected.len(), "seed={seed}");
            assert_eq!(queue, expected, "seed={seed} node_count={node_count}");
        }
    }

    #[test]
    fn generated_exact_count_frontier_queue_matches_scalar_scan_across_10000_shapes() {
        for seed in 0..10_000_u32 {
            let node_count = 1 + (mix32(seed ^ 0xECA7_C011) % 8_192);
            let words = frontier_words(node_count);
            let mut frontier = (0..words)
                .map(|word| mix32(seed ^ (word as u32).wrapping_mul(0x85EB_CA6B)))
                .collect::<Vec<_>>();
            if seed & 15 == 0 {
                frontier.fill(0);
                let node = mix32(seed ^ 0xD47A_F10D) % node_count;
                frontier[(node / 32) as usize] |= 1_u32 << (node % 32);
            }
            let expected = scalar_frontier_queue(node_count, &frontier);
            let mut queue = Vec::new();

            let len = materialize_frontier_queue_exact_count_into(
                node_count,
                &frontier,
                expected.len() as u32,
                expected.len(),
                &mut queue,
            )
            .expect("Fix: generated exact-count frontier queue should fit exactly");

            assert_eq!(len as usize, expected.len(), "seed={seed}");
            assert_eq!(queue, expected, "seed={seed} node_count={node_count}");
        }
    }

    #[test]
    fn generated_prefix_frontier_queue_matches_scalar_scan_across_10000_shapes() {
        for seed in 0..10_000_u32 {
            let node_count = 1 + (mix32(seed ^ 0xB17C_0DE5) % 8_192);
            let words = frontier_words(node_count);
            let mut frontier = (0..words)
                .map(|word| mix32(seed ^ (word as u32).wrapping_mul(0x27D4_EB2D)))
                .collect::<Vec<_>>();
            if seed & 31 == 0 {
                frontier.fill(0);
                let node = mix32(seed ^ 0xA11C_EED5) % node_count;
                frontier[(node / 32) as usize] |= 1_u32 << (node % 32);
            }
            let expected = scalar_frontier_queue(node_count, &frontier);
            let capacity = (mix32(seed ^ 0xCAFE_BA5E) as usize) % (expected.len() + 17);
            let mut queue = Vec::new();

            let len =
                materialize_frontier_queue_prefix_into(node_count, &frontier, capacity, &mut queue)
                    .expect("Fix: generated prefix frontier queue should materialize");

            assert_eq!(len as usize, expected.len(), "seed={seed}");
            assert_eq!(
                queue,
                expected.iter().copied().take(capacity).collect::<Vec<_>>(),
                "seed={seed} node_count={node_count} capacity={capacity}"
            );
        }
    }

    #[test]
    fn absorb_reuses_next_wave_and_clears_stale_tail() {
        let mut visited = vec![0b0001, 0b0001];
        let neighbors = vec![0b0111, 0b1000_0111];
        let mut next_wave = Vec::with_capacity(8);
        next_wave.extend_from_slice(&[u32::MAX; 8]);
        let ptr = next_wave.as_ptr();

        let summary = absorb_new_frontier_bits(35, &mut visited, &neighbors, &mut next_wave)
            .expect("Fix: valid frontier");

        assert_eq!(summary.added_popcount, 4);
        assert_eq!(next_wave, vec![0b0110, 0b0110]);
        assert_eq!(next_wave.as_ptr(), ptr);
    }

    #[test]
    fn absorb_rejects_bad_shape_without_mutating_buffers() {
        let mut visited = vec![0b0001, 0b0010];
        let before_visited = visited.clone();
        let neighbors = vec![0b0111];
        let mut next_wave = vec![0xDEAD_BEEF];

        let err = absorb_new_frontier_bits(35, &mut visited, &neighbors, &mut next_wave)
            .expect_err("bad neighbor shape must fail before mutation");

        assert!(matches!(
            err,
            FrontierError::BadShape {
                name: "neighbors",
                ..
            }
        ));
        assert_eq!(visited, before_visited);
        assert_eq!(next_wave, vec![0xDEAD_BEEF]);
    }

    #[test]
    fn generated_absorb_matches_scalar_reference() {
        let patterns = [0, u32::MAX, 0x5555_5555, 0xAAAA_AAAA, 0x1357_9BDF];
        for node_count in 0..=512 {
            let words = frontier_words(node_count);
            for (case_index, pattern) in patterns.into_iter().enumerate() {
                let mut visited = (0..words)
                    .map(|word| pattern.rotate_left((word as u32 + case_index as u32) % 32))
                    .collect::<Vec<_>>();
                let neighbors = (0..words)
                    .map(|word| (!pattern).rotate_right((word as u32 * 7) % 32))
                    .collect::<Vec<_>>();
                mask_frontier_tail_bits(node_count, &mut visited);
                let before = visited.clone();
                let mut next_wave = Vec::new();
                let summary =
                    absorb_new_frontier_bits(node_count, &mut visited, &neighbors, &mut next_wave)
                        .expect("Fix: generated shapes are valid");
                let tail_index = words.saturating_sub(1);
                let tail_mask = frontier_tail_mask(node_count);
                let expected_next = before
                    .iter()
                    .zip(neighbors.iter())
                    .enumerate()
                    .map(|(idx, (&old, &neighbor))| {
                        let in_domain = if idx == tail_index {
                            neighbor & tail_mask
                        } else {
                            neighbor
                        };
                        in_domain & !old
                    })
                    .collect::<Vec<_>>();
                assert_eq!(next_wave, expected_next, "node_count={node_count}");
                assert_eq!(
                    summary.added_popcount,
                    expected_next
                        .iter()
                        .map(|word| word.count_ones())
                        .sum::<u32>()
                );
            }
        }
    }

    fn scalar_frontier_queue(node_count: u32, frontier: &[u32]) -> Vec<u32> {
        (0..node_count)
            .filter(|node| {
                let word = (*node / 32) as usize;
                let bit = 1_u32 << (*node % 32);
                frontier[word] & bit != 0
            })
            .collect()
    }

    fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7FEB_352D);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846C_A68B);
        value ^ (value >> 16)
    }
}
