//! `dominator_frontier`  -  query the dominance frontier of a node
//! set, packed as a per-node bitset.
//!
//! The dominance frontier of node `n` is the set of nodes `m` such
//! that `n` dominates a predecessor of `m` but does NOT dominate `m`
//! itself. SSA phi placement uses this directly; rule pipelines can
//! reach for it via the `vyre.graph.dominator_frontier.v1` ExternCall.
//!
//! Soundness: exact when the supplied dominator-tree CSR is
//! correctly computed (the caller is responsible for that  -  usually
//! via `vyre-libs::dataflow::ssa::compute_dominators`).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::csr_forward_traverse::bitset_words;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::dominator_frontier";

/// Dominance-closure CSR offsets input binding.
pub const DOMINATOR_FRONTIER_DOM_OFFSETS_BUFFER: u32 = 0;
/// Dominance-closure CSR targets input binding.
pub const DOMINATOR_FRONTIER_DOM_TARGETS_BUFFER: u32 = 1;
/// Predecessor CSR offsets input binding.
pub const DOMINATOR_FRONTIER_PRED_OFFSETS_BUFFER: u32 = 2;
/// Predecessor CSR targets input binding.
pub const DOMINATOR_FRONTIER_PRED_TARGETS_BUFFER: u32 = 3;
/// Seed bitset input binding.
pub const DOMINATOR_FRONTIER_SEED_BUFFER: u32 = 4;
/// Frontier bitset output binding.
pub const DOMINATOR_FRONTIER_OUT_BUFFER: u32 = 5;
/// Candidate-node workgroup for dominance-frontier queries.
pub const DOMINATOR_FRONTIER_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for one dominance-frontier query over candidate nodes.
#[must_use]
pub const fn dominator_frontier_dispatch_grid(node_count: u32) -> [u32; 3] {
    if node_count == 0 {
        [0, 1, 1]
    } else {
        [
            node_count.div_ceil(DOMINATOR_FRONTIER_WORKGROUP_SIZE[0]),
            1,
            1,
        ]
    }
}

/// Validated dominance-frontier dispatch layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorFrontierLayout {
    /// Number of u32 words in the frontier/seed bitset.
    pub words: usize,
    /// Number of dominance-closure CSR edges.
    pub dom_edge_count: u32,
    /// Number of predecessor CSR edges.
    pub pred_edge_count: u32,
}

/// Program-shape key for dominance-frontier IR materialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorFrontierProgramShape {
    /// Number of candidate nodes.
    pub node_count: u32,
    /// Number of dominance-closure CSR edges.
    pub dom_edge_count: u32,
    /// Number of predecessor CSR edges.
    pub pred_edge_count: u32,
}

/// Content fingerprint for one immutable dominance-frontier input slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorFrontierSliceFingerprint {
    len: usize,
    first: u32,
    last: u32,
    xor: u32,
    sum: u64,
}

/// Primitive-owned identity for immutable dominance-frontier dispatch inputs.
///
/// Dynamic seed/frontier buffers are intentionally excluded: wrappers refresh
/// those every dispatch. This key covers only graph shape and graph content
/// that determine whether static device inputs can be reused safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorFrontierStaticInputKey {
    shape: DominatorFrontierProgramShape,
    layout: DominatorFrontierLayout,
    dom_target_words: usize,
    pred_target_words: usize,
    frontier_words: usize,
    dom_offsets: DominatorFrontierSliceFingerprint,
    dom_targets: DominatorFrontierSliceFingerprint,
    pred_offsets: DominatorFrontierSliceFingerprint,
    pred_targets: DominatorFrontierSliceFingerprint,
}

/// Compute the primitive-owned fingerprint used for immutable dispatch inputs.
#[must_use]
pub fn dominator_frontier_slice_fingerprint(words: &[u32]) -> DominatorFrontierSliceFingerprint {
    let mut xor = 0u32;
    let mut sum = 0u64;
    for &word in words {
        xor ^= word;
        sum = sum.wrapping_add(u64::from(word));
    }
    DominatorFrontierSliceFingerprint {
        len: words.len(),
        first: words.first().copied().unwrap_or(0),
        last: words.last().copied().unwrap_or(0),
        xor,
        sum,
    }
}

#[cfg(test)]
mod static_input_key_tests {
    use super::*;

    #[test]
    fn slice_fingerprint_tracks_interior_content_not_only_len_edges() {
        let baseline = dominator_frontier_slice_fingerprint(&[7, 11, 13, 17]);
        let changed = dominator_frontier_slice_fingerprint(&[7, 11, 19, 17]);

        assert_ne!(baseline, changed);
    }

