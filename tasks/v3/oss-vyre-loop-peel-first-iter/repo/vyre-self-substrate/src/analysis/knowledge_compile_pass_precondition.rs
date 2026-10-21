//! Pass-precondition compilation via #38 knowledge compilation
//! (#38 self-consumer).
//!
//! Closes the recursion thesis for #38  -  d-DNNF compilation +
//! evaluation ships to user dialects (neuro-symbolic systems,
//! probabilistic policy engines) AND compiles vyre's optimizer
//! pass-precondition predicates into tractable evaluation circuits.
//!
//! # The self-use
//!
//! Each vyre optimizer pass declares a precondition: a boolean
//! formula over Program features (e.g. "no Region contains atomic
//! ops" AND "all Loop nodes have unit stride"). Today these
//! preconditions are evaluated by hand-rolled match-on-Node
//! traversals  -  re-implemented per pass with no shared structure.
//!
//! Knowledge compilation reframes the precondition as a
//! propositional formula `φ`. Compile `φ` to d-DNNF (Darwiche 2002):
//! the resulting decision-DNNF circuit can be evaluated in
//! linear time vs the formula's CNF, AND supports model counting
//! and conditioning queries that hand-rolled validators can't.
//!
//! Once preconditions are d-DNNF circuits:
//!
//! - **Conditioning**: "given the current Program features, is
//!   pass X applicable?" reduces to ddnnf_evaluate under the
//!   feature assignment.
//! - **Counterexample search**: "find a feature assignment that
//!   makes the precondition false" is one #SAT query on the d-DNNF.
//! - **Conflict detection**: "passes A and B have contradictory
//!   preconditions" is `φ_A ∧ φ_B` compiled jointly; UNSAT iff they
//!   conflict.
//!
//! # Algorithm
//!
//! ```text
//! 1. each pass declares its precondition as a propositional formula
//!    over Program features
//! 2. host-side compiler: compile the formula to d-DNNF
//!    (one-time per pass, cached)
//! 3. per Program: extract feature assignments, run
//!    ddnnf_evaluate_cpu  -  returns 1 iff the precondition holds
//! ```
//!
//! This module consumes compiled d-DNNF circuits and evaluates them
//! against Program feature assignments. Circuit construction is owned
//! by the pass framework that supplies the `nodes`, `children`, and
//! topological order buffers.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::knowledge_compile::ddnnf_evaluate;
#[cfg(test)]
use vyre_primitives::graph::knowledge_compile::ddnnf_evaluate_cpu;

/// Caller-owned scratch for pass-precondition d-DNNF dispatch.
#[derive(Debug, Default)]
pub struct KnowledgeCompilePassScratch {
    node_kinds: Vec<u32>,
    child_offsets: Vec<u32>,
    child_counts: Vec<u32>,
    inputs: Vec<Vec<u8>>,
}

