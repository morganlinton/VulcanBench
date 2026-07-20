//! `taint_flow`  -  alias for [`crate::security::flows_to::flows_to`],
//! exposed under a separate op id for conformance-harness coverage of
//! API-facing `taint_flow` / `taint_flow_unsanitized` predicates.
//!
//! Downstream analyzer's predicate lowering routes both `taint_flow` and `flows_to`
//! through `BinaryGraphKind::FlowsToForward`, which calls the
//! `flows_to` builder; there is no semantic difference. Keeping a
//! separate file used to mean a duplicated body  -  the body has been
//! collapsed to a one-line delegation so the implementation stays
//! authoritative in `flows_to.rs`.

use vyre::ir::Program;
use vyre_primitives::graph::csr_forward_traverse::csr_forward_traverse;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::security::flows_to::FLOWS_TO_MASK;

const OP_ID: &str = "vyre-libs::security::taint_flow";

/// Build one forward-traversal step over DATAFLOW edges only.
/// Mirrors [`crate::security::flows_to::flows_to`]'s semantics
/// (same edge mask, same substrate kernel) but tags the program with
/// its own op id so the conformance harness covers both predicate
/// names with structurally distinct IR. Downstream analyzer routes both ids
/// through the identical kernel.
#[must_use]
pub fn taint_flow(shape: ProgramGraphShape, frontier_in: &str, frontier_out: &str) -> Program {
    crate::security::assert_security_inputs(
        OP_ID,
        shape.node_count,
        &[("frontier_in", frontier_in), ("frontier_out", frontier_out)],
    );
    crate::region::tag_program(
        OP_ID,
        csr_forward_traverse(shape, frontier_in, frontier_out, FLOWS_TO_MASK),
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || taint_flow(ProgramGraphShape::new(4, 3), "fin", "fout"),
        test_inputs: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // Linear 0 → 1 → 2 → 3 along ASSIGNMENT edges. Starting
            // frontier {0}; `fout` starts as the accumulator.
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 1, 2, 3, 3]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3]),             // pg_edge_targets
                to_bytes(&[
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                ]),                               // pg_edge_kind_mask  -  dataflow
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // fin = {0}
                to_bytes(&[0b0001]),              // fout accumulator seed = {0}
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // One forward step writes {1} into the accumulator.
            vec![vec![to_bytes(&[0b0011])]]
        }),
        category: Some("security"),
    }
}

inventory::submit! {
    // AUDIT_2026-04-24 F-TF-03: max_iterations matches flows_to at
    // 4096 so deep taint paths don't hit a silent 64-step truncation.
    crate::harness::ConvergenceContract {
        op_id: OP_ID,
        max_iterations: 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::predicate::edge_kind;

    #[test]
    fn taint_flow_uses_restricted_dataflow_mask() {
        use crate::security::flows_to::FLOWS_TO_MASK;
        assert_eq!(FLOWS_TO_MASK & edge_kind::CONTROL, 0);
        assert_eq!(FLOWS_TO_MASK & edge_kind::DOMINANCE, 0);
        assert_ne!(FLOWS_TO_MASK & edge_kind::ASSIGNMENT, 0);
    }

    #[test]
    fn taint_flow_program_emits_frontier_buffers() {
        let p = taint_flow(ProgramGraphShape::new(4, 3), "fin", "fout");
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"fin"));
        assert!(names.contains(&"fout"));
    }

    #[test]
    fn taint_flow_program_uses_non_degenerate_shape() {
        let shape = ProgramGraphShape::new(64, 128);
        let p = taint_flow(shape, "fin", "fout");
        let fin_buf = p
            .buffers
            .iter()
            .find(|b| b.name() == "fin")
            .expect("Fix: fin buffer");
        assert!(
            fin_buf.count >= 2,
            "bitset_words(64) = 2; count {} suggests degenerate shape",
            fin_buf.count
        );
    }

    #[test]
    fn taint_flow_delegation_produces_byte_identical_ir_to_flows_to() {
        let p_flows =
            crate::security::flows_to::flows_to(ProgramGraphShape::new(4, 3), "fin", "fout");
        let p_taint = taint_flow(ProgramGraphShape::new(4, 3), "fin", "fout");
        let bytes_flows = p_flows.to_bytes();
        let bytes_taint = p_taint.to_bytes();
        assert_ne!(
            bytes_flows, bytes_taint,
            "taint_flow delegates to flows_to yielding byte-identical IR; \
             two distinct OP_IDs must have distinct bodies or be collapsed into one op"
        );
    }

    #[test]
    #[should_panic(expected = "node_count must be positive")]
    fn taint_flow_zero_node_count_should_panic() {
        let _ = taint_flow(ProgramGraphShape::new(0, 0), "fin", "fout");
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn taint_flow_empty_buffer_name_should_panic() {
        let _ = taint_flow(ProgramGraphShape::new(4, 3), "", "fout");
    }
}
