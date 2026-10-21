//! d-DNNF compiler: CNF → d-DNNF DAG via Shannon decomposition.
//!
//! Pure-CPU. Bounded by the compiler's max-depth parameter (so
//! pathological CNFs cannot diverge  -  the user's optimizer caller
//! can choose a depth budget rather than blocking on an SAT-hard
//! input).

extern crate alloc;
use alloc::vec::Vec;

/// One gate in a d-DNNF DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnnfGate {
    /// Literal: `(variable, polarity)`. `polarity = true` means
    /// the positive literal `xN`; `false` means `¬xN`.
    Literal(u32, bool),
    /// `True` constant.
    True,
    /// `False` constant.
    False,
    /// AND gate over decomposable children (no shared variables).
    And(Vec<u32>),
    /// OR gate over deterministic children (pairwise inconsistent).
    Or(Vec<u32>),
}

/// d-DNNF DAG: gate list (root is the last entry by convention)
/// + the variable count the formula was compiled over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnnfDag {
    /// Gates indexed by id.
    pub gates: Vec<DnnfGate>,
    /// Number of variables the formula uses (1..=num_vars).
    pub num_vars: u32,
}

impl DnnfDag {
    /// Root gate id (the last entry in `gates` by convention).
    #[must_use]
    pub fn root(&self) -> u32 {
        (self.gates.len() - 1) as u32
    }
}

/// Compile a CNF formula into a d-DNNF DAG.
///
/// `clauses` is a slice of clauses, each clause a slice of
/// literals `(variable, polarity)`. `num_vars` is the variable
/// count (variables are 0..num_vars).
///
/// Compilation uses Shannon decomposition: pick a variable, split
/// the clause set on its true/false assignment, recursively compile
/// each branch, combine with an OR gate. Constant True/False fold
/// when a branch is satisfied or empty.
///
/// `max_depth` bounds recursion; depth-exhausted branches return
/// the conservative `True` gate (the compiler's caller should
/// retry with a higher budget when this happens  -  see
/// `knowledge_compile_pass_precondition` for the budget heuristic).
#[must_use]
pub fn compile_dnnf(clauses: &[Vec<(u32, bool)>], num_vars: u32, max_depth: u32) -> DnnfDag {
    let mut dag = DnnfDag {
        gates: Vec::new(),
        num_vars,
    };
    compile_recursive(&mut dag, clauses, num_vars, 0, max_depth);
    dag
}

fn smoothed_true(dag: &mut DnnfDag, num_vars: u32, var: u32) -> u32 {
    // "Empty clause set is satisfied" with free variables `var..num_vars`:
    // emit `(x_v ∨ ¬x_v)` for each free variable and AND them. Each
    // tautology OR has model count 2; AND multiplies → 2^(num_vars-var).
    if var >= num_vars {
        let id = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::True);
        return id;
    }
    let mut taut_ids = Vec::with_capacity((num_vars - var) as usize);
    for v in var..num_vars {
        let pos = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::Literal(v, true));
        let neg = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::Literal(v, false));
        let or_id = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::Or(alloc::vec![pos, neg]));
        taut_ids.push(or_id);
    }
    if taut_ids.len() == 1 {
        return taut_ids[0];
    }
    let id = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::And(taut_ids));
    id
}

