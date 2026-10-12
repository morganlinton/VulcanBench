//! Device-side active-frontier queues for sparse CSR expansion.
//!
//! Low-density dataflow frontiers should not launch one useful lane and
//! thousands of empty source-node lanes. This module splits sparse expansion
//! into two GPU-resident primitives:
//!
//! 1. `frontier_to_queue` compacts active source-node ids from a packed bitset
//!    into an active queue with an atomic device-side length. The legacy variant
//!    uses one cooperative workgroup and a strided scan so the queue length can
//!    be initialized inside the same dispatch without an unsupported grid barrier.
//!    The packed-word variants let resident traversal pipelines initialize
//!    `queue_len` once, then reserve one queue slice per nonzero frontier word.
//! 2. `csr_queue_forward_traverse` consumes only queued sources and expands
//!    their CSR rows into `frontier_out`.
//!
//! The queue length can exceed queue capacity to expose overflow pressure; the
//! traversal consumes only the first `queue_capacity` entries.

use std::fmt;
use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

use crate::bitset::bitset_words;

/// Canonical op id for bitset-to-queue compaction.
pub const FRONTIER_TO_QUEUE_OP_ID: &str = "vyre-primitives::graph::frontier_to_queue";
/// Canonical op id for multi-workgroup bitset-to-queue compaction.
pub const FRONTIER_TO_QUEUE_PARALLEL_OP_ID: &str =
    "vyre-primitives::graph::frontier_to_queue_parallel";
/// Canonical op id for word-level multi-workgroup bitset-to-queue compaction.
pub const FRONTIER_WORDS_TO_QUEUE_PARALLEL_OP_ID: &str =
    "vyre-primitives::graph::frontier_words_to_queue_parallel";
/// Canonical op id for word-level compaction that also clears an output bitset.
pub const FRONTIER_WORDS_TO_QUEUE_CLEAR_OUT_PARALLEL_OP_ID: &str =
    "vyre-primitives::graph::frontier_words_to_queue_clear_out_parallel";
/// Canonical op id for packed-frontier word popcount prefix-scan pass A.
pub const FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID: &str =
    "vyre-primitives::graph::frontier_word_counts_scan_pass_a";
/// Canonical op id for deterministic packed-frontier block-prefix scatter.
pub const FRONTIER_WORD_BLOCK_PREFIX_TO_QUEUE_PARALLEL_OP_ID: &str =
    "vyre-primitives::graph::frontier_word_block_prefix_to_queue_parallel";
/// Canonical op id for in-place packed-frontier block-offset scan.
pub const FRONTIER_WORD_BLOCK_OFFSETS_IN_PLACE_OP_ID: &str =
    "vyre-primitives::graph::frontier_word_block_offsets_in_place";
/// Canonical op id for packed-frontier scatter with precomputed block offsets.
pub const FRONTIER_WORD_BLOCK_OFFSETS_TO_QUEUE_PARALLEL_OP_ID: &str =
    "vyre-primitives::graph::frontier_word_block_offsets_to_queue_parallel";
/// Workgroup lanes used by the deterministic packed-frontier scan path.
pub const FRONTIER_WORD_SCAN_BLOCK_LANES: u32 = 1024;
/// Canonical op id for device-side queue length initialization.
pub const FRONTIER_QUEUE_LEN_INIT_OP_ID: &str = "vyre-primitives::graph::frontier_queue_len_init";
/// Canonical op id for queue-driven CSR expansion.
pub const CSR_QUEUE_FORWARD_OP_ID: &str = "vyre-primitives::graph::csr_queue_forward_traverse";

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrontierQueueSizingError {
    message: String,
}

impl FrontierQueueSizingError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for FrontierQueueSizingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

fn checked_frontier_u32_product(
    lhs: u32,
    rhs: u32,
    context: &str,
) -> Result<u32, FrontierQueueSizingError> {
    lhs.checked_mul(rhs).ok_or_else(|| {
        FrontierQueueSizingError::new(format!(
            "Fix: {context} overflows u32 word count for lhs={lhs} rhs={rhs}. Shard the frontier queue before GPU dispatch."
        ))
    })
}

fn try_u32_byte_range(words: u32, context: &str) -> Result<usize, FrontierQueueSizingError> {
    try_u32_byte_range_with_word_size(words, std::mem::size_of::<u32>(), context)
}

fn try_u32_byte_range_with_word_size(
    words: u32,
    word_size: usize,
    context: &str,
) -> Result<usize, FrontierQueueSizingError> {
    let count = usize::try_from(words).map_err(|_| {
        FrontierQueueSizingError::new(format!(
            "Fix: {context} words={words} cannot fit usize on this target. Shard the frontier queue before GPU dispatch."
        ))
    })?;
    count.checked_mul(word_size).ok_or_else(|| {
        FrontierQueueSizingError::new(format!(
            "Fix: {context} words={words} word_size={word_size} overflows output byte range. Shard the frontier queue before GPU dispatch."
        ))
    })
}

fn invalid_frontier_queue_sizing_program(
    op_id: &'static str,
    output: &str,
    error: FrontierQueueSizingError,
) -> Program {
    crate::invalid_output_program(op_id, output, DataType::U32, error.to_string())
}

/// Build a GPU program that initializes the active queue length scalar.
///
/// This replaces a per-wave host-to-device zero upload in resident sparse
/// traversal pipelines. Keeping initialization as a separate single-lane
/// device step avoids the global-synchronization race that would occur if the
/// multi-workgroup compaction kernel tried to clear and atomically increment
/// the same scalar.
#[must_use]
pub fn frontier_queue_len_init(queue_len: &str) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(queue_len, 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_QUEUE_LEN_INIT_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::store(queue_len, Expr::u32(0), Expr::u32(0))]),
        }],
    )
}

