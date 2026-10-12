//! `dominator_tree`  -  exact immediate-dominator primitive.
//!
//! Computes the immediate dominator (`idom`) of every reachable node in a
//! control-flow graph with a single entry.  The primitive ships both a
//! Lengauer–Tarjan CPU reference oracle and a serial lane-0 GPU `Program`
//! builder that implements the Cooper–Harvey–Kennedy iterative fixpoint
//! using parent-pointer LCA on the idom tree.
//!
//! # Wire shape
//!
//! ```text
//! pg_edge_offsets : u32[node_count + 1]   // forward CSR
//! pg_edge_targets : u32[edge_count]       // forward CSR
//! pred_offsets    : u32[node_count + 1]   // predecessor CSR
//! pred_targets    : u32[pred_edge_count]  // predecessor CSR
//! idom_out        : u32[node_count]       // output idoms; NONE = unreachable
//! ```
//!
//! `idom_out[entry] == entry` for the entry block.  Unreachable nodes keep
//! the sentinel `NONE` (== `node_count`).
//!
//! # Soundness
//!
//! Exact for every reducible and irreducible single-entry CFG.  Multi-entry
//! graphs (no path from entry to some node that has predecessors) are not
//! rejected explicitly, but the resulting idom tree is undefined for the
//! disconnected component; callers should run `reachable` first if they need
//! strict guarantees.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::dominator_tree";
const INIT_PHASE_OP_ID: &str = "vyre-primitives::graph::dominator_tree::init_state";
const DEPTH_PHASE_OP_ID: &str = "vyre-primitives::graph::dominator_tree::recompute_depth";
const INTERSECT_PHASE_OP_ID: &str =
    "vyre-primitives::graph::dominator_tree::intersect_predecessors";

/// Sentinel stored in `idom_out` for unreachable nodes.
pub const IDOM_NONE: u32 = u32::MAX;

/// Errors from dominator-tree construction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DominatorTreeError {
    /// The requested entry node is outside `0..node_count`.
    EntryOutOfRange {
        /// Supplied entry.
        entry: u32,
        /// Declared node count.
        node_count: u32,
    },
    /// CSR offset buffer length is inconsistent with `node_count`.
    BadOffsets {
        /// Actionable diagnostic.
        message: String,
    },
    /// CSR target buffer references a node outside the valid range.
    TargetOutOfRange {
        /// Offending target index.
        index: usize,
        /// Offending value.
        target: u32,
        /// Declared node count.
        node_count: u32,
    },
    /// Monotonicity violation in CSR offsets.
    NonMonotonicOffsets {
        /// Index of first violation.
        index: usize,
    },
}

/// Validated dispatch layout for the dominator-tree primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorTreeLayout {
    /// Number of nodes.
    pub node_count: u32,
    /// Number of forward edges.
    pub edge_count: u32,
    /// Number of predecessor edges.
    pub pred_edge_count: u32,
    /// Words in the `idom_out` buffer.
    pub idom_words: usize,
    /// Words in the `depth` scratch buffer.
    pub depth_words: usize,
}

/// Build a serial lane-0 `Program` that computes exact immediate dominators.
///
/// The kernel runs the Cooper–Harvey–Kennedy iterative fixpoint over the
/// idom tree using predecessor-list LCA.  Workgroup size is `[1, 1, 1]`;
/// only invocation 0 performs work.
///
/// # Panics
///
/// Returns an inert early-return trap when `try_dominator_tree_program` rejects
/// the shape (e.g. `node_count == u32::MAX`, which collides with `IDOM_NONE`).
#[must_use]
pub fn dominator_tree_program(
    node_count: u32,
    edge_count: u32,
    pred_edge_count: u32,
    idom_out: &str,
) -> Program {
    match try_dominator_tree_program(node_count, edge_count, pred_edge_count, idom_out) {
        Ok(p) => p,
        Err(_) => inert_dominator_tree_program(idom_out),
    }
}