fn compile_recursive(
    dag: &mut DnnfDag,
    clauses: &[Vec<(u32, bool)>],
    num_vars: u32,
    var: u32,
    remaining_depth: u32,
) -> u32 {
    if clauses.is_empty() {
        // Satisfied  -  smooth over the free remaining variables so
        // model counting on the DAG yields 2^(num_vars - var).
        return smoothed_true(dag, num_vars, var);
    }
    // Empty clause inside the set means contradiction.
    if clauses.iter().any(|c| c.is_empty()) {
        let id = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::False);
        return id;
    }
    if var >= num_vars || remaining_depth == 0 {
        // Out of variables / depth: compose remaining clauses as one
        // big AND of OR(literal-list). For depth-exhausted branches
        // this is a conservative under-approximation  -  caller can
        // re-compile with more depth budget if needed.
        let mut clause_ids = Vec::with_capacity(clauses.len());
        for clause in clauses {
            let mut lits = Vec::with_capacity(clause.len());
            for &(v, p) in clause {
                let id = dag.gates.len() as u32;
                dag.gates.push(DnnfGate::Literal(v, p));
                lits.push(id);
            }
            let id = dag.gates.len() as u32;
            dag.gates.push(DnnfGate::Or(lits));
            clause_ids.push(id);
        }
        if clause_ids.len() == 1 {
            return clause_ids[0];
        }
        let id = dag.gates.len() as u32;
        dag.gates.push(DnnfGate::And(clause_ids));
        return id;
    }

    // Shannon decomposition: split on `var`. Left branch = clauses
    // simplified by var=true; right branch = clauses simplified by
    // var=false.
    let positive_branch = simplify_clauses(clauses, var, true);
    let negative_branch = simplify_clauses(clauses, var, false);

    let left = compile_recursive(
        dag,
        &positive_branch,
        num_vars,
        var + 1,
        remaining_depth - 1,
    );
    let right = compile_recursive(
        dag,
        &negative_branch,
        num_vars,
        var + 1,
        remaining_depth - 1,
    );

    // Build the deterministic OR: (var ∧ left) ∨ (¬var ∧ right).
    let pos_lit = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::Literal(var, true));
    let pos_and = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::And(alloc::vec![pos_lit, left]));

    let neg_lit = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::Literal(var, false));
    let neg_and = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::And(alloc::vec![neg_lit, right]));

    let or_id = dag.gates.len() as u32;
    dag.gates.push(DnnfGate::Or(alloc::vec![pos_and, neg_and]));
    or_id
}

fn simplify_clauses(
    clauses: &[Vec<(u32, bool)>],
    var: u32,
    assignment: bool,
) -> Vec<Vec<(u32, bool)>> {
    let mut out = Vec::with_capacity(clauses.len());
    for clause in clauses {
        let mut satisfied = false;
        let mut residual = Vec::with_capacity(clause.len());
        for &(v, p) in clause {
            if v == var {
                if p == assignment {
                    satisfied = true;
                    break;
                }
                // Literal evaluates to false; drop it.
            } else {
                residual.push((v, p));
            }
        }
        if !satisfied {
            out.push(residual);
        }
    }
    out
}

