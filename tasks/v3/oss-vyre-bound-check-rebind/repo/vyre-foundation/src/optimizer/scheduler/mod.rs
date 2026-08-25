#![allow(clippy::expect_used)]
//! Topological scheduling from [`PassMetadata::requires`](crate::optimizer::PassMetadata),
//! then a fixpoint runner that clears capabilities listed in `invalidates` when a pass
//! rewrites the program.
//!
//! **Hand-curated pass-pair list:** There is no longer a static list of ~30 `(before, after)`
//! pairs in this module (location *N/A*  -  not present in this revision). Constraints that map
//! to named predecessor passes are encoded only via each pass’s `requires` entries and are
//! honored by [`schedule_passes`] and the runtime requirement check inside `PassScheduler`'s fixpoint step.
//!
//! **Adjustment-set / causal edges:** Ordering beyond that DAG needs a separate row-major
//! influence matrix `adj` (`adj[i·n+j] ≠ 0` ⇒ pass `i` may influence `j`). [`PassMetadata`](crate::optimizer::PassMetadata)
//! exposes `requires` and `invalidates` capability tags, not a full pass→pass influence graph
//! or `produces` facts, so **extra causal pairs are not derivable** from metadata alone.
//! When `adj` is supplied (substrate analysis, TOML rules, etc.), use
//! [`crate::pass_substrate::adjustment_set_pass_dependency::pass_descendants`] for transitive
//! downstream passes and [`crate::pass_substrate::adjustment_set_pass_dependency::ordering_is_safe`]
//! to validate a proposed “run treatment before outcome” ordering.
use crate::ir::{BufferDecl, Expr, Node};
use crate::ir_inner::model::program::Program;
use crate::optimizer::{registered_passes, OptimizerError, ProgramPassKind};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::OnceLock;

pub(super) const DEFAULT_MAX_ITERATIONS: usize = 50;

/// Fixpoint scheduler for optimizer passes.
pub struct PassScheduler {
    passes: Vec<ProgramPassKind>,
    pass_index: FxHashMap<&'static str, usize>,
    research_traces: FxHashMap<&'static str, PassResearchTrace>,
    execution_order: Vec<usize>,
    /// True when `execution_order` was produced by the topological scheduler.
    /// In that case per-iteration requirement hash-set reconstruction is
    /// redundant: unsatisfied requirements were already rejected while building
    /// the scheduler. Explicit malformed `with_passes` inputs keep this false
    /// so tests and diagnostics still surface `UnsatisfiedRequirement`.
    requirements_prevalidated: bool,
    max_iterations: usize,
    invalidation_adjacency_cache: OnceLock<Vec<u32>>,
    invalidation_closure_cache: OnceLock<FxHashMap<&'static str, FxHashSet<&'static str>>>,
    /// Tag → pass indices that should be re-marked dirty when the tag is
    /// invalidated. A tag matches a pass if it equals the pass's `name`
    /// OR appears in its `requires` list. Replaces the per-iteration
    /// O(passes × invalidates × requires) scan inside
    /// `mark_invalidated_passes` with O(invalidates × dependents).
    dirty_trigger_index_cache: OnceLock<FxHashMap<&'static str, Vec<usize>>>,
    /// Indexed variant of `initial_dirty_cache` used by the hot `run()` path.
    /// This avoids rebuilding/cloning a string hash set and turns dirty checks
    /// into direct indexed loads.
    initial_dirty_flags_cache: OnceLock<Vec<bool>>,
    /// When `true`, the scheduler enforces a cost-monotone-down post-condition
    /// on every `ProgramPass::transform` invocation: after the rewrite, the new
    /// `CostCertificate` must dominate-or-equal the old on every tracked
    /// dimension, OR the pass must have explicitly declined via
    /// `ProgramPass::try_transform` returning `Err(RefusalReason::CostIncrease { ... })`.
    /// Rewrites that increase a tracked dimension without an explicit refusal
    /// are reverted (the pre-rewrite Program is kept) and a structured warning
    /// is emitted via the per-pass `PassRunMetric`.
    ///
    /// Defaults to `false` so existing consumers and built-in pass behavior are
    /// preserved bit-for-bit. Audits, tests, and the catalog-landing pipeline
    /// (Phase 4) flip this to `true` to drive the cost contract end-to-end.
    enforce_cost_monotone: bool,
    /// When `true`, every landed pass rewrite is checked as an effect handler:
    /// it may discharge effects already present in the program, but it may not
    /// introduce new effect bits unless the pass declares them via
    /// `ProgramPass::allowed_effect_additions`.
    ///
    /// This is the production hook for P-1.0-V1 effects-handler lowering.
    /// Default remains `false` for compatibility; backend pre-lowering enables
    /// it so release paths cannot silently add stores, atomics, barriers,
    /// traps, async loads, or nested GPU dispatch during optimization.
    enforce_effect_handlers: bool,
    /// When `true`, every landed pass rewrite is checked against declared
    /// `BufferDecl::linear_type` discipline. Rewrites may repair existing
    /// violations, but they may not introduce a new linear/affine/relevant
    /// violation after frontend validation.
    ///
    /// This is the production hook for P-1.0-V2 linear BufferAccess.
    enforce_linear_types: bool,
    /// When `true`, every landed pass rewrite is checked against declared
    /// `BufferDecl::shape_predicate` refinements. Rewrites may repair existing
    /// predicate violations, but they may not introduce a new liquid-shape
    /// contradiction after frontend validation.
    ///
    /// This is the production hook for P-1.0-V3 liquid BufferDecl shapes.
    enforce_shape_predicates: bool,
}

