//! Rule-graph change-impact as a Pearl do-calculus query (#36 substrate).
//!
//! Frames vyre's cache-invalidation as a `do(rule_X)` query on the
//! dependency graph. When rule `X` changes, `do(X)` on the graph
//! predicts which downstream Programs invalidate.
//!
//! This replaces ad-hoc cache invalidation with formal causal analysis.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::dataflow_fixpoint::reachability_closure_into;
use crate::dataflow_fixpoint::reachability_closure_via_into;
#[cfg(test)]
use crate::dispatch_buffers::u32_slice_to_le_bytes;
use crate::dispatch_buffers::{
    ceil_div_u32, checked_square_cells, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_foundation::ir::Program;
use vyre_primitives::graph::do_calculus::{
    do_intervention_delete_incoming, do_rule2_reverse_incoming,
};
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::graph::do_calculus::{
    do_intervention_delete_incoming_cpu_into, do_rule2_reverse_incoming_cpu_into,
    do_rule3_subgraph_cpu_into,
};

/// Reusable matrix buffers for do-calculus impact queries.
#[derive(Debug, Default)]
pub struct DoCalculusImpactScratch {
    surgically_modified_adj: Vec<u32>,
    closure: Vec<u32>,
    scratch: Vec<u32>,
    impact_mask: Vec<u32>,
    reduced_adjacency: Vec<u32>,
    kept_indices: Vec<u32>,
    dispatch_inputs: Vec<Vec<u8>>,
}

impl DoCalculusImpactScratch {
    /// Last computed impact mask.
    #[must_use]
    pub fn impact_mask(&self) -> &[u32] {
        &self.impact_mask
    }

    /// Last computed reduced adjacency.
    #[must_use]
    pub fn reduced_adjacency(&self) -> &[u32] {
        &self.reduced_adjacency
    }

    /// Original indices retained in the last reduced adjacency.
    #[must_use]
    pub fn kept_indices(&self) -> &[u32] {
        &self.kept_indices
    }
}

/// Predict which nodes in a dependency graph are impacted by a change
/// in a subset of nodes.
///
/// This performs a `do(intervened_nodes)` intervention (removing
/// incoming edges to the changed nodes) and then computes the
/// transitive closure to find all affected downstream nodes.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn predict_impact(adj: &[u32], intervention_mask: &[u32], n: u32) -> Vec<u32> {
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);
    if n == 0 {
        return Vec::new();
    }
    let mut scratch = DoCalculusImpactScratch::default();
    predict_impact_with_scratch(adj, intervention_mask, n, &mut scratch);
    scratch.impact_mask
}

/// Predict impact using named reusable scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn predict_impact_with_scratch(
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    scratch: &mut DoCalculusImpactScratch,
) {
    reference_predict_impact_into(
        adj,
        intervention_mask,
        n,
        &mut scratch.surgically_modified_adj,
        &mut scratch.closure,
        &mut scratch.scratch,
        &mut scratch.impact_mask,
    );
}

/// Predict impact while reusing caller-owned matrix scratch buffers.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_predict_impact_into(
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    surgically_modified_adj: &mut Vec<u32>,
    closure: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
    impact_mask: &mut Vec<u32>,
) {
    if n == 0 {
        impact_mask.clear();
        return;
    }
    do_intervention_delete_incoming_cpu_into(adj, intervention_mask, n, surgically_modified_adj);

    reachability_closure_into(surgically_modified_adj, n, n, closure, scratch);

    impact_mask_from_closure(intervention_mask, closure, n, impact_mask);
}

fn impact_mask_from_closure(
    intervention_mask: &[u32],
    closure: &[u32],
    n: u32,
    impact_mask: &mut Vec<u32>,
) {
    let n_us = n as usize;
    impact_mask.clear();
    impact_mask.resize(n_us, 0);
    for i in 0..n_us {
        if intervention_mask[i] != 0 {
            impact_mask[i] = 1; // Itself is impacted.
            for j in 0..n_us {
                if closure[i * n_us + j] != 0 {
                    impact_mask[j] = 1;
                }
            }
        }
    }
}