    #[test]
    fn static_input_key_tracks_graph_content_but_not_dynamic_seed_bits() {
        let plan_a = plan_dominator_frontier_launch(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 0, 1, 2],
            &[0b0010],
        )
        .expect("Fix: valid dominator-frontier launch plan should build");
        let plan_b = plan_dominator_frontier_launch(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 0, 1, 2],
            &[0b0100],
        )
        .expect("Fix: seed-only changes should keep the same static launch shape");

        let baseline = plan_a.static_input_key(
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 0, 1, 2],
        );
        let seed_only_change = plan_b.static_input_key(
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 0, 1, 2],
        );
        let graph_content_change = plan_a.static_input_key(
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 2, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 0, 1, 2],
        );

        assert_eq!(baseline, seed_only_change);
        assert_ne!(baseline, graph_content_change);
    }
}

/// Primitive-owned dominance-frontier launch plan without eager IR materialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DominatorFrontierLaunchPlan {
    layout: DominatorFrontierLayout,
    shape: DominatorFrontierProgramShape,
    dispatch_grid: [u32; 3],
}

impl DominatorFrontierLaunchPlan {
    /// Validated CSR and bitset layout.
    #[must_use]
    pub const fn layout(&self) -> DominatorFrontierLayout {
        self.layout
    }

    /// Program-shape key for cache lookups.
    #[must_use]
    pub const fn shape(&self) -> DominatorFrontierProgramShape {
        self.shape
    }

    /// Exact GPU dispatch grid for this query.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        self.dispatch_grid
    }

    /// Number of u32 words in the seed/frontier bitsets.
    #[must_use]
    pub const fn frontier_words(&self) -> usize {
        self.layout.words
    }

    /// Number of u32 target words required by the dominance-closure input.
    #[must_use]
    pub const fn dom_target_words(&self) -> usize {
        if self.layout.dom_edge_count == 0 {
            1
        } else {
            self.layout.dom_edge_count as usize
        }
    }

    /// Number of u32 target words required by the predecessor input.
    #[must_use]
    pub const fn pred_target_words(&self) -> usize {
        if self.layout.pred_edge_count == 0 {
            1
        } else {
            self.layout.pred_edge_count as usize
        }
    }

    /// Stable identity for immutable graph inputs associated with this plan.
    #[must_use]
    pub fn static_input_key(
        &self,
        dom_offsets: &[u32],
        dom_targets: &[u32],
        pred_offsets: &[u32],
        pred_targets: &[u32],
    ) -> DominatorFrontierStaticInputKey {
        DominatorFrontierStaticInputKey {
            shape: self.shape,
            layout: self.layout,
            dom_target_words: self.dom_target_words(),
            pred_target_words: self.pred_target_words(),
            frontier_words: self.frontier_words(),
            dom_offsets: dominator_frontier_slice_fingerprint(dom_offsets),
            dom_targets: dominator_frontier_slice_fingerprint(dom_targets),
            pred_offsets: dominator_frontier_slice_fingerprint(pred_offsets),
            pred_targets: dominator_frontier_slice_fingerprint(pred_targets),
        }
    }

    /// Build the dominance-frontier Program for this launch plan.
    pub fn program(&self, seed_buffer: &str, out_buffer: &str) -> Result<Program, String> {
        try_dominator_frontier(
            self.shape.node_count,
            self.shape.dom_edge_count,
            self.shape.pred_edge_count,
            seed_buffer,
            out_buffer,
        )
    }
}

/// Primitive-owned dominance-frontier dispatch plan with eager IR materialization.
pub struct DominatorFrontierDispatchPlan {
    launch: DominatorFrontierLaunchPlan,
    program: Program,
}

impl DominatorFrontierDispatchPlan {
    /// Validated CSR and bitset layout.
    #[must_use]
    pub const fn layout(&self) -> DominatorFrontierLayout {
        self.launch.layout()
    }

    /// Program-shape key for cache lookups.
    #[must_use]
    pub const fn shape(&self) -> DominatorFrontierProgramShape {
        self.launch.shape()
    }

    /// Program wired to the canonical primitive buffer layout.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Exact GPU dispatch grid for this query.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        self.launch.dispatch_grid()
    }

    /// Number of u32 words in the seed/frontier bitsets.
    #[must_use]
    pub const fn frontier_words(&self) -> usize {
        self.launch.frontier_words()
    }

    /// Number of u32 target words required by the dominance-closure input.
    #[must_use]
    pub const fn dom_target_words(&self) -> usize {
        self.launch.dom_target_words()
    }

    /// Number of u32 target words required by the predecessor input.
    #[must_use]
    pub const fn pred_target_words(&self) -> usize {
        self.launch.pred_target_words()
    }
}

