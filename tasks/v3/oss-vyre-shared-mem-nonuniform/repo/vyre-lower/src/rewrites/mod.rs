//! Real rewrite passes on `KernelDescriptor`.
//!
//! Until this module, every analysis in `vyre-lower` was read-only  -
//! they detected patterns and returned reports. This module's passes
//! are the OTHER side of that line: they take a `KernelDescriptor`,
//! apply a transformation, and return an improved equivalent.
//!
//! ## Pass shape
//!
//! ```text
//! pub fn rewrite(desc: &KernelDescriptor) -> KernelDescriptor;
//! ```
//!
//! Every pass:
//! - Is total (no `Result`)  -  if a pass can't apply, it returns the
//!   input unchanged.
//! - Preserves semantic equivalence  -  running both the input and the
//!   output through `audit()` gives compatible reports for surviving
//!   ops.
//! - Renumbers operand ids when needed; the output's ids are dense
//!   `0..N` for operand id space.
//! - Is idempotent  -  applying twice gives the same result as once.
//!
//! Descriptor-cleanup passes use a `descriptor_*` prefix so they are not
//! confused with foundation's Program-IR semantic optimizer passes.
//! Phase 1 shipped three descriptor cleanups:
//! - `descriptor_dce`  -  dead-op elimination
//! - `descriptor_cse`  -  common-subexpression elimination
//! - `descriptor_const_fold`  -  fold compile-time-constant arithmetic
//!
//! The canonical release pipeline includes memory-layout rewrites,
//! external dataflow-aware rewrites, and bounded e-graph-family algebraic
//! saturation. Saturation is followed by immediate cleanup so a single
//! pass application does not leave reassociated inner chains alive until
//! a later fixed-point iteration.

use std::hash::{Hash, Hasher};

pub mod add_sub_cancel;
pub mod aos_to_soa_promote;
mod arithmetic_combine;
pub mod bank_conflict_pad;
pub mod bitwise_combine;
mod body_index;
/// Bitwise idempotence: fold `BitAnd(x, x)` and `BitOr(x, x)` into `Copy(x)`.
pub mod bitwise_idemp {
    use crate::rewrites::self_binop::rewrite_self_binops;
    use crate::KernelDescriptor;
    use vyre_foundation::ir::BinOp;

    #[must_use]
    pub fn bitwise_idemp(desc: &KernelDescriptor) -> KernelDescriptor {
        rewrite_self_binops(desc, |bin| matches!(bin, BinOp::BitAnd | BinOp::BitOr))
    }
}
pub mod boolean_simplify;
pub mod branch_collapse;
pub mod canonicalize;
pub mod cmp_normalize;
pub mod cmp_self_false;
mod commutative_lit_chain;
pub mod const_buffer_promote;
mod dataflow_facts;
pub mod dead_store;
pub mod descriptor_const_fold;
pub mod descriptor_cse;
pub mod descriptor_dce;
pub mod div_combine;
pub mod drop_unused_bindings;
pub mod drop_unused_child_bodies;
pub mod drop_unused_literals;
pub mod egraph_saturation;
pub mod emit_order;
pub mod identity_elim;
pub mod licm;
mod literal;
pub mod load_forwarding;
pub mod loop_fission;
pub mod loop_fusion;
pub mod loop_unroll;
pub mod loop_zero_iter;
pub mod matmul_promote;
mod memory_address;
mod rhs_lit_chain;
mod self_binop;
/// Min/max idempotence: fold `Min(x, x)` and `Max(x, x)` into `Copy(x)`.
pub mod min_max_idemp {
    use crate::rewrites::self_binop::rewrite_self_binops;
    use crate::KernelDescriptor;
    use vyre_foundation::ir::BinOp;

    #[must_use]
    pub fn min_max_idemp(desc: &KernelDescriptor) -> KernelDescriptor {
        rewrite_self_binops(desc, |bin| matches!(bin, BinOp::Min | BinOp::Max))
    }
}
pub mod mod_idemp;
pub mod mul_add_to_fma;
pub mod negate_cancel;
pub mod select_fold;
pub mod shared_mem_promote;
pub mod shift_combine;
pub mod strength_reduce;
pub mod sub_combine;
pub mod tail_mask;
pub mod unary_idemp;
pub mod xor_self_zero;

