//! Pre-lowering optimization pipeline.
//!
//! Composes the small set of expression-level passes (`canonicalize`,
//! `region_inline`, `const_fold`, `loop_strip_mine`, `loop_unroll`,
//! `strength_reduce`, `normalize_atomics`, then CSE+DCE) that every backend wants run
//! before lowering. Frontends emit naive IR and rely on this entry
//! to clean it up; backends with fixed bind-group layouts can call
//! it directly without spinning up the full `PassScheduler`.
//!
//! Buffer-level passes (dead_buffer_elim, fusion, autotune) are
//! available via [`crate::optimizer::PassScheduler`] for callers
//! that control the full pipeline and can reconcile ABI changes
//! with their host dispatch.

use crate::ir_inner::model::program::Program;
use crate::optimizer::{
    registered_passes_for_profile, CostModelFamily, OptimizerError, OptimizerProfile, PassPhase,
    PassScheduler, ProgramPassKind,
};
use std::sync::OnceLock;

use crate::optimizer::passes::algebraic::canonicalize_engine;
use crate::optimizer::passes::algebraic::const_fold::ConstFold;
use crate::optimizer::passes::cleanup::region_inline_engine;
use crate::optimizer::passes::cleanup::rematerialize_cheap_let::RematerializeCheapLetPass;
use crate::optimizer::passes::fusion_cse::cse::engine::cse;
use crate::optimizer::passes::fusion_cse::dce::engine::dce;

// Per-phase PassScheduler instances are stateless across runs (their
// only mutation lives inside `run()`'s local variables) so a single
// OnceLock-cached scheduler can serve every `optimize()` invocation.
// Avoids re-running the topological sort + per-pass metadata clone +
// pass_index hashmap construction on every call  -  pre_lowering is on
// the per-program optimization hot path.
static PHASE2_SCHEDULER: OnceLock<Result<PassScheduler, OptimizerError>> = OnceLock::new();
static PHASE4_SCHEDULER: OnceLock<Result<PassScheduler, OptimizerError>> = OnceLock::new();

const PHASE2_SELECTION: &[PassPhase] =
    &[PassPhase::ScalarAlgebra, PassPhase::Loop, PassPhase::Sync];
const PHASE4_SELECTION: &[PassPhase] = &[
    PassPhase::ScalarAlgebra,
    PassPhase::Canonicalization,
    PassPhase::Cleanup,
    PassPhase::FusionCse,
    PassPhase::Memory,
];

fn pre_lowering_scheduler(phases: &'static [PassPhase]) -> Result<PassScheduler, OptimizerError> {
    let passes: Vec<ProgramPassKind> = registered_passes_for_profile(OptimizerProfile::Release)?
        .into_iter()
        .filter(|pass| {
            let metadata = pass.metadata();
            phases.contains(&metadata.phase)
                && metadata.cost_model_family != CostModelFamily::Megakernel
                && metadata.cost_model_family != CostModelFamily::Dataflow
                && !metadata.invalidates.contains(&"buffer_layout")
        })
        .collect();
    Ok(PassScheduler::try_with_passes(passes)?
        .with_cost_monotone_enforcement(true)
        .with_effect_handler_enforcement(true)
        .with_linear_type_enforcement(true)
        .with_shape_predicate_enforcement(true))
}

/// Run the unified pre-lowering optimization pipeline, propagating errors.
///
/// Identical to [`optimize`] but returns `Err(OptimizerError)` when a
/// phase-2 or phase-4 scheduler fails to converge or cannot be constructed,
/// instead of silently returning the un-optimized program. Callers that can
/// handle failure should prefer this function; `optimize` is kept for
/// backward-compatible call sites in crates that cannot handle `Result`.
///
/// # Errors
///
/// Returns `Err` when the phase-2 or phase-4 `PassScheduler` fails to
/// converge within its iteration cap, or when scheduler construction fails
/// due to inconsistent pass metadata.
pub fn try_optimize(program: Program) -> Result<Program, OptimizerError> {
    let prepared = prepare(program);
    let phase2_output = run_phase2(prepared)?;
    let cleaned = cleanup_after_phase2(phase2_output);
    let phase4 = run_phase4(cleaned)?;
    Ok(stabilize(phase4))
}

// ---- Shared pre-lowering pipeline stages -----------------------------------
// `try_optimize` (fallible) and `optimize` (infallible, back-compat) run the
// identical 5-stage pipeline; only the phase-2/phase-4 scheduler error policy
// differs (propagate vs log-and-continue-with-the-stage-input). Factor the
// stages here so the two entry points cannot drift.

/// Phase 1: canonicalize + region-inline into a stable, content-addressable
/// form with a single runnable top level.
fn prepare(program: Program) -> Program {
    region_inline_engine::run(canonicalize_engine::run(program)).reconcile_runnable_top_level()
}