/// Validate inputs and build a dominance-frontier launch plan without
/// materializing IR.
///
/// # Errors
///
/// Returns an actionable diagnostic when either CSR is malformed, the seed
/// bitset is not exactly shaped for `node_count`, or the dispatch shape would
/// overflow.
pub fn plan_dominator_frontier_launch(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<DominatorFrontierLaunchPlan, String> {
    let layout = validate_dominator_frontier_inputs(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )?;
    let _offset_count = node_count.checked_add(1).ok_or_else(|| {
        format!(
            "dominator_frontier node_count={node_count} overflows CSR offset buffer count. Fix: shard the graph before GPU dispatch."
        )
    })?;

    Ok(DominatorFrontierLaunchPlan {
        layout,
        shape: DominatorFrontierProgramShape {
            node_count,
            dom_edge_count: layout.dom_edge_count,
            pred_edge_count: layout.pred_edge_count,
        },
        dispatch_grid: dominator_frontier_dispatch_grid(node_count),
    })
}

/// Validate inputs and build the canonical dominance-frontier dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic when either CSR is malformed, the seed
/// bitset is not exactly shaped for `node_count`, or the generated dispatch
/// program would overflow its CSR launch shape.
pub fn plan_dominator_frontier_dispatch(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
    seed_buffer: &str,
    out_buffer: &str,
) -> Result<DominatorFrontierDispatchPlan, String> {
    let launch = plan_dominator_frontier_launch(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )?;
    let program = launch.program(seed_buffer, out_buffer)?;

    Ok(DominatorFrontierDispatchPlan { launch, program })
}

/// Build a Program that evaluates the exact dominance-frontier
/// predicate:
///
/// `m ∈ DF(seed)` iff some seeded node `n` dominates at least one
/// predecessor of `m`, and `n` does not strictly dominate `m`.
///
/// `dom_offsets`/`dom_targets` encode dominance closure by dominator.
/// `pred_offsets`/`pred_targets` encode CFG predecessors by candidate
/// node.
#[must_use]
pub fn dominator_frontier(
    node_count: u32,
    dom_edge_count: u32,
    pred_edge_count: u32,
    seed: &str,
    out: &str,
) -> Program {
    // Fail fast on an overflowing CSR launch shape rather than silently
    // degrading to an inert empty kernel (silent recall loss). Use
    // `try_dominator_frontier` for structured handling.
    try_dominator_frontier(node_count, dom_edge_count, pred_edge_count, seed, out)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Build a dominance-frontier Program with checked CSR launch-shape
/// validation.
pub fn try_dominator_frontier(
    node_count: u32,
    dom_edge_count: u32,
    pred_edge_count: u32,
    seed: &str,
    out: &str,
) -> Result<Program, String> {
    let words = bitset_words(node_count).max(1);
    let offset_count = node_count.checked_add(1).ok_or_else(|| {
        format!(
            "dominator_frontier node_count={node_count} overflows CSR offset buffer count. Fix: shard the graph before GPU dispatch."
        )
    });
    let offset_count = offset_count?;
    let t = Expr::InvocationId { axis: 0 };
    let dominator_is_seed = vec![
        Node::let_bind(
            "seed_word",
            Expr::load(seed, Expr::shr(Expr::var("n"), Expr::u32(5))),
        ),
        Node::let_bind(
            "seed_bit",
            Expr::shl(Expr::u32(1), Expr::bitand(Expr::var("n"), Expr::u32(31))),
        ),
        Node::if_then(
            Expr::ne(
                Expr::bitand(Expr::var("seed_word"), Expr::var("seed_bit")),
                Expr::u32(0),
            ),
            vec![
                Node::let_bind(
                    "pred_start",
                    Expr::load("pred_offsets", Expr::var("candidate")),
                ),
                Node::let_bind(
                    "pred_end",
                    Expr::load(
                        "pred_offsets",
                        Expr::add(Expr::var("candidate"), Expr::u32(1)),
                    ),
                ),
                Node::let_bind("dominates_a_predecessor", Expr::u32(0)),
                Node::loop_for(
                    "p",
                    Expr::var("pred_start"),
                    Expr::var("pred_end"),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("dominates_a_predecessor"), Expr::u32(0)),
                        vec![
                            Node::let_bind("pred", Expr::load("pred_targets", Expr::var("p"))),
                            Node::let_bind(
                                "dom_start_pred",
                                Expr::load("dom_offsets", Expr::var("n")),
                            ),
                            Node::let_bind(
                                "dom_end_pred",
                                Expr::load("dom_offsets", Expr::add(Expr::var("n"), Expr::u32(1))),
                            ),
                            Node::loop_for(
                                "d_pred",
                                Expr::var("dom_start_pred"),
                                Expr::var("dom_end_pred"),
                                vec![Node::if_then(
                                    Expr::eq(
                                        Expr::load("dom_targets", Expr::var("d_pred")),
                                        Expr::var("pred"),
                                    ),
                                    vec![Node::assign("dominates_a_predecessor", Expr::u32(1))],
                                )],
                            ),
                        ],
                    )],
                ),
                Node::let_bind("dominates_candidate", Expr::u32(0)),
                Node::let_bind(
                    "dom_start_candidate",
                    Expr::load("dom_offsets", Expr::var("n")),
                ),
                Node::let_bind(
                    "dom_end_candidate",
                    Expr::load("dom_offsets", Expr::add(Expr::var("n"), Expr::u32(1))),
                ),
                Node::loop_for(
                    "d_candidate",
                    Expr::var("dom_start_candidate"),
                    Expr::var("dom_end_candidate"),
                    vec![Node::if_then(
                        Expr::eq(
                            Expr::load("dom_targets", Expr::var("d_candidate")),
                            Expr::var("candidate"),
                        ),
                        vec![Node::assign("dominates_candidate", Expr::u32(1))],
                    )],
                ),
                Node::let_bind("strictly_dominates_candidate", Expr::u32(0)),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("dominates_candidate"), Expr::u32(1)),
                        Expr::ne(Expr::var("n"), Expr::var("candidate")),
                    ),
                    vec![Node::assign("strictly_dominates_candidate", Expr::u32(1))],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("dominates_a_predecessor"), Expr::u32(1)),
                        Expr::eq(Expr::var("strictly_dominates_candidate"), Expr::u32(0)),
                    ),
                    vec![
                        Node::let_bind(
                            "candidate_word",
                            Expr::shr(Expr::var("candidate"), Expr::u32(5)),
                        ),
                        Node::let_bind(
                            "candidate_bit",
                            Expr::shl(
                                Expr::u32(1),
                                Expr::bitand(Expr::var("candidate"), Expr::u32(31)),
                            ),
                        ),
                        Node::let_bind(
                            "_prev",
                            Expr::atomic_or(
                                out,
                                Expr::var("candidate_word"),
                                Expr::var("candidate_bit"),
                            ),
                        ),
                    ],
                ),
            ],
        ),
    ];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(
                "dom_offsets",
                DOMINATOR_FRONTIER_DOM_OFFSETS_BUFFER,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(offset_count),
            BufferDecl::storage(
                "dom_targets",
                DOMINATOR_FRONTIER_DOM_TARGETS_BUFFER,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(dom_edge_count.max(1)),
            BufferDecl::storage(
                "pred_offsets",
                DOMINATOR_FRONTIER_PRED_OFFSETS_BUFFER,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(offset_count),
            BufferDecl::storage(
                "pred_targets",
                DOMINATOR_FRONTIER_PRED_TARGETS_BUFFER,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(pred_edge_count.max(1)),
            BufferDecl::storage(
                seed,
                DOMINATOR_FRONTIER_SEED_BUFFER,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(words),
            BufferDecl::storage(
                out,
                DOMINATOR_FRONTIER_OUT_BUFFER,
                BufferAccess::ReadWrite,
                DataType::U32,
            )
            .with_count(words),
        ],
        DOMINATOR_FRONTIER_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(node_count)),
                vec![
                    Node::let_bind("candidate", t),
                    Node::loop_for("n", Expr::u32(0), Expr::u32(node_count), dominator_is_seed),
                ],
            )]),
        }],
    ))
}

