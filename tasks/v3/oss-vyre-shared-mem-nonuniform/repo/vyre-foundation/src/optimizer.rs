//! Fixpoint optimizer pass framework for vyre IR.
//!
//! Passes are registered with [`vyre_macros::vyre_pass`] and discovered through
//! a process-wide registry. The scheduler applies registered passes until the
//! program reaches a fixed point or a safety cap rejects non-convergence.

use crate::ir_inner::model::program::Program;
use rustc_hash::FxHashSet;
use std::sync::{Arc, LazyLock};

/// Cost certificates for cost-monotone-down pass enforcement.
/// `CostCertificate::for_program` reads cached `ProgramStats`; the optimizer
/// post-condition gate compares pre/post and refuses cost-up rewrites that
/// did not declare `RefusalReason::CostIncrease`.
pub mod cost;
pub mod ctx;
/// Derived pass-order artifact for optimizer release validation.
pub mod derived_order;
/// Differential compilation via wire-content-hash Merkle. Per-Node + per-Region
/// content hashes derived from the canonical wire encoding let backends maintain
/// `<subtree_hash, CompiledArtifact>` caches that survive deep IR rewrites where
/// most subtrees are unchanged.
pub mod diff_compile;
/// Effect lattice  -  composition-aware refusal for fusion + dispatch.
/// Lifts `SideEffectClass` declarations into the lattice
/// `Pure ⊑ ReadAtomic ⊑ ReadWriteAtomic(Ordering) ⊑ Synchronized(Scope) ⊑ Diverging`
/// and exposes `compose(producer, consumer)` returning either the combined effect
/// or `RefusalReason::EffectLatticeViolation` with a structured fix string.
pub mod effect_lattice;
pub mod fact_substrate;
pub mod fusion_cert;
/// Program-level shape-facts analysis (audit P0 #38). Derives one
/// `BufferShapeFacts` per `BufferDecl`; downstream passes consume the
/// derived map instead of recomputing buffer sizes ad hoc.
pub mod program_shape_facts;
/// Verified shape-fact queries (P-1.0-V3.3) optimizer passes use to
/// read the consequences of `ShapePredicate` declarations (validated
/// by P-1.0-V3.2).
pub mod shape_facts;