/// Evaluate a compiled pass-precondition circuit against a Program's
/// feature assignment. Returns 1 iff the precondition holds, 0
/// otherwise. The circuit is the bottom-up-topologically-ordered
/// d-DNNF representation; `var_assignments[i]` is feature i's value
/// (`0` / `1` / `u32::MAX` = unknown).
#[must_use]
#[cfg(test)]
pub fn reference_pass_applies(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> u32 {
    use crate::observability::{bump, knowledge_compile_pass_precondition_calls};
    bump(&knowledge_compile_pass_precondition_calls);
    let evals = ddnnf_evaluate_cpu(nodes, node_var, children, var_assignments, topo_order);
    // The root of the topological order is the formula's overall
    // truth value. By d-DNNF construction the root is the LAST node
    // in topo_order.
    if topo_order.is_empty() {
        return 0;
    }
    let Some(root) = topo_order.last().copied() else {
        return 0;
    };
    let root = root as usize;
    evals[root]
}

/// Evaluate a compiled pass-precondition circuit through the dispatcher.
///
/// The d-DNNF primitive evaluates one topological wave at a time. `waves`
/// must contain node ids grouped so every child appears in an earlier wave
/// than its parent. This function dispatches one wave after another while
/// keeping the full node-output buffer as an explicit RW input/output.
///
/// # Errors
///
/// Returns [`DispatchError`] when circuit shape validation fails, wave ordering
/// is invalid, or a backend dispatch/output contract is malformed.
pub fn pass_applies_via(
    dispatcher: &impl OptimizerDispatcher,
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    waves: &[Vec<u32>],
) -> Result<u32, DispatchError> {
    let mut scratch = KnowledgeCompilePassScratch::default();
    let mut evals = Vec::new();
    pass_applies_via_with_scratch_into(
        dispatcher,
        nodes,
        node_var,
        children,
        var_assignments,
        waves,
        &mut scratch,
        &mut evals,
    )?;
    let Some(root) = waves.last().and_then(|wave| wave.last()).copied() else {
        return Ok(0);
    };
    Ok(evals[root as usize])
}

/// Evaluate a compiled pass-precondition circuit through the dispatcher and
/// keep all per-node evaluation results in caller-owned storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn pass_applies_via_into(
    dispatcher: &impl OptimizerDispatcher,
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    waves: &[Vec<u32>],
    evals_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = KnowledgeCompilePassScratch::default();
    pass_applies_via_with_scratch_into(
        dispatcher,
        nodes,
        node_var,
        children,
        var_assignments,
        waves,
        &mut scratch,
        evals_out,
    )
}

/// Evaluate a compiled pass-precondition circuit with reusable dispatch
/// storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn pass_applies_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    waves: &[Vec<u32>],
    scratch: &mut KnowledgeCompilePassScratch,
    evals_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, knowledge_compile_pass_precondition_calls};
    bump(&knowledge_compile_pass_precondition_calls);

    if nodes.is_empty() {
        evals_out.clear();
        return Ok(());
    }
    if var_assignments.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: pass_applies_via requires at least one variable assignment for non-empty circuits."
                .to_string(),
        ));
    }
    if node_var.len() != nodes.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: pass_applies_via requires node_var.len() == nodes.len(), got {} vs {}.",
            node_var.len(),
            nodes.len()
        )));
    }
    let n_nodes = u32::try_from(nodes.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: pass_applies_via node count exceeds u32 lane space: {}.",
            nodes.len()
        ))
    })?;
    let n_children = u32::try_from(children.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: pass_applies_via child count exceeds u32 lane space: {}.",
            children.len()
        ))
    })?;
    let n_vars = u32::try_from(var_assignments.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: pass_applies_via variable count exceeds u32 lane space: {}.",
            var_assignments.len()
        ))
    })?;

    scratch.node_kinds.clear();
    scratch.child_offsets.clear();
    scratch.child_counts.clear();
    reserve_vec_capacity(
        &mut scratch.node_kinds,
        nodes.len(),
        "pass_applies_via node kinds",
    )?;
    reserve_vec_capacity(
        &mut scratch.child_offsets,
        nodes.len(),
        "pass_applies_via child offsets",
    )?;
    reserve_vec_capacity(
        &mut scratch.child_counts,
        nodes.len(),
        "pass_applies_via child counts",
    )?;
    for (idx, &(kind, offset, count)) in nodes.iter().enumerate() {
        let end = offset.checked_add(count).ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: pass_applies_via node {idx} child range overflows u32."
            ))
        })?;
        if end as usize > children.len() {
            return Err(DispatchError::BadInputs(format!(
                "Fix: pass_applies_via node {idx} child range [{offset}, {end}) exceeds children.len()={}.",
                children.len()
            )));
        }
        scratch.node_kinds.push(kind);
        scratch.child_offsets.push(offset);
        scratch.child_counts.push(count);
    }
    for (idx, &var) in node_var.iter().enumerate() {
        if var >= n_vars {
            return Err(DispatchError::BadInputs(format!(
                "Fix: pass_applies_via node_var[{idx}]={var} outside n_vars={n_vars}."
            )));
        }
    }
    for (idx, &child) in children.iter().enumerate() {
        if child >= n_nodes {
            return Err(DispatchError::BadInputs(format!(
                "Fix: pass_applies_via children[{idx}]={child} outside n_nodes={n_nodes}."
            )));
        }
    }
    validate_waves(n_nodes, nodes, children, waves)?;

    evals_out.clear();
    evals_out.resize(nodes.len(), 0);
    let program = ddnnf_evaluate(
        "node_kinds",
        "node_var",
        "child_offsets",
        "child_counts",
        "children",
        "var_assignments",
        "out",
        n_nodes,
        n_children,
        n_vars,
    );
    ensure_input_slots(&mut scratch.inputs, 7);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &scratch.node_kinds);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], node_var);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.child_offsets);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], &scratch.child_counts);
    write_u32_slice_le_bytes(&mut scratch.inputs[4], children);
    write_u32_slice_le_bytes(&mut scratch.inputs[5], var_assignments);

    for wave in waves {
        if wave.is_empty() {
            continue;
        }
        write_u32_slice_le_bytes(&mut scratch.inputs[6], evals_out);
        let outputs = dispatcher.dispatch(
            &program,
            &scratch.inputs[..7],
            Some([ceil_div_u32(n_nodes, 256), 1, 1]),
        )?;
        if outputs.is_empty() {
            return Err(DispatchError::BackendError(format!(
                "Fix: pass_applies_via expected exactly one eval output buffer, got {}.",
                outputs.len()
            )));
        }
        decode_u32_output_exact(&outputs[0], nodes.len(), "pass_applies_via", evals_out)?;
    }
    Ok(())
}