/// CPU oracle: returns the dominance-frontier bitset for the seed set.
///
/// `dom_offsets` / `dom_targets` encode the dominance closure by dominator:
/// row `n` contains every node dominated by `n`, including `n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Vec<u32> {
    try_cpu_ref(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )
    .expect("Fix: reject malformed oracle input via try_* APIs; do not call panicking wrappers on hostile data - dominator_frontier CPU oracle received malformed input or could not reserve output")
}

/// Fallible CPU oracle: returns the dominance-frontier bitset for the seed set.
///
/// This is the primitive-owned entry point for wrappers and generated tests that
/// must reject hostile CSR/seed inputs without panicking.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<Vec<u32>, String> {
    let mut frontier = Vec::new();
    try_cpu_ref_into(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
        &mut frontier,
    )?;
    Ok(frontier)
}

/// CPU oracle into caller-owned output storage.
///
/// `dom_offsets` / `dom_targets` encode the dominance closure by dominator:
/// row `n` contains every node dominated by `n`, including `n`.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
    frontier: &mut Vec<u32>,
) {
    try_cpu_ref_into(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
        frontier,
    )
    .expect("Fix: reject malformed oracle input via try_* APIs; do not call panicking wrappers on hostile data - dominator_frontier CPU oracle received malformed input or could not reserve output")
}

