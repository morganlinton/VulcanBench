//! `bounded_by_comparison`  -  Tier-3 shim over
//! [`vyre_primitives::graph::csr_backward_traverse`] with the
//! `DOMINANCE` edge-kind mask.
//!
//! AUDIT_2026-04-24 F-BBC-02 (doc fix): the primitive computes
//! reverse reachability along dominance edges  -  i.e. the set of
//! dominance-tree *ancestors* of each node in `frontier_in`. The
//! stdlib rule intersects that ancestor set with the bound-check
//! NodeSet. Prior doc text claimed "every access is reachable
//! backward along dominance edges from some bound check," which
//! describes descendant reachability, not ancestor reachability  -
//! the directions were swapped. Correct framing: "for each access
//! in `frontier_in`, compute the dominators via ancestor walk,
//! then a bound-check intersects to prove the access is covered
//! by some dominating bound-check."

use vyre::ir::Program;
use vyre_primitives::graph::csr_backward_traverse::csr_backward_traverse;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::region::{reparent_program_children, wrap_anonymous};

const OP_ID: &str = "vyre-libs::security::bounded_by_comparison";

/// Build one reverse-traversal step filtered to dominance edges.
#[must_use]
pub fn bounded_by_comparison(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
) -> Program {
    crate::security::assert_security_inputs(
        OP_ID,
        shape.node_count,
        &[("frontier_in", frontier_in), ("frontier_out", frontier_out)],
    );
    let primitive = csr_backward_traverse(shape, frontier_in, frontier_out, edge_kind::DOMINANCE);
    Program::wrapped(
        primitive.buffers().to_vec(),
        primitive.workgroup_size(),
        vec![wrap_anonymous(
            OP_ID,
            reparent_program_children(&primitive, OP_ID),
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || bounded_by_comparison(ProgramGraphShape::new(4, 4), "fin", "fout"),
        test_inputs: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
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
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // One backward step from {3}: nodes 1 and 2 have edges to 3,
            // so they light up. Accumulator preserves seed {3}.
            vec![vec![to_bytes(&[0b1110])]]
        }),
        category: Some("security"),
    }
}

inventory::submit! {
    // AUDIT_2026-04-24 F-BBC-01: raised from 64 to 4096 so deep
    // dominance trees don't silently truncate; same reasoning as
    // dominator_tree.
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
    fn bounded_by_comparison_mask_is_dominance_only() {
        let _p = bounded_by_comparison(ProgramGraphShape::new(4, 4), "fin", "fout");
        // The primitive is a wrapper around csr_backward_traverse;
        // we verify the mask constant at the module level.
        assert_eq!(edge_kind::DOMINANCE & edge_kind::ASSIGNMENT, 0);
        assert_eq!(edge_kind::DOMINANCE & edge_kind::CONTROL, 0);
    }

    #[test]
    fn bounded_by_comparison_backward_step_reaches_ancestors() {
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
        // Seed is NOT merged by cpu_ref; it returns only newly reached bits.
        assert_eq!(out[0], 0b0110, "backward from 3 must reach 1 and 2");
    }

    #[test]
    fn bounded_by_comparison_program_emits_frontier_buffers() {
        let _p = bounded_by_comparison(ProgramGraphShape::new(4, 4), "fin", "fout");
        let names: Vec<&str> = _p.buffers().iter().map(|b| b.name()).collect();
        assert!(names.contains(&"fin"));
        assert!(names.contains(&"fout"));
    }

    #[test]
    fn bounded_by_comparison_deep_chain_reaches_all_ancestors() {
        let node_count = 10u32;
        let mut offsets = vec![0u32; (node_count + 1) as usize];
        let mut targets = Vec::new();
        let mut masks = Vec::new();
        for i in 0..node_count {
            offsets[i as usize] = i;
            if i + 1 < node_count {
                targets.push(i + 1);
                masks.push(edge_kind::DOMINANCE);
            }
        }
        offsets[node_count as usize] = node_count.saturating_sub(1);

        let mut accumulated = vec![0u32; 1];
        accumulated[0] = 1 << (node_count - 1);

        for _ in 0..node_count {
            let out = cpu_ref(
                node_count,
                &offsets,
                &targets,
                &masks,
                &accumulated,
                edge_kind::DOMINANCE,
            );
            let new_accumulated: Vec<u32> =
                accumulated.iter().zip(&out).map(|(a, b)| a | b).collect();
            if new_accumulated == accumulated {
                break;
            }
            accumulated = new_accumulated;
        }

        let expected = (1u32 << node_count) - 1;
        assert_eq!(
            accumulated[0],
            expected,
            "backward reachability from node {} must reach all ancestors in a {}-node chain; \
             if max_iterations truncates, this test fails",
            node_count - 1,
            node_count
        );

        let contract = crate::harness::convergence_contract(OP_ID)
            .expect("Fix: bounded_by_comparison must have a ConvergenceContract");
        assert!(
            contract.max_iterations >= node_count,
            "ConvergenceContract max_iterations ({}) must be >= chain depth ({}) to avoid silent truncation",
            contract.max_iterations, node_count
        );
    }

    #[test]
    #[should_panic(expected = "node_count must be positive")]
    fn bounded_by_comparison_zero_node_count_should_panic() {
        let _ = bounded_by_comparison(ProgramGraphShape::new(0, 0), "fin", "fout");
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn bounded_by_comparison_empty_buffer_name_should_panic() {
        let _ = bounded_by_comparison(ProgramGraphShape::new(4, 4), "", "fout");
    }
}
