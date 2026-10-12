//! GPU DCE program with early-exit on convergence.
//!
//! `vyre_primitives::graph::persistent_bfs` always runs its full
//! `max_iters` loop because its `changed` flag is never reset between
//! iterations. For shallow DAGs (most real Programs) the BFS frontier
//! converges in a handful of hops while the kernel keeps churning
//! through hundreds of no-op iterations.
//!
//! This builder emits a DCE-tailored variant: each iteration zeroes
//! `changed` first, runs the CSR-forward step, and the kernel returns
//! as soon as `changed == 0` after a step. For wide DAGs (diameter ≪
//! `max_iters`) this drops the persistent-loop cost from
//! `O(max_iters)` to `O(actual_diameter)`. For chains
//! (`diameter == n`) it matches the original.
//!
//! Buffer + binding layout matches `persistent_bfs` exactly so the
//! handles can be allocated and dispatched the same way.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::bitset::bitset_words;
use vyre_primitives::graph::persistent_bfs::{
    BINDING_CHANGED, BINDING_FRONTIER_IN, BINDING_FRONTIER_OUT,
};
use vyre_primitives::graph::program_graph::{
    ProgramGraphShape, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS, NAME_EDGE_TARGETS,
};

/// Canonical op id for the optimizer's DCE program.
pub const OP_ID: &str = "vyre-self-substrate::optimizer::dce_program";

/// Workgroup size  -  each BFS step's per-thread parallelism is
/// bounded by this. Single-workgroup design keeps cross-workgroup
/// sync simple (workgroup-scope barrier inside the persistent loop)
/// and avoids the changed-flag race that multi-workgroup would
/// introduce.
const DCE_WORKGROUP_X: u32 = 1024;

