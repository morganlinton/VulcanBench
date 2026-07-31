//! `flows_to`  -  Tier-3 shim over
//! [`vyre_primitives::graph::csr_forward_traverse`].
//!
//! The taint-reachability semantics (*does taint flow from source
//! NodeSet to sink NodeSet given this ProgramGraph?*) live in the
//! generic rule-dialect stdlib:
//!
//! ```text
//! rec reached = source ∪ csr_forward_traverse(reached, all_edges)
//!   where fixpoint on reached
//! ```
//!
//! vyre-libs ships one dispatch step that an analysis-stage fixpoint driver
//! iterates. Op id stays stable; the dead v2 edges_from/edges_to
//! signature from the inert v2 API has been deleted  -  the shim
//! now takes only the canonical frontier / sink buffer names.

use vyre::ir::Program;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::security::flow_composition::dataflow_reach_program;

pub(crate) const OP_ID: &str = "vyre-libs::security::flows_to";

/// Bitmask of edge kinds that represent genuine dataflow edges.
/// Per AUDIT_2026-04-24 F-FT-01 (kimi) a previous `0xFFFF_FFFF`
/// over-approximation caused taint to propagate along CONTROL and
/// DOMINANCE edges, producing massive false-positive noise at
/// internet scale. Restricted now to the set the generic taint-flow
/// standard library explicitly enumerates.
pub const FLOWS_TO_MASK: u32 = edge_kind::ASSIGNMENT
    | edge_kind::CALL_ARG
    | edge_kind::RETURN
    | edge_kind::PHI
    | edge_kind::ALIAS
    | edge_kind::MEM_STORE
    | edge_kind::MEM_LOAD
    | edge_kind::MUT_REF;

/// Bitmask of edge kinds that represent genuine *aliasing* relations
/// (same memory cell, not just "data flowed through this op").
/// Aliasing is a strict subset of `FLOWS_TO_MASK`:
///
/// - `ASSIGNMENT` (`a = b`)  -  `a` aliases `b`'s referenced cell.
/// - `ALIAS` (init-decl, init-alias-bridge, etc.)  -  explicit alias
///   edge emitted by the walker.
/// - `MUT_REF` (`&mut x`)  -  the reference aliases `x`.
/// - `PHI`  -  SSA phi result aliases each incoming source.
///
/// `CALL_ARG` / `RETURN` / `MEM_STORE` / `MEM_LOAD` are EXCLUDED:
/// passing a value into a function or storing it to memory does NOT
/// imply the result aliases the source. The pre-T11 `aliases`
/// primitive used `FLOWS_TO_MASK` directly and reported
/// `aliases(msg, copy) = true` on `char *copy = strdup(msg);`
/// because forward-reach traversed `msg → strdup [CALL_ARG] →
/// copy [ALIAS]`. That FP pattern hit every `dup_*` /
/// `realloc_*_fresh_alloc` / `copy-then-use` negative on the
/// per_shape_truth gate's `use_after_free_double_drop` shape.
pub const ALIAS_PROPAGATION_MASK: u32 =
    edge_kind::ASSIGNMENT | edge_kind::ALIAS | edge_kind::MUT_REF | edge_kind::PHI;

/// Build one forward-traversal step along DATAFLOW edges only.
/// `frontier_in` reads the current reached set, `frontier_out`
/// receives the union of nodes reachable in one more dataflow hop.
#[must_use]
pub fn flows_to(shape: ProgramGraphShape, frontier_in: &str, frontier_out: &str) -> Program {
    crate::security::assert_security_inputs(
        OP_ID,
        shape.node_count,
        &[("frontier_in", frontier_in), ("frontier_out", frontier_out)],
    );
    dataflow_reach_program(OP_ID, shape, frontier_in, frontier_out, FLOWS_TO_MASK)
}