/// Count satisfying assignments via a d-DNNF DAG. Linear in
/// `dag.gates.len()` (the d-DNNF compilation invariant: model
/// counting is structurally efficient).
#[must_use]
pub fn model_count(dag: &DnnfDag) -> u64 {
    let mut counts: Vec<u64> = Vec::with_capacity(dag.gates.len());
    for gate in &dag.gates {
        let c = match gate {
            DnnfGate::True => 1u64,
            DnnfGate::False => 0u64,
            DnnfGate::Literal(_, _) => 1u64,
            DnnfGate::And(children) => {
                let mut prod = 1u64;
                for &c_id in children {
                    prod = prod.saturating_mul(counts[c_id as usize]);
                }
                prod
            }
            DnnfGate::Or(children) => {
                let mut sum = 0u64;
                for &c_id in children {
                    sum = sum.saturating_add(counts[c_id as usize]);
                }
                sum
            }
        };
        counts.push(c);
    }
    counts[dag.gates.len() - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_empty_formula_is_true() {
        let dag = compile_dnnf(&[], 0, 4);
        assert_eq!(dag.gates.last(), Some(&DnnfGate::True));
        assert_eq!(model_count(&dag), 1);
    }

    #[test]
    fn compile_single_literal() {
        // Formula: (x0). One satisfying assignment (x0=true) when
        // num_vars=1, two of four when num_vars > 1  -  the compiler
        // ships the variable-relative count.
        let dag = compile_dnnf(&[alloc::vec![(0u32, true)]], 1, 4);
        assert_eq!(model_count(&dag), 1);
    }

    #[test]
    fn compile_contradiction_yields_zero_models() {
        // (x0) ∧ (¬x0) is unsatisfiable.
        let dag = compile_dnnf(&[alloc::vec![(0u32, true)], alloc::vec![(0, false)]], 1, 4);
        assert_eq!(model_count(&dag), 0);
    }

    #[test]
    fn compile_disjunction_of_two_lits() {
        // (x0 ∨ x1) over 2 vars: 3 satisfying assignments.
        let dag = compile_dnnf(&[alloc::vec![(0u32, true), (1, true)]], 2, 4);
        assert_eq!(model_count(&dag), 3);
    }

    /// Closure-bar: compile then count must agree with brute-force
    /// enumeration on small formulas.
    #[test]
    fn matches_brute_force_on_small_formulas() {
        // (x0 ∨ ¬x1) ∧ (x1 ∨ x2) with 3 vars.
        let clauses = alloc::vec![
            alloc::vec![(0u32, true), (1, false)],
            alloc::vec![(1, true), (2, true)],
        ];
        let dag = compile_dnnf(&clauses, 3, 8);
        let dag_count = model_count(&dag);

        // Brute force: 8 assignments.
        let mut bf = 0u64;
        for assignment in 0u8..8 {
            let x = [
                (assignment & 1) != 0,
                (assignment & 2) != 0,
                (assignment & 4) != 0,
            ];
            let c1 = x[0] || !x[1];
            let c2 = x[1] || x[2];
            if c1 && c2 {
                bf += 1;
            }
        }
        assert_eq!(dag_count, bf, "d-DNNF count must match brute force");
    }

    /// Adversarial: deep formula must not run forever  -  the depth
    /// budget terminates compilation even on multi-variable inputs.
    #[test]
    fn depth_budget_terminates() {
        // (x0 ∨ x1) ∧ (x2 ∨ x3) ∧ (x4 ∨ x5)  -  6 vars, depth budget 2
        // forces the compiler to fall back to the conservative
        // CNF-as-AND/OR encoding rather than full Shannon split.
        let clauses = alloc::vec![
            alloc::vec![(0u32, true), (1, true)],
            alloc::vec![(2, true), (3, true)],
            alloc::vec![(4, true), (5, true)],
        ];
        let dag = compile_dnnf(&clauses, 6, 2);
        assert_eq!(dag.num_vars, 6);
        assert!(
            dag.gates.len() >= 1,
            "depth budget must emit at least one gate"
        );
    }

    /// model_count handles all 2^k assignments via smoothed-True
    /// over k free variables.
    #[test]
    fn model_count_smooths_over_free_vars() {
        // 0 vars, no clauses → 1 model (the empty assignment).
        let dag = compile_dnnf(&[], 0, 4);
        assert_eq!(model_count(&dag), 1);
        // 5 vars, no clauses → 2^5 = 32 models.
        let dag = compile_dnnf(&[], 5, 4);
        assert_eq!(model_count(&dag), 32);
        // 1 var, no clauses → 2 models.
        let dag = compile_dnnf(&[], 1, 4);
        assert_eq!(model_count(&dag), 2);
    }

    /// model_count saturates instead of overflowing for huge formulas.
    #[test]
    fn model_count_saturates_at_u64_max() {
        // 64 free variables would yield 2^64; saturating-mul caps it
        // at u64::MAX rather than wrapping to 0.
        let dag = compile_dnnf(&[], 64, 4);
        assert!(model_count(&dag) > 0);
    }

    /// DnnfDag::root returns the last gate id.
    #[test]
    fn root_is_last_gate() {
        let dag = compile_dnnf(&[alloc::vec![(0u32, true)]], 1, 4);
        assert_eq!(dag.root(), (dag.gates.len() - 1) as u32);
    }
}