/// Fallible CPU oracle into caller-owned output storage.
///
/// On error, `frontier` is left unchanged so dispatch wrappers and parity tests
/// can surface malformed input as a typed finding instead of losing the last
/// useful diagnostic output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
    frontier: &mut Vec<u32>,
) -> Result<(), String> {
    let layout = validate_dominator_frontier_inputs(
        node_count,
        dom_offsets,
        dom_targets,
        pred_offsets,
        pred_targets,
        seed,
    )?;
    let words = layout.words;
    crate::graph::scratch::reserve_graph_items(
        frontier,
        words,
        "dominator frontier CPU oracle",
        "frontier output",
    )?;
    frontier.clear();
    frontier.resize(words, 0);
    for n in 0..node_count {
        let n_word = (n / 32) as usize;
        let n_bit = 1u32 << (n % 32);
        if seed[n_word] & n_bit == 0 {
            continue;
        }
        for m in 0..node_count {
            let pred_start = pred_offsets[m as usize] as usize;
            let pred_end = pred_offsets[m as usize + 1] as usize;
            let dominates_a_predecessor = pred_targets[pred_start..pred_end]
                .iter()
                .copied()
                .any(|pred| dominates(dom_offsets, dom_targets, n, pred));
            let strictly_dominates_m = n != m && dominates(dom_offsets, dom_targets, n, m);
            if dominates_a_predecessor && !strictly_dominates_m {
                let m_word = (m / 32) as usize;
                let m_bit = 1u32 << (m % 32);
                frontier[m_word] |= m_bit;
            }
        }
    }
    Ok(())
}

/// Number of nodes flagged in a dominance-frontier bitset.
#[must_use]
pub fn frontier_size(frontier: &[u32]) -> u32 {
    frontier.iter().map(|word| word.count_ones()).sum()
}

/// Validate a CSR buffer pair for `node_count` rows.
///
/// # Errors
///
/// Returns an actionable diagnostic when offsets are the wrong length,
/// non-monotonic, inconsistent with target count, or targets point outside
/// `0..node_count`.
pub fn validate_csr_shape(
    label: &str,
    node_count: u32,
    offsets: &[u32],
    targets: &[u32],
) -> Result<u32, String> {
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!(
            "Fix: dominator_frontier {label} node_count + 1 overflows usize for node_count={node_count}."
        )
    })?;
    if offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: dominator_frontier {label} offsets length must be {expected_offsets}, got {}.",
            offsets.len()
        ));
    }
    let mut previous = 0u32;
    for (idx, &offset) in offsets.iter().enumerate() {
        if idx > 0 && offset < previous {
            return Err(format!(
                "Fix: dominator_frontier {label} offsets must be monotonic; offsets[{idx}]={offset} after {previous}."
            ));
        }
        previous = offset;
    }
    if offsets.last().copied().unwrap_or(0) as usize != targets.len() {
        return Err(format!(
            "Fix: dominator_frontier {label} final offset must equal target count {}, got {}.",
            targets.len(),
            offsets.last().copied().unwrap_or(0)
        ));
    }
    for (idx, &target) in targets.iter().enumerate() {
        if target >= node_count {
            return Err(format!(
                "Fix: dominator_frontier {label} target[{idx}]={target} is outside node_count {node_count}."
            ));
        }
    }
    u32::try_from(targets.len()).map_err(|_| {
        format!(
            "Fix: dominator_frontier {label} target count {} exceeds u32 index space.",
            targets.len()
        )
    })
}

/// Validate the full dominance-frontier CPU/dispatch input contract.
///
/// # Errors
///
/// Returns an actionable diagnostic when either CSR is malformed or when the
/// seed bitset does not contain exactly the required number of words.
pub fn validate_dominator_frontier_inputs(
    node_count: u32,
    dom_offsets: &[u32],
    dom_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
    seed: &[u32],
) -> Result<DominatorFrontierLayout, String> {
    let words = bitset_words(node_count) as usize;
    if seed.len() != words {
        return Err(format!(
            "Fix: dominator_frontier expected seed length {words} words for {node_count} nodes, got {}.",
            seed.len()
        ));
    }
    let dom_edge_count = validate_csr_shape("dominance", node_count, dom_offsets, dom_targets)?;
    let pred_edge_count =
        validate_csr_shape("predecessor", node_count, pred_offsets, pred_targets)?;
    Ok(DominatorFrontierLayout {
        words,
        dom_edge_count,
        pred_edge_count,
    })
}