/// Shared algebraic rewrite legality rules consumed by both Program-IR passes
/// and lowered KernelDescriptor rewrites.
pub mod algebraic_rules;
pub mod dsl;
/// Equality-saturation engine substrate: minimal EGraph, rewrite families,
/// saturation, and cost-based extraction.
pub mod eqsat;
/// GPU-resident e-graph snapshot substrate. Mirrors
/// the CPU `EGraph` into a flat columnar layout uploadable to GPU
/// scratch; backend saturation kernels walk the columns in
/// parallel and feed discovered equivalences back through
/// `apply_equivalences` to the CPU EGraph.
pub mod eqsat_gpu;
/// Tier-B TOML rule database  -  load equivalence rules from
/// community-contributable `*.toml` files (ROADMAP A6).
pub mod eqsat_toml;
/// Hash-consed Expr arena. Side-table substrate that
/// collapses structurally-equal `Expr` subtrees into shared 32-bit
/// `ExprId`s. Additive  -  does not change the IR shape; passes opt in
/// via `ExprArena::intern` and operate on `ExprId`s.
pub mod expr_arena;
/// Program-level analysis built on the [`expr_arena`] substrate.
/// Walks every `Expr` in a `Program` and returns aggregate stats +
/// stable structural fingerprint usable as a cheap structural-
/// similarity key. T020 SEED-2 first concrete consumer.
pub mod expr_arena_analysis;
/// Bounded LRU store of per-region dispatch performance
/// records. Backends populate via `record(region, kernel_ns,
/// bytes_touched)`; the optimizer queries `is_hot(region)` /
/// `mean_kernel_ns(region)` to prioritise pass scheduling per
/// region. Default-empty so passes that consume the hint must
/// remain correct on the cold path.
pub mod hot_path_hints;
/// Megakernel-fusion-scheduler subsystem (homotopy weight oracle +
/// matroid subset selection). Hoisted from `pass_substrate/` in audit
/// cleanup A9 (2026-04-30) so megakernel scheduling lives in one place.
pub mod megakernel;
/// Contributor-facing optimizer catalog derived from the live pass registry.
pub mod pass_catalog;
/// Stable pass-explanation records derived from scheduler metrics and the live
/// optimizer catalog.
pub mod pass_explain;
/// Pass verifier: runs every registered pass against a synthetic corpus
/// and asserts cost-monotone-down plus structural validity. Surfaces
/// contract violations at test time so they're caught before merge instead
/// of at the scheduler's runtime gate.
pub mod pass_invariants;
/// Machine-checkable pass-order validation for release gates and contributors.
pub mod pass_order;
/// Benchmark/hot-path-driven pass selection.
pub mod pass_selection;
pub mod passes;
/// Backend-neutral planar rewrite batching used by optimizer passes.
pub mod planar_batch;
/// Pre-lowering optimization pipeline  -  stable composed `optimize(program)`
/// entry every backend calls before lowering. Replaces the old
/// `transform::optimize` facade after audit cleanup T7 (2026-05-01).
pub mod pre_lowering;
/// Columnar / SoA fact view of a `Program` that hot
/// optimizer passes can opt into. Built once via
/// `ProgramFacts::build(&program)` and then queried in O(1) hash
/// lookup or O(K) over the answer instead of paying a fresh tree
/// walk per query.
pub mod program_soa;
mod rewrite;
/// SMT-LIB proof obligations for proof-carrying rewrites.
pub mod rewrite_proof;
/// N3  -  registry of shipped rewrite proof obligations consumed by the
/// `vyre xtask verify-rewrite-proofs` runner and the
/// `vyre-rewrite-proofs` GitHub Actions workflow.
pub mod rewrite_proof_registry;
mod scheduler;
#[cfg(test)]
mod tests;

pub use ctx::{scheduling_error_to_diagnostic, AdapterCaps, AnalysisCache, PassCtx};
pub use derived_order::{
    derive_pass_order, derive_registered_pass_order, DerivedPassEdge, DerivedPassEdgeKind,
    DerivedPassNode, DerivedPassOrder,
};
pub use fusion_cert::FusionCertificate;
pub use pass_explain::{
    explain_optimizer_report, explain_optimizer_report_with_catalog, CatalogLookupStatus,
    PassExplanation, PassMetricDelta,
};
pub use pass_order::{
    validate_registered_pass_order, validate_scheduled_pass_order, PassOrderValidation,
};
pub use pass_selection::{
    registered_passes_for_profile_and_program, select_pass_metadata_for_program,
    PassSelectionDecision, PassSelectionReason,
};
pub use planar_batch::{
    default_planar_rewrite_batch_threshold, planar_rewrite_schedule_mask, RewriteBatch,
    RewriteBatchCandidates, RewriteBatchItem, RewriteBatchPlan, RewriteCandidate,
};
// ProgramPassKind is now a newtype over `Box<dyn ProgramPass>`; built-in passes are
// auto-discovered via `inventory::iter::<ProgramPassRegistration>` (the same
// mechanism external passes already use). The hand-maintained import
// block was removed in audit cleanup A4 (2026-04-30)  -  adding a new
// pass no longer requires editing this file.
pub use scheduler::{
    schedule_passes, OptimizerRunReport, PassRunDecision, PassRunMetric, PassScheduler,
    PassSchedulingError,
};
pub use vyre_macros::vyre_pass;

/// Static metadata declared by an optimizer pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassMetadata {
    /// Stable pass name.
    pub name: &'static str,
    /// Capabilities or prior passes required before this pass can run.
    pub requires: &'static [&'static str],
    /// Capabilities invalidated when this pass rewrites the program.
    pub invalidates: &'static [&'static str],
    /// Scheduler phase this pass belongs to.
    pub phase: PassPhase,
    /// Architectural boundary this pass is allowed to cross.
    pub boundary_class: PassBoundaryClass,
    /// Backend/runtime capabilities required before this pass may run.
    pub requires_caps: &'static [&'static str],
    /// Whether this pass preserves the program's externally visible buffer ABI.
    pub preserves_abi: bool,
    /// Cost model family used to interpret pass deltas.
    pub cost_model_family: CostModelFamily,
}