/// Build one forward-traversal step along ALIAS-only edges.
/// Used by [`crate::security::aliases_dataflow::aliases_dataflow`]
/// to compute bidirectional aliasing without conflating "data
/// flowed through this op" with "same memory cell."
#[must_use]
pub fn flows_to_alias_only(
    shape: ProgramGraphShape,
    frontier_in: &str,
    frontier_out: &str,
) -> Program {
    dataflow_reach_program(
        OP_ID,
        shape,
        frontier_in,
        frontier_out,
        ALIAS_PROPAGATION_MASK,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || flows_to(ProgramGraphShape::new(4, 3), "fin", "fout"),
        test_inputs: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // Linear chain 0 → 1 → 2 → 3. Starting frontier {0}.
            // `fout` starts as the accumulator frontier so the
            // convergence lens monotonically grows {0,1,2,3}.
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 1, 2, 3, 3]),       // pg_edge_offsets: 0→{1}, 1→{2}, 2→{3}, 3→{}
                to_bytes(&[1, 2, 3]),             // pg_edge_targets
                to_bytes(&[
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                ]),                               // pg_edge_kind_mask  -  all dataflow
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // fin = {0}
                to_bytes(&[0b0001]),              // fout accumulator seed = {0}
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // One forward-reach step from {0}: the step writes {1}
            // into the accumulator. A no-op that leaves fout at {0}
            // fails this oracle.
            vec![vec![to_bytes(&[0b0011])]]
        }),
        category: Some("security"),
    }
}

