//! Row-strided queue-to-queue sparse CSR expansion.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::bitset::bitset_words;

use super::CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE;

/// Canonical op id for row-strided queue-to-queue delta CSR expansion.
pub const CSR_QUEUE_DELTA_STRIDED_ENQUEUE_OP_ID: &str =
    "vyre-primitives::graph::csr_queue_delta_strided_enqueue";

/// Fixed lane team assigned to each queued source row in the strided delta path.
pub const CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE: u32 =
    crate::graph::csr_queue_strided::CSR_QUEUE_STRIDED_FORWARD_LANES_PER_SOURCE;

/// Maximum queued source rows assigned one logical lane team in a delta launch.
///
/// Larger queue waves are covered by a grid-stride loop inside the kernel. This
/// keeps resident queue closure on the fused repeated-sequence path without
/// launching worst-wave-sized grids for every half-wave.
pub const CSR_QUEUE_DELTA_STRIDED_MAX_SOURCE_SLOTS_PER_LAUNCH: u32 = 65_536;

/// Queue capacity above which launch compaction is worth grid-striding.
///
/// Medium queues keep one source row per logical lane. That avoids trading
/// empty-lane elision for extra loop work on graph waves that still have enough
/// active rows to occupy the device.
pub const CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY: u32 = 65_536;

/// Queued source rows covered directly by one row-strided delta launch.
#[must_use]
pub const fn csr_queue_delta_strided_source_slots_per_launch(active_queue_capacity: u32) -> u32 {
    if active_queue_capacity == 0 {
        1
    } else if active_queue_capacity > CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY {
        CSR_QUEUE_DELTA_STRIDED_MAX_SOURCE_SLOTS_PER_LAUNCH
    } else {
        active_queue_capacity
    }
}

/// Logical source-row lanes covered directly by one row-strided delta launch.
#[must_use]
pub const fn csr_queue_delta_strided_logical_lanes_per_launch(active_queue_capacity: u32) -> u32 {
    csr_queue_delta_strided_source_slots_per_launch(active_queue_capacity)
        .saturating_mul(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE)
}