impl PassMetadata {
    /// Construct metadata for legacy/simple tests and passes.
    #[must_use]
    pub const fn new(
        name: &'static str,
        requires: &'static [&'static str],
        invalidates: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            requires,
            invalidates,
            phase: PassPhase::Unclassified,
            boundary_class: PassBoundaryClass::Unknown,
            requires_caps: &[],
            preserves_abi: true,
            cost_model_family: CostModelFamily::Unknown,
        }
    }
}

/// Coarse optimizer phase used by profile selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PassPhase {
    /// Temporary default for passes that have not been classified yet.
    Unclassified,
    /// Deterministic IR normalization.
    Canonicalization,
    /// Scalar algebraic rewrites.
    ScalarAlgebra,
    /// Loop normalization and loop transforms.
    Loop,
    /// Memory, buffer, and load/store transforms.
    Memory,
    /// Fusion, CSE, and DCE.
    FusionCse,
    /// Synchronization and barrier transforms.
    Sync,
    /// Backend-capability-aware specialization.
    Specialization,
    /// Structural cleanup that should not change semantics.
    Cleanup,
    /// Dataflow-specific profile pass.
    Dataflow,
    /// Megakernel/runtime-residency profile pass.
    Megakernel,
}

/// Boundary a pass may legally cross.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PassBoundaryClass {
    /// Temporary default for unclassified passes.
    Unknown,
    /// Preserves public program ABI and backend-neutral semantics.
    AbiPreserving,
    /// May change public buffer ABI and requires caller opt-in.
    AbiChanging,
    /// Requires backend capabilities but not backend identity checks.
    BackendAware,
    /// Owns runtime/megakernel/residency decisions.
    RuntimeAware,
    /// Belongs to a domain layer, not generic core IR.
    DomainSpecific,
}

/// Cost family used by reports and profile selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CostModelFamily {
    /// Temporary default for unclassified passes.
    Unknown,
    /// Scalar instruction and expression cost.
    Scalar,
    /// Loop trip count and loop-body cost.
    Loop,
    /// Memory bandwidth, buffer traffic, and readback cost.
    Memory,
    /// Launch count, fusion legality, and redundant work cost.
    Fusion,
    /// Barrier, atomic, and synchronization cost.
    Sync,
    /// Dataflow frontier/fixpoint cost.
    Dataflow,
    /// Megakernel slot, queue, residency, and divergence cost.
    Megakernel,
}

/// Lightweight pass analysis result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassAnalysis {
    /// Whether the scheduler should invoke `transform`.
    pub should_run: bool,
}

impl PassAnalysis {
    /// Analysis result that asks the scheduler to run the pass.
    pub const RUN: Self = Self { should_run: true };

    /// Analysis result that asks the scheduler to skip the pass.
    pub const SKIP: Self = Self { should_run: false };
}

/// Result of one pass transformation.
#[derive(Debug, Clone, PartialEq)]
pub struct PassResult {
    /// Rewritten program.
    pub program: Program,
    /// Whether the program changed.
    pub changed: bool,
}