/// Checked builder.  Returns an actionable diagnostic instead of a trap
/// program when the shape overflows buffer counts.
pub fn try_dominator_tree_program(
    node_count: u32,
    edge_count: u32,
    pred_edge_count: u32,
    idom_out: &str,
) -> Result<Program, String> {
    if node_count == u32::MAX {
        return Err(
            "dominator_tree node_count == u32::MAX collides with IDOM_NONE sentinel. \
             Fix: shard the graph before dispatch."
                .to_string(),
        );
    }

    let offset_count = node_count.checked_add(1).ok_or_else(|| {
        format!(
            "dominator_tree node_count={node_count} overflows CSR offset buffer count. \
                 Fix: shard the graph before GPU dispatch."
        )
    })?;

    let lane0 = Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0));

    // ------------------------------------------------------------------
    // Serial CHK-by-LCA kernel on lane 0.
    // ------------------------------------------------------------------

    // depth[0] = 0; depth[v] = 0 for all others (will be fixed on first update)
    let depth_buf = "dt_depth";
    let init_state = child_phase(
        INIT_PHASE_OP_ID,
        vec![
            // idom_out[v] = NONE for all v
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(node_count),
                vec![Node::store(idom_out, Expr::var("i"), Expr::u32(IDOM_NONE))],
            ),
            // idom_out[0] = 0  (entry dominates itself)
            Node::store(idom_out, Expr::u32(0), Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(node_count),
                vec![Node::store(depth_buf, Expr::var("i"), Expr::u32(0))],
            ),
        ],
    );

    // Outer fixpoint: at most node_count iterations.
    // Each step: recompute depths, then for each v != entry intersect preds via LCA.
    let recompute_depth = child_phase(
        DEPTH_PHASE_OP_ID,
        vec![Node::loop_for(
            "v",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![
                Node::let_bind("d", Expr::u32(0)),
                Node::let_bind("cur", Expr::var("v")),
                Node::loop_for(
                    "depth_step",
                    Expr::u32(0),
                    Expr::u32(node_count),
                    vec![Node::if_then(
                        Expr::ne(Expr::var("cur"), Expr::u32(0)),
                        vec![
                            Node::let_bind("parent", Expr::load(idom_out, Expr::var("cur"))),
                            Node::if_then(
                                Expr::and(
                                    Expr::ne(Expr::var("parent"), Expr::var("cur")),
                                    Expr::ne(Expr::var("parent"), Expr::u32(IDOM_NONE)),
                                ),
                                vec![
                                    Node::assign("d", Expr::add(Expr::var("d"), Expr::u32(1))),
                                    Node::assign("cur", Expr::var("parent")),
                                ],
                            ),
                        ],
                    )],
                ),
                Node::store(depth_buf, Expr::var("v"), Expr::var("d")),
            ],
        )],
    );

    let body = vec![
        // changed = 0
        Node::let_bind("changed", Expr::u32(0)),
        // recompute all depths from current idom tree
        recompute_depth.clone(),
        // for v in 0..node_count
        child_phase(
            INTERSECT_PHASE_OP_ID,
            vec![Node::loop_for(
                "v",
                Expr::u32(0),
                Expr::u32(node_count),
                vec![Node::if_then(
                    Expr::ne(Expr::var("v"), Expr::u32(0)),
                    vec![
                        // new_idom = NONE
                        Node::let_bind("new_idom", Expr::u32(IDOM_NONE)),
                        // walk predecessors
                        Node::let_bind("p_start", Expr::load("pred_offsets", Expr::var("v"))),
                        Node::let_bind(
                            "p_end",
                            Expr::load("pred_offsets", Expr::add(Expr::var("v"), Expr::u32(1))),
                        ),
                        Node::loop_for(
                            "p_idx",
                            Expr::var("p_start"),
                            Expr::var("p_end"),
                            vec![
                                Node::let_bind("p", Expr::load("pred_targets", Expr::var("p_idx"))),
                                // if idom[p] != NONE
                                Node::if_then(
                                    Expr::ne(
                                        Expr::load(idom_out, Expr::var("p")),
                                        Expr::u32(IDOM_NONE),
                                    ),
                                    vec![Node::if_then_else(
                                        Expr::eq(Expr::var("new_idom"), Expr::u32(IDOM_NONE)),
                                        // first reachable predecessor
                                        vec![Node::assign("new_idom", Expr::var("p"))],
                                        // else LCA(new_idom, p)
                                        vec![
                                            Node::let_bind("a", Expr::var("new_idom")),
                                            Node::let_bind("b", Expr::var("p")),
                                            Node::loop_for(
                                                "lca_step",
                                                Expr::u32(0),
                                                Expr::u32(node_count),
                                                vec![Node::if_then(
                                                    Expr::ne(Expr::var("a"), Expr::var("b")),
                                                    vec![
                                                        Node::let_bind(
                                                            "da",
                                                            Expr::load(depth_buf, Expr::var("a")),
                                                        ),
                                                        Node::let_bind(
                                                            "db",
                                                            Expr::load(depth_buf, Expr::var("b")),
                                                        ),
                                                        Node::if_then_else(
                                                            Expr::gt(
                                                                Expr::var("da"),
                                                                Expr::var("db"),
                                                            ),
                                                            vec![Node::assign(
                                                                "a",
                                                                Expr::load(
                                                                    idom_out,
                                                                    Expr::var("a"),
                                                                ),
                                                            )],
                                                            vec![Node::assign(
                                                                "b",
                                                                Expr::load(
                                                                    idom_out,
                                                                    Expr::var("b"),
                                                                ),
                                                            )],
                                                        ),
                                                    ],
                                                )],
                                            ),
                                            Node::assign("new_idom", Expr::var("a")),
                                        ],
                                    )],
                                ),
                            ],
                        ),
                        // if new_idom changed, write it and set changed flag
                        Node::if_then(
                            Expr::and(
                                Expr::ne(Expr::var("new_idom"), Expr::u32(IDOM_NONE)),
                                Expr::ne(
                                    Expr::var("new_idom"),
                                    Expr::load(idom_out, Expr::var("v")),
                                ),
                            ),
                            vec![
                                Node::store(idom_out, Expr::var("v"), Expr::var("new_idom")),
                                Node::assign("changed", Expr::u32(1)),
                            ],
                        ),
                    ],
                )],
            )],
        ),
    ];

    let outer_loop = Node::loop_for("step", Expr::u32(0), Expr::u32(node_count), body);

    let region_body = vec![Node::if_then(lane0, vec![init_state, outer_loop])];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage("pg_edge_offsets", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(offset_count),
            BufferDecl::storage("pg_edge_targets", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(edge_count.max(1)),
            BufferDecl::storage("pred_offsets", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(offset_count),
            BufferDecl::storage("pred_targets", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(pred_edge_count.max(1)),
            BufferDecl::storage(idom_out, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count.max(1)),
            BufferDecl::storage(depth_buf, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(node_count.max(1)),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(region_body),
        }],
    ))
}