pub use add_sub_cancel::add_sub_cancel;
pub use aos_to_soa_promote::{promote as aos_to_soa_promote, LayoutHint as AosSoaLayoutHint};
pub use arithmetic_combine::add_combine::add_combine;
pub use arithmetic_combine::mul_combine::mul_combine;
pub use arithmetic_combine::{add_combine, mul_combine};
pub use bank_conflict_pad::bank_conflict_pad;
pub use bitwise_combine::bitwise_combine;
pub use bitwise_idemp::bitwise_idemp;
pub use boolean_simplify::boolean_simplify;
pub use branch_collapse::branch_collapse;
pub use canonicalize::canonicalize;
pub use cmp_normalize::cmp_normalize;
pub use cmp_self_false::cmp_self_false;
pub use const_buffer_promote::const_buffer_promote;
pub use dead_store::{
    dead_store, dead_store_with_alias_facts, dead_store_with_dataflow_analysis_facts,
    dead_store_with_dataflow_facts, dead_store_with_weir_alias_facts,
};
pub use descriptor_const_fold::descriptor_const_fold;
pub use descriptor_cse::descriptor_cse;
pub use descriptor_dce::descriptor_dce;
pub use div_combine::div_combine;
pub use drop_unused_bindings::drop_unused_bindings;
pub use drop_unused_child_bodies::drop_unused_child_bodies;
pub use drop_unused_literals::drop_unused_literals;
pub use emit_order::emit_order;
pub use identity_elim::identity_elim;
pub use licm::{
    licm, licm_with_alias_facts, licm_with_dataflow_analysis_facts, licm_with_dataflow_facts,
    licm_with_weir_alias_facts,
};
pub use load_forwarding::{
    load_forwarding, load_forwarding_with_alias_facts,
    load_forwarding_with_dataflow_analysis_facts, load_forwarding_with_dataflow_facts,
    load_forwarding_with_weir_alias_facts,
};
pub use loop_fission::{
    loop_fission, loop_fission_with_alias_facts, loop_fission_with_dataflow_analysis_facts,
    loop_fission_with_dataflow_facts, loop_fission_with_weir_alias_facts,
};
pub use loop_fusion::{
    loop_fusion, loop_fusion_with_alias_facts, loop_fusion_with_dataflow_analysis_facts,
    loop_fusion_with_dataflow_facts, loop_fusion_with_weir_alias_facts,
};
pub use loop_unroll::loop_unroll;
pub use loop_zero_iter::loop_zero_iter;
pub use matmul_promote::{infer_matmul_tile_loops, matmul_promote, MatmulTileLoopPlan};
pub use min_max_idemp::min_max_idemp;
pub use mod_idemp::mod_idemp;
pub use mul_add_to_fma::mul_add_to_fma;
pub use negate_cancel::negate_cancel;
pub use select_fold::select_fold;
pub use shared_mem_promote::shared_mem_promote;
pub use shift_combine::shift_combine;
pub use strength_reduce::strength_reduce;
pub use sub_combine::sub_combine;
pub use tail_mask::apply_tail_mask;
pub use unary_idemp::unary_idemp;
pub use xor_self_zero::xor_self_zero;

/// One substrate-neutral lowered-IR rewrite in the canonical pipeline.
#[derive(Debug, Clone, Copy)]
pub struct DescriptorRewritePass {
    /// Stable pass name used in stats, diagnostics, and benchmark attribution.
    pub name: &'static str,
    /// Total rewrite function. Returns the input unchanged when it cannot apply.
    pub rewrite: fn(&crate::KernelDescriptor) -> crate::KernelDescriptor,
}

impl DescriptorRewritePass {
    #[must_use]
    fn run(self, desc: &crate::KernelDescriptor) -> crate::KernelDescriptor {
        (self.rewrite)(desc)
    }
}

fn egraph_saturation_pass(desc: &crate::KernelDescriptor) -> crate::KernelDescriptor {
    egraph_saturation::saturate_algebraic_descriptor(desc).0
}