/// Why a pass declined to apply a transformation it would otherwise have run.
///
/// Refusal is the principled alternative to "silently emit the same program back"  -  it lets
/// the scheduler tell the user *why* a transformation was skipped (cost would go up, effect
/// lattice forbids the fusion, the wire contract would be broken). Cost-certificate-bounded
/// fusion, effect-lattice fusion, and divergence-aware barrier insertion all produce these.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RefusalReason {
    /// The pass's cost certificate predicts the rewrite would increase total cost beyond the
    /// declared monotone-down budget. The scheduler treats this as a hard refusal  -  it must
    /// not run the rewrite, even if `analyze` returned `RUN`.
    CostIncrease {
        /// Predicted cost delta (post − pre); positive means cost goes up.
        delta: i64,
        /// Free-form reason naming what increased.
        detail: &'static str,
    },
    /// The effect lattice composition rule forbids the rewrite. Surfaced when a pass would
    /// fuse two ops whose effect profiles don't compose (e.g. `Pure ∘ Diverging` without an
    /// explicit `GridSync`). Carries a suggested fix string the user can act on.
    EffectLatticeViolation {
        /// Producer `op_id` whose effect is incompatible with the consumer.
        producer: &'static str,
        /// Consumer `op_id` whose effect is incompatible with the producer.
        consumer: &'static str,
        /// Concrete fix the user can apply (insert barrier, refuse to fuse, etc.).
        suggested_fix: &'static str,
    },
    /// The pass would break the wire contract  -  `op_id` drift, Region-chain break, or any
    /// invariant the scheduler enforces by construction. The scheduler converts this into a
    /// hard error (the pass is buggy), not a soft refusal.
    WireContractViolation {
        /// Free-form description of the violation.
        detail: &'static str,
    },
    /// Catch-all refusal with a free-form reason. Use this only when none of the above fits;
    /// preferred path is to add a typed variant.
    Other {
        /// Free-form reason.
        detail: &'static str,
    },
}

impl RefusalReason {
    /// Stable kind tag for diagnostics + scheduler telemetry.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::CostIncrease { .. } => "cost_increase",
            Self::EffectLatticeViolation { .. } => "effect_lattice_violation",
            Self::WireContractViolation { .. } => "wire_contract_violation",
            Self::Other { .. } => "other",
        }
    }
}

impl std::fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CostIncrease { delta, detail } => {
                write!(f, "cost_increase: delta={delta} reason={detail}")
            }
            Self::EffectLatticeViolation {
                producer,
                consumer,
                suggested_fix,
            } => write!(
                f,
                "effect_lattice_violation: producer={producer} consumer={consumer} fix={suggested_fix}"
            ),
            Self::WireContractViolation { detail } => {
                write!(f, "wire_contract_violation: {detail}")
            }
            Self::Other { detail } => write!(f, "other: {detail}"),
        }
    }
}

impl PassResult {
    /// Build a transformation result by comparing before and after programs.
    #[must_use]
    #[inline]
    pub fn from_programs(before: &Program, program: Program) -> Self {
        let changed = before != &program;
        Self { program, changed }
    }

    /// Declare the pass left the program unchanged. `VYRE_IR_HOTSPOTS`
    /// CRIT-2/CRIT-3: `from_programs(&program, program.clone())` pays
    /// a full `Program` clone + O(N) `PartialEq` comparison on every
    /// no-op call. When a pass has already proven it will not rewrite
    /// the program, it should `return PassResult::unchanged(program)`
    /// to move the input through without cloning or comparing.
    #[must_use]
    #[inline]
    pub fn unchanged(program: Program) -> Self {
        Self {
            program,
            changed: false,
        }
    }
}

/// Constructor and metadata submitted by each registered pass.
#[derive(Debug)]
pub struct ProgramPassRegistration {
    /// Pass metadata available without constructing the pass.
    pub metadata: PassMetadata,
    /// Construct a fresh pass instance.
    pub factory: fn() -> Box<dyn ProgramPass>,
}

inventory::collect!(ProgramPassRegistration);

pub(crate) mod private {
    pub trait Sealed {}
}

/// One IR-to-IR optimizer pass.
pub trait ProgramPass: private::Sealed + Send + Sync {
    /// Static metadata for scheduling and diagnostics.
    fn metadata(&self) -> PassMetadata;

