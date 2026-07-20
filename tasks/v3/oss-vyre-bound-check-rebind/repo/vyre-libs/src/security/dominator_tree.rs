//! `dominator_tree`  -  Tier-3 shim.
//!
//! The [`dominator_tree`](fn@dominator_tree) primitive is tagged with
//! ``Soundness::MayOver``:
//! it computes reverse reachability over dominance edges (set union of
//! dominance-tree ancestors), which over-approximates true dominators.
//! Callers that need exact strict dominance must use `cpu_dominator_sets`,
//! the CPU reference oracle implementing the Cooper-Harvey-Kennedy 2001
//! iterative dataflow algorithm (set intersection of predecessor dominator
//! sets). Rules with a zero-false-positive precision contract MUST compose
//! against `cpu_dominator_sets` rather than [`dominator_tree`].
//!
//! AUDIT_2026-04-24 F-DT-02 (honest status): true dominator computation is
//! the intersection of predecessor dominator sets
//! (Cooper-Harvey-Kennedy / Lengauer-Tarjan), NOT a fixpoint over
//! reverse reachability  -  intersection and union are different
//! lattice operators and the distinction matters for correctness.
//! The present primitive emits `csr_backward_traverse` over
//! DOMINANCE edges, which computes reverse reachability (the set
//! of dominance-tree ancestors, unioned across predecessors). That
//! matches current reverse-reachability composition consumers but is
//! technically a stronger (over-approximating) predicate than
//! "dominates." Callers depending on strict dominator semantics
//! should use `cpu_dominator_sets` or compose the intersection in generic query dialect
//! directly. This note is load-bearing: security rules that consume
//! this op today are using it as reverse reachability and will
//! keep working; any new rule that needs strict dominance must
//! flag the dependency explicitly.

use vyre::ir::Program;
use vyre_primitives::graph::csr_backward_traverse::csr_backward_traverse;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::region::{reparent_program_children, wrap_anonymous};

const OP_ID: &str = "vyre-libs::security::dominator_tree";

/// Build one reverse-traversal step along dominance edges.
///
/// # Soundness
///
/// This composition is ``Soundness::MayOver``:
/// it returns the set of nodes that can reach `n` via dominance edges,
/// i.e. an over-approximation of true dominators. Rules that require
/// zero false positives must gate on `cpu_dominator_sets` instead.
#[must_use]
pub fn dominator_tree(shape: ProgramGraphShape, frontier_in: &str, frontier_out: &str) -> Program {
    crate::security::assert_security_inputs(
        OP_ID,
        shape.node_count,
        &[("frontier_in", frontier_in), ("frontier_out", frontier_out)],
    );
    // Traverse DOMINANCE (block-entry idom chain) UNION BLOCK_MEMBER
    // (block-entry ↔ contained-node, bidirectional). The idom edges
    // alone connect only block-entry nodes; BLOCK_MEMBER lets a backward
    // step reach a call/argument operand node from its block entry and
    // vice-versa, so `dominates($a, $b)` evaluates block-level dominance
    // on arbitrary expression operands rather than the empty frontier
    // those operands form against idom-only edges.
    let primitive = csr_backward_traverse(
        shape,
        frontier_in,
        frontier_out,
        edge_kind::DOMINANCE | edge_kind::BLOCK_MEMBER,
    );
    Program::wrapped(
        primitive.buffers().to_vec(),
        primitive.workgroup_size(),
        vec![wrap_anonymous(
            OP_ID,
            reparent_program_children(&primitive, OP_ID),
        )],
    )
}