/// Build a GPU program that appends every active frontier node to a queue.
///
/// This is intentionally a single-workgroup cooperative scan: lane 0 clears
/// `queue_len`, a workgroup barrier orders that clear, then all lanes walk
/// `node_count` in 256-wide strides. Sparse queue traversal is selected only
/// for low-density frontiers, so avoiding a separate queue-length init launch
/// is more valuable than spreading this scan across every SM.
#[must_use]
pub fn frontier_to_queue(
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    if node_count == 0 || queue_capacity == 0 {
        return crate::invalid_output_program(
            FRONTIER_TO_QUEUE_OP_ID,
            queue_len,
            DataType::U32,
            format!(
                "Fix: frontier_to_queue requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }
    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let scan_iters = node_count.div_ceil(256).max(1);
    let body = vec![
        Node::let_bind("q_lane", lane.clone()),
        Node::if_then(
            Expr::eq(Expr::var("q_lane"), Expr::u32(0)),
            vec![Node::store(queue_len, Expr::u32(0), Expr::u32(0))],
        ),
        Node::barrier_with_ordering(MemoryOrdering::SeqCst),
        Node::loop_for(
            "q_iter",
            Expr::u32(0),
            Expr::u32(scan_iters),
            vec![
                Node::let_bind(
                    "q_src",
                    Expr::add(
                        Expr::mul(Expr::var("q_iter"), Expr::u32(256)),
                        Expr::var("q_lane"),
                    ),
                ),
                Node::if_then(
                    Expr::lt(Expr::var("q_src"), Expr::u32(node_count)),
                    vec![
                        Node::let_bind("q_word_idx", Expr::shr(Expr::var("q_src"), Expr::u32(5))),
                        Node::let_bind(
                            "q_bit_mask",
                            Expr::shl(
                                Expr::u32(1),
                                Expr::bitand(Expr::var("q_src"), Expr::u32(31)),
                            ),
                        ),
                        Node::let_bind(
                            "q_src_word",
                            Expr::load(frontier_in, Expr::var("q_word_idx")),
                        ),
                        Node::if_then(
                            Expr::ne(
                                Expr::bitand(Expr::var("q_src_word"), Expr::var("q_bit_mask")),
                                Expr::u32(0),
                            ),
                            vec![
                                Node::let_bind(
                                    "q_slot",
                                    Expr::atomic_add(queue_len, Expr::u32(0), Expr::u32(1)),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var("q_slot"), Expr::u32(queue_capacity)),
                                    vec![Node::store(
                                        active_queue,
                                        Expr::var("q_slot"),
                                        Expr::var("q_src"),
                                    )],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(active_queue, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(queue_capacity),
            BufferDecl::storage(queue_len, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_TO_QUEUE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build a multi-workgroup GPU program that appends active frontier nodes to a queue.
///
/// The caller must clear `queue_len` before dispatch, for example with
/// `frontier_queue_len_init` or a fused resident reset step. Unlike
/// `frontier_to_queue`, this variant maps one lane to one source node and is
/// the right materializer for large packed frontiers.
#[must_use]
pub fn frontier_to_queue_parallel(
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    if node_count == 0 || queue_capacity == 0 {
        return crate::invalid_output_program(
            FRONTIER_TO_QUEUE_PARALLEL_OP_ID,
            queue_len,
            DataType::U32,
            format!(
                "Fix: frontier_to_queue_parallel requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }
    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let body = vec![
        Node::let_bind("qp_src", lane),
        Node::if_then(
            Expr::lt(Expr::var("qp_src"), Expr::u32(node_count)),
            vec![
                Node::let_bind("qp_word_idx", Expr::shr(Expr::var("qp_src"), Expr::u32(5))),
                Node::let_bind(
                    "qp_bit_mask",
                    Expr::shl(
                        Expr::u32(1),
                        Expr::bitand(Expr::var("qp_src"), Expr::u32(31)),
                    ),
                ),
                Node::let_bind(
                    "qp_src_word",
                    Expr::load(frontier_in, Expr::var("qp_word_idx")),
                ),
                Node::if_then(
                    Expr::ne(
                        Expr::bitand(Expr::var("qp_src_word"), Expr::var("qp_bit_mask")),
                        Expr::u32(0),
                    ),
                    vec![
                        Node::let_bind(
                            "qp_slot",
                            Expr::atomic_add(queue_len, Expr::u32(0), Expr::u32(1)),
                        ),
                        Node::if_then(
                            Expr::lt(Expr::var("qp_slot"), Expr::u32(queue_capacity)),
                            vec![Node::store(
                                active_queue,
                                Expr::var("qp_slot"),
                                Expr::var("qp_src"),
                            )],
                        ),
                    ],
                ),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(active_queue, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(queue_capacity),
            BufferDecl::storage(queue_len, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_TO_QUEUE_PARALLEL_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build a multi-workgroup GPU program that appends active frontier nodes to a
/// queue by scanning packed frontier words.
///
/// The caller must clear `queue_len` before dispatch. This variant maps one
/// lane to one packed u32 frontier word and performs one atomic queue
/// reservation per nonzero word, so sparse packed frontiers launch 32x fewer
/// lanes than `frontier_to_queue_parallel` and avoid per-active-bit atomics.
#[must_use]
pub fn frontier_words_to_queue_parallel(
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    frontier_words_to_queue_parallel_program(
        FRONTIER_WORDS_TO_QUEUE_PARALLEL_OP_ID,
        frontier_in,
        active_queue,
        queue_len,
        None,
        node_count,
        queue_capacity,
    )
}

/// Build a packed-frontier queue materializer that also clears `frontier_out`.
///
/// The caller must still clear `queue_len` before dispatch. Folding the output
/// clear into this packed-word scan removes a separate full-frontier reset pass
/// from resident sparse traversal sequences without changing the queue ABI.
#[must_use]
pub fn frontier_words_to_queue_clear_out_parallel(
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    frontier_out: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    frontier_words_to_queue_parallel_program(
        FRONTIER_WORDS_TO_QUEUE_CLEAR_OUT_PARALLEL_OP_ID,
        frontier_in,
        active_queue,
        queue_len,
        Some(frontier_out),
        node_count,
        queue_capacity,
    )
}

fn frontier_words_to_queue_parallel_program(
    op_id: &'static str,
    frontier_in: &str,
    active_queue: &str,
    queue_len: &str,
    frontier_out_to_clear: Option<&str>,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    if node_count == 0 || queue_capacity == 0 {
        return crate::invalid_output_program(
            op_id,
            queue_len,
            DataType::U32,
            format!(
                "Fix: {op_id} requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }
    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let tail_bits = node_count & 31;
    let tail_mask = if tail_bits == 0 {
        u32::MAX
    } else {
        (1_u32 << tail_bits) - 1
    };
    let mut word_body = vec![
        Node::let_bind(
            "qw_src_base",
            Expr::mul(Expr::var("qw_word_idx"), Expr::u32(32)),
        ),
        Node::let_bind(
            "qw_remaining",
            Expr::load(frontier_in, Expr::var("qw_word_idx")),
        ),
    ];
    if tail_bits != 0 {
        word_body.push(Node::if_then(
            Expr::eq(Expr::var("qw_word_idx"), Expr::u32(words - 1)),
            vec![Node::assign(
                "qw_remaining",
                Expr::bitand(Expr::var("qw_remaining"), Expr::u32(tail_mask)),
            )],
        ));
    }
    word_body.push(Node::if_then(
        Expr::ne(Expr::var("qw_remaining"), Expr::u32(0)),
        vec![
            Node::let_bind("qw_active_bits", Expr::popcount(Expr::var("qw_remaining"))),
            Node::let_bind(
                "qw_base_slot",
                Expr::atomic_add(queue_len, Expr::u32(0), Expr::var("qw_active_bits")),
            ),
            Node::loop_for(
                "qw_rank",
                Expr::u32(0),
                Expr::var("qw_active_bits"),
                vec![
                    Node::let_bind("qw_bit", Expr::ctz(Expr::var("qw_remaining"))),
                    Node::let_bind(
                        "qw_src",
                        Expr::add(Expr::var("qw_src_base"), Expr::var("qw_bit")),
                    ),
                    Node::let_bind(
                        "qw_slot",
                        Expr::add(Expr::var("qw_base_slot"), Expr::var("qw_rank")),
                    ),
                    Node::if_then(
                        Expr::lt(Expr::var("qw_slot"), Expr::u32(queue_capacity)),
                        vec![Node::store(
                            active_queue,
                            Expr::var("qw_slot"),
                            Expr::var("qw_src"),
                        )],
                    ),
                    Node::assign(
                        "qw_remaining",
                        Expr::bitand(
                            Expr::var("qw_remaining"),
                            Expr::sub(Expr::var("qw_remaining"), Expr::u32(1)),
                        ),
                    ),
                ],
            ),
        ],
    ));
    if let Some(frontier_out) = frontier_out_to_clear {
        word_body.insert(
            0,
            Node::store(frontier_out, Expr::var("qw_word_idx"), Expr::u32(0)),
        );
    }

    let body = vec![
        Node::let_bind("qw_word_idx", lane),
        Node::if_then(
            Expr::lt(Expr::var("qw_word_idx"), Expr::u32(words)),
            word_body,
        ),
    ];
    let mut buffers = vec![
        BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(words),
        BufferDecl::storage(active_queue, 1, BufferAccess::ReadWrite, DataType::U32)
            .with_count(queue_capacity),
        BufferDecl::storage(queue_len, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
    ];
    if let Some(frontier_out) = frontier_out_to_clear {
        buffers.push(
            BufferDecl::storage(frontier_out, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        );
    }
    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build Pass A for deterministic packed-frontier queue materialization.
///
/// Each workgroup scans one block of packed frontier words. Lane `L` in block
/// `B` computes the in-range popcount for word `B*1024 + L`, then participates
/// in a local inclusive Hillis-Steele scan. The program writes one per-word
/// inclusive count into `word_partials` and one per-block total into
/// `block_totals`.
#[must_use]
pub fn frontier_word_counts_scan_pass_a(
    frontier_in: &str,
    word_partials: &str,
    block_totals: &str,
    node_count: u32,
) -> Program {
    if node_count == 0 {
        return crate::invalid_output_program(
            FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID,
            word_partials,
            DataType::U32,
            "Fix: frontier_word_counts_scan_pass_a requires node_count > 0.".to_string(),
        );
    }
    let words = bitset_words(node_count);
    let num_blocks = words.div_ceil(FRONTIER_WORD_SCAN_BLOCK_LANES).max(1);
    let total_partials = match checked_frontier_u32_product(
        num_blocks,
        FRONTIER_WORD_SCAN_BLOCK_LANES,
        "frontier_word_counts_scan_pass_a partial word count",
    ) {
        Ok(total_partials) => total_partials,
        Err(error) => {
            return invalid_frontier_queue_sizing_program(
                FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID,
                word_partials,
                error,
            );
        }
    };
    let partial_bytes =
        match try_u32_byte_range(total_partials, "frontier_word_counts_scan_pass_a partials") {
            Ok(partial_bytes) => partial_bytes,
            Err(error) => {
                return invalid_frontier_queue_sizing_program(
                    FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID,
                    word_partials,
                    error,
                );
            }
        };
    let block_total_bytes =
        match try_u32_byte_range(num_blocks, "frontier_word_counts_scan_pass_a block totals") {
            Ok(block_total_bytes) => block_total_bytes,
            Err(error) => {
                return invalid_frontier_queue_sizing_program(
                    FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID,
                    block_totals,
                    error,
                );
            }
        };
    let tail_bits = node_count & 31;
    let tail_mask = if tail_bits == 0 {
        u32::MAX
    } else {
        (1_u32 << tail_bits) - 1
    };

    let lane = Expr::var("fwcs_lane");
    let block = Expr::var("fwcs_block");
    let global = Expr::var("fwcs_global");
    let scratch_a = format!("__{word_partials}_fwcs_scratch_a");
    let scratch_b = format!("__{word_partials}_fwcs_scratch_b");

    let mut body = Vec::new();
    body.push(Node::let_bind("fwcs_lane", Expr::LocalId { axis: 0 }));
    body.push(Node::let_bind("fwcs_block", Expr::WorkgroupId { axis: 0 }));
    body.push(Node::let_bind(
        "fwcs_global",
        Expr::add(
            Expr::mul(block.clone(), Expr::u32(FRONTIER_WORD_SCAN_BLOCK_LANES)),
            lane.clone(),
        ),
    ));
    body.push(Node::store(&scratch_a, lane.clone(), Expr::u32(0)));
    let mut load_word = vec![Node::let_bind(
        "fwcs_word",
        Expr::load(frontier_in, global.clone()),
    )];
    if tail_bits != 0 {
        load_word.push(Node::if_then(
            Expr::eq(global.clone(), Expr::u32(words - 1)),
            vec![Node::assign(
                "fwcs_word",
                Expr::bitand(Expr::var("fwcs_word"), Expr::u32(tail_mask)),
            )],
        ));
    }
    load_word.push(Node::store(
        &scratch_a,
        lane.clone(),
        Expr::popcount(Expr::var("fwcs_word")),
    ));
    body.push(Node::if_then(
        Expr::lt(global.clone(), Expr::u32(words)),
        load_word,
    ));
    body.push(Node::Barrier {
        ordering: MemoryOrdering::SeqCst,
    });

    let mut stride = 1_u32;
    while stride < FRONTIER_WORD_SCAN_BLOCK_LANES {
        body.push(Node::store(
            &scratch_b,
            lane.clone(),
            Expr::load(&scratch_a, lane.clone()),
        ));
        let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                &scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(&scratch_a, lane.clone()),
                    Expr::load(&scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            &scratch_a,
            lane.clone(),
            Expr::load(&scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }

    body.push(Node::if_then(
        Expr::lt(global.clone(), Expr::u32(words)),
        vec![Node::store(
            word_partials,
            global.clone(),
            Expr::load(&scratch_a, lane.clone()),
        )],
    ));
    body.push(Node::if_then(
        Expr::eq(lane.clone(), Expr::u32(FRONTIER_WORD_SCAN_BLOCK_LANES - 1)),
        vec![Node::store(
            block_totals,
            block.clone(),
            Expr::load(&scratch_a, lane.clone()),
        )],
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::output(word_partials, 1, DataType::U32)
                .with_count(total_partials)
                .with_output_byte_range(0..partial_bytes),
            BufferDecl::storage(block_totals, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_blocks)
                .with_pipeline_live_out(true)
                .with_output_byte_range(0..block_total_bytes),
            BufferDecl::workgroup(&scratch_a, FRONTIER_WORD_SCAN_BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(&scratch_b, FRONTIER_WORD_SCAN_BLOCK_LANES, DataType::U32),
        ],
        [FRONTIER_WORD_SCAN_BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_WORD_COUNTS_SCAN_PASS_A_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Convert per-block active counts into exclusive per-block queue offsets.
///
/// The conversion is in-place: after this program runs, `block_totals[B]`
/// contains the number of active nodes in all prior blocks. For up to 1024
/// blocks this uses one guarded workgroup scan; beyond that it falls back to a
/// single-lane linear scan over block metadata, which is still O(blocks)
/// instead of the old O(words * blocks) scatter-side prefix work.
#[must_use]
pub fn frontier_word_block_offsets_in_place(block_totals: &str, node_count: u32) -> Program {
    if node_count == 0 {
        return crate::invalid_output_program(
            FRONTIER_WORD_BLOCK_OFFSETS_IN_PLACE_OP_ID,
            block_totals,
            DataType::U32,
            "Fix: frontier_word_block_offsets_in_place requires node_count > 0.".to_string(),
        );
    }
    let words = bitset_words(node_count);
    let num_blocks = words.div_ceil(FRONTIER_WORD_SCAN_BLOCK_LANES).max(1);
    let block_total_bytes = match try_u32_byte_range(
        num_blocks,
        "frontier_word_block_offsets_in_place block totals",
    ) {
        Ok(block_total_bytes) => block_total_bytes,
        Err(error) => {
            return invalid_frontier_queue_sizing_program(
                FRONTIER_WORD_BLOCK_OFFSETS_IN_PLACE_OP_ID,
                block_totals,
                error,
            );
        }
    };
    if num_blocks <= FRONTIER_WORD_SCAN_BLOCK_LANES {
        return frontier_word_block_offsets_single_workgroup(
            block_totals,
            num_blocks,
            block_total_bytes,
        );
    }
    frontier_word_block_offsets_single_lane(block_totals, num_blocks, block_total_bytes)
}

fn frontier_word_block_offsets_single_workgroup(
    block_totals: &str,
    num_blocks: u32,
    block_total_bytes: usize,
) -> Program {
    let lane = Expr::var("fwbo_lane");
    let scratch_a = format!("__{block_totals}_fwbo_scratch_a");
    let scratch_b = format!("__{block_totals}_fwbo_scratch_b");
    let mut body = Vec::new();
    body.push(Node::let_bind("fwbo_lane", Expr::LocalId { axis: 0 }));
    body.push(Node::store(&scratch_a, lane.clone(), Expr::u32(0)));
    body.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(num_blocks)),
        vec![Node::store(
            &scratch_a,
            lane.clone(),
            Expr::load(block_totals, lane.clone()),
        )],
    ));
    body.push(Node::Barrier {
        ordering: MemoryOrdering::SeqCst,
    });

    let mut stride = 1_u32;
    while stride < FRONTIER_WORD_SCAN_BLOCK_LANES {
        body.push(Node::store(
            &scratch_b,
            lane.clone(),
            Expr::load(&scratch_a, lane.clone()),
        ));
        let previous_lane = Expr::add(lane.clone(), Expr::u32(0u32.wrapping_sub(stride)));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                &scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(&scratch_a, lane.clone()),
                    Expr::load(&scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            &scratch_a,
            lane.clone(),
            Expr::load(&scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }

    body.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(num_blocks)),
        vec![
            Node::if_then(
                Expr::eq(lane.clone(), Expr::u32(0)),
                vec![Node::store(block_totals, lane.clone(), Expr::u32(0))],
            ),
            Node::if_then(
                Expr::ne(lane.clone(), Expr::u32(0)),
                vec![Node::store(
                    block_totals,
                    lane.clone(),
                    Expr::load(&scratch_a, Expr::sub(lane.clone(), Expr::u32(1))),
                )],
            ),
        ],
    ));

    Program::wrapped(
        vec![
            BufferDecl::storage(block_totals, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_blocks)
                .with_pipeline_live_out(true)
                .with_output_byte_range(0..block_total_bytes),
            BufferDecl::workgroup(&scratch_a, FRONTIER_WORD_SCAN_BLOCK_LANES, DataType::U32),
            BufferDecl::workgroup(&scratch_b, FRONTIER_WORD_SCAN_BLOCK_LANES, DataType::U32),
        ],
        [FRONTIER_WORD_SCAN_BLOCK_LANES, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_WORD_BLOCK_OFFSETS_IN_PLACE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

fn frontier_word_block_offsets_single_lane(
    block_totals: &str,
    num_blocks: u32,
    block_total_bytes: usize,
) -> Program {
    let body = vec![
        Node::let_bind("fwbo_running", Expr::u32(0)),
        Node::loop_for(
            "fwbo_block",
            Expr::u32(0),
            Expr::u32(num_blocks),
            vec![
                Node::let_bind(
                    "fwbo_total",
                    Expr::load(block_totals, Expr::var("fwbo_block")),
                ),
                Node::store(
                    block_totals,
                    Expr::var("fwbo_block"),
                    Expr::var("fwbo_running"),
                ),
                Node::assign(
                    "fwbo_running",
                    Expr::add(Expr::var("fwbo_running"), Expr::var("fwbo_total")),
                ),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(block_totals, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_blocks)
                .with_pipeline_live_out(true)
                .with_output_byte_range(0..block_total_bytes),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(FRONTIER_WORD_BLOCK_OFFSETS_IN_PLACE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build the deterministic scatter pass for packed-frontier queue materialization.
///
/// `word_partials` must come from [`frontier_word_counts_scan_pass_a`], and
/// `block_totals` must be the block-total output from that same pass. The
/// scatter computes the tiny block prefix locally, preserving source-node order
/// without an additional block-scan dispatch. It writes `queue_len` as the full
/// in-range active-node count even when the bounded queue truncates the
/// materialized entries.
#[must_use]
pub fn frontier_word_block_prefix_to_queue_parallel(
    frontier_in: &str,
    word_partials: &str,
    block_totals: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    frontier_word_queue_scatter_program(
        FRONTIER_WORD_BLOCK_PREFIX_TO_QUEUE_PARALLEL_OP_ID,
        FrontierWordBlockOffsetSource::SumPreviousTotals { block_totals },
        frontier_in,
        word_partials,
        active_queue,
        queue_len,
        node_count,
        queue_capacity,
    )
}

/// Build the deterministic scatter pass using precomputed per-block offsets.
///
/// `block_offsets` must be the in-place output of
/// [`frontier_word_block_offsets_in_place`]. This keeps scatter work O(words)
/// for multi-block frontiers by replacing the per-word previous-block loop with
/// one block-offset load.
#[must_use]
pub fn frontier_word_block_offsets_to_queue_parallel(
    frontier_in: &str,
    word_partials: &str,
    block_offsets: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    frontier_word_queue_scatter_program(
        FRONTIER_WORD_BLOCK_OFFSETS_TO_QUEUE_PARALLEL_OP_ID,
        FrontierWordBlockOffsetSource::PrecomputedOffsets { block_offsets },
        frontier_in,
        word_partials,
        active_queue,
        queue_len,
        node_count,
        queue_capacity,
    )
}

#[derive(Clone, Copy)]
enum FrontierWordBlockOffsetSource<'a> {
    SumPreviousTotals { block_totals: &'a str },
    PrecomputedOffsets { block_offsets: &'a str },
}

impl FrontierWordBlockOffsetSource<'_> {
    fn buffer_name(&self) -> &str {
        match self {
            FrontierWordBlockOffsetSource::SumPreviousTotals { block_totals } => block_totals,
            FrontierWordBlockOffsetSource::PrecomputedOffsets { block_offsets } => block_offsets,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn frontier_word_queue_scatter_program(
    op_id: &'static str,
    block_offset_source: FrontierWordBlockOffsetSource<'_>,
    frontier_in: &str,
    word_partials: &str,
    active_queue: &str,
    queue_len: &str,
    node_count: u32,
    queue_capacity: u32,
) -> Program {
    if node_count == 0 || queue_capacity == 0 {
        return crate::invalid_output_program(
            op_id,
            queue_len,
            DataType::U32,
            format!(
                "Fix: {op_id} requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }
    let words = bitset_words(node_count);
    let num_blocks = words.div_ceil(FRONTIER_WORD_SCAN_BLOCK_LANES).max(1);
    let total_partials =
        match checked_frontier_u32_product(num_blocks, FRONTIER_WORD_SCAN_BLOCK_LANES, op_id) {
            Ok(total_partials) => total_partials,
            Err(error) => return invalid_frontier_queue_sizing_program(op_id, queue_len, error),
        };
    let tail_bits = node_count & 31;
    let tail_mask = if tail_bits == 0 {
        u32::MAX
    } else {
        (1_u32 << tail_bits) - 1
    };
    let lane = Expr::InvocationId { axis: 0 };
    let mut block_offset_body = Vec::new();
    match block_offset_source {
        FrontierWordBlockOffsetSource::SumPreviousTotals { block_totals } => {
            block_offset_body.push(Node::let_bind("fwq_block_offset", Expr::u32(0)));
            block_offset_body.push(Node::loop_for(
                "fwq_prev_block",
                Expr::u32(0),
                Expr::var("fwq_block"),
                vec![Node::assign(
                    "fwq_block_offset",
                    Expr::add(
                        Expr::var("fwq_block_offset"),
                        Expr::load(block_totals, Expr::var("fwq_prev_block")),
                    ),
                )],
            ));
        }
        FrontierWordBlockOffsetSource::PrecomputedOffsets { block_offsets } => {
            block_offset_body.push(Node::let_bind(
                "fwq_block_offset",
                Expr::load(block_offsets, Expr::var("fwq_block")),
            ));
        }
    }
    let mut word_body = vec![
        Node::let_bind(
            "fwq_src_base",
            Expr::mul(Expr::var("fwq_word_idx"), Expr::u32(32)),
        ),
        Node::let_bind(
            "fwq_block",
            Expr::div(
                Expr::var("fwq_word_idx"),
                Expr::u32(FRONTIER_WORD_SCAN_BLOCK_LANES),
            ),
        ),
        Node::let_bind(
            "fwq_word",
            Expr::load(frontier_in, Expr::var("fwq_word_idx")),
        ),
    ];
    if tail_bits != 0 {
        word_body.push(Node::if_then(
            Expr::eq(Expr::var("fwq_word_idx"), Expr::u32(words - 1)),
            vec![Node::assign(
                "fwq_word",
                Expr::bitand(Expr::var("fwq_word"), Expr::u32(tail_mask)),
            )],
        ));
    }
    word_body.extend(block_offset_body);
    word_body.extend([
        Node::let_bind("fwq_active_bits", Expr::popcount(Expr::var("fwq_word"))),
        Node::let_bind(
            "fwq_end",
            Expr::add(
                Expr::load(word_partials, Expr::var("fwq_word_idx")),
                Expr::var("fwq_block_offset"),
            ),
        ),
        Node::let_bind(
            "fwq_start",
            Expr::sub(Expr::var("fwq_end"), Expr::var("fwq_active_bits")),
        ),
        Node::let_bind("fwq_remaining", Expr::var("fwq_word")),
        Node::loop_for(
            "fwq_rank",
            Expr::u32(0),
            Expr::var("fwq_active_bits"),
            vec![
                Node::let_bind("fwq_bit", Expr::ctz(Expr::var("fwq_remaining"))),
                Node::let_bind(
                    "fwq_src",
                    Expr::add(Expr::var("fwq_src_base"), Expr::var("fwq_bit")),
                ),
                Node::let_bind(
                    "fwq_slot",
                    Expr::add(Expr::var("fwq_start"), Expr::var("fwq_rank")),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::lt(Expr::var("fwq_slot"), Expr::u32(queue_capacity)),
                        Expr::lt(Expr::var("fwq_src"), Expr::u32(node_count)),
                    ),
                    vec![Node::store(
                        active_queue,
                        Expr::var("fwq_slot"),
                        Expr::var("fwq_src"),
                    )],
                ),
                Node::assign(
                    "fwq_remaining",
                    Expr::bitand(
                        Expr::var("fwq_remaining"),
                        Expr::sub(Expr::var("fwq_remaining"), Expr::u32(1)),
                    ),
                ),
            ],
        ),
        Node::if_then(
            Expr::eq(Expr::var("fwq_word_idx"), Expr::u32(words - 1)),
            vec![Node::store(queue_len, Expr::u32(0), Expr::var("fwq_end"))],
        ),
    ]);

    let body = vec![
        Node::let_bind("fwq_word_idx", lane),
        Node::if_then(
            Expr::lt(Expr::var("fwq_word_idx"), Expr::u32(words)),
            word_body,
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(word_partials, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(total_partials),
            BufferDecl::storage(
                block_offset_source.buffer_name(),
                2,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(num_blocks),
            BufferDecl::storage(active_queue, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(queue_capacity),
            BufferDecl::storage(queue_len, 4, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build a GPU program that expands only queued CSR source rows.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn csr_queue_forward_traverse(
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
            CSR_QUEUE_FORWARD_OP_ID,
            frontier_out,
            DataType::U32,
            format!(
                "Fix: csr_queue_forward_traverse requires node_count > 0 and queue_capacity > 0, got node_count={node_count} queue_capacity={queue_capacity}."
            ),
        );
    }
    let lane = Expr::InvocationId { axis: 0 };
    let words = bitset_words(node_count);
    let physical_edge_count = edge_count.max(1);
    let edge_offset_count = match node_count.checked_add(1) {
        Some(edge_offset_count) => edge_offset_count,
        None => {
            return crate::invalid_output_program(
                CSR_QUEUE_FORWARD_OP_ID,
                frontier_out,
                DataType::U32,
                format!(
                    "Fix: csr_queue_forward_traverse node_count + 1 overflows u32 for node_count={node_count}. Shard the CSR graph before GPU dispatch."
                ),
            );
        }
    };
    let body = vec![
        Node::let_bind("qt_idx", lane.clone()),
        Node::if_then(
            Expr::lt(Expr::var("qt_idx"), Expr::u32(queue_capacity)),
            vec![Node::if_then(
                Expr::lt(Expr::var("qt_idx"), Expr::load(queue_len, Expr::u32(0))),
                vec![
                    Node::let_bind("qt_src", Expr::load(active_queue, Expr::var("qt_idx"))),
                    Node::if_then(
                        Expr::lt(Expr::var("qt_src"), Expr::u32(node_count)),
                        vec![
                            Node::let_bind(
                                "qt_edge_start",
                                Expr::load(edge_offsets, Expr::var("qt_src")),
                            ),
                            Node::let_bind(
                                "qt_edge_end",
                                Expr::load(
                                    edge_offsets,
                                    Expr::add(Expr::var("qt_src"), Expr::u32(1)),
                                ),
                            ),
                            Node::loop_for(
                                "qt_e",
                                Expr::var("qt_edge_start"),
                                Expr::var("qt_edge_end"),
                                vec![Node::if_then(
                                    Expr::lt(Expr::var("qt_e"), Expr::u32(edge_count)),
                                    vec![
                                        Node::let_bind(
                                            "qt_kind",
                                            Expr::load(edge_kind_mask, Expr::var("qt_e")),
                                        ),
                                        Node::if_then(
                                            Expr::ne(
                                                Expr::bitand(
                                                    Expr::var("qt_kind"),
                                                    Expr::u32(allow_mask),
                                                ),
                                                Expr::u32(0),
                                            ),
                                            vec![
                                                Node::let_bind(
                                                    "qt_dst",
                                                    Expr::load(edge_targets, Expr::var("qt_e")),
                                                ),
                                                Node::if_then(
                                                    Expr::lt(
                                                        Expr::var("qt_dst"),
                                                        Expr::u32(node_count),
                                                    ),
                                                    vec![
                                                        Node::let_bind(
                                                            "qt_dst_word",
                                                            Expr::shr(
                                                                Expr::var("qt_dst"),
                                                                Expr::u32(5),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "qt_dst_bit",
                                                            Expr::shl(
                                                                Expr::u32(1),
                                                                Expr::bitand(
                                                                    Expr::var("qt_dst"),
                                                                    Expr::u32(31),
                                                                ),
                                                            ),
                                                        ),
                                                        Node::let_bind(
                                                            "_qt_prev",
                                                            Expr::atomic_or(
                                                                frontier_out,
                                                                Expr::var("qt_dst_word"),
                                                                Expr::var("qt_dst_bit"),
                                                            ),
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
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(CSR_QUEUE_FORWARD_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference for queue materialization.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn frontier_to_queue_cpu(
    frontier_in: &[u32],
    node_count: u32,
    queue_capacity: usize,
) -> (Vec<u32>, u32) {
    try_frontier_to_queue_cpu(frontier_in, node_count, queue_capacity).unwrap_or_else(|err| {
        panic!("frontier_to_queue CPU oracle received malformed input. {err}")
    })
}

/// Fallible CPU reference for queue materialization.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_frontier_to_queue_cpu(
    frontier_in: &[u32],
    node_count: u32,
    queue_capacity: usize,
) -> Result<(Vec<u32>, u32), String> {
    let mut queue: Vec<u32> = Vec::new();
    let seen = try_frontier_to_queue_cpu_into(frontier_in, node_count, queue_capacity, &mut queue)?;
    Ok((queue, seen))
}

/// Fallible CPU reference for queue materialization into caller-owned storage.
///
/// On error, `queue` is left unchanged. This keeps parity harnesses and
/// resident dispatch diagnostics from losing the last queue snapshot when a
/// malformed frontier arrives.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_frontier_to_queue_cpu_into(
    frontier_in: &[u32],
    node_count: u32,
    queue_capacity: usize,
    queue: &mut Vec<u32>,
) -> Result<u32, String> {
    crate::bitset::frontier::materialize_frontier_queue_prefix_into(
        node_count,
        frontier_in,
        queue_capacity,
        queue,
    )
    .map_err(|error| match error {
        crate::bitset::frontier::FrontierError::BadShape {
            expected_words,
            actual_words,
            ..
        } => format!(
            "Fix: frontier_to_queue requires frontier_in.len() == bitset_words(node_count), got len={actual_words} but expected {expected_words} for node_count={node_count}."
        ),
        other => format!(
            "Fix: frontier_to_queue CPU oracle could not materialize the active frontier queue: {other}"
        ),
    })
}

/// CPU reference for queue-driven CSR expansion.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn csr_queue_forward_traverse_cpu(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Vec<u32> {
    try_csr_queue_forward_traverse_cpu(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
    )
    .unwrap_or_else(|err| {
        panic!("csr_queue_forward_traverse CPU oracle received malformed input. {err}")
    })
}

/// Fallible CPU reference for queue-driven CSR expansion.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_csr_queue_forward_traverse_cpu(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
) -> Result<Vec<u32>, String> {
    let mut out: Vec<u32> = Vec::new();
    try_csr_queue_forward_traverse_cpu_into(
        active_queue,
        queue_len,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        node_count,
        allow_mask,
        &mut out,
    )?;
    Ok(out)
}

/// Fallible CPU reference for queue-driven CSR expansion into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn try_csr_queue_forward_traverse_cpu_into(
    active_queue: &[u32],
    queue_len: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    node_count: u32,
    allow_mask: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let layout = validate_csr_queue_graph(node_count, edge_offsets, edge_targets, edge_kind_mask)?;
    crate::graph::scratch::reserve_graph_items(
        out,
        layout.words,
        "CSR frontier queue CPU oracle",
        "frontier output bitset",
    )?;
    out.clear();
    out.resize(layout.words, 0);
    let take = (queue_len as usize).min(active_queue.len());
    for &src in &active_queue[..take] {
        if src >= node_count {
            continue;
        }
        let start = edge_offsets[src as usize] as usize;
        let end = edge_offsets[src as usize + 1] as usize;
        for edge in start..end.min(edge_targets.len()).min(edge_kind_mask.len()) {
            if edge_kind_mask[edge] & allow_mask == 0 {
                continue;
            }
            let dst = edge_targets[edge];
            if dst < node_count {
                out[dst as usize / 32] |= 1u32 << (dst % 32);
            }
        }
    }
    Ok(())
}

#[cfg(test)]

mod generated_cpu_oracle_tests {
    use super::*;

    #[test]
    fn frontier_to_queue_rejects_missing_words_without_clobbering_queue() {
        let mut queue = vec![7, 3, 1];

        let err = try_frontier_to_queue_cpu_into(&[0b101], 64, 4, &mut queue)
            .expect_err("short frontier bitset must fail exact-width validation");

        assert!(
            err.contains("frontier_in.len() == bitset_words(node_count)"),
            "Fix: frontier width error must identify the exact bitset contract, got: {err}"
        );
        assert_eq!(
            queue,
            vec![7, 3, 1],
            "failed frontier materialization must preserve previous queue diagnostics"
        );
    }

    #[test]
    fn frontier_to_queue_clamps_queue_prefix_and_masks_tail_bits() {
        let frontier = [0b1010_u32, u32::MAX];
        let mut queue = Vec::new();

        let seen = try_frontier_to_queue_cpu_into(&frontier, 33, 2, &mut queue)
            .expect("Fix: canonical frontier should materialize through the CPU oracle");

        assert_eq!(seen, 3);
        assert_eq!(queue, vec![1, 3]);
        assert!(
            queue.iter().all(|node| *node < 33),
            "out-of-domain tail bits must not enter the compact queue prefix"
        );
    }

    #[test]
    fn queue_forward_traverse_into_rejects_bad_graph_without_clobbering_output() {
        let mut out = vec![0xDEAD_BEEF];

        let err = try_csr_queue_forward_traverse_cpu_into(
            &[0],
            1,
            &[0, 1, 1],
            &[2],
            &[1],
            2,
            1,
            &mut out,
        )
        .expect_err("out-of-range target must fail CSR queue graph validation");

        assert!(
            err.contains("outside node_count"),
            "Fix: queue traversal graph errors must identify invalid targets, got: {err}"
        );
        assert_eq!(
            out,
            vec![0xDEAD_BEEF],
            "failed queue traversal preflight must preserve previous output diagnostics"
        );
    }

    #[test]
    fn generated_frontier_queue_and_traverse_cpu_oracles_match_shape_contracts() {
        for node_count in 1u32..=128 {
            let edge_offsets: Vec<u32> = (0..=node_count).collect();
            let edge_targets: Vec<u32> = (0..node_count)
                .map(|node| (node + 1) % node_count)
                .collect();
            let edge_kind_mask = vec![1u32; node_count as usize];
            for queue_capacity in 0usize..32 {
                let mut frontier = vec![0u32; bitset_words(node_count) as usize];
                let period = (queue_capacity as u32 % 7) + 1;
                let mut expected_seen = 0u32;
                for node in 0..node_count {
                    if node % period == 0 {
                        frontier[node as usize / 32] |= 1u32 << (node % 32);
                        expected_seen = expected_seen.saturating_add(1);
                    }
                }
                let (queue, seen) =
                    try_frontier_to_queue_cpu(&frontier, node_count, queue_capacity).unwrap();
                assert_eq!(seen, expected_seen);
                assert_eq!(queue.len(), queue_capacity.min(expected_seen as usize));
                let out = try_csr_queue_forward_traverse_cpu(
                    &queue,
                    seen,
                    &edge_offsets,
                    &edge_targets,
                    &edge_kind_mask,
                    node_count,
                    1,
                )
                .unwrap();
                assert_eq!(out.len(), bitset_words(node_count) as usize);
                for &src in &queue {
                    let dst = (src + 1) % node_count;
                    assert_ne!(out[dst as usize / 32] & (1u32 << (dst % 32)), 0);
                }
            }
        }
    }
}

/// Validated resident graph layout for queue-driven sparse traversal.
///
/// The primitive owns these derived counts so resident dispatch wrappers do not
/// fork CSR edge-count, edge-padding, or frontier bitset sizing rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsrQueueGraphLayout {
    /// Number of graph nodes accepted by the primitive.
    pub node_count: u32,
    /// Exact physical edge count declared by `edge_offsets[node_count]`.
    pub edge_count: u32,
    /// Largest CSR row degree in the graph.
    pub max_row_degree: u32,
    /// Number of u32 words in each packed frontier bitset.
    pub words: usize,
    /// Number of u32 words to allocate/upload for edge target and kind arrays.
    pub edge_storage_words: usize,
}

/// Validate the CSR graph consumed by queue-driven sparse traversal.
///
/// Returns the resident graph layout so dispatch wrappers can construct padded
/// buffers without owning CSR validation locally.
///
/// # Errors
///
/// Returns an actionable diagnostic for zero-node graphs, malformed offsets,
/// mismatched edge arrays, or out-of-range destinations.
pub fn validate_csr_queue_graph(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<CsrQueueGraphLayout, String> {
    if node_count == 0 {
        return Err("Fix: csr_queue_forward_traverse requires node_count > 0.".to_string());
    }
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!(
            "Fix: csr_queue_forward_traverse node_count + 1 overflows usize for node_count={node_count}."
        )
    })?;
    if edge_offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: csr_queue_forward_traverse requires edge_offsets.len() == node_count + 1, got len={}, node_count={node_count}.",
            edge_offsets.len()
        ));
    }
    if edge_targets.len() != edge_kind_mask.len() {
        return Err(format!(
            "Fix: csr_queue_forward_traverse requires edge_targets.len() == edge_kind_mask.len(), got {} vs {}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    if edge_offsets[0] != 0 {
        return Err(format!(
            "Fix: csr_queue_forward_traverse requires edge_offsets[0] == 0, got {}.",
            edge_offsets[0]
        ));
    }
    let mut max_row_degree = 0u32;
    for (row, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(format!(
                "Fix: csr_queue_forward_traverse offsets must be monotonic at row {row}: {} > {}.",
                pair[0], pair[1]
            ));
        }
        max_row_degree = max_row_degree.max(pair[1] - pair[0]);
    }
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    if edge_targets.len() != edge_count {
        return Err(format!(
            "Fix: csr_queue_forward_traverse final offset declares edge_count={edge_count}, but targets_len={} and kind_mask_len={}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    for (index, &target) in edge_targets.iter().enumerate() {
        if target >= node_count {
            return Err(format!(
                "Fix: csr_queue_forward_traverse edge_targets[{index}]={target} is outside node_count {node_count}."
            ));
        }
    }
    let edge_count = u32::try_from(edge_count).map_err(|_| {
        format!("Fix: csr_queue_forward_traverse edge count {edge_count} exceeds u32 index space.")
    })?;
    Ok(CsrQueueGraphLayout {
        node_count,
        edge_count,
        max_row_degree,
        words: bitset_words(node_count) as usize,
        edge_storage_words: edge_targets.len().max(1),
    })
}

/// Validate a batch of packed frontiers for queue-driven CSR traversal.
///
/// Returns the exact packed frontier word count implied by `node_count`, so
/// dispatch wrappers can size resident scratch without duplicating the
/// primitive's batch-shape contract.
///
/// # Errors
///
/// Returns an actionable diagnostic for zero-node graphs, empty batches, zero
/// queue capacity, or any query frontier whose packed bitset width does not
/// match `node_count`.
pub fn validate_frontier_queue_batch(
    node_count: u32,
    frontiers: &[&[u32]],
    queue_capacity: u32,
) -> Result<usize, String> {
    if node_count == 0 {
        return Err("Fix: resident CSR queue batch requires node_count > 0.".to_string());
    }
    if frontiers.is_empty() {
        return Err("Fix: resident CSR queue batch requires at least one frontier.".to_string());
    }
    if queue_capacity == 0 {
        return Err("Fix: resident CSR queue batch requires queue_capacity > 0.".to_string());
    }

    let expected_words = bitset_words(node_count) as usize;
    for (query_index, frontier) in frontiers.iter().enumerate() {
        if frontier.len() != expected_words {
            return Err(format!(
                "Fix: resident CSR queue batch query {query_index} expected {expected_words} frontier word(s) for node_count={node_count} but received {}.",
                frontier.len()
            ));
        }
    }
    Ok(expected_words)
}

/// Validate one packed frontier for queue-driven CSR traversal.
///
/// Returns the exact packed frontier word count implied by `node_count`, so a
/// resident dispatch wrapper can size scratch without duplicating queue and
/// frontier-shape policy.
///
/// # Errors
///
/// Returns an actionable diagnostic for zero-node graphs, zero queue capacity,
/// or a frontier whose packed bitset width does not match `node_count`.
pub fn validate_frontier_queue_query(
    node_count: u32,
    frontier: &[u32],
    queue_capacity: u32,
) -> Result<usize, String> {
    validate_frontier_queue_batch(node_count, &[frontier], queue_capacity).map_err(|error| {
        error
            .replace("resident CSR queue batch", "resident CSR queue query")
            .replace("query 0", "query")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::transform::visit::{walk_exprs, walk_nodes};

    #[test]
    fn cpu_queue_preserves_node_order_and_reports_overflow_pressure() {
        let (queue, len) = frontier_to_queue_cpu(&[0b10111], 5, 3);
        assert_eq!(queue, vec![0, 1, 2]);
        assert_eq!(len, 4);
    }

    #[test]
    fn cpu_queue_traverse_expands_only_queued_sources() {
        let edge_offsets = vec![0, 2, 3, 3, 3];
        let edge_targets = vec![1, 2, 3];
        let edge_kind_mask = vec![1, 2, 1];
        let out = csr_queue_forward_traverse_cpu(
            &[0, 1],
            2,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            4,
            1,
        );
        assert_eq!(out, vec![0b1010]);
    }

    #[test]
    fn packed_word_queue_reserves_once_per_nonzero_word() {
        let program = frontier_words_to_queue_parallel("frontier", "queue", "len", 35, 16);

        assert_eq!(
            count_atomic_exprs(&program),
            1,
            "packed-word queue materialization should have one static queue reservation site"
        );
        assert_eq!(
            loop_atomic_count(&program, "qw_rank"),
            Some(0),
            "per-active-bit scatter loop must not issue atomics after the word-level reservation"
        );
        assert!(
            assignment_contains_u32(&program, "qw_remaining", 0b111),
            "the packed-word materializer must mask tail bits before popcounting the final frontier word"
        );
    }

    #[test]
    fn emitted_programs_have_stable_shapes() {
        let queue_len_init = frontier_queue_len_init("len");
        assert_eq!(queue_len_init.workgroup_size, [1, 1, 1]);
        assert_eq!(queue_len_init.buffers.len(), 1);
        let queue = frontier_to_queue("frontier", "queue", "len", 64, 8);
        assert_eq!(queue.workgroup_size, [256, 1, 1]);
        assert_eq!(queue.buffers.len(), 3);
        let parallel_queue = frontier_to_queue_parallel("frontier", "queue", "len", 64, 8);
        assert_eq!(parallel_queue.workgroup_size, [256, 1, 1]);
        assert_eq!(parallel_queue.buffers.len(), 3);
        let word_queue = frontier_words_to_queue_parallel("frontier", "queue", "len", 64, 8);
        assert_eq!(word_queue.workgroup_size, [256, 1, 1]);
        assert_eq!(word_queue.buffers.len(), 3);
        assert_eq!(word_queue.buffers[0].count, 2);
        let word_queue_clear =
            frontier_words_to_queue_clear_out_parallel("frontier", "queue", "len", "out", 64, 8);
        assert_eq!(word_queue_clear.workgroup_size, [256, 1, 1]);
        assert_eq!(word_queue_clear.buffers.len(), 4);
        assert_eq!(word_queue_clear.buffers[0].count, 2);
        assert_eq!(word_queue_clear.buffers[3].name.as_ref(), "out");
        assert_eq!(word_queue_clear.buffers[3].count, 2);
        let word_scan =
            frontier_word_counts_scan_pass_a("frontier", "partials", "block_totals", 64);
        assert_eq!(word_scan.workgroup_size, [1024, 1, 1]);
        assert_eq!(word_scan.buffers.len(), 5);
        assert_eq!(word_scan.buffers[0].count, 2);
        assert_eq!(word_scan.buffers[1].count, 1024);
        assert_eq!(word_scan.buffers[2].count, 1);
        let block_offsets = frontier_word_block_offsets_in_place("block_totals", 32_897);
        assert_eq!(block_offsets.workgroup_size, [1024, 1, 1]);
        assert_eq!(block_offsets.buffers.len(), 3);
        assert_eq!(block_offsets.buffers[0].count, 2);
        let huge_block_offsets = frontier_word_block_offsets_in_place("block_totals", 33_554_433);
        assert_eq!(huge_block_offsets.workgroup_size, [1, 1, 1]);
        assert_eq!(huge_block_offsets.buffers.len(), 1);
        assert_eq!(huge_block_offsets.buffers[0].count, 1025);
        let prefix_queue = frontier_word_block_prefix_to_queue_parallel(
            "frontier",
            "partials",
            "block_totals",
            "queue",
            "len",
            64,
            8,
        );
        assert_eq!(prefix_queue.workgroup_size, [256, 1, 1]);
        assert_eq!(prefix_queue.buffers.len(), 5);
        assert_eq!(prefix_queue.buffers[0].count, 2);
        assert_eq!(prefix_queue.buffers[1].count, 1024);
        assert_eq!(prefix_queue.buffers[2].count, 1);
        let offset_queue = frontier_word_block_offsets_to_queue_parallel(
            "frontier",
            "partials",
            "block_offsets",
            "queue",
            "len",
            32_897,
            8,
        );
        assert_eq!(offset_queue.workgroup_size, [256, 1, 1]);
        assert_eq!(offset_queue.buffers.len(), 5);
        assert_eq!(offset_queue.buffers[0].count, 1029);
        assert_eq!(offset_queue.buffers[1].count, 2048);
        assert_eq!(offset_queue.buffers[2].count, 2);
        assert!(
            !format!("{:?}", offset_queue.entry()).contains("fwq_prev_block"),
            "precomputed-offset scatter must not retain the per-word previous-block loop"
        );
        let traverse = csr_queue_forward_traverse(
            "queue", "len", "offsets", "targets", "kinds", "out", 64, 7, 8, 1,
        );
        assert_eq!(traverse.workgroup_size, [256, 1, 1]);
        assert_eq!(traverse.buffers.len(), 6);
    }

    #[test]
    fn frontier_queue_sizing_overflow_returns_error_without_panic() {
        let byte_result = std::panic::catch_unwind(|| {
            try_u32_byte_range_with_word_size(2, usize::MAX, "test frontier queue bytes")
        });
        assert!(
            byte_result.is_ok(),
            "checked frontier byte sizing must return an error instead of panicking"
        );
        let err = byte_result.unwrap().unwrap_err().to_string();
        assert!(
            err.contains("overflows output byte range"),
            "Fix: byte sizing overflow must name the byte-range contract, got: {err}"
        );
        assert!(
            err.contains("Shard the frontier queue"),
            "Fix: byte sizing overflow must tell the operator how to recover, got: {err}"
        );

        let product_result = std::panic::catch_unwind(|| {
            checked_frontier_u32_product(u32::MAX, 2, "test partial word count")
        });
        assert!(
            product_result.is_ok(),
            "checked frontier word products must return an error instead of panicking"
        );
        let err = product_result.unwrap().unwrap_err().to_string();
        assert!(
            err.contains("overflows u32 word count"),
            "Fix: word-count overflow must name the u32 product contract, got: {err}"
        );
    }

    #[test]
    fn csr_queue_traverse_rejects_offset_count_overflow_without_panic() {
        let result = std::panic::catch_unwind(|| {
            csr_queue_forward_traverse(
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
            "csr_queue_forward_traverse must emit an invalid program instead of panicking"
        );

        let program = result.unwrap();
        assert_eq!(program.workgroup_size, [1, 1, 1]);
        assert_eq!(program.buffers.len(), 1);
        assert_eq!(program.buffers[0].name.as_ref(), "out");
        let entry = format!("{:?}", program.entry());
        assert!(
            entry.contains("node_count + 1 overflows u32"),
            "Fix: invalid CSR queue program must preserve the offset overflow diagnostic, got: {entry}"
        );
    }

    fn count_atomic_exprs(program: &Program) -> usize {
        let mut count = 0;
        walk_exprs(program, |expr| {
            if matches!(expr, Expr::Atomic { .. }) {
                count += 1;
            }
        });
        count
    }

    fn loop_atomic_count(program: &Program, loop_var: &str) -> Option<usize> {
        let mut count = None;
        walk_nodes(program, |node| {
            if count.is_some() {
                return;
            }
            if let Node::Loop { var, body, .. } = node {
                if var.as_ref() == loop_var {
                    let loop_program = Program::wrapped(Vec::new(), [1, 1, 1], body.clone());
                    count = Some(count_atomic_exprs(&loop_program));
                }
            }
        });
        count
    }

    fn assignment_contains_u32(program: &Program, target: &str, value: u32) -> bool {
        let mut found = false;
        walk_nodes(program, |node| {
            if found {
                return;
            }
            if let Node::Assign { name, value: expr } = node {
                if name.as_ref() == target && expr_contains_u32(expr, value) {
                    found = true;
                }
            }
        });
        found
    }

    fn expr_contains_u32(expr: &Expr, value: u32) -> bool {
        let mut found = false;
        let expr_program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![Node::let_bind("__expr_probe", expr.clone())],
        );
        walk_exprs(&expr_program, |expr| {
            if matches!(expr, Expr::LitU32(found_value) if *found_value == value) {
                found = true;
            }
        });
        found
    }

    #[test]
    fn validate_csr_queue_graph_accepts_zero_edge_graph_and_canonical_graph() {
        assert_eq!(
            validate_csr_queue_graph(3, &[0, 0, 0, 0], &[], &[]).unwrap(),
            CsrQueueGraphLayout {
                node_count: 3,
                edge_count: 0,
                max_row_degree: 0,
                words: 1,
                edge_storage_words: 1,
            }
        );
        assert_eq!(
            validate_csr_queue_graph(4, &[0, 2, 3, 3, 3], &[1, 2, 3], &[1, 2, 1]).unwrap(),
            CsrQueueGraphLayout {
                node_count: 4,
                edge_count: 3,
                max_row_degree: 2,
                words: 1,
                edge_storage_words: 3,
            }
        );
    }

    #[test]
    fn validate_csr_queue_graph_rejects_malformed_inputs() {
        let err = validate_csr_queue_graph(0, &[0], &[], &[]).unwrap_err();
        assert!(err.contains("node_count > 0"));

        let err = validate_csr_queue_graph(2, &[0, 1, 1], &[1], &[]).unwrap_err();
        assert!(err.contains("edge_targets.len() == edge_kind_mask.len()"));

        let err = validate_csr_queue_graph(2, &[0, 2, 1], &[1], &[1]).unwrap_err();
        assert!(err.contains("offsets must be monotonic"));

        let err = validate_csr_queue_graph(2, &[0, 1, 1], &[5], &[1]).unwrap_err();
        assert!(err.contains("outside node_count"));
    }

    #[test]
    fn validate_frontier_queue_batch_accepts_canonical_frontiers() {
        let frontiers: [&[u32]; 2] = [&[1, 0], &[0, 2]];

        let words = validate_frontier_queue_batch(64, &frontiers, 8)
            .expect("Fix: two 64-node frontiers should be valid");

        assert_eq!(words, 2);
    }

    #[test]
    fn validate_frontier_queue_batch_rejects_invalid_batch_shapes() {
        let frontier: [&[u32]; 1] = [&[1]];

        let err = validate_frontier_queue_batch(0, &frontier, 8).unwrap_err();
        assert!(err.contains("node_count > 0"));

        let empty: [&[u32]; 0] = [];
        let err = validate_frontier_queue_batch(64, &empty, 8).unwrap_err();
        assert!(err.contains("at least one frontier"));

        let err = validate_frontier_queue_batch(64, &frontier, 0).unwrap_err();
        assert!(err.contains("queue_capacity > 0"));

        let err = validate_frontier_queue_batch(64, &frontier, 8).unwrap_err();
        assert!(err.contains("query 0 expected 2 frontier word"));
    }

    #[test]
    fn validate_frontier_queue_query_delegates_single_frontier_contract() {
        assert_eq!(validate_frontier_queue_query(64, &[1, 0], 8).unwrap(), 2);

        let err = validate_frontier_queue_query(64, &[1], 8).unwrap_err();
        assert!(err.contains("query expected 2 frontier word"));

        let err = validate_frontier_queue_query(64, &[1, 0], 0).unwrap_err();
        assert!(err.contains("queue_capacity > 0"));
    }
}