    /// Analyses this pass leaves valid after running. Default empty  -  passes that prove
    /// they preserve a named analysis (dominators, def-use chains, points-to, region-chain
    /// integrity, etc.) override this so the scheduler can skip recomputation on the next
    /// pass that requires the same analysis. The scheduler treats any analysis NOT in this
    /// list as invalidated when `transform` returns `changed = true`.
    fn preserves(&self) -> &'static [&'static str] {
        &[]
    }

    /// Unique pass identifier for diagnostics.
    ///
    /// Defaults to `metadata().name`, but external passes may override this
    /// to provide richer instance-level identity (e.g. a plugin crate name +
    /// pass name) that makes scheduler errors actionable in seconds.
    fn pass_id(&self) -> &'static str {
        self.metadata().name
    }

    /// Pre-transform analysis hook.
    fn analyze(&self, program: &Program) -> PassAnalysis;

    /// Transform a program.
    fn transform(&self, program: Program) -> PassResult;

    /// Whether this pass implements candidate-granular planar batching.
    ///
    /// The default is false so legacy passes keep exact behavior. Passes that
    /// override [`ProgramPass::rewrite_candidates`] and
    /// [`ProgramPass::apply_rewrite_batch`] return true; the scheduler then
    /// calls [`ProgramPass::batch_apply`] instead of forcing one rewrite per
    /// pass invocation.
    fn supports_planar_batching(&self) -> bool {
        false
    }

    /// Candidate rewrites available for planar batching.
    fn rewrite_candidates(&self, _program: &Program) -> RewriteBatchCandidates {
        RewriteBatchCandidates::empty()
    }

    /// Apply one selected disjoint rewrite wave.
    fn apply_rewrite_batch(&self, program: Program, _batch: &RewriteBatch) -> PassResult {
        self.transform(program)
    }

    /// Batch-aware transform entry point used by the scheduler.
    fn batch_apply(&self, program: Program) -> PassResult {
        if !self.supports_planar_batching() {
            return self.transform(program);
        }
        let candidates = self.rewrite_candidates(&program);
        if candidates.is_empty() {
            return PassResult::unchanged(program);
        }
        if !candidates.should_batch() {
            return self.transform(program);
        }
        let plan = candidates.plan();
        if !plan.has_batches() {
            return PassResult::unchanged(program);
        }

        let mut changed = false;
        let mut program = program;
        for batch in plan.batches() {
            let result = self.apply_rewrite_batch(program, batch);
            changed |= result.changed;
            program = result.program;
        }
        PassResult { program, changed }
    }

    /// Refusal-aware transform. Default delegates to [`ProgramPass::transform`] and wraps the result
    /// in `Ok`. Passes that want to refuse a rewrite (cost certificate predicts cost up,
    /// effect lattice forbids the fusion, etc.) override this and return
    /// [`Err(RefusalReason)`]. The scheduler treats refusals as a no-op rewrite plus a
    /// telemetry record naming the reason  -  never silently miscompiles.
    ///
    /// # Errors
    ///
    /// Returns [`RefusalReason`] when the pass proves that applying its rewrite
    /// would violate cost, effect, or wire-contract constraints.
    fn try_transform(&self, program: Program) -> Result<PassResult, RefusalReason> {
        Ok(self.transform(program))
    }

    /// Refusal-aware batch transform. Passes with typed refusal contracts
    /// override this when their batched application can be rejected before
    /// touching the IR.
    ///
    /// # Errors
    ///
    /// Returns [`RefusalReason`] when a batched rewrite would violate cost,
    /// effect, or wire-contract constraints.
    fn try_batch_apply(&self, program: Program) -> Result<PassResult, RefusalReason> {
        self.try_transform(program)
    }

    /// Effects this pass is explicitly allowed to introduce.
    ///
    /// The scheduler's effects-handler gate treats every pass as a handler
    /// from `pre_effect_row -> post_effect_row`: effects may be discharged
    /// (removed) by optimization, but newly introduced effects are rejected
    /// unless the pass declares them here. Defaulting to empty keeps existing
    /// passes conservative and prevents backend release paths from silently
    /// adding writes, atomics, barriers, traps, async loads, or nested GPU
    /// dispatch.
    fn allowed_effect_additions(&self) -> crate::lower::effects::ProgramEffects {
        crate::lower::effects::ProgramEffects::empty()
    }

    /// Fingerprint the pass-visible program state.
    fn fingerprint(&self, program: &Program) -> u64;
}