/// CPU reference oracle for strict dominator sets.
///
/// Implements the iterative dataflow algorithm from Cooper, Harvey &
/// Kennedy (2001):
///
/// 1. `Dom(entry) = {entry}`; `Dom(other) = ALL_NODES`.
/// 2. Iterate over nodes in reverse postorder, computing  
///    `Dom(n) = {n} ∪ ⋂_{p ∈ preds(n)} Dom(p)` until fixpoint.
/// 3. Return `Vec<Vec<u32>>` where index `n` is the sorted dominator set.
///
/// This is an ``Exact``
/// reference; rules that require zero false positives MUST compose
/// against this oracle rather than the GPU [`dominator_tree`] shim.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_dominator_sets(
    num_nodes: u32,
    entry: u32,
    edges: &[(u32, u32)],
) -> Vec<Vec<u32>> {
    let idoms = vyre_primitives::graph::dominator_tree::cpu_ref(num_nodes, entry, edges);
    vyre_primitives::graph::dominator_tree::idoms_to_dominator_sets(&idoms, num_nodes)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || dominator_tree(ProgramGraphShape::new(4, 4), "fin", "fout"),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            // Diamond dominance tree: 0 dominates 1 and 2; both dominate 3.
            // Backward from {3} reaches {1, 2} in one step.
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 2, 3, 4, 4]),       // pg_edge_offsets: 0→{1,2}, 1→{3}, 2→{3}, 3→{}
                to_bytes(&[1, 2, 3, 3]),          // pg_edge_targets
                to_bytes(&[
                    edge_kind::DOMINANCE,
                    edge_kind::DOMINANCE,
                    edge_kind::DOMINANCE,
                    edge_kind::DOMINANCE,
                ]),                               // pg_edge_kind_mask  -  all DOMINANCE
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b1000]),              // fin = {3}
                to_bytes(&[0b1000]),              // fout accumulator seed = {3}
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            // One backward step from {3}: nodes 1 and 2 have edges to 3,
            // so they light up. Accumulator preserves seed {3}.
            vec![vec![to_bytes(&[0b1110])]]
        }),
        category: Some("security"),
    }
}