/// Dispatch grid for row-strided queue-to-queue delta expansion.
#[must_use]
pub const fn csr_queue_delta_strided_dispatch_grid(active_queue_capacity: u32) -> [u32; 3] {
    let total_lanes = csr_queue_delta_strided_logical_lanes_per_launch(active_queue_capacity);
    let blocks = total_lanes.div_ceil(CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Build a row-strided delta enqueue program for skewed CSR source rows.
///
/// This uses the same resident buffer ABI as
/// [`super::csr_queue_delta_enqueue`], but assigns a fixed lane team to each
/// queued source and stripes that source row across the team. It keeps
/// high-degree IFDS hubs from serializing all edge work behind a single
/// invocation.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_delta_strided_enqueue(
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
            CSR_QUEUE_DELTA_STRIDED_ENQUEUE_OP_ID,
            next_len,
            DataType::U32,
            format!(
                "Fix: csr_queue_delta_strided_enqueue requires node_count > 0 and non-zero queue capacities, got node_count={node_count} active_queue_capacity={active_queue_capacity} next_queue_capacity={next_queue_capacity}."
            ),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let physical_edge_count = edge_count.max(1);
    let edge_offset_count =
        match crate::graph::checked_csr_offset_count(node_count, "csr_queue_delta_strided_enqueue")
        {
            Ok(edge_offset_count) => edge_offset_count,
            Err(error) => {
                return crate::invalid_output_program(
                    CSR_QUEUE_DELTA_STRIDED_ENQUEUE_OP_ID,
                    next_len,
                    DataType::U32,
                    error,
                );
            }
        };
    let logical_lanes_per_launch =
        csr_queue_delta_strided_logical_lanes_per_launch(active_queue_capacity);
    let launch_is_capped = csr_queue_delta_strided_source_slots_per_launch(active_queue_capacity)
        < active_queue_capacity;
    let mut body = vec![
        Node::let_bind("qds_lane", lane),
        Node::let_bind(
            "qds_active_slots",
            Expr::min(
                Expr::load(active_len, Expr::u32(0)),
                Expr::u32(active_queue_capacity),
            ),
        ),
        Node::let_bind(
            "qds_active_lanes",
            Expr::mul(
                Expr::var("qds_active_slots"),
                Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
            ),
        ),
    ];
    if launch_is_capped {
        body.push(Node::let_bind(
            "qds_launch_lanes",
            Expr::u32(logical_lanes_per_launch),
        ));
        body.push(Node::if_then(
            Expr::and(
                Expr::lt(Expr::var("qds_lane"), Expr::var("qds_launch_lanes")),
                Expr::lt(Expr::var("qds_lane"), Expr::var("qds_active_lanes")),
            ),
            vec![
                Node::let_bind(
                    "qds_remaining_lanes",
                    Expr::sub(Expr::var("qds_active_lanes"), Expr::var("qds_lane")),
                ),
                Node::let_bind(
                    "qds_lane_iters",
                    Expr::add(
                        Expr::u32(1),
                        Expr::div(
                            Expr::sub(Expr::var("qds_remaining_lanes"), Expr::u32(1)),
                            Expr::var("qds_launch_lanes"),
                        ),
                    ),
                ),
                Node::loop_for(
                    "qds_lane_iter",
                    Expr::u32(0),
                    Expr::var("qds_lane_iters"),
                    csr_queue_delta_strided_enqueue_lane_body(
                        active_queue,
                        edge_offsets,
                        edge_targets,
                        edge_kind_mask,
                        accumulator,
                        next_queue,
                        next_len,
                        node_count,
                        edge_count,
                        next_queue_capacity,
                        allow_mask,
                        Expr::add(
                            Expr::var("qds_lane"),
                            Expr::mul(Expr::var("qds_lane_iter"), Expr::var("qds_launch_lanes")),
                        ),
                    ),
                ),
            ],
        ));
    } else {
        body.push(Node::if_then(
            Expr::lt(Expr::var("qds_lane"), Expr::var("qds_active_lanes")),
            csr_queue_delta_strided_enqueue_lane_body(
                active_queue,
                edge_offsets,
                edge_targets,
                edge_kind_mask,
                accumulator,
                next_queue,
                next_len,
                node_count,
                edge_count,
                next_queue_capacity,
                allow_mask,
                Expr::var("qds_lane"),
            ),
        ));
    }

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
            generator: Ident::from(CSR_QUEUE_DELTA_STRIDED_ENQUEUE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[allow(clippy::too_many_arguments)]
fn csr_queue_delta_strided_enqueue_lane_body(
    active_queue: &str,
    edge_offsets: &str,
    edge_targets: &str,
    edge_kind_mask: &str,
    accumulator: &str,
    next_queue: &str,
    next_len: &str,
    node_count: u32,
    edge_count: u32,
    next_queue_capacity: u32,
    allow_mask: u32,
    logical_lane: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("qds_logical_lane", logical_lane),
        Node::let_bind(
            "qds_queue_idx",
            Expr::div(
                Expr::var("qds_logical_lane"),
                Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
            ),
        ),
        Node::let_bind(
            "qds_edge_lane",
            Expr::rem(
                Expr::var("qds_logical_lane"),
                Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
            ),
        ),
        Node::let_bind(
            "qds_src",
            Expr::load(active_queue, Expr::var("qds_queue_idx")),
        ),
        Node::if_then(
            Expr::lt(Expr::var("qds_src"), Expr::u32(node_count)),
            vec![
                Node::let_bind(
                    "qds_edge_start",
                    Expr::load(edge_offsets, Expr::var("qds_src")),
                ),
                Node::let_bind(
                    "qds_edge_end",
                    Expr::load(edge_offsets, Expr::add(Expr::var("qds_src"), Expr::u32(1))),
                ),
                Node::let_bind(
                    "qds_degree",
                    Expr::sub(Expr::var("qds_edge_end"), Expr::var("qds_edge_start")),
                ),
                Node::let_bind(
                    "qds_full_iters",
                    Expr::div(
                        Expr::var("qds_degree"),
                        Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
                    ),
                ),
                Node::let_bind(
                    "qds_tail_iter",
                    Expr::select(
                        Expr::ne(
                            Expr::rem(
                                Expr::var("qds_degree"),
                                Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
                            ),
                            Expr::u32(0),
                        ),
                        Expr::u32(1),
                        Expr::u32(0),
                    ),
                ),
                Node::let_bind(
                    "qds_iters",
                    Expr::add(Expr::var("qds_full_iters"), Expr::var("qds_tail_iter")),
                ),
                Node::loop_for(
                    "qds_iter",
                    Expr::u32(0),
                    Expr::var("qds_iters"),
                    vec![
                        Node::let_bind(
                            "qds_edge_offset",
                            Expr::add(
                                Expr::var("qds_edge_lane"),
                                Expr::mul(
                                    Expr::var("qds_iter"),
                                    Expr::u32(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
                                ),
                            ),
                        ),
                        Node::if_then(
                            Expr::lt(Expr::var("qds_edge_offset"), Expr::var("qds_degree")),
                            vec![
                                Node::let_bind(
                                    "qds_e",
                                    Expr::add(
                                        Expr::var("qds_edge_start"),
                                        Expr::var("qds_edge_offset"),
                                    ),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var("qds_e"), Expr::u32(edge_count)),
                                    vec![
                                        Node::let_bind(
                                            "qds_kind",
                                            Expr::load(edge_kind_mask, Expr::var("qds_e")),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::bitand(
                                                    Expr::var("qds_kind"),
                                                    Expr::u32(allow_mask),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            vec![
                                                Node::let_bind(
                                                    "qds_dst",
                                                    Expr::load(edge_targets, Expr::var("qds_e")),
                                                ),
                                                Node::if_then(
                                                    Expr::lt(
                                                        Expr::var("qds_dst"),
                                                        Expr::u32(node_count),
                                                    ),
                                                    vec![
                                                        Node::let_bind(
                                                            "qds_dst_word",
                                                            Expr::shr(
                                                                Expr::var("qds_dst"),
                                                                Expr::u32(5),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "qds_dst_bit",
                                                            Expr::shl(
                                                                Expr::u32(1),
                                                                Expr::bitand(
                                                                    Expr::var("qds_dst"),
                                                                    Expr::u32(31),
                                                                ),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "qds_old",
                                                            Expr::atomic_or(
                                                                accumulator,
                                                                Expr::var("qds_dst_word"),
                                                                Expr::var("qds_dst_bit"),
                                                            ),
                                                        ),
                                                        Node::if_then(
                                                            Expr::eq(
                                                                Expr::bitand(
                                                                    Expr::var("qds_old"),
                                                                    Expr::var("qds_dst_bit"),
                                                                ),
                                                                Expr::u32(0),
                                                            ),
                                                            vec![
                                                                Node::let_bind(
                                                                    "qds_slot",
                                                                    Expr::atomic_add(
                                                                        next_len,
                                                                        Expr::u32(0),
                                                                        Expr::u32(1),
                                                                    ),
                                                                ),
                                                                Node::if_then(
                                                                    Expr::lt(
                                                                        Expr::var("qds_slot"),
                                                                        Expr::u32(
                                                                            next_queue_capacity,
                                                                        ),
                                                                    ),
                                                                    vec![Node::store(
                                                                        next_queue,
                                                                        Expr::var("qds_slot"),
                                                                        Expr::var("qds_dst"),
                                                                    )],
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
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_strided_program_keeps_delta_queue_abi_and_expands_grid() {
        let program = csr_queue_delta_strided_enqueue(
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
        assert_eq!(program.buffers[0].name.as_ref(), "active_queue");
        assert_eq!(program.buffers[0].count, 8);
        assert_eq!(program.buffers[6].name.as_ref(), "next_queue");
        assert_eq!(program.buffers[6].count, 16);
        assert_eq!(
            csr_queue_delta_strided_dispatch_grid(8),
            [
                (8 * CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE)
                    .div_ceil(CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE[0]),
                1,
                1,
            ]
        );
        let program_debug = format!("{:?}", program.entry);
        assert!(!program_debug.contains("qds_lane_iter"));
        assert!(program_debug.contains("qds_logical_lane"));

        let capped_program = csr_queue_delta_strided_enqueue(
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
            CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY + 1,
            16,
            1,
        );
        let capped_debug = format!("{:?}", capped_program.entry);
        assert!(capped_debug.contains("qds_lane_iter"));
        assert!(capped_debug.contains("qds_logical_lane"));
    }

    #[test]
    fn strided_delta_rejects_offset_count_overflow_without_panic() {
        let result = std::panic::catch_unwind(|| {
            csr_queue_delta_strided_enqueue(
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
            "CSR queue delta strided builder must reject offset-count overflow without panicking"
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
    fn generated_strided_delta_launch_grid_caps_capacity_and_preserves_coverage() {
        const CASES: u32 = 20_000;
        let mut capped_cases = 0_u32;

        for case in 0..CASES {
            let capacity = mix32(case ^ 0x5D17_1D3A);
            let source_slots = csr_queue_delta_strided_source_slots_per_launch(capacity);
            let logical_lanes = csr_queue_delta_strided_logical_lanes_per_launch(capacity);
            let grid = csr_queue_delta_strided_dispatch_grid(capacity);
            let launched_lanes = grid[0].saturating_mul(CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE[0]);

            assert!(source_slots > 0, "source slots case {case}");
            if capacity == 0 {
                assert_eq!(source_slots, 1, "zero capacity source slots case {case}");
            } else if capacity > CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY {
                assert_eq!(
                    source_slots, CSR_QUEUE_DELTA_STRIDED_MAX_SOURCE_SLOTS_PER_LAUNCH,
                    "source slot cap case {case}"
                );
            } else {
                assert_eq!(
                    source_slots, capacity,
                    "medium queue source slots case {case}"
                );
            }
            assert_eq!(
                logical_lanes,
                source_slots.saturating_mul(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE),
                "logical lanes case {case}"
            );
            assert!(
                launched_lanes >= logical_lanes,
                "grid underlaunch case {case}"
            );
            assert!(
                launched_lanes < logical_lanes + CSR_QUEUE_DELTA_ENQUEUE_WORKGROUP_SIZE[0],
                "grid overlaunch case {case}"
            );
            if capacity > CSR_QUEUE_DELTA_STRIDED_CAPPED_LAUNCH_MIN_CAPACITY {
                capped_cases += 1;
                let active_lanes =
                    capacity.saturating_mul(CSR_QUEUE_DELTA_STRIDED_LANES_PER_SOURCE);
                let iterations = 1 + active_lanes.saturating_sub(1) / logical_lanes;
                assert!(iterations > 1, "grid-stride iterations case {case}");
            }
        }

        assert!(capped_cases > CASES * 9 / 10);
    }

    const fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7FEB_352D);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846C_A68B);
        value ^ (value >> 16)
    }
}