/// Optimizer pass container  -  a thin newtype over a trait object.
///
/// Audit cleanup A4 (2026-04-30) collapsed the previous 19-typed-variant
/// enum into this newtype: every built-in pass now goes through
/// `inventory::submit!`-based autodiscovery (the same path external
/// passes always used), so adding a new pass no longer requires
/// touching `optimizer.rs`. The cost is one indirect call per pass
/// invocation  -  negligible against the actual rewrite work.
pub struct ProgramPassKind(Box<dyn ProgramPass>);

impl ProgramPassKind {
    /// Wrap a typed pass instance in a `ProgramPassKind`. Used by registrations
    /// and tests; production paths call [`registered_passes`].
    #[must_use]
    #[inline]
    pub fn new<P: ProgramPass + 'static>(pass: P) -> Self {
        Self(Box::new(pass))
    }

    /// Wrap a pre-boxed pass  -  used by the inventory iterator.
    #[must_use]
    #[inline]
    pub fn from_boxed(pass: Box<dyn ProgramPass>) -> Self {
        Self(pass)
    }

    /// Static metadata for scheduling.
    #[must_use]
    #[inline]
    pub fn metadata(&self) -> PassMetadata {
        self.0.metadata()
    }

    /// Instance-level pass identifier for diagnostics.
    #[must_use]
    #[inline]
    pub fn pass_id(&self) -> &'static str {
        self.0.pass_id()
    }

    /// Pre-transform analysis.
    #[must_use]
    #[inline]
    pub fn analyze(&self, program: &Program) -> PassAnalysis {
        self.0.analyze(program)
    }

    /// Transform a program.
    #[must_use]
    #[inline]
    pub fn transform(&self, program: Program) -> PassResult {
        self.0.transform(program)
    }

    /// Batch-aware transform.
    #[must_use]
    #[inline]
    pub fn batch_apply(&self, program: Program) -> PassResult {
        self.0.batch_apply(program)
    }

    /// Refusal-aware transform.
    ///
    /// # Errors
    /// Returns the [`RefusalReason`] reported by the underlying pass.
    #[inline]
    pub fn try_transform(&self, program: Program) -> Result<PassResult, RefusalReason> {
        self.0.try_transform(program)
    }

    /// Refusal-aware batch transform.
    ///
    /// # Errors
    /// Returns the [`RefusalReason`] reported by the underlying pass.
    #[inline]
    pub fn try_batch_apply(&self, program: Program) -> Result<PassResult, RefusalReason> {
        self.0.try_batch_apply(program)
    }

    /// Analyses preserved by this pass after running. See [`ProgramPass::preserves`].
    #[must_use]
    #[inline]
    pub fn preserves(&self) -> &'static [&'static str] {
        self.0.preserves()
    }

    /// Effects the wrapped pass may introduce under the effects-handler gate.
    #[must_use]
    #[inline]
    pub fn allowed_effect_additions(&self) -> crate::lower::effects::ProgramEffects {
        self.0.allowed_effect_additions()
    }
}

/// High-level optimizer profile used to select registered passes without
/// centralizing ownership of the passes themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OptimizerProfile {
    /// Backend-neutral release path: only ABI-preserving passes that require
    /// no special backend/runtime capabilities.
    Release,
    /// Dataflow profile for frontier/fixpoint workloads.
    Dataflow,
    /// Runtime profile for megakernel queue, slot, and resident-buffer work.
    Megakernel,
}

impl OptimizerProfile {
    /// Return true when `metadata` belongs in this profile.
    #[must_use]
    pub fn accepts(self, metadata: PassMetadata) -> bool {
        match self {
            Self::Release => {
                metadata.preserves_abi
                    && metadata.boundary_class == PassBoundaryClass::AbiPreserving
                    && metadata.requires_caps.is_empty()
                    && !matches!(
                        metadata.phase,
                        PassPhase::Dataflow | PassPhase::Megakernel | PassPhase::Unclassified
                    )
            }
            Self::Dataflow => {
                metadata.preserves_abi
                    && matches!(
                        metadata.boundary_class,
                        PassBoundaryClass::AbiPreserving | PassBoundaryClass::DomainSpecific
                    )
                    && metadata.phase == PassPhase::Dataflow
            }
            Self::Megakernel => {
                metadata.preserves_abi
                    && matches!(
                        metadata.boundary_class,
                        PassBoundaryClass::AbiPreserving | PassBoundaryClass::RuntimeAware
                    )
                    && metadata.phase == PassPhase::Megakernel
            }
        }
    }
}