inventory::submit! {
    // AUDIT_2026-04-24 F-FT-03: max_iterations raised from 64 to
    // 4096 so deep call graphs (Linux kernel-scale code) don't hit
    // a silent truncation ceiling during the closure. The fixpoint
    // driver aborts early whenever the frontier stops growing, so
    // a higher ceiling costs nothing on small graphs; the only
    // case where this matters is a pathologically deep reachability
    // walk, where the old 64-step cap was producing false negatives.
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
    fn flows_to_mask_excludes_control_and_dominance() {
        assert_eq!(FLOWS_TO_MASK & edge_kind::CONTROL, 0);
        assert_eq!(FLOWS_TO_MASK & edge_kind::DOMINANCE, 0);
    }

    #[test]
    fn flows_to_mask_includes_assignment_and_call_arg() {
        assert_ne!(FLOWS_TO_MASK & edge_kind::ASSIGNMENT, 0);
        assert_ne!(FLOWS_TO_MASK & edge_kind::CALL_ARG, 0);
    }

    #[test]
    fn flows_to_mask_is_not_universal() {
        assert_ne!(FLOWS_TO_MASK, 0xFFFF_FFFF, "regression to universal mask");
    }

    #[test]
    fn flows_to_cpu_fixture_excludes_control_and_dominance_false_positive_edges() {
        let edge_offsets = [0, 2, 3, 3, 3];
        let edge_targets = [1, 3, 2];
        let edge_kind_mask = [
            edge_kind::CONTROL,
            edge_kind::ASSIGNMENT,
            edge_kind::DOMINANCE,
        ];
        let out = crate::security::flow_composition::dataflow_reach_step_cpu_ref(
            4,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &[0b0001],
        );

        assert_eq!(
            out,
            vec![0b1000],
            "Fix: flows_to must follow assignment dataflow edges while excluding CONTROL/DOMINANCE false-positive paths."
        );
    }

    #[test]
    fn alias_only_fixture_keeps_copy_and_realloc_but_excludes_dup_call_edges() {
        let edge_offsets = [0, 5, 5, 5, 5, 5, 5];
        let edge_targets = [1, 2, 3, 4, 5];
        let edge_kind_mask = [
            edge_kind::CALL_ARG,
            edge_kind::ASSIGNMENT,
            edge_kind::ALIAS,
            edge_kind::MUT_REF,
            edge_kind::RETURN,
        ];
        let alias_only = vyre_primitives::graph::csr_forward_traverse::cpu_ref(
            6,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &[0b000001],
            ALIAS_PROPAGATION_MASK,
        );
        let full_flow = vyre_primitives::graph::csr_forward_traverse::cpu_ref(
            6,
            &edge_offsets,
            &edge_targets,
            &edge_kind_mask,
            &[0b000001],
            FLOWS_TO_MASK,
        );

        assert_eq!(
            alias_only,
            vec![0b011100],
            "Fix: alias-only propagation should keep direct copy/alias/realloc edges and reject dup-style CALL_ARG/RETURN taint."
        );
        assert_eq!(
            full_flow,
            vec![0b111110],
            "Fix: full flows_to should still carry regular source-to-sink CALL_ARG/RETURN dataflow."
        );
    }

    #[test]
    fn flows_to_program_emits_frontier_buffers() {
        let p = flows_to(ProgramGraphShape::new(4, 3), "fin", "fout");
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert!(names.contains(&"fin"));
        assert!(names.contains(&"fout"));
    }

    #[test]
    fn flows_to_program_uses_non_degenerate_shape() {
        let shape = ProgramGraphShape::new(64, 128);
        let p = flows_to(shape, "fin", "fout");
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
    fn flows_to_and_taint_flow_convergence_contracts_match_intentionally() {
        // taint_flow is an alternate API-facing predicate name; flows_to is the
        // core primitive. Both close the same FLOWS_TO_MASK forward
        // closure and therefore share the same convergence regime  -
        // matching `max_iterations` is the contract, not a hygiene gap.
        // Their IR differs (distinct OP_ID tags) but their fixpoint
        // depths are identical by construction.
        let c_flows = crate::harness::convergence_contract("vyre-libs::security::flows_to")
            .expect("Fix: flows_to must have a ConvergenceContract");
        let c_taint = crate::harness::convergence_contract("vyre-libs::security::taint_flow")
            .expect("Fix: taint_flow must have a ConvergenceContract");
        assert_eq!(
            c_flows.max_iterations, c_taint.max_iterations,
            "flows_to and taint_flow MUST share max_iterations: they close the \
             same forward-mask fixpoint and a divergence here would silently \
             truncate one path while letting the other run to completion."
        );
    }

    #[test]
    #[should_panic(expected = "node_count must be positive")]
    fn flows_to_zero_node_count_should_panic() {
        let _ = flows_to(ProgramGraphShape::new(0, 0), "fin", "fout");
    }

    #[test]
    #[should_panic(expected = "empty buffer name")]
    fn flows_to_empty_buffer_name_should_panic() {
        let _ = flows_to(ProgramGraphShape::new(4, 3), "", "fout");
    }

    #[test]
    fn flows_to_edge_count_exceeds_actual_edges_traps_in_reference() {
        let p = flows_to(ProgramGraphShape::new(4, 10), "fin", "fout");
        let to_bytes = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
        let inputs = vec![
            to_bytes(&[0, 0, 0, 0]),    // pg_nodes
            to_bytes(&[0, 1, 2, 3, 3]), // pg_edge_offsets (only 3 edges)
            to_bytes(&[1, 2, 3]),       // pg_edge_targets (3 elements, declared 10)
            to_bytes(&[
                edge_kind::ASSIGNMENT,
                edge_kind::ASSIGNMENT,
                edge_kind::ASSIGNMENT,
            ]),
            to_bytes(&[0, 0, 0, 0]), // pg_node_tags
            to_bytes(&[0b0001]),     // fin
            to_bytes(&[0b0001]),     // fout
        ];
        let values: Vec<vyre_reference::value::Value> = inputs
            .into_iter()
            .map(vyre_reference::value::Value::from)
            .collect();
        let error = vyre_reference::reference_eval(&p, &values).expect_err(
            "edge_count (10) exceeds actual edges (3) must trap or error in reference_eval",
        );
        let msg = error.to_string();
        assert!(
            msg.contains("trap") || msg.contains("Fix:") || msg.contains("edge"),
            "flows_to edge-count mismatch error must be actionable: {msg}"
        );
    }
}
