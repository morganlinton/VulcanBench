//! Mixed sparse CSR queue traversal for active sets with a small number of hubs.
//!
//! A global row-strided pass is excellent for true hub rows, but wastes lanes on
//! the many one-edge and three-edge rows that usually travel in the same active
//! queue. This primitive keeps low-degree rows in a scalar queue pass and
//! compacts only high-degree sources into a second queue for row-strided
//! traversal.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::bitset::bitset_words;
use crate::graph::csr_queue_strided::CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE;

/// Canonical op id for mixed low-row traversal and high-row compaction.
pub const CSR_QUEUE_SPLIT_LOW_FORWARD_OP_ID: &str =
    "vyre-primitives::graph::csr_queue_split_low_forward_traverse";

/// Workgroup shape for the low-row split pass.
pub const CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Degree at which a queued row has enough work to amortize a 32-lane team.
pub const CSR_QUEUE_SPLIT_HIGH_DEGREE_THRESHOLD: u32 =
    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE * CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE;

/// Dispatch grid for the one-lane-per-active-source low split pass.
#[must_use]
pub const fn csr_queue_split_low_dispatch_grid(queue_capacity: u32) -> [u32; 3] {
    let blocks = queue_capacity.div_ceil(CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Logical lanes consumed by low split plus a high row-strided follow-up pass.
#[must_use]
pub const fn csr_queue_split_mixed_logical_lanes(
    queue_capacity: u32,
    high_queue_capacity: u32,
) -> u64 {
    (queue_capacity as u64).saturating_add(
        (high_queue_capacity as u64)
            .saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE as u64),
    )
}

/// Build the low-row half of a mixed queue traversal.
///
/// Low-degree rows are expanded directly into `frontier_out`. High-degree rows
/// are appended to `high_queue` and counted in `high_len`; callers then run
/// `csr_queue_strided_forward_traverse` over that compact high queue. If
/// `high_queue` is undersized, overflow high rows are expanded by the scalar
/// lane in this pass so correctness does not depend on perfect sizing.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_split_low_forward_traverse(
    active_queue: &str,
    queue_len: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    frontier_out: &str,
    high_queue: &str,
    high_len: &str,
    node_count: u32,
    edge_count: u32,
    queue_capacity: u32,
    high_queue_capacity: u32,
    high_degree_threshold: u32,
    allow_mask: u32,
) -> Program {
    if node_count == 0
        || queue_capacity == 0
        || high_queue_capacity == 0
        || high_degree_threshold == 0
    {
        return crate::invalid_output_program(
            CSR_QUEUE_SPLIT_LOW_FORWARD_OP_ID,
            frontier_out,
            DataType::U32,
            format!(
                "Fix: csr_queue_split_low_forward_traverse requires node_count > 0, non-zero queue capacities, and high_degree_threshold > 0; got node_count={node_count} queue_capacity={queue_capacity} high_queue_capacity={high_queue_capacity} high_degree_threshold={high_degree_threshold}."
            ),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let physical_edge_count = edge_count.max(1);
    let edge_offset_count = match crate::graph::checked_csr_offset_count(
        node_count,
        "csr_queue_split_low_forward_traverse",
    ) {
        Ok(edge_offset_count) => edge_offset_count,
        Err(error) => {
            return crate::invalid_output_program(
                CSR_QUEUE_SPLIT_LOW_FORWARD_OP_ID,
                frontier_out,
                DataType::U32,
                error,
            );
        }
    };
    let scalar_emit = || {
        scalar_emit_nodes(
            edge_targets,
            edge_kind_mask,
            frontier_out,
            node_count,
            edge_count,
            allow_mask,
        )
    };
    let body = vec![
        Node::let_bind("qsl_idx", lane),
        Node::if_then(
            Expr::lt(Expr::var("qsl_idx"), Expr::u32(queue_capacity)),
            vec![Node::if_then(
                Expr::lt(Expr::var("qsl_idx"), Expr::load(queue_len, Expr::u32(0))),
                vec![
                    Node::let_bind("qsl_src", Expr::load(active_queue, Expr::var("qsl_idx"))),
                    Node::if_then(
                        Expr::lt(Expr::var("qsl_src"), Expr::u32(node_count)),
                        vec![
                            Node::let_bind(
                                "qsl_edge_start",
                                Expr::load(edge_offsets, Expr::var("qsl_src")),
                            ),
                            Node::let_bind(
                                "qsl_edge_end",
                                Expr::load(
                                    edge_offsets,
                                    Expr::add(Expr::var("qsl_src"), Expr::u32(1)),
                                ),
                            ),
                            Node::let_bind(
                                "qsl_degree",
                                Expr::sub(Expr::var("qsl_edge_end"), Expr::var("qsl_edge_start")),
                            ),
                            Node::if_then_else(
                                Expr::ge(Expr::var("qsl_degree"), Expr::u32(high_degree_threshold)),
                                vec![
                                    Node::let_bind(
                                        "qsl_high_slot",
                                        Expr::atomic_add(high_len, Expr::u32(0), Expr::u32(1)),
                                    ),
                                    Node::if_then_else(
                                        Expr::lt(
                                            Expr::var("qsl_high_slot"),
                                            Expr::u32(high_queue_capacity),
                                        ),
                                        vec![Node::store(
                                            high_queue,
                                            Expr::var("qsl_high_slot"),
                                            Expr::var("qsl_src"),
                                        )],
                                        scalar_emit(),
                                    ),
                                ],
                                scalar_emit(),
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
                .with_count(queue_capacity),
            BufferDecl::storage(queue_len, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(edge_offsets, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(edge_offset_count),
            BufferDecl::storage(edge_targets, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(edge_kind_mask, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(physical_edge_count),
            BufferDecl::storage(frontier_out, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(high_queue, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(high_queue_capacity),
            BufferDecl::storage(high_len, 7, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        CSR_QUEUE_SPLIT_LOW_FORWARD_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(CSR_QUEUE_SPLIT_LOW_FORWARD_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

fn scalar_emit_nodes(
    edge_targets: &str,
    edge_kind_mask: &str,
    frontier_out: &str,
    node_count: u32,
    edge_count: u32,
    allow_mask: u32,
) -> Vec<Node> {
    vec![Node::loop_for(
        "qsl_e",
        Expr::var("qsl_edge_start"),
        Expr::var("qsl_edge_end"),
        vec![Node::if_then(
            Expr::lt(Expr::var("qsl_e"), Expr::u32(edge_count)),
            vec![
                Node::let_bind("qsl_kind", Expr::load(edge_kind_mask, Expr::var("qsl_e"))),
                Node::if_then(
                    Expr::ne(
                        Expr::bitand(Expr::var("qsl_kind"), Expr::u32(allow_mask)),
                        Expr::u32(0),
                    ),
                    vec![
                        Node::let_bind("qsl_dst", Expr::load(edge_targets, Expr::var("qsl_e"))),
                        Node::if_then(
                            Expr::lt(Expr::var("qsl_dst"), Expr::u32(node_count)),
                            vec![
                                Node::let_bind(
                                    "qsl_dst_word",
                                    Expr::shr(Expr::var("qsl_dst"), Expr::u32(5)),
                                ),
                                Node::let_bind(
                                    "qsl_dst_bit",
                                    Expr::shl(
                                        Expr::u32(1),
                                        Expr::bitand(Expr::var("qsl_dst"), Expr::u32(31)),
                                    ),
                                ),
                                Node::let_bind(
                                    "_qsl_prev",
                                    Expr::atomic_or(
                                        frontier_out,
                                        Expr::var("qsl_dst_word"),
                                        Expr::var("qsl_dst_bit"),
                                    ),
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        )],
    )]
}

/// CPU result for the low split pass.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsrQueueSplitLowForwardCpuResult {
    /// Frontier bitset after low-degree rows and overflow high rows were emitted.
    pub frontier_out: Vec<u32>,
    /// Compact queue of high-degree sources that fit in the high queue capacity.
    pub high_queue: Vec<u32>,
    /// Total high-degree source count observed, including entries beyond capacity.
    pub high_len: u32,
}

/// Fallible CPU reference for the low split pass.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_csr_queue_split_low_forward_traverse_cpu(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    frontier_out_seed: &[u32],
    node_count: u32,
    high_queue_capacity: usize,
    high_degree_threshold: u32,
    allow_mask: u32,
) -> Result<CsrQueueSplitLowForwardCpuResult, String> {
    let layout = super::csr_frontier_queue::validate_csr_queue_graph(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
    )?;
    if frontier_out_seed.len() != layout.words {
        return Err(format!(
            "Fix: csr_queue_split_low_forward_traverse requires frontier_out_seed.len() == bitset_words(node_count), got len={} but expected {} for node_count={node_count}.",
            frontier_out_seed.len(),
            layout.words
        ));
    }
    let mut high_queue_probe: Vec<u32> = Vec::new();
    crate::graph::scratch::reserve_graph_items(
        &mut high_queue_probe,
        high_queue_capacity,
        "CSR queue split CPU oracle",
        "high-degree active queue",
    )?;

    let mut frontier_out = frontier_out_seed.to_vec();
    let mut high_queue = Vec::with_capacity(high_queue_capacity);
    let mut high_len = 0_u32;
    let take = (queue_len as usize).min(active_queue.len());

    for &src in &active_queue[..take] {
        if src >= node_count {
            continue;
        }
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        if end.saturating_sub(start) as u32 >= high_degree_threshold {
            high_len = high_len.saturating_add(1);
            if high_queue.len() < high_queue_capacity {
                high_queue.push(src);
                continue;
            }
        }
        emit_scalar_row_cpu(
            start,
            end,
            edge_targets,
            edge_kind_mask,
            node_count,
            allow_mask,
            &mut frontier_out,
        );
    }

    Ok(CsrQueueSplitLowForwardCpuResult {
        frontier_out,
        high_queue,
        high_len,
    })
}

#[cfg(any(test, feature = "cpu-parity"))]
fn emit_scalar_row_cpu(
    start: usize,
    end: usize,
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
    frontier_out: &mut [u32],
) {
    for edge in start..end {
        if edge_kind_mask[edge] & allow_mask == 0 {
            continue;
        }
        let dst = edge_targets[edge];
        if dst >= node_count {
            continue;
        }
        frontier_out[dst as usize / 32] |= 1_u32 << (dst % 32);
    }
}

#[cfg(test)]
mod tests;