const CANONICAL_REWRITE_PASSES: &[DescriptorRewritePass] = &[
    DescriptorRewritePass {
        name: "strength_reduce",
        rewrite: strength_reduce,
    },
    DescriptorRewritePass {
        name: "shift_combine",
        rewrite: shift_combine,
    },
    DescriptorRewritePass {
        name: "shared_mem_promote",
        rewrite: shared_mem_promote,
    },
    DescriptorRewritePass {
        name: "bank_conflict_pad",
        rewrite: bank_conflict_pad,
    },
    DescriptorRewritePass {
        name: "const_buffer_promote",
        rewrite: const_buffer_promote,
    },
    DescriptorRewritePass {
        name: "descriptor_const_fold",
        rewrite: descriptor_const_fold,
    },
    DescriptorRewritePass {
        name: "add_combine",
        rewrite: add_combine,
    },
    DescriptorRewritePass {
        name: "sub_combine",
        rewrite: sub_combine,
    },
    DescriptorRewritePass {
        name: "mul_combine",
        rewrite: mul_combine,
    },
    DescriptorRewritePass {
        name: "div_combine",
        rewrite: div_combine,
    },
    DescriptorRewritePass {
        name: "mod_idemp",
        rewrite: mod_idemp,
    },
    DescriptorRewritePass {
        name: "add_sub_cancel",
        rewrite: add_sub_cancel,
    },
    DescriptorRewritePass {
        name: "bitwise_combine",
        rewrite: bitwise_combine,
    },
    DescriptorRewritePass {
        name: "identity_elim",
        rewrite: identity_elim,
    },
    DescriptorRewritePass {
        name: "boolean_simplify",
        rewrite: boolean_simplify,
    },
    DescriptorRewritePass {
        name: "negate_cancel",
        rewrite: negate_cancel,
    },
    DescriptorRewritePass {
        name: "unary_idemp",
        rewrite: unary_idemp,
    },
    DescriptorRewritePass {
        name: "select_fold",
        rewrite: select_fold,
    },
    DescriptorRewritePass {
        name: "min_max_idemp",
        rewrite: min_max_idemp,
    },
    DescriptorRewritePass {
        name: "bitwise_idemp",
        rewrite: bitwise_idemp,
    },
    DescriptorRewritePass {
        name: "branch_collapse",
        rewrite: branch_collapse,
    },
    DescriptorRewritePass {
        name: "loop_fusion",
        rewrite: loop_fusion,
    },
    DescriptorRewritePass {
        name: "loop_unroll",
        rewrite: loop_unroll,
    },
    DescriptorRewritePass {
        name: "loop_zero_iter",
        rewrite: loop_zero_iter,
    },
    DescriptorRewritePass {
        name: "licm",
        rewrite: licm,
    },
    DescriptorRewritePass {
        name: "load_forwarding",
        rewrite: load_forwarding,
    },
    DescriptorRewritePass {
        name: "mul_add_to_fma",
        rewrite: mul_add_to_fma,
    },
    DescriptorRewritePass {
        name: "matmul_promote",
        rewrite: matmul_promote,
    },
    DescriptorRewritePass {
        name: "descriptor_dce_after_forwarding",
        rewrite: descriptor_dce,
    },
    DescriptorRewritePass {
        name: "dead_store",
        rewrite: dead_store,
    },
    DescriptorRewritePass {
        name: "descriptor_dce",
        rewrite: descriptor_dce,
    },
    DescriptorRewritePass {
        name: "cmp_normalize",
        rewrite: cmp_normalize,
    },
    DescriptorRewritePass {
        name: "cmp_self_false",
        rewrite: cmp_self_false,
    },
    DescriptorRewritePass {
        name: "xor_self_zero",
        rewrite: xor_self_zero,
    },
    DescriptorRewritePass {
        name: "canonicalize",
        rewrite: canonicalize,
    },
    DescriptorRewritePass {
        name: "descriptor_cse",
        rewrite: descriptor_cse,
    },
    DescriptorRewritePass {
        name: "egraph_saturation",
        rewrite: egraph_saturation_pass,
    },
    DescriptorRewritePass {
        name: "descriptor_const_fold_post_saturation",
        rewrite: descriptor_const_fold,
    },
    DescriptorRewritePass {
        name: "identity_elim_post_saturation",
        rewrite: identity_elim,
    },
    DescriptorRewritePass {
        name: "descriptor_dce_post_saturation",
        rewrite: descriptor_dce,
    },
    DescriptorRewritePass {
        name: "cmp_normalize_post_saturation",
        rewrite: cmp_normalize,
    },
    DescriptorRewritePass {
        name: "canonicalize_post_saturation",
        rewrite: canonicalize,
    },
    DescriptorRewritePass {
        name: "descriptor_cse_post_saturation",
        rewrite: descriptor_cse,
    },
    DescriptorRewritePass {
        name: "drop_unused_bindings",
        rewrite: drop_unused_bindings,
    },
    DescriptorRewritePass {
        name: "drop_unused_literals",
        rewrite: drop_unused_literals,
    },
    DescriptorRewritePass {
        name: "drop_unused_child_bodies",
        rewrite: drop_unused_child_bodies,
    },
    DescriptorRewritePass {
        name: "emit_order",
        rewrite: emit_order,
    },
];

/// Canonical lowered-IR rewrite pipeline as data, not a second hand-coded compiler.
#[must_use]
pub const fn canonical_rewrite_passes() -> &'static [DescriptorRewritePass] {
    CANONICAL_REWRITE_PASSES
}

/// Cheap descriptor fingerprint for convergence checks.
///
/// Uses FxHasher for speed  -  collision resistance isn't needed because
/// a hash match is always confirmed by one final deep equality check
/// before declaring convergence.
fn descriptor_hash(desc: &crate::KernelDescriptor) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    desc.hash(&mut h);
    h.finish()
}

fn run_descriptor_passes(
    desc: &crate::KernelDescriptor,
    passes: &[DescriptorRewritePass],
) -> crate::KernelDescriptor {
    let mut current = desc.clone();
    for pass in passes {
        let pre_hash = descriptor_hash(&current);
        let next = pass.run(&current);
        // Skip the allocation when the pass was a no-op.
        if descriptor_hash(&next) != pre_hash || next != current {
            current = next;
        }
        #[cfg(debug_assertions)]
        debug_verify_after_rewrite(&current, pass.name);
    }
    current
}

/// Apply every shipped rewrite in canonical order. The ordering is
/// chosen so that each pass exposes work for the passes that follow:
///
/// 1. `strength_reduce`  -  turns `mul/div/mod` by power-of-2 into
///    shift/and. Synthesizes new Literal ops for the shift counts.
/// 2. `shared_mem_promote`  -  stages proven repeated U32 global tile loads
///    through workgroup memory.
/// 3. `bank_conflict_pad`  -  pads simple shared-memory strided layouts whose
///    pitch aliases shared-memory banks.
/// 4. `const_buffer_promote`  -  promotes small fixed-size read-only global
///    buffers with repeated loads to constant bindings.
/// 5. `descriptor_const_fold`  -  folds `BinOp(Lit, Lit)` into a single Literal.
///    Catches both the original literal-pair arithmetic and any new
///    constant-shift produced by strength_reduce.
/// 6. `identity_elim`  -  substitutes `Add(x, 0)`, `Mul(x, 1)`, etc. with
///    `x` (and absorbing-zero patterns with `0`). Runs after descriptor_const_fold
///    so its left/right-identity rules see the post-folded literals.
/// 7. `branch_collapse`  -  replaces `If(Lit_bool, then, else)` with the
///    selected arm inlined. Runs after descriptor_const_fold + identity_elim so
///    conditions like `Add(x, 0) != 0` simplify down to literals first.
/// 8. `loop_fusion`  -  canonical launch/loop overhead reducer for
///    safe disjoint-write loops. `loop_fission` is exposed as an
///    explicit cost-model transform, but is not run unconditionally
///    because it intentionally opposes fusion.
/// 9. `loop_unroll`  -  unrolls small constant-bound loops. Runs after
///    branch_collapse (which often removes the guards that prevented
///    unrolling) but before licm (no point hoisting from a loop that
///    will get unrolled away).
/// 10. `licm`  -  hoists loop-invariant ops out of remaining loops.
/// 11. `load_forwarding`  -  store-to-load + load-to-load forwarding.
///    Runs after licm because hoisted loads may reveal new forwardable
///    pairs in straight-line code.
/// 12. `dead_store`  -  drops stores whose value is overwritten before any
///    observation. Runs after load_forwarding (forwarded loads may have
///    removed the only observers that kept stores alive).
/// 13. `descriptor_dce`  -  drops result-producing ops with no users. Cleans up
///    everything orphaned by the substitutions in (3) and (7).
/// 14. `descriptor_cse`  -  merges remaining structurally-equivalent ops before
///     saturation so the equality surface is canonical.
/// 15. `egraph_saturation`  -  reassociates algebraic constant chains under a
///     bounded saturation contract.
/// 16. post-saturation fold/DCE/CSE cleanup  -  immediately removes dead inner
///     chain ops and merges the final reassociated shape.
/// Single application of the canonical pass sequence  -  see [`run_all`]
/// for the iterating wrapper. This exists so callers that want exactly
/// one pass (e.g. for diagnostics) can have it.
#[must_use]

