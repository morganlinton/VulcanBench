use std::sync::Arc;

use super::layout::{
    BATCH_OP_ID, BINDING_CHANGED, BINDING_FRONTIER_IN, BINDING_FRONTIER_OUT, OP_ID,
    PERSISTENT_BFS_WORKGROUP_SIZE,
};
use crate::graph::csr_forward_or_changed::csr_forward_or_changed_parallel_snapshot_child_prefixed_with_active;
use crate::graph::persistent_bfs_step::persistent_bfs_step_child_prefixed_with_active;
use crate::graph::program_graph::ProgramGraphShape;
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

/// Words needed to hold a bitset over `node_count` nodes.
#[must_use]
pub const fn bitset_words(node_count: u32) -> u32 {
    crate::bitset::bitset_words(node_count)
}

/// Build the IR `Program` for persistent BFS.
///
/// The kernel copies `frontier_in` into `frontier_out`, then performs up
/// to `max_iters` forward traversal steps.  The first four iterations are
/// unrolled with inter-step workgroup barriers and a shared `wg_scratch`
/// array; any additional iterations run in a plain bounded loop.
///
/// `changed` is a single u32 word that is set to `1` if *any* step produced
/// a new reachable node.
#[must_use]
pub fn persistent_bfs(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Program {
    if shape.node_count > PERSISTENT_BFS_WORKGROUP_SIZE[0] {
        return persistent_bfs_grid_sync_parallel(
            shape,
            frontier_in,
            frontier_out,
            edge_kind_mask,
            max_iters,
        );
    }
    persistent_bfs_single_workgroup(shape, frontier_in, frontier_out, edge_kind_mask, max_iters)
}

fn persistent_bfs_single_workgroup(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Program {
    let words = bitset_words(shape.node_count);
    let t = Expr::gid_x();

    let unrolled_iter = |iter: u32| -> Node {
        persistent_bfs_step_child_prefixed_with_active(
            OP_ID,
            shape,
            frontier_out,
            "changed",
            "wg_scratch",
            "wg_active",
            edge_kind_mask,
            &format!("unroll_{iter}"),
        )
    };

    let mut entry: Vec<Node> = vec![
        // Seed frontier_out from frontier_in.
        Node::if_then(
            Expr::eq(Expr::local_x(), Expr::u32(0)),
            vec![Node::loop_for(
                "seed_word_idx",
                Expr::u32(0),
                Expr::u32(words),
                vec![Node::store(
                    frontier_out,
                    Expr::var("seed_word_idx"),
                    Expr::load(frontier_in, Expr::var("seed_word_idx")),
                )],
            )],
        ),
        // Zero the global changed flag.
        Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            vec![
                Node::store("changed", Expr::u32(0), Expr::u32(0)),
                Node::store("wg_active", Expr::u32(0), Expr::u32(1)),
            ],
        ),
        // Barrier clears fusion hazards from the plain store above before the
        // first atomic access inside the unrolled steps.
        Node::barrier(),
    ];

    let unroll_count = max_iters.min(4);
    for iter in 0..unroll_count {
        entry.push(unrolled_iter(iter));
    }

    let remaining = max_iters.saturating_sub(unroll_count);
    if remaining > 0 {
        entry.push(Node::loop_for(
            "iter",
            Expr::u32(0),
            Expr::u32(remaining),
            vec![Node::if_then(
                Expr::ne(
                    Expr::load("wg_active", Expr::u32(0)),
                    Expr::u32(0),
                ),
                vec![
                    Node::let_bind("local_changed", Expr::u32(0)),
                    Node::if_then(
                        Expr::lt(t.clone(), Expr::u32(shape.node_count)),
                        vec![
                            crate::graph::csr_forward_or_changed::csr_forward_or_changed_child_prefixed(
                                OP_ID,
                                shape,
                                frontier_out,
                                "local_changed",
                                edge_kind_mask,
                                "remaining_csr",
                            ),
                        ],
                    ),
                    Node::if_then(
                        Expr::eq(t.clone(), Expr::u32(0)),
                        vec![Node::store(
                            "wg_active",
                            Expr::u32(0),
                            Expr::var("local_changed"),
                        )],
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("local_changed"), Expr::u32(1)),
                        vec![Node::let_bind(
                            "_",
                            Expr::atomic_or("changed", Expr::u32(0), Expr::u32(1)),
                        )],
                    ),
                ],
            )],
        ));
    }

    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_in,
            BINDING_FRONTIER_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_FRONTIER_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            "changed",
            BINDING_CHANGED,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(1),
    );
    buffers.push(BufferDecl::workgroup("wg_scratch", 256, DataType::U32));
    buffers.push(BufferDecl::workgroup("wg_active", 1, DataType::U32));

    Program::wrapped(
        buffers,
        PERSISTENT_BFS_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    )
}