/// GPU-backed impact prediction using primitive-native graph surgery and
/// reachability closure dispatch.
///
/// This keeps the graph rewrite and transitive closure off the CPU. The final
/// host projection only materializes the already-read-back `n`-word impact mask
/// needed by cache invalidation callers.
#[must_use = "GPU impact prediction returns a mask or dispatch error that must be handled"]
pub fn predict_impact_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = DoCalculusImpactScratch::default();
    predict_impact_via_into(dispatcher, adj, intervention_mask, n, &mut scratch)?;
    Ok(scratch.impact_mask)
}

/// GPU-backed impact prediction into caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn predict_impact_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    scratch: &mut DoCalculusImpactScratch,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);
    if n == 0 {
        scratch.impact_mask.clear();
        scratch.surgically_modified_adj.clear();
        scratch.closure.clear();
        return Ok(());
    }
    intervention_delete_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        intervention_mask,
        n,
        &mut scratch.dispatch_inputs,
        &mut scratch.surgically_modified_adj,
    )?;
    reachability_closure_via_into(
        dispatcher,
        &scratch.surgically_modified_adj,
        n,
        n,
        &mut scratch.closure,
        &mut scratch.scratch,
    )?;
    impact_mask_from_closure(
        intervention_mask,
        &scratch.closure,
        n,
        &mut scratch.impact_mask,
    );
    Ok(())
}

/// Primitive-native dispatcher path for Pearl Rule 1 graph surgery:
/// remove incoming edges to every intervened node.
///
/// This is the GPU-backed first stage of [`predict_impact`]. Full impact
/// prediction also needs reachability closure; callers that already keep the
/// closure on-device can compose this output with the closure primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid, lane counts overflow,
/// or the backend returns malformed output.
pub fn intervention_delete_incoming_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    let mut inputs = Vec::new();
    intervention_delete_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        intervention_mask,
        n,
        &mut inputs,
        &mut out,
    )?;
    Ok(out)
}

/// Dispatcher-backed intervention graph surgery into caller-owned storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn intervention_delete_incoming_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut inputs = Vec::new();
    intervention_delete_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        intervention_mask,
        n,
        &mut inputs,
        out,
    )
}

fn intervention_delete_incoming_via_into_with_inputs(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    inputs: &mut Vec<Vec<u8>>,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    dispatch_do_calculus_surgery_into(
        dispatcher,
        adj,
        intervention_mask,
        n,
        inputs,
        out,
        "intervention_delete_incoming_via",
        "intervention_mask",
        do_intervention_delete_incoming,
    )
}

/// Primitive-native dispatcher path for Pearl Rule 2 graph surgery:
/// reverse incoming edges to every observed/treatment node.
///
/// This is the GPU-backed first stage of [`predict_impact_observation_form`].
/// Full observation-form impact also needs reachability closure; callers that
/// keep closure on-device can compose this output directly with the closure
/// primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid, lane counts overflow, or
/// the backend returns malformed output.
pub fn rule2_reverse_incoming_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    treatment_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    let mut inputs = Vec::new();
    rule2_reverse_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        treatment_mask,
        n,
        &mut inputs,
        &mut out,
    )?;
    Ok(out)
}

/// Dispatcher-backed Rule 2 graph surgery into caller-owned storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn rule2_reverse_incoming_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    treatment_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut inputs = Vec::new();
    rule2_reverse_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        treatment_mask,
        n,
        &mut inputs,
        out,
    )
}

fn rule2_reverse_incoming_via_into_with_inputs(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    treatment_mask: &[u32],
    n: u32,
    inputs: &mut Vec<Vec<u8>>,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    dispatch_do_calculus_surgery_into(
        dispatcher,
        adj,
        treatment_mask,
        n,
        inputs,
        out,
        "rule2_reverse_incoming_via",
        "treatment_mask",
        do_rule2_reverse_incoming,
    )
}