pub(crate) const PASS_RESEARCH_TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassResearchTrace {
    pub schema_version: u32,
    pub research_basis_key: &'static str,
    pub baseline_id: &'static str,
    pub proof_artifact_id: &'static str,
}

impl PassResearchTrace {
    /// Create optimizer research/proof metadata for a pass run.
    ///
    /// The scheduler keeps this metadata outside pass definitions so research
    /// evidence can be attached by experiment, benchmark, or VX innovation row
    /// without widening the optimizer pass API.
    ///
    /// # Errors
    ///
    /// Returns [`PassResearchTraceError`] when any required identifier is empty.
    pub fn try_new(
        research_basis_key: &'static str,
        baseline_id: &'static str,
        proof_artifact_id: &'static str,
    ) -> Result<Self, PassResearchTraceError> {
        if research_basis_key.trim().is_empty() {
            return Err(PassResearchTraceError::MissingResearchBasisKey);
        }
        if baseline_id.trim().is_empty() {
            return Err(PassResearchTraceError::MissingBaselineId);
        }
        if proof_artifact_id.trim().is_empty() {
            return Err(PassResearchTraceError::MissingProofArtifactId);
        }
        Ok(Self {
            schema_version: PASS_RESEARCH_TRACE_SCHEMA_VERSION,
            research_basis_key,
            baseline_id,
            proof_artifact_id,
        })
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.schema_version == PASS_RESEARCH_TRACE_SCHEMA_VERSION
            && !self.research_basis_key.trim().is_empty()
            && !self.baseline_id.trim().is_empty()
            && !self.proof_artifact_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassResearchTraceError {
    MissingResearchBasisKey,
    MissingBaselineId,
    MissingProofArtifactId,
}

impl std::fmt::Display for PassResearchTraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResearchBasisKey => {
                f.write_str("pass research trace is missing research_basis_key")
            }
            Self::MissingBaselineId => f.write_str("pass research trace is missing baseline_id"),
            Self::MissingProofArtifactId => {
                f.write_str("pass research trace is missing proof_artifact_id")
            }
        }
    }
}

impl std::error::Error for PassResearchTraceError {}

/// Optimized program plus per-pass runtime/size counters.
#[derive(Debug)]
pub struct OptimizerRunReport {
    /// Final program after convergence.
    pub program: Program,
    /// One metric row per pass considered by the scheduler.
    pub passes: Vec<PassRunMetric>,
}