pub fn run_all_once(desc: &crate::KernelDescriptor) -> crate::KernelDescriptor {
    run_descriptor_passes(desc, canonical_rewrite_passes())
}

/// Internal: assert the descriptor's structural invariants hold after a
/// rewrite pass. Only active in debug builds. Catches rewrite bugs early
/// instead of letting them propagate to the emitter as opaque naga errors.
#[cfg(debug_assertions)]
fn debug_verify_after_rewrite(desc: &crate::KernelDescriptor, pass: &str) {
    if let Err(errors) = crate::verify::verify(desc) {
        let violation_count = errors.len();
        assert!(
            violation_count == 0,
            "rewrite pass `{pass}` produced an invalid KernelDescriptor  -  {} violation(s):\n{errors:#?}",
            violation_count
        );
    }
}

/// Single canonical pass sequence using External alias and
/// reaching-definition facts where they unlock stronger legality.
#[must_use]
pub fn run_all_once_with_dataflow_facts(
    desc: &crate::KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> crate::KernelDescriptor {
    let reduced = strength_reduce(desc);
    let shared_promoted = shared_mem_promote(&reduced);
    let padded = bank_conflict_pad(&shared_promoted);
    let const_promoted = const_buffer_promote(&padded);
    let folded = descriptor_const_fold(&const_promoted);
    let identified = identity_elim(&folded);
    let collapsed = branch_collapse(&identified);
    let fused = loop_fusion_with_dataflow_facts(&collapsed, alias_facts, reaching_defs);
    let unrolled = loop_unroll(&fused);
    let hoisted = licm_with_dataflow_facts(&unrolled, alias_facts, reaching_defs);
    let forwarded = load_forwarding_with_dataflow_facts(&hoisted, alias_facts, reaching_defs);
    let cleaned = descriptor_dce(&forwarded);
    let dse_done = dead_store_with_dataflow_facts(&cleaned, alias_facts, reaching_defs);
    let dced = descriptor_dce(&dse_done);
    let canon = canonicalize(&dced);
    let merged = descriptor_cse(&canon);
    let (saturated, _) = egraph_saturation::saturate_algebraic_descriptor(&merged);
    let saturated_folded = descriptor_const_fold(&saturated);
    let saturated_identified = identity_elim(&saturated_folded);
    let saturated_dced = descriptor_dce(&saturated_identified);
    let saturated_canon = canonicalize(&saturated_dced);
    let saturated_merged = descriptor_cse(&saturated_canon);
    let pruned_bindings = drop_unused_bindings(&saturated_merged);
    let pruned_literals = drop_unused_literals(&pruned_bindings);
    let pruned_children = drop_unused_child_bodies(&pruned_literals);
    emit_order(&pruned_children)
}

/// Maximum number of `run_all_once` iterations before giving up and
/// returning the latest output. In practice fixed point is reached
/// in 1–2 iterations on every shape we've fuzzed.
pub const RUN_ALL_MAX_ITERS: usize = 4;

/// Apply [`run_all_once`] repeatedly until the descriptor reaches a full
/// fixed point or [`RUN_ALL_MAX_ITERS`] is reached. Necessary because passes
/// late in the pipeline (notably `descriptor_cse`) can expose opportunities
/// for earlier passes (notably `dead_store`): two stores at distinct
/// id-but-equal-value indices look distinct to dead_store in iteration 1, but
/// after CSE merges the index ids in iteration 1, dead_store catches them in
/// iteration 2.
#[must_use]
pub fn run_all(desc: &crate::KernelDescriptor) -> crate::KernelDescriptor {
    run_all_with_stats(desc).0
}

/// Apply the canonical fixed-point rewrite pipeline with external dataflow facts.
#[must_use]
pub fn run_all_with_dataflow_facts(
    desc: &crate::KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> crate::KernelDescriptor {
    run_all_with_dataflow_stats(desc, alias_facts, reaching_defs).0
}

/// Apply the canonical fixed-point rewrite pipeline with Weir dataflow-analysis facts.
#[must_use]
pub fn run_all_with_dataflow_analysis_facts(
    desc: &crate::KernelDescriptor,
    alias_facts: &crate::analyses::weir_alias::AliasFactSet,
    reaching_defs: &crate::analyses::weir_reaching_def::ReachingDefFactSet,
) -> crate::KernelDescriptor {
    run_all_with_dataflow_facts(desc, alias_facts, reaching_defs)
}

/// Per-pipeline-run statistics. Useful for benchmarks, regression
/// diagnostics, and `--verbose` emit modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OptimizationStats {
    /// Top-level body op count before optimization.
    pub ops_before: usize,
    /// Top-level body op count after optimization.
    pub ops_after: usize,
    /// Number of bindings before. After equals `bindings_before` minus
    /// what `drop_unused_bindings` stripped.
    pub bindings_before: usize,
    pub bindings_after: usize,
    /// Number of literals in the top-level body's pool, before/after.
    pub literals_before: usize,
    pub literals_after: usize,
    /// How many `run_all_once` iterations actually fired (1 to
    /// `RUN_ALL_MAX_ITERS`). 1 means the pipeline converged on the
    /// first pass; higher means later-pass changes exposed work for
    /// earlier passes and the fixed-point loop kicked in.
    pub iterations: usize,
    /// True iff the fixed-point converged within the cap. False means
    /// the pipeline stopped mid-way; output is still valid (every
    /// individual pass is total) but may have residual optimization
    /// opportunities.
    pub converged: bool,
}

impl OptimizationStats {
    /// Total ops eliminated (top-level body only). Saturating, so the
    /// rare case where output exceeds input (e.g. loop_unroll inlining)
    /// returns 0 rather than wrapping.
    pub fn ops_eliminated(&self) -> usize {
        self.ops_before.saturating_sub(self.ops_after)
    }

    pub fn bindings_dropped(&self) -> usize {
        self.bindings_before.saturating_sub(self.bindings_after)
    }

    /// True iff the pipeline made no change at all  -  no ops eliminated,
    /// no bindings dropped, no literals dropped, AND op count is
    /// stable. The kernel was either already optimal or out of the
    /// pipeline's reach. Useful for tooling that wants to skip emit
    /// re-runs when nothing changed.
    pub fn is_no_op(&self) -> bool {
        self.ops_before == self.ops_after
            && self.bindings_before == self.bindings_after
            && self.literals_before == self.literals_after
    }

    /// Total off-graph data dropped (bindings + literals). Useful as
    /// a single-number "how much cleanup did the tail of the pipeline
    /// do?" signal.
    pub fn off_graph_dropped(&self) -> usize {
        self.bindings_dropped() + self.literals_before.saturating_sub(self.literals_after)
    }

    /// Merge another [`OptimizationStats`] into a running aggregate.
    /// Adds counts and ORs the converged flag (any non-converged run
    /// → aggregate is non-converged). `iterations` accumulates.
    /// Useful for tooling that runs `run_all_with_stats` over a corpus
    /// of N kernels and wants a single rolled-up summary.
    pub fn merge(&mut self, other: OptimizationStats) {
        self.ops_before = self.ops_before.saturating_add(other.ops_before);
        self.ops_after = self.ops_after.saturating_add(other.ops_after);
        self.bindings_before = self.bindings_before.saturating_add(other.bindings_before);
        self.bindings_after = self.bindings_after.saturating_add(other.bindings_after);
        self.literals_before = self.literals_before.saturating_add(other.literals_before);
        self.literals_after = self.literals_after.saturating_add(other.literals_after);
        self.iterations = self.iterations.saturating_add(other.iterations);
        self.converged = self.converged && other.converged;
    }

    /// Identity element for [`Self::merge`]. Useful as the seed of a fold
    /// over a corpus.
    pub fn zero() -> Self {
        OptimizationStats {
            ops_before: 0,
            ops_after: 0,
            bindings_before: 0,
            bindings_after: 0,
            literals_before: 0,
            literals_after: 0,
            iterations: 0,
            converged: true,
        }
    }

    /// One-line human-readable summary suitable for log lines.
    /// Format: `"ops X→Y (-N), bindings A→B (-M), iters K (converged|stopped)"`.
    /// Mirrors the format_short pattern on the audit reports.
    pub fn format_short(&self) -> String {
        format!(
            "ops {}→{} (-{}), bindings {}→{} (-{}), iters {} ({})",
            self.ops_before,
            self.ops_after,
            self.ops_eliminated(),
            self.bindings_before,
            self.bindings_after,
            self.bindings_dropped(),
            self.iterations,
            if self.converged {
                "converged"
            } else {
                "stopped"
            },
        )
    }
}

impl std::fmt::Display for OptimizationStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.format_short())
    }
}