fn dispatch_do_calculus_surgery_into<F>(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    mask: &[u32],
    n: u32,
    inputs: &mut Vec<Vec<u8>>,
    out: &mut Vec<u32>,
    op_name: &'static str,
    mask_buffer: &'static str,
    build_program: F,
) -> Result<(), DispatchError>
where
    F: FnOnce(&str, &str, &str, u32) -> Program,
{
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);

    let cells = checked_square_cells(n, op_name)?;
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {op_name} n*n exceeds the primitive u32 lane limit for n={n}."
        ))
    })?;
    if adj.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {op_name} requires adj.len() == n*n, got len={}, n={n}, n*n={cells}.",
            adj.len()
        )));
    }
    if mask.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {op_name} requires {mask_buffer}.len() == n, got len={}, n={n}.",
            mask.len()
        )));
    }

    let program = build_program("adj", mask_buffer, "out", n);
    ensure_input_slots(inputs, 2);
    write_u32_slice_le_bytes(&mut inputs[0], adj);
    write_u32_slice_le_bytes(&mut inputs[1], mask);
    let outputs = dispatcher.dispatch(
        &program,
        &inputs[..2],
        Some([ceil_div_u32(cells_u32, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: {op_name} expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], cells, op_name, out)
}

/// Compute the impacted subgraph: the adjacency restricted to the
/// nodes [`predict_impact`] flags as stale.
///
/// Uses do-calculus Rule 3 (subgraph extraction) on the impact mask.
/// Returns `(reduced_adjacency, kept_indices)` where `reduced_adjacency`
/// is row-major `k × k` with `k = kept_indices.len()`. The reduced
/// adjacency contains only edges between impacted nodes; downstream
/// analyses (lineage walks, dependency reports) iterate `k²` cells
/// instead of `n²`.
///
/// On a hot path this lets cache invalidation skip every non-impacted
/// row outright when computing per-impacted lineage details  -  `k` is
/// almost always far smaller than `n`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn impact_subgraph(adj: &[u32], intervention_mask: &[u32], n: u32) -> (Vec<u32>, Vec<u32>) {
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut scratch = DoCalculusImpactScratch::default();
    reference_impact_subgraph_with_scratch(adj, intervention_mask, n, &mut scratch);
    (scratch.reduced_adjacency, scratch.kept_indices)
}

/// Compute impacted subgraph using named reusable scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_impact_subgraph_with_scratch(
    adj: &[u32],
    intervention_mask: &[u32],
    n: u32,
    scratch: &mut DoCalculusImpactScratch,
) {
    predict_impact_with_scratch(adj, intervention_mask, n, scratch);
    do_rule3_subgraph_cpu_into(
        adj,
        &scratch.impact_mask,
        n,
        &mut scratch.reduced_adjacency,
        &mut scratch.kept_indices,
    );
}

/// Predict impact under the **observation** semantics rather than
/// the **intervention** semantics.
///
/// Pearl's Rule 2 (action / observation exchange) says that for a
/// node X, we can replace `do(X)` with an observation `X` after
/// reversing the edges incoming to X. The two yield the same
/// downstream-impact set on a DAG; on a graph with feedback edges
/// into the observed node they differ  -  the rule-2 form lets a
/// caller answer "if we OBSERVED rule X had changed (rather than
/// explicitly invalidating it), what does the dependency graph
/// predict?". Cache-invalidation telemetry uses this to model
/// "passive change detection" against "active invalidation".
///
/// Returns a 0/1 mask over the n nodes; bit `j` set means the
/// graph's reversed-edge reachability from the observed set
/// reaches `j`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]

pub fn predict_impact_observation_form(adj: &[u32], observation_mask: &[u32], n: u32) -> Vec<u32> {
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);
    if n == 0 {
        return Vec::new();
    }
    let mut scratch = DoCalculusImpactScratch::default();
    predict_impact_observation_form_with_scratch(adj, observation_mask, n, &mut scratch);
    scratch.impact_mask
}

/// Predict observation-form impact using named reusable scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn predict_impact_observation_form_with_scratch(
    adj: &[u32],
    observation_mask: &[u32],
    n: u32,
    scratch: &mut DoCalculusImpactScratch,
) {
    reference_predict_impact_observation_form_into(
        adj,
        observation_mask,
        n,
        &mut scratch.surgically_modified_adj,
        &mut scratch.closure,
        &mut scratch.scratch,
        &mut scratch.impact_mask,
    );
}

/// Predict observation-form impact while reusing caller-owned matrix scratch.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_predict_impact_observation_form_into(
    adj: &[u32],
    observation_mask: &[u32],
    n: u32,
    reversed_adj: &mut Vec<u32>,
    closure: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
    impact_mask: &mut Vec<u32>,
) {
    if n == 0 {
        impact_mask.clear();
        return;
    }
    do_rule2_reverse_incoming_cpu_into(adj, observation_mask, n, reversed_adj);
    reachability_closure_into(reversed_adj, n, n, closure, scratch);
    impact_mask_from_closure(observation_mask, closure, n, impact_mask);
}