#[cfg(any(test, feature = "cpu-parity"))]
fn dominates(dom_offsets: &[u32], dom_targets: &[u32], dominator: u32, node: u32) -> bool {
    let start = dom_offsets[dominator as usize] as usize;
    let end = dom_offsets[dominator as usize + 1] as usize;
    dom_targets[start..end].contains(&node)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || dominator_frontier(4, 4, 4, "idom", "df"),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[0, 1, 2, 3, 4]),
                crate::wire::pack_u32_slice(&[0, 1, 2, 3]),
                crate::wire::pack_u32_slice(&[0, 0, 1, 2, 3]),
                crate::wire::pack_u32_slice(&[0, 1, 2, 0]),
                crate::wire::pack_u32_slice(&[0]),
                crate::wire::pack_u32_slice(&[0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[0])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_seed_yields_empty_frontier() {
        let out = cpu_ref(4, &[0, 0, 0, 0, 0], &[], &[0, 0, 0, 0, 0], &[], &[0]);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn single_node_with_no_predecessors_has_empty_frontier() {
        // node 0 with no predecessors → df(0) = {}.
        let out = cpu_ref(2, &[0, 0, 0], &[], &[0, 0, 0], &[], &[0b01]);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn dom_frontier_picks_up_join_node() {
        // CFG: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3.
        // Predecessors of 3: [1, 2]. 1 dominates itself only, 2 same.
        // df(1) includes 3 because 1 dominates predecessor 1 of 3,
        // but 1 does not dominate 3.
        let pred_offsets = vec![0u32, 0, 1, 2, 4];
        let pred_targets = vec![0u32, 0, 1, 2];
        // Dominator sets: 0 dominates {0,1,2,3}; 1 dominates {1};
        // 2 dominates {2}; 3 dominates {3}.
        let dom_offsets = vec![0u32, 4, 5, 6, 7];
        let dom_targets = vec![0u32, 1, 2, 3, 1, 2, 3];
        let out = cpu_ref(
            4,
            &dom_offsets,
            &dom_targets,
            &pred_offsets,
            &pred_targets,
            &[0b0010],
        );
        assert_eq!(out, vec![0b1000]);
    }

    #[test]
    fn cpu_ref_into_reuses_frontier_storage() {
        let mut out = Vec::with_capacity(8);
        let dom_offsets = vec![0u32, 4, 5, 6, 7];
        let dom_targets = vec![0u32, 1, 2, 3, 1, 2, 3];
        let pred_offsets = vec![0u32, 0, 1, 2, 4];
        let pred_targets = vec![0u32, 0, 1, 2];
        cpu_ref_into(
            4,
            &dom_offsets,
            &dom_targets,
            &pred_offsets,
            &pred_targets,
            &[0b0010],
            &mut out,
        );
        let capacity = out.capacity();
        assert_eq!(out, vec![0b1000]);

        cpu_ref_into(
            4,
            &dom_offsets,
            &dom_targets,
            &pred_offsets,
            &pred_targets,
            &[0],
            &mut out,
        );
        assert_eq!(out.capacity(), capacity);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn cpu_ref_into_uses_shared_validation_before_output_mutation() {
        let mut out = vec![0xCAFE_BABEu32];
        let ptr = out.as_ptr();
        let err = try_cpu_ref_into(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 1, 2, 3],
            &[0b0010, 0],
            &mut out,
        );

        assert!(err.is_err(), "extra seed word must be rejected");
        assert_eq!(
            out,
            vec![0xCAFE_BABEu32],
            "Fix: shared validation must reject malformed input before clearing caller output."
        );
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn fallible_cpu_ref_matches_compatibility_oracle_on_generated_diamonds() {
        for diamond_count in [1_u32, 2, 7, 16, 33, 64] {
            let node_count = diamond_count
                .checked_mul(4)
                .expect("Fix: generated diamond node count should fit u32");
            let words = bitset_words(node_count) as usize;
            let mut dom_offsets = Vec::with_capacity(node_count as usize + 1);
            let mut dom_targets = Vec::new();
            let mut pred_offsets = Vec::with_capacity(node_count as usize + 1);
            let mut pred_targets = Vec::new();
            dom_offsets.push(0);
            pred_offsets.push(0);
            for diamond in 0..diamond_count {
                let base = diamond * 4;
                dom_targets.extend_from_slice(&[base, base + 1, base + 2, base + 3]);
                dom_offsets.push(dom_targets.len() as u32);
                dom_targets.push(base + 1);
                dom_offsets.push(dom_targets.len() as u32);
                dom_targets.push(base + 2);
                dom_offsets.push(dom_targets.len() as u32);
                dom_targets.push(base + 3);
                dom_offsets.push(dom_targets.len() as u32);

                pred_offsets.push(pred_targets.len() as u32);
                pred_targets.push(base);
                pred_offsets.push(pred_targets.len() as u32);
                pred_targets.push(base);
                pred_offsets.push(pred_targets.len() as u32);
                pred_targets.extend_from_slice(&[base + 1, base + 2]);
                pred_offsets.push(pred_targets.len() as u32);
            }
            let mut seed = vec![0; words];
            for diamond in 0..diamond_count {
                let node = diamond * 4 + 1;
                seed[(node / 32) as usize] |= 1_u32 << (node % 32);
            }

            let expected = cpu_ref(
                node_count,
                &dom_offsets,
                &dom_targets,
                &pred_offsets,
                &pred_targets,
                &seed,
            );
            let actual = try_cpu_ref(
                node_count,
                &dom_offsets,
                &dom_targets,
                &pred_offsets,
                &pred_targets,
                &seed,
            )
            .expect("Fix: generated dominance-frontier diamonds should run fallibly");
            assert_eq!(actual, expected, "diamond_count={diamond_count}");
        }
    }

    #[test]
    fn reusable_validation_rejects_bad_csr_and_seed() {
        let err = validate_dominator_frontier_inputs(2, &[0, 1, 1], &[1], &[0, 1, 0], &[0], &[1])
            .unwrap_err();
        assert!(err.contains("predecessor offsets must be monotonic"));

        let err =
            validate_dominator_frontier_inputs(33, &[0; 34], &[], &[0; 34], &[], &[1]).unwrap_err();
        assert!(err.contains("expected seed length 2 words"));
    }

    #[test]
    fn reusable_validation_returns_dispatch_layout() {
        let layout = validate_dominator_frontier_inputs(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 1, 2, 3],
            &[0b0010],
        )
        .expect("Fix: canonical dominance-frontier input should validate");

        assert_eq!(
            layout,
            DominatorFrontierLayout {
                words: 1,
                dom_edge_count: 7,
                pred_edge_count: 4,
            }
        );
    }

    #[test]
    fn launch_plan_validates_layout_without_eager_program_materialization() {
        let plan = plan_dominator_frontier_launch(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 1, 2, 3],
            &[0b0010],
        )
        .expect("Fix: canonical dominance-frontier launch plan should validate");

        assert_eq!(plan.dispatch_grid(), [1, 1, 1]);
        assert_eq!(plan.frontier_words(), 1);
        assert_eq!(plan.dom_target_words(), 7);
        assert_eq!(plan.pred_target_words(), 4);
        assert_eq!(
            plan.shape(),
            DominatorFrontierProgramShape {
                node_count: 4,
                dom_edge_count: 7,
                pred_edge_count: 4,
            }
        );
        assert_eq!(
            plan.program("seed", "frontier_out")
                .expect("Fix: validated launch plan should materialize IR")
                .workgroup_size,
            DOMINATOR_FRONTIER_WORKGROUP_SIZE
        );
    }

    #[test]
    fn launch_plan_packs_candidate_lanes_into_blocks() {
        assert_eq!(dominator_frontier_dispatch_grid(0), [0, 1, 1]);
        assert_eq!(dominator_frontier_dispatch_grid(1), [1, 1, 1]);
        assert_eq!(dominator_frontier_dispatch_grid(256), [1, 1, 1]);
        assert_eq!(dominator_frontier_dispatch_grid(257), [2, 1, 1]);
        assert_eq!(dominator_frontier_dispatch_grid(513), [3, 1, 1]);
    }

    #[test]
    fn generated_launch_grid_covers_candidate_shapes_to_8192() {
        for node_count in 1..=8_192 {
            let grid = dominator_frontier_dispatch_grid(node_count);
            assert_eq!(
                grid[1], 1,
                "Fix: dominator-frontier grid y dimension drifted at node_count={node_count}"
            );
            assert_eq!(
                grid[2], 1,
                "Fix: dominator-frontier grid z dimension drifted at node_count={node_count}"
            );
            assert!(
                grid[0] * DOMINATOR_FRONTIER_WORKGROUP_SIZE[0] >= node_count,
                "Fix: dominator-frontier grid under-covers node_count={node_count}"
            );
            assert!(
                grid[0] == 1 || (grid[0] - 1) * DOMINATOR_FRONTIER_WORKGROUP_SIZE[0] < node_count,
                "Fix: dominator-frontier grid over-launches an avoidable extra block at node_count={node_count}"
            );
        }
    }

    #[test]
    fn dispatch_plan_owns_buffer_slots_grid_and_readback_shape() {
        let plan = plan_dominator_frontier_dispatch(
            4,
            &[0, 4, 5, 6, 7],
            &[0, 1, 2, 3, 1, 2, 3],
            &[0, 0, 1, 2, 4],
            &[0, 1, 2, 3],
            &[0b0010],
            "seed",
            "frontier_out",
        )
        .expect("Fix: canonical dominance-frontier plan should validate");

        assert_eq!(plan.dispatch_grid(), [1, 1, 1]);
        assert_eq!(plan.frontier_words(), 1);
        assert_eq!(plan.dom_target_words(), 7);
        assert_eq!(plan.pred_target_words(), 4);
        assert_eq!(
            plan.program().workgroup_size,
            DOMINATOR_FRONTIER_WORKGROUP_SIZE
        );
        let bindings = plan
            .program()
            .buffers
            .iter()
            .map(|buffer| buffer.binding)
            .collect::<Vec<_>>();
        assert_eq!(
            bindings,
            vec![
                DOMINATOR_FRONTIER_DOM_OFFSETS_BUFFER,
                DOMINATOR_FRONTIER_DOM_TARGETS_BUFFER,
                DOMINATOR_FRONTIER_PRED_OFFSETS_BUFFER,
                DOMINATOR_FRONTIER_PRED_TARGETS_BUFFER,
                DOMINATOR_FRONTIER_SEED_BUFFER,
                DOMINATOR_FRONTIER_OUT_BUFFER,
            ]
        );
    }

    #[test]
    fn dispatch_plan_pads_empty_target_buffers_without_hiding_empty_offsets() {
        let plan = plan_dominator_frontier_dispatch(
            1,
            &[0, 0],
            &[],
            &[0, 0],
            &[],
            &[1],
            "seed",
            "frontier_out",
        )
        .expect("Fix: empty edge sets are valid CSR inputs");

        assert_eq!(plan.layout().dom_edge_count, 0);
        assert_eq!(plan.layout().pred_edge_count, 0);
        assert_eq!(plan.dom_target_words(), 1);
        assert_eq!(plan.pred_target_words(), 1);
    }

    #[test]
    fn frontier_size_counts_set_bits() {
        assert_eq!(frontier_size(&[0]), 0);
        assert_eq!(frontier_size(&[0b1011]), 3);
        assert_eq!(frontier_size(&[u32::MAX, 1]), 33);
    }

    #[test]
    fn checked_builder_rejects_offset_count_overflow() {
        let error = try_dominator_frontier(u32::MAX, 0, 0, "seed", "out")
            .expect_err("checked dominator-frontier builder must reject CSR offset overflow");

        assert!(
            error.contains("overflows CSR offset buffer count"),
            "error should describe the CSR offset overflow: {error}"
        );
    }

    #[test]
    fn legacy_builder_fails_fast_on_offset_count_overflow() {
        let panic = std::panic::catch_unwind(|| {
            let _ = dominator_frontier(u32::MAX, 0, 0, "seed", "out");
        })
        .expect_err("legacy dominator-frontier builder must fail fast on CSR offset overflow");

        let message = panic_payload_message(panic);
        assert!(
            message.contains("overflows CSR offset buffer count"),
            "error should describe the CSR offset overflow: {message}"
        );
    }

    fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            message.to_string()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            format!("{payload:?}")
        }
    }

    #[test]
    fn dominator_frontier_release_builder_has_checked_api_without_panics() {
        let source = include_str!("dominator_frontier.rs");
        let production = source
            .split("/// CPU oracle:")
            .next()
            .expect("Fix: dominator-frontier builder source must precede CPU oracle");

        assert!(
            production.contains("pub fn try_dominator_frontier(")
                && !production.contains("inert_")
                && !production.contains("Err(_) =>")
                && !production.contains("Node::return_()"),
            "Fix: dominator_frontier builder must expose checked release API and must not compile inert no-op kernels."
        );
    }

    #[test]
    fn missing_seed_word_fails_loudly() {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let err = std::panic::catch_unwind(|| {
            let _ = cpu_ref(2, &[0, 0, 0], &[], &[0, 0, 0], &[], &[]);
        });
        std::panic::set_hook(previous_hook);

        let payload = err.expect_err("missing seed word must fail loudly");
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("<non-string panic>");
        assert!(
            message.contains("expected seed length"),
            "Fix: missing seed panic should explain the exact seed length mismatch, got: {message}"
        );
    }
}