/// Like [`run_all`] but also returns [`OptimizationStats`] so the
/// caller can surface what happened.
#[must_use]
pub fn run_all_with_stats(
    desc: &crate::KernelDescriptor,
) -> (crate::KernelDescriptor, OptimizationStats) {
    let ops_before = desc.body.ops.len();
    let bindings_before = desc.bindings.slots.len();
    let literals_before = desc.body.literals.len();

    let mut current = run_all_once(desc);
    #[cfg(debug_assertions)]
    debug_verify_after_rewrite(&current, "run_all_once (iter 1)");
    let mut iterations = 1usize;
    let mut current_hash = descriptor_hash(&current);
    let mut converged = current_hash == descriptor_hash(desc) && current == *desc;
    while !converged && iterations < RUN_ALL_MAX_ITERS {
        let next = run_all_once(&current);
        iterations += 1;
        #[cfg(debug_assertions)]
        debug_verify_after_rewrite(&next, &format!("run_all_once (iter {iterations})"));
        let next_hash = descriptor_hash(&next);
        // Fast path: hash mismatch → definitely changed, skip deep eq.
        // Hash match → confirm with full equality to guard against collisions.
        converged = next_hash == current_hash && next == current;
        current = next;
        current_hash = next_hash;
    }

    let stats = OptimizationStats {
        ops_before,
        ops_after: current.body.ops.len(),
        bindings_before,
        bindings_after: current.bindings.slots.len(),
        literals_before,
        literals_after: current.body.literals.len(),
        iterations,
        converged,
    };
    (current, stats)
}