/// Phase 2: expression-level optimizer fixpoint
/// (`const_fold`/`loop_*`/`strength_reduce`/`normalize_atomics`). Logs and
/// returns `Err` on scheduler construction failure or non-convergence; the
/// caller decides whether to propagate or fall back.
fn run_phase2(input: Program) -> Result<Program, OptimizerError> {
    match PHASE2_SCHEDULER.get_or_init(|| pre_lowering_scheduler(PHASE2_SELECTION)) {
        Ok(scheduler) => scheduler.run(input).map_err(|error| {
            tracing::error!(
                error = %error,
                "pre-lowering phase 2 did not converge. Fix: inspect the pass set for oscillating rewrites."
            );
            error
        }),
        Err(error) => {
            tracing::error!(
                error = %error,
                "pre-lowering phase 2 scheduler construction failed. Fix: repair optimizer pass metadata."
            );
            Err(error.clone())
        }
    }
}

/// Phase 3: CSE + DCE, then region-inline (flatten any empty regions DCE
/// exposed) and re-canonicalize so a second optimize run is byte-stable.
fn cleanup_after_phase2(phase2_output: Program) -> Program {
    canonicalize_engine::run(region_inline_engine::run(dce(cse(phase2_output))))
}

/// Phase 4: final ConstFold sweep family. The phase-3 canonicalize can expose
/// new fold-eligible patterns by sorting commutative operands so any literal
/// lands on the right (e.g. an upstream `Ge(t, 0)` folded to `LitBool(true)`
/// then appearing as `And { right: LitBool(true) }`, which `And(x, true) -> x`
/// catches in one more pass). Without this, `optimize(p)` is not idempotent on
/// programs whose Select.cond chains mix literal and non-literal logical ops.
/// Same log-and-Err policy as [`run_phase2`].
fn run_phase4(input: Program) -> Result<Program, OptimizerError> {
    match PHASE4_SCHEDULER.get_or_init(|| pre_lowering_scheduler(PHASE4_SELECTION)) {
        Ok(scheduler) => scheduler.run(input).map_err(|error| {
            tracing::error!(
                error = %error,
                "pre-lowering phase 4 did not converge after 50 iterations. Fix: inspect the phase for oscillating rewrites or raise the cap only with a convergence certificate."
            );
            error
        }),
        Err(error) => {
            tracing::error!(
                error = %error,
                "pre-lowering phase 4 scheduler construction failed. Fix: repair optimizer pass metadata."
            );
            Err(error.clone())
        }
    }
}

/// Phase 5: stabilization sweep. Phase 4 can expose cheap aliases or foldable
/// leaf substitutions after its last CSE/DCE opportunity; finish with the same
/// ABI-preserving cleanup family (twice) so `optimize(optimize(p)) == optimize(p)`
/// for backend-visible IR.
fn stabilize(phase4: Program) -> Program {
    let rematerialized = RematerializeCheapLetPass::transform(phase4).program;
    let folded = ConstFold::transform(canonicalize_engine::run(rematerialized)).program;
    let cleaned = canonicalize_engine::run(region_inline_engine::run(dce(cse(folded))));
    let refolded = ConstFold::transform(cleaned).program;
    let stabilized = canonicalize_engine::run(region_inline_engine::run(dce(cse(refolded))));
    stabilized.reconcile_runnable_top_level()
}

/// Run the unified pre-lowering optimization pipeline.
///
/// Pipeline stages (in order):
/// 1. **Canonicalize**  -  deterministic operand ordering so downstream
///    passes see a stable, content-addressable form.
/// 2. **Region inline**  -  flatten small `Node::Region` debug-wrappers
///    so the optimizer sees one unit.
/// 3. **Expression-level optimizer fixpoint**  -  runs safe, ABI-preserving
///    passes (`const_fold`, `loop_strip_mine`, `loop_unroll`, `strength_reduce`, `normalize_atomics`)
///    to a fixed point. These passes preserve buffer declarations and the
///    top-level runnable shape.
/// 4. **CSE**  -  common-subexpression elimination on the optimized IR.
/// 5. **DCE**  -  dead-code elimination cleans up anything CSE exposed.
#[must_use]
#[inline]
pub fn optimize(program: Program) -> Program {
    // Same 5-stage pipeline as `try_optimize`, but infallible (back-compat): on a
    // phase-2/phase-4 scheduler error (a should-never-happen optimizer-metadata
    // bug or non-convergence, already logged loudly inside run_phase*), continue
    // with that stage's input rather than propagating, so GPU lowering's many
    // `optimize(_) -> Program` call sites never have to thread a Result.
    //
    // The `.clone()` retains a fallback copy because `PassScheduler::run` consumes
    // its input and does not return it on error. This is a deliberate cost: it
    // runs at pipeline-BUILD time (once per program), which `ResidentPresencePipeline`
    // amortizes over millions of scans — it is NOT on the per-scan hot path. The
    // error branch itself is unreachable in a correct build (pass metadata is
    // validated by `pass_order::tests::live_registered_order_validates`), so on the
    // taken path the clone is pure insurance against a build-level optimizer bug.
    let prepared = prepare(program);
    let phase2_output = run_phase2(prepared.clone()).unwrap_or(prepared);
    let cleaned = cleanup_after_phase2(phase2_output);
    let phase4 = run_phase4(cleaned.clone()).unwrap_or(cleaned);
    stabilize(phase4)
}