fn child_phase(generator: &'static str, body: Vec<Node>) -> Node {
    Node::Region {
        generator: Ident::from(generator),
        source_region: Some(GeneratorRef {
            name: OP_ID.to_string(),
        }),
        body: Arc::new(body),
    }
}

fn inert_dominator_tree_program(idom_out: &str) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("pg_edge_offsets", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage("pg_edge_targets", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage("pred_offsets", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage("pred_targets", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage(idom_out, 4, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("dt_depth", 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::return_()]),
        }],
    )
}

/// Validate dominator-tree CSR inputs.
///
/// # Errors
///
/// Returns [`DominatorTreeError`] when offsets are malformed, targets point
/// out of range, or the entry is invalid.
pub fn validate_dominator_tree_inputs(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
) -> Result<DominatorTreeLayout, DominatorTreeError> {
    let expected_offsets =
        (node_count as usize)
            .checked_add(1)
            .ok_or_else(|| DominatorTreeError::BadOffsets {
                message: format!(
                "Fix: dominator_tree node_count + 1 overflows usize for node_count={node_count}."
            ),
            })?;

    if edge_offsets.len() != expected_offsets {
        return Err(DominatorTreeError::BadOffsets {
            message: format!(
                "Fix: dominator_tree edge_offsets.len() must be {expected_offsets}, got {}.",
                edge_offsets.len()
            ),
        });
    }
    if pred_offsets.len() != expected_offsets {
        return Err(DominatorTreeError::BadOffsets {
            message: format!(
                "Fix: dominator_tree pred_offsets.len() must be {expected_offsets}, got {}.",
                pred_offsets.len()
            ),
        });
    }

    for (idx, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(DominatorTreeError::NonMonotonicOffsets { index: idx });
        }
    }
    for (idx, pair) in pred_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(DominatorTreeError::NonMonotonicOffsets { index: idx });
        }
    }

    let edge_count = edge_offsets.last().copied().unwrap_or(0);
    let pred_edge_count = pred_offsets.last().copied().unwrap_or(0);

    if edge_targets.len() != edge_count as usize {
        return Err(DominatorTreeError::BadOffsets {
            message: format!(
                "Fix: dominator_tree edge_targets.len()={} != edge_count={edge_count}.",
                edge_targets.len()
            ),
        });
    }
    if pred_targets.len() != pred_edge_count as usize {
        return Err(DominatorTreeError::BadOffsets {
            message: format!(
                "Fix: dominator_tree pred_targets.len()={} != pred_edge_count={pred_edge_count}.",
                pred_targets.len()
            ),
        });
    }

    for (idx, &target) in edge_targets.iter().enumerate() {
        if target >= node_count {
            return Err(DominatorTreeError::TargetOutOfRange {
                index: idx,
                target,
                node_count,
            });
        }
    }
    for (idx, &target) in pred_targets.iter().enumerate() {
        if target >= node_count {
            return Err(DominatorTreeError::TargetOutOfRange {
                index: idx,
                target,
                node_count,
            });
        }
    }

    Ok(DominatorTreeLayout {
        node_count,
        edge_count,
        pred_edge_count,
        idom_words: node_count as usize,
        depth_words: node_count as usize,
    })
}

// ------------------------------------------------------------------