/// Runtime and IR-size counters for one pass consideration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassRunMetric {
    /// Fixpoint iteration index.
    pub iteration: usize,
    /// Pass identifier.
    pub pass: &'static str,
    /// Optional research/proof metadata attached at the scheduler boundary for
    /// VX innovation rows, benchmark sweeps, and reproducible optimizer audits.
    pub research_trace: Option<PassResearchTrace>,
    /// Whether transform actually ran.
    pub ran: bool,
    /// Whether transform changed the program.
    pub changed: bool,
    /// Stable explanation for the scheduler decision on this pass.
    pub decision: PassRunDecision,
    /// Refusal kind when [`PassRunDecision::Refused`] applies.
    pub refusal_kind: Option<&'static str>,
    /// Analyses/capability tags required by this pass before it can run.
    pub required_analyses: &'static [&'static str],
    /// Analyses/capability tags this pass declares invalid when it lands a
    /// rewrite.
    pub declared_invalidations: &'static [&'static str],
    /// Whether the scheduler-owned fact substrate was reused for this dirty
    /// pass analysis.
    pub fact_substrate_reused: bool,
    /// Whether the scheduler-owned fact substrate was recomputed for this
    /// dirty pass analysis.
    pub fact_substrate_recomputed: bool,
    /// Whether this pass landed a rewrite and invalidated the scheduler-owned
    /// fact substrate.
    pub fact_substrate_invalidated: bool,
    /// Program effect-row bits before the pass when effect-handler
    /// enforcement is enabled; otherwise zero.
    pub effect_bits_before: u32,
    /// Program effect-row bits after the pass and all scheduler gates when
    /// effect-handler enforcement is enabled; otherwise zero.
    pub effect_bits_after: u32,
    /// Count of linear-type validation failures before the pass when
    /// linear-type enforcement is enabled; otherwise zero.
    pub linear_type_violations_before: usize,
    /// Count of linear-type validation failures after the pass and all
    /// scheduler gates when linear-type enforcement is enabled; otherwise zero.
    pub linear_type_violations_after: usize,
    /// Count of shape-predicate validation failures before the pass when
    /// shape-predicate enforcement is enabled; otherwise zero.
    pub shape_predicate_violations_before: usize,
    /// Count of shape-predicate validation failures after the pass and all
    /// scheduler gates when shape-predicate enforcement is enabled; otherwise zero.
    pub shape_predicate_violations_after: usize,
    /// Transform wall-clock runtime in nanoseconds. Zero when skipped.
    pub runtime_ns: u128,
    /// Node count before the pass.
    pub nodes_before: usize,
    /// Node count after the pass.
    pub nodes_after: usize,
    /// Statically-known storage bytes before the pass.
    pub static_storage_bytes_before: u64,
    /// Statically-known storage bytes after the pass.
    pub static_storage_bytes_after: u64,
    /// Estimated instruction count before the pass.
    pub instruction_count_before: u64,
    /// Estimated instruction count after the pass.
    pub instruction_count_after: u64,
    /// Memory operation count before the pass.
    pub memory_op_count_before: u64,
    /// Memory operation count after the pass.
    pub memory_op_count_after: u64,
    /// Atomic operation count before the pass.
    pub atomic_op_count_before: u64,
    /// Atomic operation count after the pass.
    pub atomic_op_count_after: u64,
    /// Control-flow operation count before the pass.
    pub control_flow_count_before: u64,
    /// Control-flow operation count after the pass.
    pub control_flow_count_after: u64,
    /// Coarse register-pressure estimate before the pass.
    pub register_pressure_before: u32,
    /// Coarse register-pressure estimate after the pass.
    pub register_pressure_after: u32,
    /// Estimated count of heap-backed IR containers before the pass.
    pub ir_heap_allocations_before: usize,
    /// Estimated count of heap-backed IR containers after the pass.
    pub ir_heap_allocations_after: usize,
    /// Estimated bytes owned by heap-backed IR containers before the pass.
    pub ir_heap_bytes_before: usize,
    /// Estimated bytes owned by heap-backed IR containers after the pass.
    pub ir_heap_bytes_after: usize,
}

/// Stable explanation code for one pass scheduler metric row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassRunDecision {
    /// Pass was not dirty, so the scheduler did not analyze or run it.
    CleanSkipped,
    /// Pass was dirty, but its analysis hook returned [`crate::optimizer::PassAnalysis::SKIP`].
    AnalysisSkipped,
    /// Pass ran and returned `changed = false`.
    RanUnchanged,
    /// Pass ran and landed a rewrite.
    Changed,
    /// Cost-monotone enforcement rejected the produced rewrite.
    CostReverted,
    /// Effects-handler enforcement rejected a rewrite that introduced
    /// undeclared effects.
    EffectReverted,
    /// Linear-type enforcement rejected a rewrite that introduced a new
    /// BufferAccess discipline violation.
    LinearTypeReverted,
    /// Shape-predicate enforcement rejected a rewrite that introduced a new
    /// liquid BufferDecl contradiction.
    ShapePredicateReverted,
    /// Pass explicitly refused through [`crate::optimizer::ProgramPass::try_transform`].
    Refused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct IrAllocationEstimate {
    allocations: usize,
    bytes: usize,
}

impl IrAllocationEstimate {
    fn add_container<T>(&mut self, len: usize) {
        self.allocations = self.allocations.saturating_add(1);
        self.bytes = self
            .bytes
            .saturating_add(len.saturating_mul(std::mem::size_of::<T>()));
    }

    fn add_box<T>(&mut self) {
        self.allocations = self.allocations.saturating_add(1);
        self.bytes = self.bytes.saturating_add(std::mem::size_of::<T>());
    }
}

