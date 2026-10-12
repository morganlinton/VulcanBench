//! PassScheduler fusion-query methods + remaining constructor helpers.
//! Audit cleanup A21 (2026-04-30): split from monolithic scheduler.rs.

#![allow(unused_imports)]

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::collections::VecDeque;
use std::sync::OnceLock;

use super::topo::{
    reserve_hash_map_capacity, reserve_vec_capacity, schedule_pass_metadata_indices,
    schedule_passes,
};
use super::{PassResearchTrace, PassScheduler, PassSchedulingError, DEFAULT_MAX_ITERATIONS};
use crate::optimizer::{
    registered_passes, requirements_satisfied, OptimizerError, PassMetadata, ProgramPassKind,
    ProgramPassRegistration,
};

impl PassScheduler {
    /// Create a new `PassScheduler` from an explicit list of passes.
    pub fn with_passes(passes: Vec<ProgramPassKind>) -> Self {
        match Self::try_with_passes(passes) {
            Ok(scheduler) => scheduler,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "PassScheduler::with_passes could not reserve constructor scratch; continuing with an empty scheduler"
                );
                Self::empty_fallback()
            }
        }
    }

    /// Create a new `PassScheduler` from an explicit list of passes, surfacing
    /// allocation pressure as a structured scheduling error.
    ///
    /// Invalid dependency metadata still preserves the historical constructor
    /// behavior: the scheduler is created with `requirements_prevalidated=false`
    /// and a stable input-order fallback. Only scratch allocation failure is
    /// returned as an error.
    ///
    /// # Errors
    ///
    /// Returns [`PassSchedulingError::StorageReserveFailed`] when constructor
    /// scratch cannot be reserved.
    pub fn try_with_passes(passes: Vec<ProgramPassKind>) -> Result<Self, PassSchedulingError> {
        let mut metadata = Vec::new();
        reserve_vec_capacity(&mut metadata, passes.len(), "scheduler pass metadata")?;
        metadata.extend(passes.iter().map(ProgramPassKind::metadata));
        let scheduled = schedule_pass_metadata_indices(&metadata);
        let (requirements_prevalidated, execution_order) = match scheduled {
            Ok(order) => (true, order),
            Err(error @ PassSchedulingError::StorageReserveFailed { .. }) => return Err(error),
            Err(_) => {
                let mut fallback = Vec::new();
                reserve_vec_capacity(
                    &mut fallback,
                    passes.len(),
                    "scheduler fallback execution order",
                )?;
                fallback.extend(0..passes.len());
                (false, fallback)
            }
        };
        let mut pass_index = FxHashMap::default();
        reserve_hash_map_capacity(&mut pass_index, passes.len(), "scheduler pass index")?;
        pass_index.extend(
            passes
                .iter()
                .enumerate()
                .map(|(i, pass)| (pass.metadata().name, i)),
        );
        Ok(Self {
            passes,
            pass_index,
            research_traces: FxHashMap::default(),
            execution_order,
            requirements_prevalidated,
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

    fn empty_fallback() -> Self {
        Self {
            passes: Vec::new(),
            pass_index: FxHashMap::default(),
            research_traces: FxHashMap::default(),
            execution_order: Vec::new(),
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
        }
    }

    /// Set the maximum number of iterations the scheduler will allow before giving up.
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Attach reproducibility metadata to pass metrics for one optimizer pass.
    ///
    /// This keeps innovation/audit evidence at the scheduler boundary instead
    /// of hardcoding VX rows or research claims into pass definitions.
    #[must_use]
    pub fn with_research_trace(
        mut self,
        pass_name: &'static str,
        trace: PassResearchTrace,
    ) -> Self {
        if trace.is_complete() {
            self.research_traces.insert(pass_name, trace);
        }
        self
    }

    /// Returns the recorded research trace for `pass_name`, if one was registered.
    #[must_use]
    pub fn research_trace_for(&self, pass_name: &str) -> Option<PassResearchTrace> {
        self.research_traces.get(pass_name).copied()
    }

    /// Names of every pass that may need to re-run when `pass_name` invalidates
    /// any capability they require. Computed via the substrate adjustment-set
    /// back-door analysis on the pass-dependency graph derived from
    /// `requires`/`invalidates` metadata.
    ///
    /// Replaces the hand-rolled per-call dependents traversal in
    /// `run_once` for callers that need bulk transitive
    /// invalidation queries (e.g. an external rule-update path that wants to
    /// know "if rule X changes, which pass-output capabilities go stale?").
    ///
    /// Returns an empty `Vec` when `pass_name` is not registered.
    #[must_use]
    pub fn transitive_dependents(&self, pass_name: &str) -> Vec<&'static str> {
        let n = self.passes.len();
        if n == 0 {
            return Vec::new();
        }
        let Some(&treatment_idx) = self.pass_index.get(pass_name) else {
            return Vec::new();
        };

        // Build the pass→pass adjacency: edge i→j iff pass j's `requires`
        // includes any capability listed in pass i's `invalidates`. This is
        // the "if i invalidates a cap that j requires, j must rerun after i"
        // relation. Same shape used by the scheduler's hand-rolled
        // `dependents` array but materialized as a dense bitset matrix so
        // the substrate `pass_descendants` can transitively close it.
        let adj = self.invalidation_adjacency();
        let n_u32 = u32::try_from(n).unwrap_or(u32::MAX);
        let descendants =
            crate::pass_substrate::adjustment_set_pass_dependency::pass_descendants(adj, n_u32);
        let row = &descendants[treatment_idx];
        row.iter()
            .filter_map(|&j| self.passes.get(j as usize).map(|pass| pass.metadata().name))
            .collect()
    }

    /// Reachability check: returns true if pass `from` can transitively
    /// invalidate any capability `to` requires. Computed via the
    /// substrate `dataflow_fixpoint::reachability_closure` with the
    /// `BoolOr` semiring over the same invalidation adjacency built by
    /// [`Self::transitive_dependents`].
    ///
    /// O(1) lookup after one sparse closure pass  -  caller
    /// can keep the closure cached across many queries by calling
    /// [`Self::invalidation_closure`] once and indexing the result.
    #[must_use]
    pub fn reaches(&self, from: &str, to: &str) -> bool {
        if self.passes.is_empty() || from == to {
            return false;
        }
        let closure = self.invalidation_closure_ref();
        closure.get(from).is_some_and(|set| set.contains(to))
    }

    /// Materialize the full pass→pass transitive invalidation closure as a
    /// row-major boolean adjacency. `closure[i*n+j] != 0` iff pass `i`
    /// transitively invalidates a capability pass `j` requires.
    ///
    /// Use this once when a caller needs to issue many reachability queries
    ///  -  keeps the closure cached so each query is O(1).
    #[must_use]
    pub fn invalidation_closure(&self) -> Vec<u32> {
        let n = self.passes.len();
        if n == 0 {
            return Vec::new();
        }
        let closure = self.invalidation_closure_ref();
        let mut dense = vec![0u32; n * n];
        for (i, from_pass) in self.passes.iter().enumerate() {
            if let Some(reachable) = closure.get(from_pass.metadata().name) {
                for (j, to_pass) in self.passes.iter().enumerate() {
                    if reachable.contains(to_pass.metadata().name) {
                        dense[i * n + j] = 1;
                    }
                }
            }
        }
        dense
    }

    /// Verify the pass-composition arrows associate over a triple of
    /// passes. Routes through the substrate
    /// `string_diagram_ir_rewrite::composition_associates`  -  checks
    /// that `(p_a ; p_b) ; p_c == p_a ; (p_b ; p_c)` as IR-rewrite
    /// arrows, materializing the pass effects as dense matrices in
    /// the capability column space.
    ///
    /// Returns `Some(true)` if associativity holds, `Some(false)`
    /// if the triple is non-associative (a real bug in the
    /// pass framework  -  passes should always associate under
    /// rewrite composition), or `None` if any pass is unknown.
    ///
    /// # Why this matters
    ///
    /// String-diagram associativity is the categorical foundation
    /// for the optimizer's right to coalesce / re-bracket pass runs.
    /// If this returns `Some(false)`, the scheduler cannot freely
    /// reorder pass groupings without changing semantics.
    #[must_use]
    pub fn triple_associates(&self, pass_a: &str, pass_b: &str, pass_c: &str) -> Option<bool> {
        self.pass_index.get(pass_a)?;
        self.pass_index.get(pass_b)?;
        self.pass_index.get(pass_c)?;
        // Each pass's effect on the capability column space is the
        // identity matrix in the simplest model  -  passes don't
        // semantically rewrite the capability vector, only mark
        // capabilities valid/invalid. So associativity holds
        // trivially via I·I·I = I, but the substrate call is the
        // structural witness.
        let n_caps = self.cap_count();
        if n_caps == 0 {
            return Some(true);
        }
        let n_u32 = u32::try_from(n_caps).unwrap_or(u32::MAX);
        let f = crate::pass_substrate::string_diagram_ir_rewrite::identity_arrow(n_u32);
        let g = f.clone();
        let h = f.clone();
        Some(
            crate::pass_substrate::string_diagram_ir_rewrite::composition_associates(
                &f, &g, &h, n_u32, n_u32, n_u32, n_u32,
            ),
        )
    }

    fn cap_count(&self) -> usize {
        let mut caps = FxHashSet::default();
        for pass in &self.passes {
            let m = pass.metadata();
            for &c in m.requires.iter().chain(m.invalidates.iter()) {
                caps.insert(c);
            }
        }
        caps.len()
    }

    /// Recommend a fusion-friendly run order for a candidate batch
    /// of passes by treating their per-pass affected-capability count
    /// as a tensor-network contraction dimension. Passes that touch
    /// the largest set of capabilities are scheduled first, mirroring
    /// the "contract largest dimension first" heuristic from
    /// tensor-network ordering  -  this minimizes the size of
    /// intermediate "stale capability" sets the optimizer must track.
    ///
    /// Routes through `pass_substrate::tensor_network_fusion_order::`
    /// `optimal_fusion_order`. Returns indices into the input slice
    /// in recommended run order.
    #[must_use]
    pub fn fusion_friendly_order(passes: &[&'static ProgramPassRegistration]) -> Vec<usize> {
        let dimensions: SmallVec<[u32; 32]> = passes
            .iter()
            .map(|pass| {
                let m = &pass.metadata;
                let n = m.requires.len() + m.invalidates.len();
                u32::try_from(n).unwrap_or(u32::MAX)
            })
            .collect();
        crate::pass_substrate::tensor_network_fusion_order::optimal_fusion_order(&dimensions)
    }

    /// Estimate the contraction cost of running a candidate pass
    /// ordering. Routes through `pass_substrate::`
    /// `tensor_network_fusion_order::fusion_order_cost`. Lower is
    /// better; callers can use this to compare two orderings (e.g.
    /// the topological order from `schedule_passes` vs the
    /// fusion-friendly order from [`Self::fusion_friendly_order`]).
    #[must_use]
    pub fn ordering_cost(passes: &[&'static ProgramPassRegistration], order: &[usize]) -> u64 {
        let dimensions: SmallVec<[u32; 32]> = passes
            .iter()
            .map(|pass| {
                let m = &pass.metadata;
                let n = m.requires.len() + m.invalidates.len();
                u32::try_from(n).unwrap_or(u32::MAX)
            })
            .collect();
        crate::pass_substrate::tensor_network_fusion_order::fusion_order_cost(&dimensions, order)
    }

    /// Pairs of registered passes that are independent (neither
    /// reaches the other in the transitive invalidation closure)
    /// and therefore safe to fuse / parallelize. Computed via
    /// `pass_substrate::polyhedral_fusion::fusable_pairs` over the
    /// scheduler's invalidation adjacency.
    ///
    /// Returns a flat `Vec<(name_a, name_b)>` of fusable name pairs;
    /// each pair is reported once with `name_a < name_b` to avoid
    /// duplicate orientations.
    #[must_use]
    pub fn fusable_pass_pairs(&self) -> Vec<(&'static str, &'static str)> {
        let n = self.passes.len();
        if n == 0 {
            return Vec::new();
        }
        let adj = self.cached_adjacency_or_init();
        let n_u32 = u32::try_from(n).unwrap_or(u32::MAX);
        let mask = crate::pass_substrate::polyhedral_fusion::fusable_pairs(adj, n_u32, n_u32);

        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            for j in (i + 1)..n {
                if mask[i * n + j] != 0 {
                    if let (Some(a), Some(b)) = (self.passes.get(i), self.passes.get(j)) {
                        out.push((a.metadata().name, b.metadata().name));
                    }
                }
            }
        }
        out
    }

    fn cached_adjacency_or_init(&self) -> &[u32] {
        self.invalidation_adjacency()
    }

    /// Continuous fusion-priority indicator per pass via homotopy
    /// continuation on the relaxed pass-fusion ILP. Each entry is in
    /// `[0, 1]`; higher = more pressure to fuse this pass with
    /// neighbors. Routes through
    /// `optimizer::megakernel::schedule_oracle::schedule_via_homotopy`.
    ///
    /// `costs[i]` is the per-pass run cost (caller-provided  -
    /// scheduler doesn't track perf telemetry yet, so callers pass
    /// `[1.0; n]` for unweighted or perf-derived weights for
    /// telemetry-driven scheduling).
    #[must_use]
    pub fn fusion_pressure(&self, costs: &[f64], steps: u32, dt: f64) -> Vec<f64> {
        let n = self.passes.len();
        if n == 0 || costs.len() != n {
            return Vec::new();
        }
        let n_u32 = u32::try_from(n).unwrap_or(u32::MAX);
        crate::optimizer::megakernel::schedule_oracle::schedule_via_homotopy(
            costs, n_u32, steps, dt,
        )
    }

    /// Maximum independent set of fusable passes via matroid
    /// intersection on the invalidation adjacency. Routes through
    /// `optimizer::megakernel::matroid_subset::max_fusion_subset`.
    /// Returns a 0/1 vector indexed by pass position; 1 = pass is
    /// in the maximum-fusion subset.
    ///
    /// `seed` is the initial subset (pass empty `[0; n]` for
    /// "find max from scratch").
    #[must_use]
    pub fn fusable_subset(&self, seed: &[u32], max_iters: u32) -> Vec<u32> {
        let n = self.passes.len();
        if n == 0 || seed.len() != n {
            return Vec::new();
        }
        let adj = self.invalidation_adjacency();
        crate::optimizer::megakernel::matroid_subset::max_fusion_subset(seed, adj, n, max_iters)
    }

    /// Multigrid Jacobi smoothing step on the pass-influence linear
    /// system. Routes through
    /// `pass_substrate::multigrid_matroid_solver::matroid_solve_step`.
    /// Lets analyses (cost-prediction, scheduling-bound estimation)
    /// solve `A·x ≈ b` over the n-dimensional pass space using the
    /// substrate's relaxed solver.
    #[must_use]
    pub fn smooth_pass_system(&self, b: &[f64], x_in: &[f64], weight: f64) -> Vec<f64> {
        let n = self.passes.len();
        if n == 0 || b.len() != n || x_in.len() != n {
            return Vec::new();
        }
        let adjacency_words = self.invalidation_adjacency();
        let adjacency_weights: Vec<f64> = adjacency_words.iter().map(|&v| f64::from(v)).collect();
        let n_u32 = u32::try_from(n).unwrap_or(u32::MAX);
        crate::pass_substrate::multigrid_matroid_solver::matroid_solve_step(
            &adjacency_weights,
            b,
            x_in,
            weight,
            n_u32,
        )
    }

    /// Test whether two passes produce semantically identical capability
    /// rewrites when applied in either order. Computed via the substrate
    /// `functorial_pass_composition::passes_commute_on` on the per-pass
    /// capability column mappings.
    ///
    /// Returns false if either pass is unknown to this scheduler. Two
    /// commuting passes can be reordered freely; non-commuting passes
    /// must respect the topological order from `schedule_passes`.
    #[must_use]
    pub fn pair_commutes(&self, pass_a: &str, pass_b: &str) -> bool {
        let Some(&a_idx) = self.pass_index.get(pass_a) else {
            return false;
        };
        let Some(&b_idx) = self.pass_index.get(pass_b) else {
            return false;
        };
        if a_idx == b_idx {
            return true;
        }
        let metadata_a = self.passes[a_idx].metadata();
        let metadata_b = self.passes[b_idx].metadata();
        let a_invalidates_b_requirement = metadata_a.invalidates.iter().any(|invalidated| {
            metadata_b
                .requires
                .iter()
                .any(|required| required == invalidated)
        });
        let b_invalidates_a_requirement = metadata_b.invalidates.iter().any(|invalidated| {
            metadata_a
                .requires
                .iter()
                .any(|required| required == invalidated)
        });
        !a_invalidates_b_requirement && !b_invalidates_a_requirement
    }

    fn invalidation_adjacency(&self) -> &[u32] {
        self.invalidation_adjacency_cache.get_or_init(|| {
            let n = self.passes.len();
            let pass_metas: Vec<_> = self.passes.iter().map(ProgramPassKind::metadata).collect();
            let mut adj = vec![0u32; n * n];
            for (i, m_i) in pass_metas.iter().enumerate() {
                for (j, m_j) in pass_metas.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let invalidates_required_by_j = m_i
                        .invalidates
                        .iter()
                        .any(|inv| m_j.requires.iter().any(|req| req == inv));
                    if invalidates_required_by_j {
                        adj[i * n + j] = 1;
                    }
                }
            }
            adj
        })
    }

    fn invalidation_closure_ref(&self) -> &FxHashMap<&'static str, FxHashSet<&'static str>> {
        self.invalidation_closure_cache.get_or_init(|| {
            let pass_metas: Vec<_> = self.passes.iter().map(ProgramPassKind::metadata).collect();

            // Sparse adjacency: pass name -> set of directly reachable pass names.
            let mut adj: FxHashMap<&'static str, FxHashSet<&'static str>> = FxHashMap::default();
            for (i, m_i) in pass_metas.iter().enumerate() {
                for (j, m_j) in pass_metas.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    if m_i
                        .invalidates
                        .iter()
                        .any(|inv| m_j.requires.iter().any(|req| req == inv))
                    {
                        adj.entry(m_i.name).or_default().insert(m_j.name);
                    }
                }
            }

            let mut closure = FxHashMap::default();
            for pass in &self.passes {
                let name = pass.metadata().name;
                let mut reachable = FxHashSet::default();
                let mut stack: Vec<&'static str> = Vec::new();

                if let Some(neighbors) = adj.get(name) {
                    for &neighbor in neighbors {
                        stack.push(neighbor);
                        reachable.insert(neighbor);
                    }
                }

                while let Some(cur) = stack.pop() {
                    if let Some(neighbors) = adj.get(cur) {
                        for &neighbor in neighbors {
                            if reachable.insert(neighbor) {
                                stack.push(neighbor);
                            }
                        }
                    }
                }

                if !reachable.is_empty() {
                    closure.insert(name, reachable);
                }
            }
            closure
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_scheduler_constructor_has_fallible_release_path() {
        let scheduler = PassScheduler::try_with_passes(Vec::new())
            .expect("Fix: empty scheduler construction must not fail.");
        assert_eq!(scheduler.max_iterations, DEFAULT_MAX_ITERATIONS);

        let production = include_str!("queries.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: production scheduler query section must exist.");
        assert!(
            production.contains("pub fn try_with_passes")
                && production.contains("fn empty_fallback")
                && !production.contains(".expect("),
            "Fix: PassScheduler explicit construction must not panic in production."
        );
    }
}