#[cfg(test)]
mod tests {
    use super::{optimize, pre_lowering_scheduler, PHASE2_SELECTION, PHASE4_SELECTION};
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};
    use crate::optimizer::{registered_passes_for_profile, OptimizerProfile};

    #[test]
    fn optimize_preserves_top_level_region_wrap_after_inline() {
        // A wrapped program with a single small region that region_inline
        // may flatten. After the full optimize() pipeline the top-level
        // region-wrap invariant must still hold.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        );
        assert!(program.is_top_level_region_wrapped());
        let optimized = optimize(program);
        assert!(
            optimized.is_top_level_region_wrapped(),
            "Fix: optimize() must preserve top-level region-wrap invariant after region_inline"
        );
    }

    #[test]
    fn pre_lowering_release_profile_exposes_hot_abi_preserving_passes() {
        let names = registered_passes_for_profile(OptimizerProfile::Release)
            .expect("Fix: release optimizer profile must schedule classified passes")
            .into_iter()
            .map(|pass| pass.metadata().name)
            .collect::<std::collections::BTreeSet<_>>();

        for required in [
            "dead_store_elim",
            "read_only_load_hoist",
            "store_to_load_forward",
            "loop_licm",
            "loop_software_pipeline",
            "branch_value_hoist",
            "rematerialize_cheap_let",
        ] {
            assert!(
                names.contains(required),
                "Fix: concrete optimization pass `{required}` exists but is not classified into the Release profile"
            );
        }
    }

    #[test]
    fn pre_lowering_schedulers_enforce_cost_monotone_contract() {
        for phases in [PHASE2_SELECTION, PHASE4_SELECTION] {
            let scheduler = pre_lowering_scheduler(phases)
                .expect("Fix: pre-lowering scheduler must build for release phases");
            assert!(
                scheduler.cost_monotone_enforcement(),
                "Fix: backend-called pre_lowering::optimize must not land cost-up rewrites silently"
            );
            assert!(
                scheduler.effect_handler_enforcement(),
                "Fix: backend-called pre_lowering::optimize must not introduce new effects silently"
            );
            assert!(
                scheduler.linear_type_enforcement(),
                "Fix: backend-called pre_lowering::optimize must not introduce linear-type violations silently"
            );
            assert!(
                scheduler.shape_predicate_enforcement(),
                "Fix: backend-called pre_lowering::optimize must not introduce shape-predicate violations silently"
            );
        }
    }

    #[test]
    fn optimize_preserves_var_snapshot_before_source_reassign_in_loop_branch() {
        fn contains_tmp_snapshot(nodes: &[Node]) -> bool {
            nodes.iter().any(|node| match node {
                Node::Let {
                    name,
                    value: Expr::Var(source),
                } => name.as_str() == "tmp" && source.as_str() == "s0",
                Node::If {
                    then, otherwise, ..
                } => contains_tmp_snapshot(then) || contains_tmp_snapshot(otherwise),
                Node::Loop { body, .. } | Node::Block(body) => contains_tmp_snapshot(body),
                Node::Region { body, .. } => contains_tmp_snapshot(body),
                _ => false,
            })
        }

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("s0", Expr::u32(1)),
                Node::let_bind("s1", Expr::u32(2)),
                Node::Loop {
                    var: "pc".into(),
                    from: Expr::u32(0),
                    to: Expr::u32(1),
                    body: vec![
                        Node::let_bind("op", Expr::LitU32(4)),
                        Node::if_then(
                            Expr::eq(Expr::var("op"), Expr::u32(0)),
                            vec![
                                Node::assign("s1", Expr::var("s0")),
                                Node::assign("s0", Expr::u32(192)),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("op"), Expr::u32(1)),
                            vec![
                                Node::assign("s0", Expr::add(Expr::var("s0"), Expr::var("s1"))),
                                Node::assign("s1", Expr::u32(0)),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("op"), Expr::u32(2)),
                            vec![
                                Node::assign("s0", Expr::mul(Expr::var("s0"), Expr::var("s1"))),
                                Node::assign("s1", Expr::u32(0)),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("op"), Expr::u32(3)),
                            vec![Node::assign("s1", Expr::var("s0"))],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("op"), Expr::u32(4)),
                            vec![
                                Node::let_bind("tmp", Expr::var("s0")),
                                Node::assign("s0", Expr::var("s1")),
                                Node::assign("s1", Expr::var("tmp")),
                            ],
                        ),
                    ],
                },
                Node::store("out", Expr::u32(0), Expr::var("s1")),
            ],
        );

        let optimized = optimize(program);

        assert!(
            contains_tmp_snapshot(optimized.entry()),
            "Fix: pre-lowering optimize must preserve Var Let snapshot boundaries when the source is reassigned later in the same control-flow scope"
        );
    }
}