fn estimate_ir_allocations(program: &Program) -> IrAllocationEstimate {
    let mut estimate = IrAllocationEstimate::default();
    // Program-owned shared containers: buffers, buffer index, entry body,
    // validation cache, plus any lazily-materialized stats/cache Arcs.
    estimate.add_container::<BufferDecl>(program.buffers().len());
    estimate.add_container::<Node>(program.entry().len());
    estimate.allocations = estimate.allocations.saturating_add(2);
    for node in program.entry() {
        estimate_node_allocations(node, &mut estimate);
    }
    estimate
}

fn estimate_node_allocations(node: &Node, estimate: &mut IrAllocationEstimate) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            estimate_expr_allocations(value, estimate);
        }
        Node::Store { index, value, .. } => {
            estimate_expr_allocations(index, estimate);
            estimate_expr_allocations(value, estimate);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            estimate_expr_allocations(cond, estimate);
            estimate.add_container::<Node>(then.len());
            estimate.add_container::<Node>(otherwise.len());
            for node in then.iter().chain(otherwise.iter()) {
                estimate_node_allocations(node, estimate);
            }
        }
        Node::Loop { from, to, body, .. } => {
            estimate_expr_allocations(from, estimate);
            estimate_expr_allocations(to, estimate);
            estimate.add_container::<Node>(body.len());
            for node in body {
                estimate_node_allocations(node, estimate);
            }
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(offset, estimate);
            estimate_expr_allocations(size, estimate);
        }
        Node::Trap { address, .. } => {
            estimate.add_box::<Expr>();
            estimate_expr_allocations(address, estimate);
        }
        Node::Block(body) => {
            estimate.add_container::<Node>(body.len());
            for node in body {
                estimate_node_allocations(node, estimate);
            }
        }
        Node::Region { body, .. } => {
            estimate.add_container::<Node>(body.len());
            for node in body.iter() {
                estimate_node_allocations(node, estimate);
            }
        }
        Node::Opaque(_) => {
            estimate.allocations = estimate.allocations.saturating_add(1);
        }
        Node::IndirectDispatch { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Return
        | Node::Barrier { .. } => {}
    }
}

fn estimate_expr_allocations(expr: &Expr, estimate: &mut IrAllocationEstimate) {
    match expr {
        Expr::Load { index, .. } => {
            estimate.add_box::<Expr>();
            estimate_expr_allocations(index, estimate);
        }
        Expr::BinOp { left, right, .. } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(left, estimate);
            estimate_expr_allocations(right, estimate);
        }
        Expr::UnOp { operand, .. }
        | Expr::Cast { value: operand, .. }
        | Expr::SubgroupBallot { cond: operand }
        | Expr::SubgroupReduce { value: operand, .. } => {
            estimate.add_box::<Expr>();
            estimate_expr_allocations(operand, estimate);
        }
        Expr::Call { args, .. } => {
            estimate.add_container::<Expr>(args.len());
            for arg in args {
                estimate_expr_allocations(arg, estimate);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(cond, estimate);
            estimate_expr_allocations(true_val, estimate);
            estimate_expr_allocations(false_val, estimate);
        }
        Expr::Fma { a, b, c } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(a, estimate);
            estimate_expr_allocations(b, estimate);
            estimate_expr_allocations(c, estimate);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(index, estimate);
            if let Some(expected) = expected {
                estimate.add_box::<Expr>();
                estimate_expr_allocations(expected, estimate);
            }
            estimate_expr_allocations(value, estimate);
        }
        Expr::SubgroupShuffle { value, lane } => {
            estimate.add_box::<Expr>();
            estimate.add_box::<Expr>();
            estimate_expr_allocations(value, estimate);
            estimate_expr_allocations(lane, estimate);
        }
        Expr::Opaque(_) => {
            estimate.allocations = estimate.allocations.saturating_add(1);
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => {}
    }
}

impl PassScheduler {
    /// Attempt to build a `PassScheduler` using the default registered passes.
    ///
    /// # Errors
    ///
    /// Returns [`OptimizerError`] when built-in pass metadata cannot be
    /// registered into a valid scheduler.
    pub fn try_default() -> Result<Self, OptimizerError> {
        let passes = registered_passes()?;
        let pass_index = passes
            .iter()
            .enumerate()
            .map(|(i, pass)| (pass.metadata().name, i))
            .collect();
        let execution_order = (0..passes.len()).collect();
        Ok(Self {
            passes,
            pass_index,
            research_traces: FxHashMap::default(),
            execution_order,
            requirements_prevalidated: true,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            invalidation_adjacency_cache: OnceLock::new(),
            invalidation_closure_cache: OnceLock::new(),
            dirty_trigger_index_cache: OnceLock::new(),
            initial_dirty_flags_cache: OnceLock::new(),
            enforce_cost_monotone: false,
            enforce_effect_handlers: false,
            enforce_linear_types: false,
            enforce_shape_predicates: false,
        })
    }