fn persistent_bfs_grid_sync_parallel(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Program {
    let words = bitset_words(shape.node_count);
    let t = Expr::gid_x();
    const GRID_CHANGED_WORDS: u32 = 3;
    const GRID_ACTIVE_BASE: u32 = 1;
    let mut entry: Vec<Node> = vec![
        Node::if_then(
            Expr::lt(t.clone(), Expr::u32(words)),
            vec![Node::store(
                frontier_out,
                t.clone(),
                Expr::load(frontier_in, t.clone()),
            )],
        ),
        Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            if max_iters > 0 {
                vec![
                    Node::store("changed", Expr::u32(0), Expr::u32(0)),
                    Node::store("changed", Expr::u32(GRID_ACTIVE_BASE), Expr::u32(1)),
                    Node::store("changed", Expr::u32(GRID_ACTIVE_BASE + 1), Expr::u32(0)),
                ]
            } else {
                vec![Node::store("changed", Expr::u32(0), Expr::u32(0))]
            },
        ),
    ];

    if max_iters > 0 {
        entry.push(grid_sync_barrier());
    }
    for iter in 0..max_iters {
        let active_index = GRID_ACTIVE_BASE + (iter & 1);
        let next_active_index = GRID_ACTIVE_BASE + ((iter + 1) & 1);
        entry.push(Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            vec![Node::store(
                "changed",
                Expr::u32(next_active_index),
                Expr::u32(0),
            )],
        ));
        entry.push(
            csr_forward_or_changed_parallel_snapshot_child_prefixed_with_active(
                OP_ID,
                shape,
                frontier_out,
                "changed",
                Expr::load("changed", Expr::u32(active_index)),
                Expr::u32(next_active_index),
                edge_kind_mask,
                &format!("grid_iter_{iter}"),
            ),
        );
        if iter + 1 < max_iters {
            entry.push(grid_sync_barrier());
        }
    }

    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            frontier_in,
            BINDING_FRONTIER_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_FRONTIER_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            "changed",
            BINDING_CHANGED,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(if max_iters > 0 { GRID_CHANGED_WORDS } else { 1 }),
    );

    Program::wrapped(
        buffers,
        PERSISTENT_BFS_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    )
}

fn grid_sync_barrier() -> Node {
    Node::barrier_with_ordering(MemoryOrdering::GridSync)
}