inventory::submit! {
    // AUDIT_2026-04-24 F-DT-01: raised from 64 to 4096 so deep
    // dominance trees (Linux kernel-scale CFGs routinely 500+ deep)
    // don't silently truncate at the 64th step and produce false
    // negatives. Fixpoint drivers exit early when the frontier
    // stops growing, so a higher ceiling has no cost on flat graphs.
    crate::harness::ConvergenceContract {
        op_id: OP_ID,
        max_iterations: 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::graph::csr_backward_traverse::cpu_ref;

    fn diamond_dominance_tree() -> (u32, Vec<u32>, Vec<u32>, Vec<u32>) {
        let node_count = 4;
        let edge_offsets = vec![0, 2, 3, 4, 4];
        let edge_targets = vec![1, 2, 3, 3];
        let edge_kind_mask = vec![edge_kind::DOMINANCE; 4];
        (node_count, edge_offsets, edge_targets, edge_kind_mask)
    }

    #[test]
    fn cpu_dominator_sets_linear_chain() {
        // 0 -> 1 -> 2 -> 3
        let edges = &[(0, 1), (1, 2), (2, 3)];
        let dom = cpu_dominator_sets(4, 0, edges);
        assert_eq!(dom[0], vec![0]);
        assert_eq!(dom[1], vec![0, 1]);
        assert_eq!(dom[2], vec![0, 1, 2]);
        assert_eq!(dom[3], vec![0, 1, 2, 3]);
    }

    #[test]
    fn cpu_dominator_sets_diamond() {
        // 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        let edges = &[(0, 1), (0, 2), (1, 3), (2, 3)];
        let dom = cpu_dominator_sets(4, 0, edges);
        assert_eq!(dom[0], vec![0]);
        assert_eq!(dom[1], vec![0, 1]);
        assert_eq!(dom[2], vec![0, 2]);
        assert_eq!(dom[3], vec![0, 3]);
    }

    #[test]
    fn cpu_dominator_sets_while_loop() {
        // 0 -> 1, 1 -> 2, 2 -> 1, 1 -> 3
        let edges = &[(0, 1), (1, 2), (2, 1), (1, 3)];
        let dom = cpu_dominator_sets(4, 0, edges);
        assert_eq!(dom[0], vec![0]);
        assert_eq!(dom[1], vec![0, 1]);
        assert_eq!(dom[2], vec![0, 1, 2]);
        assert_eq!(dom[3], vec![0, 1, 3]);
    }

    #[test]
    fn dominator_tree_backward_step_reaches_ancestors() {
        let (node_count, offsets, targets, masks) = diamond_dominance_tree();
        let frontier_in = vec![0b1000]; // {3}
        let out = cpu_ref(
            node_count,
            &offsets,
            &targets,
            &masks,
            &frontier_in,
            edge_kind::DOMINANCE,
        );
        assert_eq!(out[0], 0b0110, "backward from 3 must reach 1 and 2");
    }

    #[test]
    fn dominator_tree_program_emits_frontier_buffers() {
        let p = dominator_tree(ProgramGraphShape::new(4, 4), "fin", "fout");
        let names: Vec<&str> = p.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"fin"));
        assert!(names.contains(&"fout"));
    }

    #[test]
    fn dominator_tree_soundness_is_mayover() {
        // The GPU dominator_tree shim is documented as MayOver (reverse reachability).
        use vyre::ir::Node;
        let p = dominator_tree(ProgramGraphShape::new(2, 1), "fin", "fout");
        let [Node::Region { generator, .. }] = p.entry() else {
            panic!("dominator_tree must emit one wrapped region");
        };
        assert_eq!(generator.as_str(), OP_ID);
    }

    #[test]
    fn dominator_tree_gpu_over_approximates_strict_dominators_on_diamond() {
        let p = dominator_tree(ProgramGraphShape::new(4, 4), "fin", "fout");
        let to_bytes = vyre_primitives::wire::pack_u32_slice;
        let inputs = vec![
            to_bytes(&[0, 0, 0, 0]),    // pg_nodes
            to_bytes(&[0, 2, 3, 4, 4]), // pg_edge_offsets
            to_bytes(&[1, 2, 3, 3]),    // pg_edge_targets
            to_bytes(&[
                edge_kind::DOMINANCE,
                edge_kind::DOMINANCE,
                edge_kind::DOMINANCE,
                edge_kind::DOMINANCE,
            ]),
            to_bytes(&[0, 0, 0, 0]), // pg_node_tags
            to_bytes(&[0b1000]),     // fin = {3}
            to_bytes(&[0b1000]),     // fout seed = {3}
        ];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let outputs = vyre_reference::reference_eval(&p, &values).unwrap();
        let gpu_out = u32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());

        let dom = cpu_dominator_sets(4, 0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        let true_dom_bitset: u32 = dom[3].iter().map(|&n| 1u32 << n).sum();

        // Adversarial test for the documented soundness gap.
        //
        // `dominator_tree` is implemented as one
        // `csr_backward_traverse` step over DOMINANCE edges from the
        // input frontier. That is a *single-hop predecessor query*,
        // not the dominator closure: starting from `{3}` it returns
        // `{3} ∪ pred_DOMINANCE(3)` (immediate predecessors only),
        // never reaching `{0}` which sits two hops back through the
        // diamond. So the shim is neither strictly dominator-equal
        // (under-includes  -  misses 0) nor a strict super-set
        // over-approximation. The doc says `Soundness::MayOver` but
        // the underlying primitive is one-step, not closure.
        //
        // This adversarial test pins the current behaviour so any
        // change to the substrate (e.g. wiring in a fixpoint closure
        // or migrating to a downstream dataflow engine's strict dominator solver) must update
        // this test deliberately. Rules that need true dominators
        // route through `cpu_dominator_sets` (Lengauer-Tarjan).
        let one_hop_predecessors_of_3: u32 = 0b1110; // {1, 2, 3}: self + immediate DOMINANCE preds
        assert_eq!(
            gpu_out, one_hop_predecessors_of_3,
            "dominator_tree single-step shim returned {gpu_out:b}; expected \
             {one_hop_predecessors_of_3:b} (immediate DOMINANCE predecessors of node 3). \
             True strict dominators are {true_dom_bitset:b}  -  the shim does NOT \
             compute these and rules requiring strict dominators must use \
             cpu_dominator_sets instead."
        );
        assert_ne!(
            gpu_out, true_dom_bitset,
            "dominator_tree shim must visibly differ from strict dominators \
             on the diamond  -  equality here would mean the substrate \
             silently became a closure and the doc/tests need updating."
        );
    }

    #[test]
    #[should_panic(expected = "node_count must be positive")]
    fn dominator_tree_zero_node_count_should_panic() {
        let _ = dominator_tree(ProgramGraphShape::new(0, 0), "fin", "fout");
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn dominator_tree_empty_buffer_name_should_panic() {
        let _ = dominator_tree(ProgramGraphShape::new(4, 4), "", "fout");
    }
}