    /// Toggle the cost-monotone-down post-condition gate. See the field docs on
    /// `PassScheduler::enforce_cost_monotone`. Returns `self` so this composes
    /// with other builder-shaped configuration.
    #[must_use]
    pub fn with_cost_monotone_enforcement(mut self, enforce: bool) -> Self {
        self.enforce_cost_monotone = enforce;
        self
    }

    /// Whether the cost-monotone-down post-condition gate is active.
    #[must_use]
    pub fn cost_monotone_enforcement(&self) -> bool {
        self.enforce_cost_monotone
    }

    /// Toggle effects-handler post-condition enforcement.
    #[must_use]
    pub fn with_effect_handler_enforcement(mut self, enforce: bool) -> Self {
        self.enforce_effect_handlers = enforce;
        self
    }

    /// Whether effects-handler enforcement is active.
    #[must_use]
    pub fn effect_handler_enforcement(&self) -> bool {
        self.enforce_effect_handlers
    }

    /// Toggle linear-type post-condition enforcement.
    #[must_use]
    pub fn with_linear_type_enforcement(mut self, enforce: bool) -> Self {
        self.enforce_linear_types = enforce;
        self
    }

    /// Whether linear-type post-condition enforcement is active.
    #[must_use]
    pub fn linear_type_enforcement(&self) -> bool {
        self.enforce_linear_types
    }

    /// Toggle shape-predicate post-condition enforcement.
    #[must_use]
    pub fn with_shape_predicate_enforcement(mut self, enforce: bool) -> Self {
        self.enforce_shape_predicates = enforce;
        self
    }

    /// Whether shape-predicate post-condition enforcement is active.
    #[must_use]
    pub fn shape_predicate_enforcement(&self) -> bool {
        self.enforce_shape_predicates
    }
}

impl Default for PassScheduler {
    fn default() -> Self {
        match Self::try_default() {
            Ok(scheduler) => scheduler,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "Fix: built-in optimizer pass metadata is invalid; defaulting to an empty scheduler."
                );
                Self::with_passes(Vec::new())
            }
        }
    }
}

// Audit cleanup A21 (2026-04-30): split scheduler.rs (1161 LOC) into
// per-concern submodules. Each carries its own `impl PassScheduler`
// block; Rust merges them at link time.

/// Topological scheduling: `schedule_passes` free fn, precomputed
/// execution-order indices, and the `PassSchedulingError` enum.
mod topo;

/// Fusion-query methods on PassScheduler (transitive_dependents, reaches,
/// invalidation_closure, fusion_pressure, fusable_subset, pair_commutes,
/// etc) + remaining constructor helpers (with_passes, with_max_iterations).
mod queries;

/// Run methods on PassScheduler: run, run_with_metrics, run_once,
/// run_once_with_metrics, mark_invalidated_passes.
mod run;

pub(crate) use topo::schedule_pass_metadata_indices;
pub use topo::{schedule_passes, PassSchedulingError};

#[cfg(test)]
mod research_trace_contract_tests {
    use super::{PassResearchTrace, PassResearchTraceError, PASS_RESEARCH_TRACE_SCHEMA_VERSION};

    #[test]
    fn pass_research_trace_requires_all_identifiers() {
        assert_eq!(
            PassResearchTrace::try_new("", "baseline/vyre-default", "artifact/optimizer-proof")
                .unwrap_err(),
            PassResearchTraceError::MissingResearchBasisKey
        );
        assert_eq!(
            PassResearchTrace::try_new(
                "research/mlir-pass-replay",
                "",
                "artifact/optimizer-proof"
            )
            .unwrap_err(),
            PassResearchTraceError::MissingBaselineId
        );
        assert_eq!(
            PassResearchTrace::try_new("research/mlir-pass-replay", "baseline/vyre-default", "")
                .unwrap_err(),
            PassResearchTraceError::MissingProofArtifactId
        );
    }

    #[test]
    fn scheduler_metric_construction_carries_research_trace() {
        let trace = PassResearchTrace::try_new(
            "research/mlir-pass-replay",
            "baseline/vyre-default",
            "artifact/vx-336-optimizer-proof",
        )
        .unwrap();
        assert_eq!(trace.schema_version, PASS_RESEARCH_TRACE_SCHEMA_VERSION);
        assert!(trace.is_complete());

        let run_source = include_str!("run.rs");
        assert!(run_source.contains("research_trace: self.research_trace_for(metadata.name)"));
    }
}

#[cfg(test)]
mod tests;