/// Error returned by the pass scheduler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OptimizerError {
    /// The scheduler hit its safety cap before reaching a fixed point.
    #[error(
        "optimizer did not reach a fixpoint after {max_iterations} iterations. Fix: inspect pass `{last_pass}` for oscillating rewrites or raise the cap only with a convergence certificate."
    )]
    MaxIterations {
        /// Iteration cap that was reached.
        max_iterations: usize,
        /// Last pass that changed the program.
        last_pass: &'static str,
    },
    /// At least one pass could not run because its requirements were missing.
    #[error(
        "optimizer pass `{pass}` requires `{missing}` but no prior pass provides it. Fix: register the required analysis pass or remove the stale requirement."
    )]
    UnsatisfiedRequirement {
        /// Pass that could not run.
        pass: &'static str,
        /// First missing requirement.
        missing: &'static str,
    },
    /// Registered passes contain an invalid dependency graph.
    #[error("{0}")]
    Scheduling(#[from] PassSchedulingError),
    /// Pre-lowering optimization did not converge after the safety cap.
    #[error(
        "pre-lowering phase {phase} did not converge after {max} iterations. Fix: inspect the phase for oscillating rewrites or raise the cap only with a convergence certificate."
    )]
    PreLoweringIterationLimit {
        /// Phase index that hit the cap.
        phase: u32,
        /// Iteration cap that was reached.
        max: usize,
    },
}

/// Return pass instances from the global registry.
///
/// Built-in and external passes alike are discovered via the
/// `inventory::iter::<ProgramPassRegistration>` mechanism  -  adding a new pass
/// requires no edit to this function. Order is determined by the
/// scheduler's dependency graph (`schedule_passes`).
///
/// # Errors
/// Returns [`OptimizerError::Scheduling`] when the pass set declares an
/// unsatisfiable requirement graph.
pub fn registered_passes() -> Result<Vec<ProgramPassKind>, OptimizerError> {
    let registrations = registered_pass_registrations()?;
    let mut passes = Vec::with_capacity(registrations.len());
    for registration in registrations.iter() {
        passes.push(ProgramPassKind::from_boxed((registration.factory)()));
    }
    Ok(passes)
}

/// Return registered passes accepted by `profile` in scheduled order.
///
/// # Errors
/// Returns [`OptimizerError::Scheduling`] when the global pass inventory is
/// invalid.
pub fn registered_passes_for_profile(
    profile: OptimizerProfile,
) -> Result<Vec<ProgramPassKind>, OptimizerError> {
    let registrations = registered_pass_registrations()?;
    let mut passes = Vec::with_capacity(registrations.len());
    for registration in registrations.iter() {
        if profile.accepts(registration.metadata) {
            passes.push(ProgramPassKind::from_boxed((registration.factory)()));
        }
    }
    Ok(passes)
}

/// Return scheduled metadata for passes accepted by `profile`.
///
/// This is the lightweight inventory/reporting surface used by release gates
/// and docs generators. It does not instantiate pass objects beyond the
/// already-registered static metadata.
///
/// # Errors
/// Returns [`OptimizerError::Scheduling`] when the global pass inventory is
/// invalid.
pub fn registered_pass_metadata_for_profile(
    profile: OptimizerProfile,
) -> Result<Vec<PassMetadata>, OptimizerError> {
    Ok(registered_pass_registrations()?
        .iter()
        .map(|registration| registration.metadata)
        .filter(|&metadata| profile.accepts(metadata))
        .collect())
}

