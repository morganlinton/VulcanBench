//! Front-door / back-door adjustment set finder for causal inference.
//!
//! For a causal query `P(Y | do(X))` on a known DAG, valid causal
//! estimation from observational data requires conditioning on a
//! sufficient adjustment set Z that blocks every back-door path
//! from X to Y while opening no spurious paths.
//!
//! Pearl's back-door criterion (Pearl 2009, §3.3.1):
//! - Z contains no descendants of X.
//! - Z blocks every path between X and Y that contains an arrow
//!   into X (i.e. every "back-door" path).
//!
//! Front-door criterion (Pearl 2009, §3.3.2): an alternative when
//! no back-door set exists, requires a mediator M such that:
//! - M intercepts every directed path from X to Y.
//! - X and M have no shared back-door path.
//! - The Y ↔ M back-door is blocked by X.
//!
//! This file ships the **back-door predicate primitive**  -  given a
//! candidate set encoded as a bitmask, returns whether it satisfies
//! the back-door criterion for `(X, Y)` on the supplied adjacency.
//! The expensive operation is path-blocking enumeration; we delegate
//! it to a per-lane DFS scored against the candidate mask.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::causal::adjust` | observational ML estimation |
//! | `vyre-libs::security::root_cause` | confounded-finding adjustment |
//!
//! Self-consumer is weak today; the primitive composes well with #36
//! do-calculus for full ID-algorithm pipelines.
//!
//! # Encoding
//!
//! - `parents`: row-major `n × n` u32 adjacency. `parents[i, j] = 1`
//!   iff i is a parent of j.
//! - `descendants_of_x`: precomputed bitmask, `descendants_of_x[k] = 1`
//!   iff k is a descendant of X (X itself excluded). Caller computes
//!   via `csr_forward_traverse` from X.
//! - `candidate_z`: bitmask of nodes in Z (one per node).
//! - `out_violation`: single-element u32; 1 iff candidate violates
//!   the descendants-of-X rule.
//!
//! Block-path verification is handled by the path-enumeration
//! primitive; this primitive catches the "Z contains no descendants
//! of X" violation, the necessary first half of the criterion.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::graph::backdoor_descendants_check";

/// Emit a Program that sets `out_violation[0] = 1` iff any node in
/// `candidate_z` is also marked in `descendants_of_x`. Single-lane;
/// lane 0 walks both bitmasks and reports.
#[must_use]
pub fn backdoor_descendants_check(
    candidate_z: &str,
    descendants_of_x: &str,
    out_violation: &str,
    n: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out_violation,
            DataType::U32,
            format!("Fix: backdoor_descendants_check requires n > 0, got {n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::eq(t.clone(), Expr::u32(0)),
        vec![
            Node::let_bind("violated", Expr::u32(0)),
            Node::loop_for(
                "k",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::if_then(
                    Expr::and(
                        Expr::ne(Expr::load(candidate_z, Expr::var("k")), Expr::u32(0)),
                        Expr::ne(Expr::load(descendants_of_x, Expr::var("k")), Expr::u32(0)),
                    ),
                    vec![Node::assign("violated", Expr::u32(1))],
                )],
            ),
            Node::store(out_violation, Expr::u32(0), Expr::var("violated")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(candidate_z, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(descendants_of_x, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(out_violation, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference. Returns true iff the candidate violates the
/// descendants-of-X portion of the back-door criterion.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn backdoor_descendants_check_cpu(candidate_z: &[u32], descendants_of_x: &[u32]) -> bool {
    candidate_z
        .iter()
        .zip(descendants_of_x.iter())
        .any(|(&z, &d)| z != 0 && d != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_disjoint_z_passes() {
        // Z = {0, 1}, descendants(X) = {2, 3}. No overlap → no violation.
        let z = vec![1, 1, 0, 0];
        let d = vec![0, 0, 1, 1];
        assert!(!backdoor_descendants_check_cpu(&z, &d));
    }

    #[test]
    fn cpu_overlap_violates() {
        let z = vec![1, 0, 1, 0];
        let d = vec![0, 0, 1, 1]; // node 2 is both in Z and descendant
        assert!(backdoor_descendants_check_cpu(&z, &d));
    }

    #[test]
    fn cpu_empty_z_never_violates() {
        let z = vec![0, 0, 0, 0];
        let d = vec![1, 1, 1, 1];
        assert!(!backdoor_descendants_check_cpu(&z, &d));
    }

    #[test]
    fn cpu_empty_descendants_never_violates() {
        // X has no descendants  -  any Z is allowed by this rule.
        let z = vec![1, 1, 1, 1];
        let d = vec![0, 0, 0, 0];
        assert!(!backdoor_descendants_check_cpu(&z, &d));
    }

    #[test]
    fn cpu_mismatched_inputs_only_check_complete_pairs() {
        assert!(!backdoor_descendants_check_cpu(&[1], &[]));
        assert!(backdoor_descendants_check_cpu(&[0, 1], &[0, 1, 1]));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = backdoor_descendants_check("z", "d", "v", 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["z", "d", "v"]);
        assert_eq!(p.buffers[0].count(), 8);
        assert_eq!(p.buffers[1].count(), 8);
        assert_eq!(p.buffers[2].count(), 1);
    }

    #[test]
    fn zero_n_traps() {
        let p = backdoor_descendants_check("z", "d", "v", 0);
        assert!(p.stats().trap());
    }
}