/// Parallel BFS step with per-thread strided loop. Thread
/// `t = gid_x()` handles sources `t, t + WG, t + 2·WG, …` up to
/// `node_count`. Reads `frontier_out[src/32]`'s bit; if set, walks
/// the source's outgoing CSR edges and atomically ORs each target's
/// frontier bit. Sets outer-scope `local_changed` to 1 whenever a
/// NEW bit is added.
///
/// `allow_mask` filters edges: an edge is followed iff
/// `(kind_mask & allow_mask) != 0`. The DCE caller passes
/// `0xFFFF_FFFF` (any-kind), the generic persistent-BFS caller
/// passes the real allow_mask.
fn parallel_csr_step_per_thread_masked(node_count: u32, allow_mask: u32) -> Vec<Node> {
    let stride_count = (node_count + DCE_WORKGROUP_X - 1) / DCE_WORKGROUP_X;
    vec![Node::loop_for(
        "stride",
        Expr::u32(0),
        Expr::u32(stride_count.max(1)),
        vec![
            Node::let_bind(
                "src",
                Expr::add(
                    Expr::gid_x(),
                    Expr::mul(Expr::var("stride"), Expr::u32(DCE_WORKGROUP_X)),
                ),
            ),
            Node::if_then(
                Expr::lt(Expr::var("src"), Expr::u32(node_count)),
                vec![
                    Node::let_bind("src_word_idx", Expr::shr(Expr::var("src"), Expr::u32(5))),
                    Node::let_bind(
                        "src_bit_mask",
                        Expr::shl(Expr::u32(1), Expr::bitand(Expr::var("src"), Expr::u32(31))),
                    ),
                    Node::let_bind(
                        "src_word",
                        Expr::load("frontier_out", Expr::var("src_word_idx")),
                    ),
                    Node::if_then(
                        Expr::ne(
                            Expr::bitand(Expr::var("src_word"), Expr::var("src_bit_mask")),
                            Expr::u32(0),
                        ),
                        vec![
                            Node::let_bind(
                                "edge_start",
                                Expr::load(NAME_EDGE_OFFSETS, Expr::var("src")),
                            ),
                            Node::let_bind(
                                "edge_end",
                                Expr::load(
                                    NAME_EDGE_OFFSETS,
                                    Expr::add(Expr::var("src"), Expr::u32(1)),
                                ),
                            ),
                            Node::loop_for(
                                "e",
                                Expr::var("edge_start"),
                                Expr::var("edge_end"),
                                vec![
                                    Node::let_bind(
                                        "kind_mask",
                                        Expr::load(NAME_EDGE_KIND_MASK, Expr::var("e")),
                                    ),
                                    Node::if_then(
                                        Expr::ne(
                                            Expr::bitand(
                                                Expr::var("kind_mask"),
                                                Expr::u32(allow_mask),
                                            ),
                                            Expr::u32(0),
                                        ),
                                        vec![
                                            Node::let_bind(
                                                "dst",
                                                Expr::load(NAME_EDGE_TARGETS, Expr::var("e")),
                                            ),
                                            Node::if_then(
                                                Expr::lt(Expr::var("dst"), Expr::u32(node_count)),
                                                vec![
                                                    Node::let_bind(
                                                        "dst_word_idx",
                                                        Expr::shr(Expr::var("dst"), Expr::u32(5)),
                                                    ),
                                                    Node::let_bind(
                                                        "dst_bit",
                                                        Expr::shl(
                                                            Expr::u32(1),
                                                            Expr::bitand(
                                                                Expr::var("dst"),
                                                                Expr::u32(31),
                                                            ),
                                                        ),
                                                    ),
                                                    Node::let_bind(
                                                        "old",
                                                        Expr::atomic_or(
                                                            "frontier_out",
                                                            Expr::var("dst_word_idx"),
                                                            Expr::var("dst_bit"),
                                                        ),
                                                    ),
                                                    Node::if_then(
                                                        Expr::eq(
                                                            Expr::bitand(
                                                                Expr::var("old"),
                                                                Expr::var("dst_bit"),
                                                            ),
                                                            Expr::u32(0),
                                                        ),
                                                        vec![Node::assign(
                                                            "local_changed",
                                                            Expr::u32(1),
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
    )]
}

/// Build a generic persistent-BFS Program with early-exit and a
/// caller-supplied `allow_mask` for edge filtering.
///
/// Identical buffer layout to `build_dce_bfs_program`; differs only
/// in that the edge-follow check is `(kind_mask & allow_mask) != 0`
/// instead of `kind_mask != 0`. Use this when porting
/// `vyre_primitives::graph::persistent_bfs::cpu_ref` to GPU dispatch.
#[must_use]
pub fn build_persistent_bfs_program(
    shape: ProgramGraphShape,
    max_iters: u32,
    allow_mask: u32,
) -> Program {
    build_persistent_bfs_program_sticky(shape, max_iters, allow_mask)
}

/// Build a DCE-tailored persistent BFS Program with early-exit.
///
/// Identical RO buffer layout to `persistent_bfs` (frontier_in,
/// frontier_out, changed, plus the program-graph CSR buffers from
/// `shape.read_only_buffers()`). The kernel:
///
///  1. Seeds `frontier_out` from `frontier_in`.
///  2. Runs up to `max_iters` BFS steps. Each step:
///     a. Lane 0 zeros `changed[0]`.
///     b. Workgroup barrier.
///     c. CSR forward step; if any node grew its frontier bit, it
///     does `atomic_or(changed, 0, 1)`.
///     d. Workgroup barrier.
///     e. If `changed[0] == 0`, return (no progress this iter ⇒
///     fixpoint reached; subsequent iters are no-ops).
///  3. Final state lives in `frontier_out`.
#[must_use]
pub fn build_dce_bfs_program(shape: ProgramGraphShape, max_iters: u32) -> Program {
    build_persistent_bfs_program_inner(shape, max_iters, u32::MAX)
}

/// Shared implementation for `build_dce_bfs_program` (allow_mask =
/// `u32::MAX`, sticky_changed=false) and `build_persistent_bfs_program`
/// (caller-supplied allow_mask, sticky_changed=true).
///
/// `sticky_changed` controls the semantics of `changed[0]`:
///  - `false` (DCE): `changed[0]` reflects the LAST iter's progress
///    (the kernel zeroes it each iter for early-exit detection). DCE
///    doesn't observe the post-kernel value so this is fine.
///  - `true` (generic persistent BFS): `changed[0]` is sticky-OR'd
///    across all iterations, matching the CPU oracle's contract.
///    The kernel uses an internal scratch slot for the per-iter flag.
fn build_persistent_bfs_program_inner(
    shape: ProgramGraphShape,
    max_iters: u32,
    allow_mask: u32,
) -> Program {
    build_persistent_bfs_program_internal(shape, max_iters, allow_mask, false)
}

fn build_persistent_bfs_program_sticky(
    shape: ProgramGraphShape,
    max_iters: u32,
    allow_mask: u32,
) -> Program {
    build_persistent_bfs_program_internal(shape, max_iters, allow_mask, true)
}

fn build_persistent_bfs_program_internal(
    shape: ProgramGraphShape,
    max_iters: u32,
    allow_mask: u32,
    sticky_changed: bool,
) -> Program {
    let words = bitset_words(shape.node_count);
    let t = Expr::gid_x();

    // For sticky-changed mode, slot 0 = per-iter (zeroed each iter,
    // used for early-exit) and slot 1 = cumulative (sticky-OR'd
    // across all iters, never zeroed). Caller reads slot 1.
    // For DCE mode, only slot 0 is used.
    let mut iter_body: Vec<Node> = vec![
        // Zero `changed[0]` so this iteration's compare starts clean.
        Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            vec![Node::store("changed", Expr::u32(0), Expr::u32(0))],
        ),
        Node::barrier(),
        Node::let_bind("local_changed", Expr::u32(0)),
        Node::if_then(
            Expr::lt(t.clone(), Expr::u32(shape.node_count)),
            parallel_csr_step_per_thread_masked(shape.node_count, allow_mask),
        ),
        // OR local_changed into the per-iter early-exit flag.
        Node::if_then(
            Expr::eq(Expr::var("local_changed"), Expr::u32(1)),
            vec![Node::let_bind(
                "_dce_set",
                Expr::atomic_or("changed", Expr::u32(0), Expr::u32(1)),
            )],
        ),
    ];
    if sticky_changed {
        // Mirror the OR into slot 1 (cumulative). slot 1 is never
        // zeroed, so once any iter sets it, it stays 1.
        iter_body.push(Node::if_then(
            Expr::eq(Expr::var("local_changed"), Expr::u32(1)),
            vec![Node::let_bind(
                "_sticky_set",
                Expr::atomic_or("changed", Expr::u32(1), Expr::u32(1)),
            )],
        ));
    }
    iter_body.push(Node::barrier());
    // Early-exit on per-iter fixpoint.
    iter_body.push(Node::if_then(
        Expr::eq(Expr::load("changed", Expr::u32(0)), Expr::u32(0)),
        vec![Node::Return],
    ));

    let entry: Vec<Node> = vec![
        // Seed frontier_out <- frontier_in.
        Node::if_then(
            Expr::lt(t.clone(), Expr::u32(words)),
            vec![Node::store(
                "frontier_out",
                t.clone(),
                Expr::load("frontier_in", t.clone()),
            )],
        ),
        Node::barrier(),
        // Persistent loop with early-exit.
        Node::loop_for("iter", Expr::u32(0), Expr::u32(max_iters.max(1)), iter_body),
    ];

    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            "frontier_in",
            BINDING_FRONTIER_IN,
            BufferAccess::ReadOnly,
            DataType::U32,
        )
        .with_count(words.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            "frontier_out",
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
        .with_count(if sticky_changed { 2 } else { 1 }),
    );
    buffers.push(BufferDecl::workgroup("wg_scratch", 256, DataType::U32));

    // Workgroup size [1024, 1, 1]  -  RTX 5090's max threads-per-block.
    // Packs every BFS step across 1024 threads per workgroup so a
    // 1001-node DAG fits in a single workgroup; 32-node SIMT loops
    // amortise atomic_or hits on the global `changed` word.
    Program::wrapped(
        buffers,
        [1024, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(entry),
        }],
    )
}
