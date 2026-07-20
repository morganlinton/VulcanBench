//! Row-strided sparse CSR expansion for skewed active queues.
//!
//! `csr_frontier_queue::csr_queue_forward_traverse` maps one lane to one queued
//! source row. That is the right shape for tiny rows, but power-law graphs can
//! put thousands of edges behind one active source and leave the rest of the GPU
//! idle. This primitive keeps the same queue ABI and assigns a fixed lane team
//! to each queued source:
//!
//! ```text
//! queue index = global_lane / 32
//! edge lane   = global_lane % 32
//! for e = row_start + edge_lane; e < row_end; e += 32:
//!     emit edge target
//! ```
//!
//! It is intentionally a separate Program builder so callers can keep the
//! one-lane-per-source kernel for low-degree graphs and select this path only
//! when row skew is large enough to amortize the extra lanes.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::bitset::bitset_words;
#[cfg(any(test, feature = "cpu-parity"))]
use crate::graph::csr_frontier_queue::{
    try_csr_queue_forward_traverse_cpu, try_csr_queue_forward_traverse_cpu_into,
};

/// Canonical op id for row-strided queue-driven CSR expansion.
pub const CSR_QUEUE_STRIDED_FORWARD_OP_ID: &str =
    "vyre-primitives::graph::csr_queue_strided_forward_traverse";

/// Fixed lane team assigned to each queued source row.
pub const CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE: u32 = 32;