/// Build a batched persistent-BFS Program.
///
/// Frontier buffers are flat `[query][word]` arrays. The launch topology uses
/// `grid.y` for the query and `grid.x` for source-node lanes inside that query.
/// Each expansion pass snapshots active source bits before any lane writes new
/// destination bits, preserving the CPU oracle's one-hop-per-iteration cap.
#[must_use]
pub fn persistent_bfs_batch(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    changed: &str,
    query_count: u32,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Program {
    // Fail fast on an invalid flat-frontier shape rather than silently degrading
    // to an inert empty kernel (silent recall loss). Use
    // `try_persistent_bfs_batch` for structured handling.
    try_persistent_bfs_batch(
        shape,
        frontier_in,
        frontier_out,
        changed,
        query_count,
        edge_kind_mask,
        max_iters,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

/// Build a batched persistent-BFS Program with checked flat-frontier sizing.
pub fn try_persistent_bfs_batch(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
    changed: &str,
    query_count: u32,
    edge_kind_mask: u32,
    max_iters: u32,
) -> Result<Program, String> {
    let words = bitset_words(shape.node_count).max(1);
    let total_words = checked_batch_frontier_words(words, query_count, BATCH_OP_ID)?;
    let q = Expr::gid_y();
    let base = Expr::mul(q.clone(), Expr::u32(words));
    let lane = Expr::gid_x();
    let uses_grid_sync = persistent_bfs_batch_needs_grid_sync(shape);

    let mut entry: Vec<Node> = vec![
        Node::if_then(
            Expr::lt(lane.clone(), Expr::u32(words)),
            vec![Node::store(
                frontier_out,
                Expr::add(base.clone(), lane.clone()),
                Expr::load(frontier_in, Expr::add(base.clone(), lane.clone())),
            )],
        ),
        Node::if_then(
            Expr::eq(lane, Expr::u32(0)),
            vec![Node::store(changed, q.clone(), Expr::u32(0))],
        ),
    ];

    if max_iters > 0 {
        entry.push(persistent_bfs_batch_sync(uses_grid_sync));
    }
    if uses_grid_sync {
        for iter in 0..max_iters {
            entry.extend(persistent_bfs_batch_parallel_step_body(
                shape,
                frontier_out,
                changed,
                words,
                edge_kind_mask,
                &format!("batch_grid_iter_{iter}"),
                uses_grid_sync,
            ));
            if iter + 1 < max_iters {
                entry.push(grid_sync_barrier());
            }
        }
    } else if max_iters > 0 {
        entry.push(Node::loop_for(
            "batch_iter",
            Expr::u32(0),
            Expr::u32(max_iters),
            persistent_bfs_batch_parallel_step_body(
                shape,
                frontier_out,
                changed,
                words,
                edge_kind_mask,
                "batch_loop",
                uses_grid_sync,
            ),
        ));
    }

    let mut buffers = shape.try_read_only_buffers()?;
    buffers.push(
        BufferDecl::storage(
            frontier_in,
            BINDING_FRONTIER_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(total_words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            frontier_out,
            BINDING_FRONTIER_OUT,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(total_words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            changed,
            BINDING_CHANGED,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(query_count.max(1)),
    );

    Ok(Program::wrapped(
        buffers,
        PERSISTENT_BFS_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(BATCH_OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    ))
}

fn persistent_bfs_batch_needs_grid_sync(shape: ProgramGraphShape) -> bool {
    shape.node_count > PERSISTENT_BFS_WORKGROUP_SIZE[0]
}

fn persistent_bfs_batch_sync(uses_grid_sync: bool) -> Node {
    if uses_grid_sync {
        grid_sync_barrier()
    } else {
        Node::barrier()
    }
}

fn persistent_bfs_batch_parallel_step_body(
    shape: ProgramGraphShape,
    frontier_out: &str,
    changed: &str,
    words: u32,
    edge_kind_mask: u32,
    local_prefix: &str,
    uses_grid_sync: bool,
) -> Vec<Node> {
    let local = |name: &str| -> String { format!("{local_prefix}_{name}") };
    let q = Expr::gid_y();
    let base = Expr::mul(q.clone(), Expr::u32(words));
    let src = Expr::gid_x();
    let in_bounds = local("in_bounds");
    let word_idx = local("word_idx");
    let bit_mask = local("bit_mask");
    let src_word = local("src_word");
    let src_active = local("src_active");
    let edge_start = local("edge_start");
    let edge_end = local("edge_end");
    let edge_iter = local("edge");
    let kind_mask = local("kind_mask");
    let dst = local("dst");
    let dst_word_idx = local("dst_word_idx");
    let dst_bit = local("dst_bit");
    let old = local("old");
    let changed_old = local("changed_old");

    let edge_scan = || {
        vec![
            Node::let_bind(
                edge_start.as_str(),
                Expr::load("pg_edge_offsets", src.clone()),
            ),
            Node::let_bind(
                edge_end.as_str(),
                Expr::load("pg_edge_offsets", Expr::add(src.clone(), Expr::u32(1))),
            ),
            Node::loop_for(
                edge_iter.as_str(),
                Expr::var(edge_start.as_str()),
                Expr::var(edge_end.as_str()),
                vec![
                    Node::let_bind(
                        kind_mask.as_str(),
                        Expr::load("pg_edge_kind_mask", Expr::var(edge_iter.as_str())),
                    ),
                    Node::if_then(
                        Expr::ne(
                            Expr::bitand(Expr::var(kind_mask.as_str()), Expr::u32(edge_kind_mask)),
                            Expr::u32(0),
                        ),
                        vec![
                            Node::let_bind(
                                dst.as_str(),
                                Expr::load("pg_edge_targets", Expr::var(edge_iter.as_str())),
                            ),
                            Node::if_then(
                                Expr::lt(Expr::var(dst.as_str()), Expr::u32(shape.node_count)),
                                vec![
                                    Node::let_bind(
                                        dst_word_idx.as_str(),
                                        Expr::shr(Expr::var(dst.as_str()), Expr::u32(5)),
                                    ),
                                    Node::let_bind(
                                        dst_bit.as_str(),
                                        Expr::shl(
                                            Expr::u32(1),
                                            Expr::bitand(Expr::var(dst.as_str()), Expr::u32(31)),
                                        ),
                                    ),
                                    Node::let_bind(
                                        old.as_str(),
                                        Expr::atomic_or(
                                            frontier_out,
                                            Expr::add(
                                                base.clone(),
                                                Expr::var(dst_word_idx.as_str()),
                                            ),
                                            Expr::var(dst_bit.as_str()),
                                        ),
                                    ),
                                    Node::if_then(
                                        Expr::eq(
                                            Expr::bitand(
                                                Expr::var(old.as_str()),
                                                Expr::var(dst_bit.as_str()),
                                            ),
                                            Expr::u32(0),
                                        ),
                                        vec![Node::let_bind(
                                            changed_old.as_str(),
                                            Expr::atomic_or(changed, q.clone(), Expr::u32(1)),
                                        )],
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
        ]
    };

    let mut body = vec![
        Node::let_bind(
            in_bounds.as_str(),
            Expr::lt(src.clone(), Expr::u32(shape.node_count)),
        ),
        Node::let_bind(
            word_idx.as_str(),
            Expr::select(
                Expr::var(in_bounds.as_str()),
                Expr::shr(src.clone(), Expr::u32(5)),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            bit_mask.as_str(),
            Expr::shl(Expr::u32(1), Expr::bitand(src.clone(), Expr::u32(31))),
        ),
        Node::let_bind(
            src_word.as_str(),
            Expr::load(
                frontier_out,
                Expr::add(base.clone(), Expr::var(word_idx.as_str())),
            ),
        ),
        Node::let_bind(
            src_active.as_str(),
            Expr::select(
                Expr::var(in_bounds.as_str()),
                Expr::bitand(Expr::var(src_word.as_str()), Expr::var(bit_mask.as_str())),
                Expr::u32(0),
            ),
        ),
        persistent_bfs_batch_sync(uses_grid_sync),
        Node::if_then(
            Expr::ne(Expr::var(src_active.as_str()), Expr::u32(0)),
            edge_scan(),
        ),
    ];
    if !uses_grid_sync {
        body.push(Node::barrier());
    }
    body
}

fn checked_batch_frontier_words(
    words_per_query: u32,
    query_count: u32,
    op_id: &'static str,
) -> Result<u32, String> {
    words_per_query.checked_mul(query_count.max(1)).ok_or_else(|| {
        format!(
            "{op_id} frontier words overflow u32: words_per_query={words_per_query}, query_count={query_count}. Fix: shard the BFS query batch before GPU dispatch."
        )
    })
}