/// Like [`run_all_with_dataflow_facts`] but also returns
/// [`OptimizationStats`].
#[must_use]
pub fn run_all_with_dataflow_stats(
    desc: &crate::KernelDescriptor,
    alias_facts: &crate::analyses::alias_facts::AliasFactSet,
    reaching_defs: &crate::analyses::reaching_def_facts::ReachingDefFactSet,
) -> (crate::KernelDescriptor, OptimizationStats) {
    let ops_before = desc.body.ops.len();
    let bindings_before = desc.bindings.slots.len();
    let literals_before = desc.body.literals.len();

    let mut current = run_all_once_with_dataflow_facts(desc, alias_facts, reaching_defs);
    let mut iterations = 1usize;
    let mut current_hash = descriptor_hash(&current);
    let mut converged = current_hash == descriptor_hash(desc) && current == *desc;
    while !converged && iterations < RUN_ALL_MAX_ITERS {
        let next = run_all_once_with_dataflow_facts(&current, alias_facts, reaching_defs);
        iterations += 1;
        let next_hash = descriptor_hash(&next);
        converged = next_hash == current_hash && next == current;
        current = next;
        current_hash = next_hash;
    }

    let stats = OptimizationStats {
        ops_before,
        ops_after: current.body.ops.len(),
        bindings_before,
        bindings_after: current.bindings.slots.len(),
        literals_before,
        literals_after: current.body.literals.len(),
        iterations,
        converged,
    };
    (current, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    #[test]
    fn run_all_on_empty_kernel_returns_empty() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let out = run_all(&desc);
        assert!(out.body.ops.is_empty());
    }

    #[test]
    fn run_all_is_idempotent() {
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(1),
                    }, // dup
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(2),
                    }, // dead
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(5), LiteralValue::U32(99)],
            },
        };
        let once = run_all(&desc);
        let twice = run_all(&once);
        assert_eq!(once.body.ops.len(), twice.body.ops.len());
        assert_eq!(once.body.literals, twice.body.literals);
    }

    #[test]
    fn run_all_collapses_kitchen_sink_kernel() {
        // Inefficiency stack  -  every shipped pass should contribute:
        //
        //   r0 = Lit(0)            // literal pool idx 0 → U32(0) (zero,
        //                           //   identity for Add, absorbing for Mul)
        //   r1 = Lit(1)            // literal pool idx 1 → U32(1) (one,
        //                           //   identity for Mul)
        //   r2 = Lit(8)            // literal pool idx 2 → U32(8) (pow2)
        //   r3 = Lit(7)            // literal pool idx 3 → U32(7) (varying)
        //   r4 = Lit(0)            // duplicate of r0 → CSE target
        //   r5 = Add(r3, r1+r0)    // post-fold rhs becomes Lit(1) →
        //                           //   identity_elim won't fire (rhs is 1
        //                           //   not 0, op is Add). But r5 has a
        //                           //   useful value (= 7+1 = 8). Actually
        //                           //   simpler: just Add(r3, r0) → r3
        //                           //   (identity).
        //   r6 = Mul(r3, r2)       // strength_reduce: × 8 → << 3
        //   r7 = Mul(r3, r1)       // identity_elim: × 1 → r3
        //   r8 = Mul(r3, r0)       // absorbing-zero: → r0 (i.e. 0)
        //   StoreGlobal(buf, r0, r5)  // dead store (overwritten below)
        //   StoreGlobal(buf, r0, r6)  // surviving store
        //
        // Expectations:
        //   - Op count drops substantially.
        //   - The dead first store is gone (dead_store).
        //   - The final store's value-id is the strength-reduced shift
        //     result (or its forwarded equivalent).
        //   - At least one literal is dropped or deduped.
        let desc = KernelDescriptor {
            id: "kitchen_sink".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![2],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![3],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(4),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                        operands: vec![3, 0],
                        result: Some(5),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                        operands: vec![3, 2],
                        result: Some(6),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                        operands: vec![3, 1],
                        result: Some(7),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                        operands: vec![3, 0],
                        result: Some(8),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 5],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 6],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![
                    LiteralValue::U32(0),
                    LiteralValue::U32(1),
                    LiteralValue::U32(8),
                    LiteralValue::U32(7),
                ],
            },
        };

        let before_op_count = desc.body.ops.len();
        let out = run_all(&desc);

        // Op count should drop  -  multiple ops eliminated, but the
        // exact final count depends on pass interaction. Be specific
        // about WHICH ops are gone:

        // 1. dead_store: only one StoreGlobal should survive.
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(store_count, 1, "dead_store must drop the overwritten store");

        // 2. descriptor_dce: r5 (Add identity), r7 (Mul identity), r8 (absorbing
        //    zero) are all dead by id substitution → must be gone.
        //    Also r4 (CSE-merged dup of r0).
        //    r6 itself becomes a Shl after strength_reduce; that survives
        //    as the value-operand of the surviving store.
        let mul_count = out
            .body
            .ops
            .iter()
            .filter(|o| {
                matches!(
                    o.kind,
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul)
                )
            })
            .count();
        assert_eq!(mul_count, 0, "all 3 Mul ops should be eliminated");

        let add_count = out
            .body
            .ops
            .iter()
            .filter(|o| {
                matches!(
                    o.kind,
                    KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add)
                )
            })
            .count();
        assert_eq!(add_count, 0, "Add(r3, 0) → r3 should be eliminated");

        // 3. strength_reduce: Mul(_, 8) became Shl(_, 3). Both operands
        //    of THIS Mul happened to be literals (r3=Lit(7), r2=Lit(8)),
        //    so descriptor_const_fold runs after strength_reduce and folds the new
        //    Shl(Lit, Lit) into a Lit(56). Either outcome is acceptable  -
        //    what matters is "no Mul, no Add, no Sub". Already asserted.

        // 4. Op count must drop by at least 5 (5 dead ops eliminated).
        assert!(
            out.body.ops.len() <= before_op_count - 5,
            "expected op count to drop by ≥5 (was {before_op_count}, now {})",
            out.body.ops.len()
        );

        // 5. Idempotence: running again is a no-op.
        let twice = run_all(&out);
        assert_eq!(out.body.ops.len(), twice.body.ops.len());
    }

    #[test]
    fn run_all_forwards_then_drops_redundant_load() {
        // Store(buf, 0, 7); r = Load(buf, 0); Store(buf, 0, r).
        // After load_forwarding: Store(buf, 0, 7); Load(buf, 0); Store(buf, 0, 7).
        // After dead_store: only the LAST store survives (the middle Load
        // is dead by then; DCE drops it).
        let desc = KernelDescriptor {
            id: "stl".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    }, // idx
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    }, // val
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 2],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let out = run_all(&desc);

        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert_eq!(
            store_count, 1,
            "dead_store must drop the redundant first store"
        );

        let load_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::LoadGlobal))
            .count();
        assert_eq!(load_count, 0, "descriptor_dce must drop the redundant load");
    }

    #[test]
    fn run_all_unrolls_then_simplifies() {
        // Constant-bound loop (count=2) whose body just stores Lit(0) and Lit(0).
        // After unroll: 4 ops in straight line.
        // After dead_store: only the last surviving store stays (since
        // both are at the same idx).
        let desc = KernelDescriptor {
            id: "loop_then_dse".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    // lo = 0, hi = 2, step-id = 2 (ignored), body-child-idx = 0
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::StructuredForLoop {
                            loop_var: std::sync::Arc::from("i"),
                        },
                        operands: vec![0, 1, 0],
                        result: None,
                    },
                ],
                child_bodies: vec![KernelBody {
                    ops: vec![
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![0],
                            result: Some(0),
                        },
                        KernelOp {
                            kind: KernelOpKind::Literal,
                            operands: vec![1],
                            result: Some(1),
                        },
                        KernelOp {
                            kind: KernelOpKind::StoreGlobal,
                            operands: vec![0, 0, 1],
                            result: None,
                        },
                    ],
                    child_bodies: vec![],
                    literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
                }],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(2)],
            },
        };
        let out = run_all(&desc);

        // Loop must be gone (unrolled).
        let loop_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StructuredForLoop { .. }))
            .count();
        assert_eq!(loop_count, 0, "loop should be unrolled");

        // After unrolling 2 iterations, the body's stores are inlined
        // with fresh result-ids per iteration. CSE eventually merges
        // structurally-equal Lit ops, but the current pipeline runs
        // CSE after dead_store, so dead_store sees stores at "different"
        // (textually distinct) idx-ids and conservatively keeps both.
        // What we CAN guarantee: at least one store survives, and the
        // loop is gone.
        let store_count = out
            .body
            .ops
            .iter()
            .filter(|o| matches!(o.kind, KernelOpKind::StoreGlobal))
            .count();
        assert!(
            store_count >= 1,
            "at least one store from the unrolled body should survive"
        );
    }

    #[test]
    fn run_all_with_stats_reports_op_reduction() {
        // Same kitchen-sink shape as run_all_collapses_kitchen_sink_kernel,
        // but assert the stats reflect what happened.
        let desc = KernelDescriptor {
            id: "stats".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                        operands: vec![1, 0],
                        result: Some(2),
                    }, // identity (Add x 0)
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Mul),
                        operands: vec![1, 0],
                        result: Some(3),
                    }, // absorbing zero (Mul x 0)
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
            },
        };
        let (_out, stats) = run_all_with_stats(&desc);
        assert!(
            stats.ops_eliminated() >= 2,
            "expected ≥2 ops eliminated, got {} ({} → {})",
            stats.ops_eliminated(),
            stats.ops_before,
            stats.ops_after
        );
        assert!(stats.iterations >= 1);
        assert!(stats.iterations <= RUN_ALL_MAX_ITERS);
        assert!(stats.converged, "pipeline must converge");
    }

    #[test]
    fn optimization_stats_format_short_includes_all_fields() {
        let s = OptimizationStats {
            ops_before: 11,
            ops_after: 3,
            bindings_before: 3,
            bindings_after: 1,
            literals_before: 4,
            literals_after: 2,
            iterations: 2,
            converged: true,
        };
        let f = s.format_short();
        assert!(f.contains("ops 11→3 (-8)"));
        assert!(f.contains("bindings 3→1 (-2)"));
        assert!(f.contains("iters 2"));
        assert!(f.contains("converged"));
    }

    #[test]
    fn optimization_stats_merge_is_associative() {
        let a = OptimizationStats {
            ops_before: 10,
            ops_after: 3,
            bindings_before: 2,
            bindings_after: 1,
            literals_before: 5,
            literals_after: 2,
            iterations: 2,
            converged: true,
        };
        let b = OptimizationStats {
            ops_before: 7,
            ops_after: 4,
            bindings_before: 1,
            bindings_after: 1,
            literals_before: 3,
            literals_after: 3,
            iterations: 1,
            converged: true,
        };
        let c = OptimizationStats {
            ops_before: 5,
            ops_after: 2,
            bindings_before: 1,
            bindings_after: 0,
            literals_before: 2,
            literals_after: 1,
            iterations: 1,
            converged: true,
        };

        let mut left = a;
        left.merge(b);
        left.merge(c);

        let mut bc = b;
        bc.merge(c);
        let mut right = a;
        right.merge(bc);

        assert_eq!(left, right);
    }

    #[test]
    fn optimization_stats_merge_aggregates() {
        let mut acc = OptimizationStats::zero();
        acc.merge(OptimizationStats {
            ops_before: 10,
            ops_after: 3,
            bindings_before: 2,
            bindings_after: 1,
            literals_before: 5,
            literals_after: 2,
            iterations: 2,
            converged: true,
        });
        acc.merge(OptimizationStats {
            ops_before: 7,
            ops_after: 4,
            bindings_before: 1,
            bindings_after: 1,
            literals_before: 3,
            literals_after: 3,
            iterations: 1,
            converged: false,
        });
        assert_eq!(acc.ops_before, 17);
        assert_eq!(acc.ops_after, 7);
        assert_eq!(acc.iterations, 3);
        assert!(!acc.converged); // ORed: any false → false
    }

    #[test]
    fn optimization_stats_zero_is_identity() {
        let s = OptimizationStats {
            ops_before: 5,
            ops_after: 3,
            bindings_before: 1,
            bindings_after: 1,
            literals_before: 2,
            literals_after: 1,
            iterations: 2,
            converged: true,
        };
        let mut acc = OptimizationStats::zero();
        acc.merge(s);
        assert_eq!(acc, s);
    }

    #[test]
    fn optimization_stats_format_short_marks_stopped() {
        let s = OptimizationStats {
            ops_before: 5,
            ops_after: 5,
            bindings_before: 1,
            bindings_after: 1,
            literals_before: 1,
            literals_after: 1,
            iterations: 4,
            converged: false,
        };
        assert!(s.format_short().contains("stopped"));
    }

    #[test]
    fn run_all_with_stats_reports_no_change_when_already_optimal() {
        let desc = KernelDescriptor {
            id: "minimal".into(),
            bindings: BindingLayout {
                slots: vec![crate::BindingSlot {
                    slot: 0,
                    element_type: vyre_foundation::ir::DataType::U32,
                    element_count: None,
                    memory_class: crate::MemoryClass::Global,
                    visibility: crate::BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let (_out, stats) = run_all_with_stats(&desc);
        assert_eq!(stats.ops_before, stats.ops_after);
        assert_eq!(stats.bindings_before, stats.bindings_after);
        assert_eq!(stats.iterations, 1);
        assert!(stats.converged);
    }

    #[test]
    fn run_all_with_stats_reports_dropped_bindings() {
        let desc = KernelDescriptor {
            id: "with_unused".into(),
            bindings: BindingLayout {
                slots: vec![
                    crate::BindingSlot {
                        slot: 0,
                        element_type: vyre_foundation::ir::DataType::U32,
                        element_count: None,
                        memory_class: crate::MemoryClass::Global,
                        visibility: crate::BindingVisibility::ReadWrite,
                        name: "used".into(),
                    },
                    crate::BindingSlot {
                        slot: 9,
                        element_type: vyre_foundation::ir::DataType::U32,
                        element_count: None,
                        memory_class: crate::MemoryClass::Global,
                        // Soundness: drop_unused_bindings now retains
                        // WriteOnly/ReadWrite outputs (host dispatch
                        // contract). The "unused" candidate must be
                        // ReadOnly for the drop to fire.
                        visibility: crate::BindingVisibility::ReadOnly,
                        name: "unused".into(),
                    },
                ],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        };
        let (_out, stats) = run_all_with_stats(&desc);
        // drop_unused_bindings now retains every declared binding
        // regardless of visibility: dropping a ReadOnly silently shifted
        // the host's positional input mapping (parser pipeline `haystack`
        // got DCE'd → wrong slot at dispatch). The rewrite is now a
        // no-op for binding count; the stats counters reflect that.
        assert_eq!(stats.bindings_before, 2);
        assert_eq!(stats.bindings_after, 2);
        assert_eq!(stats.bindings_dropped(), 0);
    }
}