/// Workgroup shape for row-strided queue-driven CSR expansion.
pub const CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid that launches one 32-lane team for every queue slot.
#[must_use]
pub const fn csr_queue_strided_forward_dispatch_grid(queue_capacity: u32) -> [u32; 3] {
    let total_lanes = queue_capacity.saturating_mul(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);
    let blocks = total_lanes.div_ceil(CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Build a GPU program that expands queued CSR source rows with a fixed lane
/// team per row.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_strided_forward_traverse(
    active_queue: &str,
    queue_len: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    frontier_out: &str,
    node_count: u32,
    edge_count: u32,
    queue_capacity: u32,
    allow_mask: u32,
) -> Program {
    if node_count == 0 || queue_capacity == 0 {
        return crate::invalid_output_program(
            CSR_QUEUE_STRIDED_FORWARD_OP_ID,
            frontier_out,
            DataType::U32,
            format!(
                "Fix: csr_queue_strided_forward_traverse requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let physical_edge_count = edge_count.max(1);
    let edge_offset_count = match crate::graph::checked_csr_offset_count(
        node_count,
        "csr_queue_strided_forward_traverse",
    ) {
        Ok(edge_offset_count) => edge_offset_count,
        Err(error) => {
            return crate::invalid_output_program(
                CSR_QUEUE_STRIDED_FORWARD_OP_ID,
                frontier_out,
                DataType::U32,
                error,
            );
        }
    };
    let lanes_per_source = Expr::u32(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE);
    let body = vec![
        Node::let_bind("qs_lane", lane),
        Node::let_bind(
            "qs_queue_idx",
            Expr::div(Expr::var("qs_lane"), lanes_per_source.clone()),
        ),
        Node::let_bind(
            "qs_edge_lane",
            Expr::rem(Expr::var("qs_lane"), lanes_per_source.clone()),
        ),
        Node::if_then(
            Expr::lt(Expr::var("qs_queue_idx"), Expr::u32(queue_capacity)),
            vec![Node::if_then(
                Expr::lt(
                    Expr::var("qs_queue_idx"),
                    Expr::load(queue_len, Expr::u32(0)),
                ),
                vec![
                    Node::let_bind(
                        "qs_src",
                        Expr::load(active_queue, Expr::var("qs_queue_idx")),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("qs_src"), Expr::u32(node_count)),
                        vec![
                            Node::let_bind(
                                "qs_edge_start",
                                Expr::load(edge_offsets, Expr::var("qs_src")),
                            ),
                            Node::let_bind(
                                "qs_edge_end",
                                Expr::load(
                                    edge_offsets,
                                    Expr::add(Expr::var("qs_src"), Expr::u32(1)),
                                ),
                            ),
                            Node::let_bind(
                                "qs_degree",
                                Expr::sub(Expr::var("qs_edge_end"), Expr::var("qs_edge_start")),
                            ),
                            Node::let_bind(
                                "qs_full_iters",
                                Expr::div(Expr::var("qs_degree"), lanes_per_source),
                            ),
                            Node::let_bind(
                                "qs_tail_iter",
                                Expr::select(
                                    Expr::ne(
                                        Expr::rem(
                                            Expr::var("qs_degree"),
                                            Expr::u32(CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE),
                                        ),
                                        Expr::u32(0),
                                    ),
                                    Expr::u32(1),
                                    Expr::u32(0),
                                ),
                            ),
                            Node::let_bind(
                                "qs_iters",
                                Expr::add(Expr::var("qs_full_iters"), Expr::var("qs_tail_iter")),
                            ),
                            Node::loop_for(
                                "qs_iter",
                                Expr::u32(0),
                                Expr::var("qs_iters"),
                                vec![
                                    Node::let_bind(
                                        "qs_edge_offset",
                                        Expr::add(
                                            Expr::var("qs_edge_lane"),
                                            Expr::mul(
                                                Expr::var("qs_iter"),
                                                Expr::u32(
                                                    CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE,
                                                ),
                                            ),
                                        ),
                                    ),
                                    Node::if_then(
                                        Expr::lt(
                                            Expr::var("qs_edge_offset"),
                                            Expr::var("qs_degree"),
                                        ),
                                        vec![
                                            Node::let_bind(
                                                "qs_e",
                                                Expr::add(
                                                    Expr::var("qs_edge_start"),
                                                    Expr::var("qs_edge_offset"),
                                                ),
                                            ),
                                            Node::if_then(
                                                Expr::lt(Expr::var("qs_e"), Expr::u32(edge_count)),
                                                vec![
                                                    Node::let_bind(
                                                        "qs_kind",
                                                        Expr::load(
                                                            edge_kind_mask,
                                                            Expr::var("qs_e"),
                                                        ),
                                                    ),
                                                    Node::if_then(
                                                        Expr::ne(
                                                            Expr::bitand(
                                                                Expr::var("qs_kind"),
                                                                Expr::u32(allow_mask),
                                                            ),
                                                            Expr::u32(0),
                                                        ),
                                                        vec![
                                                            Node::let_bind(
                                                                "qs_dst",
                                                                Expr::load(
                                                                    edge_targets,
                                                                    Expr::var("qs_e"),
                                                                ),
                                                            ),
                                                            Node::if_then(
                                                                Expr::lt(
                                                                    Expr::var("qs_dst"),
                                                                    Expr::u32(node_count),
                                                                ),
                                                                vec![
                                                                    Node::let_bind(
                                                                        "qs_dst_word",
                                                                        Expr::shr(
                                                                            Expr::var("qs_dst"),
                                                                            Expr::u32(5),
                                                                        ),
                                                                    ),
                                                                    Node::let_bind(
                                                                        "qs_dst_bit",
                                                                        Expr::shl(
                                                                            Expr::u32(1),
                                                                            Expr::bitand(
                                                                                Expr::var("qs_dst"),
                                                                                Expr::u32(31),
                                                                            ),
                                                                        ),
                                                                    ),
                                                                    Node::let_bind(
                                                                        "_qs_prev",
                                                                        Expr::atomic_or(
                                                                            frontier_out,
                                                                            Expr::var(
                                                                                "qs_dst_word",
                                                                            ),
                                                                            Expr::var("qs_dst_bit"),
                                                                        ),
                                                                    ),
                                                                ],
                                                            ),
                                                        ],
                                                    ),
                                                ],
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
        ],
        CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(CSR_QUEUE_STRIDED_FORWARD_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference for the row-strided queue traversal.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_strided_forward_traverse_cpu(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    try_csr_queue_strided_forward_traverse_cpu(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
    )
    .unwrap_or_else(|err| {
        panic!("csr_queue_strided_forward_traverse CPU oracle received malformed input. {err}")
    })
}

/// Fallible CPU reference for the row-strided queue traversal.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_csr_queue_strided_forward_traverse_cpu(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Result<Vec<u32>, String> {
    try_csr_queue_forward_traverse_cpu(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
    )
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_csr_queue_strided_forward_traverse_cpu_into(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    try_csr_queue_forward_traverse_cpu_into(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
        out,
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        CSR_QUEUE_STRIDED_FORWARD_OP_ID,
        || csr_queue_strided_forward_traverse(
            "active_queue",
            "queue_len",
            "edge_offsets",
            "edge_targets",
            "edge_kind_mask",
            "frontier_out",
            4,
            4,
            2,
            1,
        ),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 3]),            // active_queue
                to_bytes(&[2]),               // queue_len
                to_bytes(&[0, 3, 3, 4, 4]),   // edge_offsets
                to_bytes(&[1, 2, 3, 0]),      // edge_targets
                to_bytes(&[1, 2, 1, 1]),      // edge_kind_mask
                to_bytes(&[0]),               // frontier_out
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0b1010])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_queue_forward(
        active_queue: &[u32],
        queue_len: u32,
        edge_offsets: &[u32],
        edge_targets: &[u32],
        edge_kind_mask: &[u32],
        node_count: u32,
        allow_mask: u32,
    ) -> Vec<u32> {
        let mut out = vec![0u32; bitset_words(node_count) as usize];
        let take = (queue_len as usize).min(active_queue.len());
        for &src in &active_queue[..take] {
            if src >= node_count {
                continue;
            }
            for edge in edge_offsets[src as usize]..edge_offsets[src as usize + 1] {
                let edge = edge as usize;
                if edge_kind_mask[edge] & allow_mask == 0 {
                    continue;
                }
                let dst = edge_targets[edge];
                out[dst as usize / 32] |= 1u32 << (dst % 32);
            }
        }
        out
    }

    #[test]
    fn dispatch_grid_assigns_32_lanes_per_queue_slot() {
        assert_eq!(csr_queue_strided_forward_dispatch_grid(0), [1, 1, 1]);
        assert_eq!(csr_queue_strided_forward_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(csr_queue_strided_forward_dispatch_grid(8), [1, 1, 1]);
        assert_eq!(csr_queue_strided_forward_dispatch_grid(9), [2, 1, 1]);
        assert_eq!(csr_queue_strided_forward_dispatch_grid(256), [32, 1, 1]);
    }

    #[test]
    fn build_program_returns_well_formed_program() {
        let program = csr_queue_strided_forward_traverse(
            "queue", "len", "offsets", "targets", "kinds", "out", 64, 4096, 9, 0x55,
        );
        assert_eq!(
            program.workgroup_size(),
            CSR_QUEUE_STRIDED_FORWARD_WORKGROUP_SIZE
        );
        assert_eq!(program.buffers().len(), 6);
        assert!(!program.stats().trap());
    }

    #[test]
    fn generated_strided_cpu_matches_scalar_reference_on_skewed_rows() {
        let mut seed = 0x51A7_7EED_u32;
        for case in 0..4096u32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let node_count = 33 + (seed % 224);
            let queue_capacity = 1 + (seed.rotate_left(5) % node_count);
            let mut offsets = Vec::with_capacity(node_count as usize + 1);
            let mut targets = Vec::new();
            let mut masks = Vec::new();
            offsets.push(0);
            for src in 0..node_count {
                seed ^= src.wrapping_mul(0x9E37_79B9).rotate_left((src & 15) + 1);
                let degree = if src == case % node_count {
                    96 + (seed % 257)
                } else {
                    seed % 5
                };
                for edge in 0..degree {
                    targets.push(src.wrapping_mul(17).wrapping_add(edge * 3 + seed) % node_count);
                    masks.push(if (edge ^ src ^ seed) & 3 == 0 { 2 } else { 1 });
                }
                offsets.push(targets.len() as u32);
            }
            let mut queue = Vec::with_capacity(queue_capacity as usize);
            for slot in 0..queue_capacity {
                queue.push(slot.wrapping_mul(7).wrapping_add(seed) % node_count);
            }
            let queue_len = queue_capacity.saturating_add(seed % 3);
            let expected =
                scalar_queue_forward(&queue, queue_len, &offsets, &targets, &masks, node_count, 1);

            assert_eq!(
                try_csr_queue_strided_forward_traverse_cpu(
                    &queue, queue_len, &offsets, &targets, &masks, node_count, 1,
                ),
                Ok(expected),
                "generated skewed CSR queue case {case}"
            );
        }
    }

    #[test]
    fn invalid_shape_returns_trap_program() {
        let program = csr_queue_strided_forward_traverse(
            "queue", "len", "offsets", "targets", "kinds", "out", 0, 0, 1, 1,
        );

        assert!(
            program.stats().trap(),
            "invalid node_count must compile to a trap program"
        );
    }

    #[test]
    fn offset_count_overflow_returns_trap_program_without_panic() {
        let result = std::panic::catch_unwind(|| {
            csr_queue_strided_forward_traverse(
                "queue",
                "len",
                "offsets",
                "targets",
                "kinds",
                "out",
                u32::MAX,
                0,
                1,
                1,
            )
        });

        assert!(
            result.is_ok(),
            "CSR queue strided builder must reject offset-count overflow without panicking"
        );
        let program = result.unwrap();
        assert!(program.stats().trap());
        let entry = format!("{:?}", program.entry());
        assert!(
            entry.contains("node_count + 1 overflows u32"),
            "Fix: trap must retain the CSR offset-count overflow diagnostic, got: {entry}"
        );
    }
}
