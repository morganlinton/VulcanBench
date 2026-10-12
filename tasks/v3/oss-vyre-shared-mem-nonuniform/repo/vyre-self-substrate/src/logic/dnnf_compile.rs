//! d-DNNF compilation substrate consumer (P-PRIM-6).
//!
//! Wires `vyre_primitives::dnnf::compile_dnnf` and `model_count`
//! into the dispatch path so the pass scheduler can compile each
//! pass-precondition CNF to a d-DNNF DAG once at startup, then
//! evaluate it in linear time per Program at dispatch time.
//!
//! The substrate exposes the compile + count steps as separate
//! entry points so callers that only need the SAT-style decision
//! can skip the (potentially saturating) model count.

use vyre_primitives::dnnf::{
    compile_dnnf as primitive_compile, model_count as primitive_count, DnnfDag,
};

/// Compile a CNF formula to a d-DNNF DAG. Bumps the
/// dataflow-fixpoint substrate counter so observability dashboards
/// register every compilation event.
#[must_use]
pub fn compile_precondition(
    clauses: &[Vec<(u32, bool)>],
    num_vars: u32,
    max_depth: u32,
) -> DnnfDag {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_compile(clauses, num_vars, max_depth)
}

/// Count satisfying assignments of a previously-compiled d-DNNF
/// DAG. Linear in the gate count; saturating at u64::MAX.
#[must_use]
pub fn count_models(dag: &DnnfDag) -> u64 {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_count(dag)
}

/// Convenience: returns true iff the formula is satisfiable
/// (model_count > 0). The pass scheduler uses this as a fast
/// "would this pass ever be applicable?" check during pre-flight.
#[must_use]
pub fn is_satisfiable(dag: &DnnfDag) -> bool {
    count_models(dag) > 0
}

/// Convenience: returns true iff the formula is a tautology over
/// `num_vars` (model count == 2^num_vars).
#[must_use]
pub fn is_tautology(dag: &DnnfDag, num_vars: u32) -> bool {
    if num_vars >= 64 {
        return false; // 2^64 saturates; conservative reject.
    }
    count_models(dag) == 1u64 << num_vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_formula_is_tautology() {
        let dag = compile_precondition(&[], 3, 4);
        assert!(is_tautology(&dag, 3));
        assert!(is_satisfiable(&dag));
    }

    #[test]
    fn contradiction_is_unsatisfiable() {
        let clauses = vec![vec![(0u32, true)], vec![(0, false)]];
        let dag = compile_precondition(&clauses, 1, 4);
        assert!(!is_satisfiable(&dag));
        assert!(!is_tautology(&dag, 1));
    }

    #[test]
    fn single_literal_satisfiable_not_tautology() {
        let dag = compile_precondition(&[vec![(0u32, true)]], 2, 4);
        assert!(is_satisfiable(&dag));
        assert!(!is_tautology(&dag, 2));
        // (x0) over 2 vars: x1 free → 2 models.
        assert_eq!(count_models(&dag), 2);
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let clauses = vec![vec![(0u32, true), (1, false)], vec![(1, true), (2, true)]];
        let via_substrate = compile_precondition(&clauses, 3, 8);
        let via_primitive = primitive_compile(&clauses, 3, 8);
        assert_eq!(via_substrate, via_primitive);
        assert_eq!(
            count_models(&via_substrate),
            primitive_count(&via_primitive)
        );
    }

    /// Adversarial: max_depth budget terminates even on a wide
    /// formula that would otherwise diverge.
    #[test]
    fn depth_budget_terminates() {
        let clauses = vec![vec![(0u32, true), (1, true)], vec![(2, true), (3, true)]];
        let dag = compile_precondition(&clauses, 4, 1);
        // No assertion on the count  -  only that compile + count
        // both terminate (via the saturating arithmetic + depth
        // budget contract).
        assert!(count_models(&dag) > 0);
    }

    /// Adversarial: tautology check is conservative for num_vars >= 64.
    #[test]
    fn tautology_conservative_at_64_vars() {
        let dag = compile_precondition(&[], 64, 4);
        // 2^64 saturates u64::MAX which is != 2^64; we conservatively
        // reject here.
        assert!(!is_tautology(&dag, 64));
    }
}