/// Convenience: does pass X conflict with the current Program?
/// Returns true iff the precondition is unsatisfied at the given
/// feature assignment.
#[must_use]
#[cfg(test)]
pub fn pass_conflicts(
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    topo_order: &[u32],
) -> bool {
    reference_pass_applies(nodes, node_var, children, var_assignments, topo_order) == 0
}

/// Dispatcher-backed pass-conflict predicate.
///
/// # Errors
///
/// Returns [`DispatchError`] when circuit validation or backend execution
/// fails.
pub fn pass_conflicts_via(
    dispatcher: &impl OptimizerDispatcher,
    nodes: &[(u32, u32, u32)],
    node_var: &[u32],
    children: &[u32],
    var_assignments: &[u32],
    waves: &[Vec<u32>],
) -> Result<bool, DispatchError> {
    Ok(pass_applies_via(
        dispatcher,
        nodes,
        node_var,
        children,
        var_assignments,
        waves,
    )? == 0)
}

fn validate_waves(
    n_nodes: u32,
    nodes: &[(u32, u32, u32)],
    children: &[u32],
    waves: &[Vec<u32>],
) -> Result<(), DispatchError> {
    let mut seen = vec![false; n_nodes as usize];
    for (wave_idx, wave) in waves.iter().enumerate() {
        for &node in wave {
            if node >= n_nodes {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: pass_applies_via wave {wave_idx} contains node {node} outside n_nodes={n_nodes}."
                )));
            }
            if seen[node as usize] {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: pass_applies_via node {node} appears in multiple waves."
                )));
            }
            let (_, offset, count) = nodes[node as usize];
            for child_idx in offset..offset + count {
                let child = children[child_idx as usize];
                if !seen[child as usize] {
                    return Err(DispatchError::BadInputs(format!(
                        "Fix: pass_applies_via node {node} appears before child {child}; waves must be child-before-parent."
                    )));
                }
            }
        }
        for &node in wave {
            seen[node as usize] = true;
        }
    }
    if let Some((missing, _)) = seen.iter().enumerate().find(|(_, present)| !**present) {
        return Err(DispatchError::BadInputs(format!(
            "Fix: pass_applies_via waves omit node {missing}."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;
    use vyre_primitives::graph::knowledge_compile::{AND_NODE, LITERAL_TRUE};

    #[test]
    fn unconditional_pass_always_applies() {
        // Single LITERAL_TRUE node with var 0 unconditionally true.
        let nodes = vec![(LITERAL_TRUE, 0u32, 0u32)];
        let node_var = vec![0u32];
        let children: Vec<u32> = vec![];
        // var 0 = 1 (true).
        let assignments = vec![1u32];
        let topo = vec![0u32];
        assert_eq!(
            reference_pass_applies(&nodes, &node_var, &children, &assignments, &topo),
            1
        );
        assert!(!pass_conflicts(
            &nodes,
            &node_var,
            &children,
            &assignments,
            &topo
        ));
    }

    #[test]
    fn unconditional_pass_blocked_by_false_var() {
        // Same single literal-true node, but var 0 assigned 0 → fails.
        let nodes = vec![(LITERAL_TRUE, 0u32, 0u32)];
        let node_var = vec![0u32];
        let children: Vec<u32> = vec![];
        let assignments = vec![0u32];
        let topo = vec![0u32];
        assert_eq!(
            reference_pass_applies(&nodes, &node_var, &children, &assignments, &topo),
            0
        );
        assert!(pass_conflicts(
            &nodes,
            &node_var,
            &children,
            &assignments,
            &topo
        ));
    }

    #[test]
    fn conjunctive_pass_requires_both() {
        // (LITERAL_TRUE var 0) AND (LITERAL_TRUE var 1) → AND node at index 2.
        let nodes = vec![
            (LITERAL_TRUE, 0u32, 0u32), // node 0: literal var 0
            (LITERAL_TRUE, 0u32, 0u32), // node 1: literal var 1
            (AND_NODE, 0u32, 2u32),     // node 2: AND of children at children[0..2]
        ];
        let node_var = vec![0u32, 1u32, 0u32];
        let children = vec![0u32, 1u32];

        let topo = vec![0u32, 1u32, 2u32];

        // both true.
        let both_true = vec![1u32, 1u32];
        assert_eq!(
            reference_pass_applies(&nodes, &node_var, &children, &both_true, &topo),
            1
        );

        // one false.
        let one_false = vec![1u32, 0u32];
        assert_eq!(
            reference_pass_applies(&nodes, &node_var, &children, &one_false, &topo),
            0
        );
    }

    #[test]
    fn empty_topo_returns_zero() {
        let nodes: Vec<(u32, u32, u32)> = vec![];
        let node_var: Vec<u32> = vec![];
        let children: Vec<u32> = vec![];
        let assignments: Vec<u32> = vec![];
        let topo: Vec<u32> = vec![];
        assert_eq!(
            reference_pass_applies(&nodes, &node_var, &children, &assignments, &topo),
            0
        );
    }

    struct DdnnfDispatcher;

    impl OptimizerDispatcher for DdnnfDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 7);
            let node_kinds = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let node_var = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let child_offsets = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
            let child_counts = crate::hardware::dispatch_buffers::read_u32s(&inputs[3]);
            let children = crate::hardware::dispatch_buffers::read_u32s(&inputs[4]);
            let assignments = crate::hardware::dispatch_buffers::read_u32s(&inputs[5]);
            let mut out = crate::hardware::dispatch_buffers::read_u32s(&inputs[6]);
            for node in 0..node_kinds.len() {
                match node_kinds[node] {
                    LITERAL_TRUE => {
                        let assigned = assignments[node_var[node] as usize];
                        out[node] = u32::from(assigned == 1 || assigned == u32::MAX);
                    }
                    AND_NODE => {
                        let start = child_offsets[node] as usize;
                        let end = start + child_counts[node] as usize;
                        out[node] = children[start..end]
                            .iter()
                            .map(|&child| out[child as usize])
                            .product();
                    }
                    _ => {}
                }
            }
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn pass_applies_via_dispatches_in_waves() {
        let nodes = vec![
            (LITERAL_TRUE, 0u32, 0u32),
            (LITERAL_TRUE, 0u32, 0u32),
            (AND_NODE, 0u32, 2u32),
        ];
        let node_var = vec![0u32, 1u32, 0u32];
        let children = vec![0u32, 1u32];
        let assignments = vec![1u32, 1u32];
        let waves = vec![vec![0u32, 1u32], vec![2u32]];
        let applies = pass_applies_via(
            &DdnnfDispatcher,
            &nodes,
            &node_var,
            &children,
            &assignments,
            &waves,
        )
        .unwrap();
        assert_eq!(applies, 1);
    }

    #[test]
    fn pass_applies_via_reuses_dispatch_buffers_and_evals() {
        let nodes = vec![
            (LITERAL_TRUE, 0u32, 0u32),
            (LITERAL_TRUE, 0u32, 0u32),
            (AND_NODE, 0u32, 2u32),
        ];
        let node_var = vec![0u32, 1u32, 0u32];
        let children = vec![0u32, 1u32];
        let assignments = vec![1u32, 1u32];
        let waves = vec![vec![0u32, 1u32], vec![2u32]];
        let mut scratch = KnowledgeCompilePassScratch {
            inputs: vec![
                Vec::with_capacity(32),
                Vec::with_capacity(32),
                Vec::with_capacity(32),
                Vec::with_capacity(32),
                Vec::with_capacity(32),
                Vec::with_capacity(32),
                Vec::with_capacity(32),
            ],
            ..KnowledgeCompilePassScratch::default()
        };
        let mut evals = Vec::with_capacity(8);
        pass_applies_via_with_scratch_into(
            &DdnnfDispatcher,
            &nodes,
            &node_var,
            &children,
            &assignments,
            &waves,
            &mut scratch,
            &mut evals,
        )
        .unwrap();
        let input_caps = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let evals_cap = evals.capacity();
        pass_applies_via_with_scratch_into(
            &DdnnfDispatcher,
            &nodes,
            &node_var,
            &children,
            &[1u32, 0u32],
            &waves,
            &mut scratch,
            &mut evals,
        )
        .unwrap();
        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_caps
        );
        assert_eq!(evals.capacity(), evals_cap);
        assert_eq!(evals[2], 0);
    }

    #[test]
    fn pass_conflicts_via_matches_unsatisfied_precondition() {
        let nodes = vec![
            (LITERAL_TRUE, 0u32, 0u32),
            (LITERAL_TRUE, 0u32, 0u32),
            (AND_NODE, 0u32, 2u32),
        ];
        let node_var = vec![0u32, 1u32, 0u32];
        let children = vec![0u32, 1u32];
        let waves = vec![vec![0u32, 1u32], vec![2u32]];
        let conflicts = pass_conflicts_via(
            &DdnnfDispatcher,
            &nodes,
            &node_var,
            &children,
            &[1, 0],
            &waves,
        )
        .unwrap();
        assert!(conflicts);
    }

    #[test]
    fn release_via_path_does_not_call_cpu_or_reference_helpers() {
        let source = include_str!("knowledge_compile_pass_precondition.rs");
        let start = source
            .find("pub fn pass_applies_via")
            .expect("Fix: via path marker must exist");
        let end = source
            .find("\n/// Convenience: does pass X conflict")
            .expect("Fix: test-only CPU marker must exist");
        let release_path = &source[start..end];
        assert!(!release_path.contains("_cpu"));
        assert!(!release_path.contains("reference_"));
        assert!(!release_path.contains("u32_slice_to_le_bytes("));
    }

    #[test]
    fn pass_applies_via_rejects_parent_before_child() {
        let nodes = vec![
            (LITERAL_TRUE, 0u32, 0u32),
            (LITERAL_TRUE, 0u32, 0u32),
            (AND_NODE, 0u32, 2u32),
        ];
        let err = pass_applies_via(
            &DdnnfDispatcher,
            &nodes,
            &[0, 1, 0],
            &[0, 1],
            &[1, 1],
            &[vec![2], vec![0, 1]],
        )
        .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }
}
