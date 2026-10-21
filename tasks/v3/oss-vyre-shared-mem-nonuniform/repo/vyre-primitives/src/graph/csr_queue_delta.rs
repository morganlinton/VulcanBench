//! Queue-to-queue sparse CSR expansion for delta fixpoint waves.
//!
//! A full frontier bitset scan is the wrong shape once a dataflow pipeline has
//! already compacted the active wave. This primitive consumes only queued
//! sources, updates a resident accumulator bitset, and appends first-time
//! discoveries directly into the next active queue.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::bitset::bitset_words;

mod strided;

pub use strided::{
    csr_queue_delta_strided_dispatch_grid, csr_queue_delta_strided_enqueue,
    csr_queue_delta_strided_logical_lanes_per_launch,
    csr_queue_delta_strided_source_slots_per_launch,
    CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY, CSR_QUEUE_DELTA_STRIDED_ENQUEUE_OP_ID,
    CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE, CSR_QUEUE_DELTA_STRIDED_MAX_SOURCE_SLOTS_PER_LAUNCH,
};

/// Canonical op id for queue-to-queue delta CSR expansion.
pub const CSR_QUEUE_DELTA_ENQUEUE_OP_ID: &str = "vyre-primitives::graph::csr_queue_delta_enqueue";

/// Default workgroup size for queue-to-queue delta expansion.
pub const CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Build a GPU program that expands queued CSR rows and enqueues only new nodes.
///
/// `accumulator` is the monotone reachability bitset. When an allowed edge
/// reaches a destination whose bit was absent, the destination is appended to
/// `next_queue` and `next_len` is incremented. The observed next length can
/// exceed `next_queue_capacity`; stores are clamped so callers can detect
/// overflow pressure without corrupting resident memory.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_delta_enqueue(
    active_queue: &str,
    active_len: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    accumulator: &str,
    next_queue: &str,
    next_len: &str,
    node_count: u32,
    edge_count: u32,
    active_queue_capacity: u32,
    next_queue_capacity: u32,
    allow_mask: u32,
) -> Program {
    if node_count == 0 || active_queue_capacity == 0 || next_queue_capacity == 0 {
        return crate::invalid_output_program(
            CSR_QUEUE_DELTA_ENQUEUE_OP_ID,
            next_len,
            DataType::U32,
            format!(
                "Fix: csr_queue_delta_enqueue requires node_count > 0 and non-zero queue capacities, got node_count={node_count} active_queue_capacity={active_queue_capacity} next_queue_capacity={next_queue_capacity}."
            ),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let physical_edge_count = edge_count.max(1);
    let edge_offset_count =
        match crate::graph::checked_csr_offset_count(node_count, "csr_queue_delta_enqueue") {
            Ok(edge_offset_count) => edge_offset_count,
            Err(error) => {
                return crate::invalid_output_program(
                    CSR_QUEUE_DELTA_ENQUEUE_OP_ID,
                    next_len,
                    DataType::U32,
                    error,
                );
            }
        };
    let body = vec![
        Node::let_bind("qd_idx", lane.clone()),
        Node::if_then(
            Expr::lt(Expr::var("qd_idx"), Expr::u32(active_queue_capacity)),
            vec![Node::if_then(
                Expr::lt(Expr::var("qd_idx"), Expr::load(active_len, Expr::u32(0))),
                vec![
                    Node::let_bind("qd_src", Expr::load(active_queue, Expr::var("qd_idx"))),
                    Node::if_then(
                        Expr::lt(Expr::var("qd_src"), Expr::u32(node_count)),
                        vec![
                            Node::let_bind(
                                "qd_edge_start",
                                Expr::load(edge_offsets, Expr::var("qd_src")),
                            ),
                            Node::let_bind(
                                "qd_edge_end",
                                Expr::load(
                                    edge_offsets,
                                    Expr::add(Expr::var("qd_src"), Expr::u32(1)),
                                ),
                            ),
                            Node::loop_for(
                                "qd_e",
                                Expr::var("qd_edge_start"),
                                Expr::var("qd_edge_end"),
                                vec![Node::if_then(
                                    Expr::lt(Expr::var("qd_e"), Expr::u32(edge_count)),
                                    vec![
                                        Node::let_bind(
                                            "qd_kind",
                                            Expr::load(edge_kind_mask, Expr::var("qd_e")),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::bitand(
                                                    Expr::var("qd_kind"),
                                                    Expr::u32(allow_mask),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            vec![
                                                Node::let_bind(
                                                    "qd_dst",
                                                    Expr::load(edge_targets, Expr::var("qd_e")),
                                                ),
                                                Node::if_then(
                                                    Expr::lt(
                                                        Expr::var("qd_dst"),
                                                        Expr::u32(node_count),
                                                    ),
                                                    vec![
                                                        Node::let_bind(
                                                            "qd_dst_word",
                                                            Expr::shr(
                                                                Expr::var("qd_dst"),
                                                                Expr::u32(5),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "qd_dst_bit",
                                                            Expr::shl(
                                                                Expr::u32(1),
                                                                Expr::bitand(
                                                                    Expr::var("qd_dst"),
                                                                    Expr::u32(31),
                                                                ),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "qd_old",
                                                            Expr::atomic_or(
                                                                accumulator,
                                                                Expr::var("qd_dst_word"),
                                                                Expr::var("qd_dst_bit"),
                                                            ),
                                                        ),
                                                        Node::if_then(
                                                            Expr::eq(
                                                                Expr::bitand(
                                                                    Expr::var("qd_old"),
                                                                    Expr::var("qd_dst_bit"),
                                                                ),
                                                                Expr::u32(0),
                                                            ),
                                                            vec![
                                                                Node::let_bind(
                                                                    "qd_slot",
                                                                    Expr::atomic_add(
                                                                        next_len,
                                                                        Expr::u32(0),
                                                                        Expr::u32(1),
                                                                    ),
                                                                ),
                                                                Node::if_then(
                                                                    Expr::lt(
                                                                        Expr::var("qd_slot"),
                                                                        Expr::u32(
                                                                            next_queue_capacity,
                                                                        ),
                                                                    ),
                                                                    vec![Node::store(
                                                                        next_queue,
                                                                        Expr::var("qd_slot"),
                                                                        Expr::var("qd_dst"),
                                                                    )],
                                                                ),
                                                            ],
                                                        ),
                                                    ],
                                                ),
                                            ],
                                        ),
                                    ],
                                )],
                            ),
                        ],
                    ),
                ],
            )],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(active_queue, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(active_queue_capacity),
            BufferDecl::storage(active_len, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(edge_offsets, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(edge_offset_count),
            BufferDecl::storage(edge_targets, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(edge_kind_mask, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(accumulator, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(next_queue, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(next_queue_capacity),
            BufferDecl::storage(next_len, 7, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(CSR_QUEUE_DELTA_ENQUEUE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference for queue-to-queue delta expansion.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_delta_enqueue_cpu(
    active_queue: &[u32],
    active_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    accumulator: &[u32],
    node_count: u32,
    next_queue_capacity: usize,
    allow_mask: u32,
) -> (Vec<u32>, Vec<u32>, u32) {
    let mut accumulator = accumulator.to_vec();
    let mut next_queue = Vec::new();
    let next_len = try_csr_queue_delta_enqueue_cpu_into(
        active_queue,
        active_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        &mut accumulator,
        node_count,
        next_queue_capacity,
        allow_mask,
        &mut next_queue,
    )
    .unwrap_or_else(|err| {
        panic!("csr_queue_delta_enqueue CPU oracle received malformed input. {err}")
    });
    (accumulator, next_queue, next_len)
}

/// Fallible CPU reference for queue-to-queue delta expansion into caller storage.
///
/// On validation failure both `accumulator` and `next_queue` are left unchanged.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_csr_queue_delta_enqueue_cpu_into(
    active_queue: &[u32],
    active_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    accumulator: &mut Vec<u32>,
    node_count: u32,
    next_queue_capacity: usize,
    allow_mask: u32,
    next_queue: &mut Vec<u32>,
) -> Result<u32, String> {
    let layout = super::csr_frontier_queue::validate_csr_queue_graph(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    )?;
    if accumulator.len() != layout.words {
        return Err(format!(
            "Fix: csr_queue_delta_enqueue requires accumulator.len() == bitset_words(node_count), got len={} but expected {} for node_count={node_count}.",
            accumulator.len(),
            layout.words
        ));
    }
    crate::graph::scratch::reserve_graph_items(
        next_queue,
        next_queue_capacity,
        "CSR queue delta CPU oracle",
        "next active frontier queue",
    )?;

    let mut next_tmp = Vec::with_capacity(next_queue_capacity);
    let mut accumulator_tmp = accumulator.clone();
    let take = (active_len as usize).min(active_queue.len());
    let mut next_seen = 0_u32;

    for &src in &active_queue[..take] {
        if src >= node_count {
            continue;
        }
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        for edge in start..end {
            if edge_kind_mask[edge] & allow_mask == 0 {
                continue;
            }
            let dst = edge_targets[edge];
            let word = dst as usize / 32;
            let bit = 1_u32 << (dst % 32);
            let old = accumulator_tmp[word];
            if old & bit != 0 {
                continue;
            }
            accumulator_tmp[word] = old | bit;
            if next_tmp.len() < next_queue_capacity {
                next_tmp.push(dst);
            }
            next_seen = next_seen.saturating_add(1);
        }
    }

    *accumulator = accumulator_tmp;
    next_queue.clear();
    next_queue.extend_from_slice(&next_tmp);
    Ok(next_seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_program_has_stable_delta_queue_shape() {
        let program = csr_queue_delta_enqueue(
            "active_queue",
            "active_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "accumulator",
            "next_queue",
            "next_len",
            64,
            7,
            8,
            16,
            1,
        );

        assert_eq!(
            program.workgroup_size,
            CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE
        );
        assert_eq!(program.buffers.len(), 8);
    }

    #[test]
    fn delta_enqueue_rejects_offset_count_overflow_without_panic() {
        let result = std::panic::catch_unwind(|| {
            csr_queue_delta_enqueue(
                "active_queue",
                "active_len",
                "edge_offsets",
                "edge_targets",
                "edge_kind_mask",
                "accumulator",
                "next_queue",
                "next_len",
                u32::MAX,
                0,
                1,
                1,
                1,
            )
        });

        assert!(
            result.is_ok(),
            "CSR queue delta builder must reject offset-count overflow without panicking"
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
    fn cpu_delta_enqueue_only_emits_first_time_discoveries() {
        let edge_offsets = [0, 3, 4, 4, 4, 4];
        let edge_targets = [1, 2, 3, 4];
        let edge_kind_mask = [1, 1, 2, 1];
        let accumulator = vec![0b00001];

        let (accumulator, next_queue, next_len) = csr_queue_delta_enqueue_cpu(
            &[0, 1],
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &accumulator,
            5,
            8,
            1,
        );

        assert_eq!(accumulator, vec![0b10111]);
        assert_eq!(next_queue, vec![1, 2, 4]);
        assert_eq!(next_len, 3);
    }

    #[test]
    fn cpu_delta_enqueue_reports_queue_pressure_without_clobbering_accumulator() {
        let edge_offsets = [0, 3, 3, 3, 3];
        let edge_targets = [1, 2, 3];
        let edge_kind_mask = [1, 1, 1];
        let mut accumulator = vec![0b0001];
        let mut next_queue = Vec::new();

        let next_len = try_csr_queue_delta_enqueue_cpu_into(
            &[0],
            1,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &mut accumulator,
            4,
            2,
            1,
            &mut next_queue,
        )
        .expect("Fix: canonical queue delta graph should enqueue bounded discoveries");

        assert_eq!(accumulator, vec![0b1111]);
        assert_eq!(next_queue, vec![1, 2]);
        assert_eq!(next_len, 3);
    }

    #[test]
    fn cpu_delta_enqueue_rejects_bad_accumulator_without_clobbering_outputs() {
        let mut accumulator = vec![0xCAFE_BABE, 0xDEAD_BEEF];
        let mut next_queue = vec![9, 8, 7];

        let err = try_csr_queue_delta_enqueue_cpu_into(
            &[0],
            1,
            &[0, 1],
            &[0],
            &[1],
            &mut accumulator,
            1,
            4,
            1,
            &mut next_queue,
        )
        .expect_err("wrong accumulator width must fail before mutation");

        assert!(err.contains("accumulator.len() == bitset_words(node_count)"));
        assert_eq!(accumulator, vec![0xCAFE_BABE, 0xDEAD_BEEF]);
        assert_eq!(next_queue, vec![9, 8, 7]);
    }
}