/// Return registered pass metadata in scheduled execution order.
///
/// # Errors
///
/// Returns [`OptimizerError::Scheduling`] when a linked pass declares an
/// unknown requirement or a cyclic requirement graph.
#[must_use]
pub fn registered_pass_registrations(
) -> Result<Arc<[&'static ProgramPassRegistration]>, OptimizerError> {
    static SCHEDULED: LazyLock<
        Result<Arc<[&'static ProgramPassRegistration]>, PassSchedulingError>,
    > = LazyLock::new(|| {
        let registrations: Vec<&'static ProgramPassRegistration> =
            inventory::iter::<ProgramPassRegistration>().collect();
        schedule_passes(&registrations).map(|scheduled| scheduled.into_boxed_slice().into())
    });
    match &*SCHEDULED {
        Ok(registrations) => Ok(Arc::clone(registrations)),
        Err(error) => Err(OptimizerError::from(error.clone())),
    }
}

/// Run the globally registered optimizer passes to a fixed point.
///
/// # Errors
///
/// Returns [`OptimizerError`] when requirements cannot be satisfied or when
/// the pass pipeline oscillates past the configured iteration cap.
pub fn optimize(program: Program) -> Result<Program, OptimizerError> {
    // Cache the default scheduler so the box-per-pass instantiation
    // (~120 boxed `ProgramPassKind`s, plus FxHashMap pass_index construction
    // and topological execution_order vector) only runs once per process.
    // PassScheduler::run takes &self and is stateless across runs, so a
    // single instance is safe to share across optimize() invocations.
    static DEFAULT_SCHEDULER: LazyLock<Result<PassScheduler, OptimizerError>> =
        LazyLock::new(PassScheduler::try_default);
    match &*DEFAULT_SCHEDULER {
        Ok(scheduler) => scheduler.run(program),
        Err(err) => Err(err.clone()),
    }
}

/// Run selected optimizer passes for `program` using hot-path telemetry.
///
/// Expensive pass families are instantiated only when the Program's shape or
/// recorded dispatch telemetry justifies them; required dependencies are kept
/// automatically so scheduler invariants remain intact.
///
/// # Errors
/// Returns [`OptimizerError`] when selection or scheduling fails, or when the
/// selected pass pipeline does not converge.
pub fn optimize_with_hot_path_hints(
    program: Program,
    profile: OptimizerProfile,
    hints: &hot_path_hints::HotPathHints,
) -> Result<Program, OptimizerError> {
    let passes =
        pass_selection::registered_passes_for_profile_and_program(profile, &program, hints)?;
    PassScheduler::try_with_passes(passes)?.run(program)
}

/// 32-byte BLAKE3 fingerprint of a Program for content-addressed pipeline
/// caches (audit P0 #26). Two semantically-equal Programs that differ only
/// in author-visible buffer declaration order share this fingerprint, so
/// AOT-emitted artifacts and runtime-cache blobs key into the same bucket.
///
/// The algorithm delegates to [`Program::fingerprint`], which hashes canonical
/// wire bytes after normalizing declaration order, commutative operands, and
/// semantically transparent nested blocks.
///
/// This matches `vyre_runtime::PipelineFingerprint::of` byte-for-byte; both
/// callers route through this helper so the algorithms cannot drift apart.
#[must_use]
pub fn pipeline_fingerprint_bytes(program: &Program) -> [u8; 32] {
    program.fingerprint()
}

/// Stable 8-byte content-addressed fingerprint of a program.
///
/// This is the first 8 bytes of [`pipeline_fingerprint_bytes`], so optimizer
/// change detection uses the same canonical program identity as pipeline and
/// validation caches. It is intentionally not based on raw wire bytes because
/// declaration-order-only differences would otherwise invalidate optimizer
/// facts and force avoidable re-derivation.
#[must_use]
pub fn fingerprint_program(program: &Program) -> u64 {
    let first8 = pipeline_fingerprint_bytes(program);
    u64::from_le_bytes([
        first8[0], first8[1], first8[2], first8[3], first8[4], first8[5], first8[6], first8[7],
    ])
}

#[inline]
fn requirements_satisfied(metadata: PassMetadata, available: &FxHashSet<&'static str>) -> bool {
    metadata
        .requires
        .iter()
        .all(|requirement| available.contains(requirement))
}

#[cfg(test)]
mod framework_tests;