/// GPU-backed observation-form impact prediction.
///
/// Uses the Rule 2 graph-surgery primitive plus GPU reachability closure. The
/// remaining host work only projects the returned closure into the `n`-word
/// mask required by cache invalidation and diagnostics.
#[must_use = "GPU observation-form impact prediction returns a mask or dispatch error that must be handled"]
pub fn predict_impact_observation_form_via(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    observation_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = DoCalculusImpactScratch::default();
    predict_impact_observation_form_via_into(dispatcher, adj, observation_mask, n, &mut scratch)?;
    Ok(scratch.impact_mask)
}

/// GPU-backed observation-form impact prediction into caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn predict_impact_observation_form_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    adj: &[u32],
    observation_mask: &[u32],
    n: u32,
    scratch: &mut DoCalculusImpactScratch,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, do_calculus_change_impact_calls};
    bump(&do_calculus_change_impact_calls);
    if n == 0 {
        scratch.impact_mask.clear();
        scratch.surgically_modified_adj.clear();
        scratch.closure.clear();
        return Ok(());
    }
    rule2_reverse_incoming_via_into_with_inputs(
        dispatcher,
        adj,
        observation_mask,
        n,
        &mut scratch.dispatch_inputs,
        &mut scratch.surgically_modified_adj,
    )?;
    reachability_closure_via_into(
        dispatcher,
        &scratch.surgically_modified_adj,
        n,
        n,
        &mut scratch.closure,
        &mut scratch.scratch,
    )?;
    impact_mask_from_closure(
        observation_mask,
        &scratch.closure,
        n,
        &mut scratch.impact_mask,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    #[test]
    fn chain_impact() {
        // 0 -> 1 -> 2
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        // Change node 0
        let mask = vec![1, 0, 0];
        let impact = predict_impact(&adj, &mask, 3);
        // All impacted
        assert_eq!(impact, vec![1, 1, 1]);
    }

    #[test]
    fn impact_scratch_reuses_matrix_buffers() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mask = vec![1, 0, 0];
        let mut scratch = DoCalculusImpactScratch::default();
        predict_impact_with_scratch(&adj, &mask, 3, &mut scratch);
        let modified_capacity = scratch.surgically_modified_adj.capacity();
        let closure_capacity = scratch.closure.capacity();
        let temp_capacity = scratch.scratch.capacity();
        let mask_capacity = scratch.impact_mask.capacity();
        assert_eq!(scratch.impact_mask(), &[1, 1, 1]);

        predict_impact_with_scratch(&adj, &[0, 1, 0], 3, &mut scratch);
        assert_eq!(
            scratch.surgically_modified_adj.capacity(),
            modified_capacity
        );
        assert_eq!(scratch.closure.capacity(), closure_capacity);
        assert_eq!(scratch.scratch.capacity(), temp_capacity);
        assert_eq!(scratch.impact_mask.capacity(), mask_capacity);
        assert_eq!(scratch.impact_mask(), &[0, 1, 1]);
    }

    #[test]
    fn middle_chain_impact() {
        // 0 -> 1 -> 2
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        // Change node 1
        let mask = vec![0, 1, 0];
        let impact = predict_impact(&adj, &mask, 3);
        // 1 and 2 impacted, 0 not impacted
        assert_eq!(impact, vec![0, 1, 1]);
    }

    #[test]
    fn branched_impact() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let adj = vec![0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0];
        // Change node 2
        let mask = vec![0, 0, 1, 0];
        let impact = predict_impact(&adj, &mask, 4);
        // 2 and 3 impacted
        assert_eq!(impact, vec![0, 0, 1, 1]);
    }

    #[test]
    fn disjoint_impact() {
        // 0 -> 1, 2 -> 3
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        // Change node 0
        let mask = vec![1, 0, 0, 0];
        let impact = predict_impact(&adj, &mask, 4);
        // 0 and 1 impacted
        assert_eq!(impact, vec![1, 1, 0, 0]);
    }

    #[test]
    fn cycle_impact() {
        // 0 -> 1, 1 -> 0, 1 -> 2
        let adj = vec![0, 1, 0, 1, 0, 1, 0, 0, 0];
        // Change node 0.
        // do(0) removes 1 -> 0.
        // 0 -> 1 -> 2 remains.
        let mask = vec![1, 0, 0];
        let impact = predict_impact(&adj, &mask, 3);
        // All impacted
        assert_eq!(impact, vec![1, 1, 1]);
    }

    #[test]
    fn empty_graph() {
        let impact = predict_impact(&[], &[], 0);
        assert!(impact.is_empty());
    }

    // ---- impact_subgraph (Rule 3 consumer) ----

    #[test]
    fn impact_subgraph_chain_extracts_downstream() {
        // 0 -> 1 -> 2. Intervene 0: impact = {0,1,2}, subgraph = full.
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mask = vec![1, 0, 0];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 3);
        assert_eq!(kept, vec![0, 1, 2]);
        assert_eq!(reduced, adj);
    }

    #[test]
    fn impact_subgraph_branch_compresses_unimpacted_rows() {
        // 0 -> 1, 2 -> 3 (disjoint). Intervene 0: impact = {0,1};
        // reduced is 2×2, kept = [0, 1].
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let mask = vec![1, 0, 0, 0];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 4);
        assert_eq!(kept, vec![0, 1]);
        // Edge 0->1 preserved, 2x2 layout.
        assert_eq!(reduced, vec![0, 1, 0, 0]);
    }

    #[test]
    fn impact_subgraph_scratch_reuses_reduction_buffers() {
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
        let mut scratch = DoCalculusImpactScratch::default();
        reference_impact_subgraph_with_scratch(&adj, &[1, 0, 0, 0], 4, &mut scratch);
        let reduced_capacity = scratch.reduced_adjacency.capacity();
        let kept_capacity = scratch.kept_indices.capacity();
        assert_eq!(scratch.kept_indices(), &[0, 1]);
        assert_eq!(scratch.reduced_adjacency(), &[0, 1, 0, 0]);

        reference_impact_subgraph_with_scratch(&adj, &[0, 0, 1, 0], 4, &mut scratch);
        assert_eq!(scratch.reduced_adjacency.capacity(), reduced_capacity);
        assert_eq!(scratch.kept_indices.capacity(), kept_capacity);
        assert_eq!(scratch.kept_indices(), &[2, 3]);
        assert_eq!(scratch.reduced_adjacency(), &[0, 1, 0, 0]);
    }

    #[test]
    fn impact_subgraph_empty_intervention_empty_subgraph() {
        let adj = vec![0, 1, 0, 0];
        let mask = vec![0, 0];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 2);
        assert!(reduced.is_empty());
        assert!(kept.is_empty());
    }

    #[test]
    fn impact_subgraph_empty_graph() {
        let (r, k) = impact_subgraph(&[], &[], 0);
        assert!(r.is_empty());
        assert!(k.is_empty());
    }

    /// Closure-bar test: the reduced adjacency must have **exactly**
    /// `kept.len()²` cells AND every cell must equal the original
    /// adjacency restricted to the corresponding kept-index pair. If
    /// the consumer ever drifts (off-by-one indexing into the kept
    /// vector, mis-sized output buffer, etc.) this test fires.
    #[test]
    fn impact_subgraph_size_invariant_holds_under_partial_impact() {
        // 0 -> 1 -> 2, plus disjoint 3 -> 4. Intervene 1.
        // Impact = {1, 2}; subgraph keeps those two with edge 1->2.
        let adj = vec![
            0, 1, 0, 0, 0, // 0 -> 1
            0, 0, 1, 0, 0, // 1 -> 2
            0, 0, 0, 0, 0, // 2
            0, 0, 0, 0, 1, // 3 -> 4
            0, 0, 0, 0, 0, // 4
        ];
        let mask = vec![0, 1, 0, 0, 0];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 5);
        // Exact size invariant.
        assert_eq!(reduced.len(), kept.len() * kept.len());
        assert_eq!(kept, vec![1, 2]);
        // Edge 1->2 preserved at (0,1) in the reduced 2×2.
        assert_eq!(reduced, vec![0, 1, 0, 0]);
    }

    /// Adversarial: intervention on a leaf must not pull in upstream
    /// nodes. `do(leaf)` only impacts leaf itself; if the consumer
    /// accidentally also kept ancestors, the kept vec would grow.
    #[test]
    fn impact_subgraph_adversarial_leaf_intervention_keeps_only_leaf() {
        // 0 -> 1 -> 2. Intervene 2 (leaf).
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mask = vec![0, 0, 1];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 3);
        assert_eq!(kept, vec![2]);
        // 1×1, value = adj[2,2] = 0.
        assert_eq!(reduced, vec![0]);
    }

    /// Adversarial: every edge between kept nodes must survive in
    /// the reduced adjacency, and no edge to a dropped node may
    /// appear. A common bug is to copy the edge weight from the
    /// wrong (i, j) cell of the original  -  a permutation error.
    #[test]
    fn impact_subgraph_adversarial_dense_must_drop_unkept_edges() {
        // K3 over {0,1,2} plus isolated 3.
        let adj = vec![
            0, 1, 1, 0, // 0 -> 1, 0 -> 2
            1, 0, 1, 0, // 1 -> 0, 1 -> 2
            1, 1, 0, 0, // 2 -> 0, 2 -> 1
            0, 0, 0, 0, // 3 isolated
        ];
        // Intervene 0: rule-1 impact closure walks 0 -> 1 -> 2.
        let mask = vec![1, 0, 0, 0];
        let (reduced, kept) = impact_subgraph(&adj, &mask, 4);
        assert_eq!(kept, vec![0, 1, 2]);
        // Reduced is the original 3×3 corner. Every original edge
        // among {0,1,2} preserved; no row/col for 3.
        assert_eq!(
            reduced,
            vec![
                0, 1, 1, // 0 -> 1, 0 -> 2
                1, 0, 1, // 1 -> 0, 1 -> 2
                1, 1, 0, // 2 -> 0, 2 -> 1
            ]
        );
    }

    // ---- predict_impact_observation_form (Rule 2 consumer) ----

    /// On a DAG, observation-form impact equals intervention-form
    /// impact at the observed node itself (no feedback edges to
    /// reverse).
    #[test]
    fn observation_form_dag_observed_self_only() {
        // 0 -> 1 -> 2 (no incoming edges into observed node 0).
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mask = vec![1, 0, 0];
        let observed = predict_impact_observation_form(&adj, &mask, 3);
        let intervened = predict_impact(&adj, &mask, 3);
        // On this DAG, observing 0 = intervening on 0.
        assert_eq!(observed, intervened);
    }

    #[test]
    fn observation_form_scratch_reuses_buffers() {
        let adj = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mut scratch = DoCalculusImpactScratch::default();
        predict_impact_observation_form_with_scratch(&adj, &[1, 0, 0], 3, &mut scratch);
        let reversed_capacity = scratch.surgically_modified_adj.capacity();
        let closure_capacity = scratch.closure.capacity();
        assert_eq!(scratch.impact_mask(), &[1, 1, 1]);

        predict_impact_observation_form_with_scratch(&adj, &[0, 1, 0], 3, &mut scratch);
        assert_eq!(
            scratch.surgically_modified_adj.capacity(),
            reversed_capacity
        );
        assert_eq!(scratch.closure.capacity(), closure_capacity);
        assert_eq!(scratch.impact_mask(), &[1, 1, 1]);
    }

    /// Closure-bar: observation-form must include the observed node
    /// itself as impact.
    #[test]
    fn observation_form_marks_observed_node() {
        let adj = vec![0, 1, 0, 0];
        let mask = vec![0, 1];
        let impact = predict_impact_observation_form(&adj, &mask, 2);
        assert_eq!(impact[1], 1, "observed node must be in impact set");
    }

    /// Adversarial: feedback loop into observed node. Rule-2 reverses
    /// the loop edge, so observation-form sees the loop's source as
    /// reachable along the reversed edge.
    #[test]
    fn observation_form_walks_reversed_feedback_edge() {
        // 0 -> 1, 1 -> 0 (mutual feedback), 1 -> 2.
        // Observe 0. Rule-2 reverses 1 -> 0 to 0 -> 1 (already exists,
        // OR-merged); it does NOT reverse 0 -> 1 (target is 0 only).
        // Reachable from 0 in modified graph: 0, 1, 2.
        let adj = vec![0, 1, 0, 1, 0, 1, 0, 0, 0];
        let mask = vec![1, 0, 0];
        let impact = predict_impact_observation_form(&adj, &mask, 3);
        assert_eq!(impact, vec![1, 1, 1]);
    }

    /// Adversarial: empty observation yields empty impact.
    #[test]
    fn observation_form_empty_mask_yields_empty() {
        let adj = vec![0, 1, 0, 0];
        let mask = vec![0, 0];
        let impact = predict_impact_observation_form(&adj, &mask, 2);
        assert_eq!(impact, vec![0, 0]);
    }

    /// Adversarial: empty graph returns empty result.
    #[test]
    fn observation_form_empty_graph() {
        assert!(predict_impact_observation_form(&[], &[], 0).is_empty());
    }

    #[test]
    fn release_via_paths_do_not_import_cpu_reference_helpers() {
        let source = include_str!("do_calculus_change_impact.rs");
        let regions = [
            (
                "pub fn predict_impact_via",
                "/// Primitive-native dispatcher path for Pearl Rule 1 graph surgery",
            ),
            (
                "pub fn intervention_delete_incoming_via",
                "/// Primitive-native dispatcher path for Pearl Rule 2 graph surgery",
            ),
            (
                "pub fn rule2_reverse_incoming_via",
                "/// Compute the impacted subgraph:",
            ),
            (
                "pub fn predict_impact_observation_form_via",
                "\n#[cfg(test)]\nmod tests",
            ),
        ];
        for (start_marker, end_marker) in regions {
            let start = source
                .find(start_marker)
                .expect("Fix: via start marker must exist");
            let end = source[start..]
                .find(end_marker)
                .map(|offset| start + offset)
                .expect("Fix: via end marker must exist");
            let release_path = &source[start..end];
            assert!(!release_path.contains("_cpu"), "{start_marker}");
            assert!(!release_path.contains("reference_"), "{start_marker}");
            assert!(
                !release_path.contains("u32_slice_to_le_bytes("),
                "{start_marker}"
            );
        }
    }

    struct InterventionDispatcher;

    impl OptimizerDispatcher for InterventionDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 2);
            let adj = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let mask = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let n = mask.len();
            let mut out = adj;
            for j in 0..n {
                if mask[j] != 0 {
                    for i in 0..n {
                        out[i * n + j] = 0;
                    }
                }
            }
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn intervention_delete_incoming_via_dispatches_rule1() {
        let adj = vec![1, 2, 3, 4];
        let out =
            intervention_delete_incoming_via(&InterventionDispatcher, &adj, &[1, 0], 2).unwrap();
        assert_eq!(out, vec![0, 2, 0, 4]);
    }

    #[test]
    fn intervention_delete_incoming_via_rejects_bad_shape() {
        let err = intervention_delete_incoming_via(&InterventionDispatcher, &[1, 2, 3], &[1, 0], 2)
            .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    struct Rule2Dispatcher;

    impl OptimizerDispatcher for Rule2Dispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 2);
            let adj = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let mask = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let n = mask.len();
            assert_eq!(adj.len(), n * n);
            let mut out = vec![0u32; n * n];
            for row in 0..n {
                for col in 0..n {
                    let idx = row * n + col;
                    if row == col {
                        out[idx] = adj[idx];
                        continue;
                    }
                    if mask[col] == 0 {
                        out[idx] |= adj[idx];
                    }
                    if mask[row] != 0 {
                        out[idx] |= adj[col * n + row];
                    }
                }
            }
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn rule2_reverse_incoming_via_dispatches_rule2() {
        let adj = vec![
            0, 1, 0, //
            0, 0, 1, //
            0, 0, 0,
        ];
        let out = rule2_reverse_incoming_via(&Rule2Dispatcher, &adj, &[0, 1, 0], 3).unwrap();
        assert_eq!(
            out,
            vec![
                0, 0, 0, //
                1, 0, 1, //
                0, 0, 0,
            ]
        );
    }

    #[test]
    fn rule2_reverse_incoming_via_preserves_bidirectional_fully_treated_edges() {
        let adj = vec![0, 1, 1, 0];
        let out = rule2_reverse_incoming_via(&Rule2Dispatcher, &adj, &[1, 1], 2).unwrap();
        assert_eq!(out, adj);
    }

    #[test]
    fn rule2_reverse_incoming_via_rejects_bad_shape() {
        let err = rule2_reverse_incoming_via(&Rule2Dispatcher, &[1, 2, 3], &[1, 0], 2).unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }
}
