//! `taint_pollution`  -  "did taint reach a label-tagged node?"
//!
//! The CodeQL `globalAllowingExtras` shape compressed to one
//! Region. Composes a one-step BFS with intersection against a
//! family-tagged node set, then any-reduce.

use vyre::ir::Program;
use vyre_primitives::graph::program_graph::ProgramGraphShape;
use vyre_primitives::predicate::edge_kind;

use crate::security::flow_composition::dataflow_hit_program;
#[cfg(test)]
use crate::security::flows_to::FLOWS_TO_MASK;

pub(crate) const OP_ID: &str = "vyre-libs::security::taint_pollution";

/// Build a one-step taint-pollution Program: source → reach
/// (FLOWS_TO_MASK) → AND with label-tagged sink set → any-reduce.
#[must_use]
pub fn taint_pollution(
    shape: ProgramGraphShape,
    source_buf: &str,
    label_set: &str,
    reach_buf: &str,
    hits_buf: &str,
    out_scalar: &str,
) -> Program {
    dataflow_hit_program(
        OP_ID, shape, source_buf, label_set, reach_buf, hits_buf, out_scalar,
    )
}

/// CPU oracle.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    source: &[u32],
    label_set: &[u32],
) -> u32 {
    use vyre_primitives::bitset::and::cpu_ref as and_ref;
    use vyre_primitives::graph::csr_forward_traverse::cpu_ref as fwd_ref;
    let reach = fwd_ref(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        source,
        FLOWS_TO_MASK,
    );
    let hits = and_ref(&reach, label_set);
    u32::from(hits.iter().any(|w| *w != 0))
}

/// Soundness marker for [`taint_pollution`].
pub struct TaintPollution;
impl vyre::soundness::SoundnessTagged for TaintPollution {
    fn soundness(&self) -> vyre::soundness::Soundness {
        vyre::soundness::Soundness::MayOver
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || taint_pollution(ProgramGraphShape::new(4, 3), "source", "label_set", "reach", "hits", "out_scalar"),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 1, 2, 3, 3]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3]),             // pg_edge_targets
                to_bytes(&[
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                    edge_kind::ASSIGNMENT,
                ]),                               // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0b0001]),              // source = {0}
                to_bytes(&[0b0001]),              // reach = {0}
                to_bytes(&[0b0010]),              // label_set = {1}
                to_bytes(&[0b0000]),              // hits
                to_bytes(&[0b0000]),              // out_scalar
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0b0011]),              // reach = {0,1}
                to_bytes(&[0b0010]),              // hits = {1}
                to_bytes(&[0b0001]),              // out_scalar = 1
            ]]
        }),
        category: Some("security"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::predicate::edge_kind;

    #[test]
    fn one_hop_to_labeled_returns_one() {
        // 0 -> 1, label = {1}
        let off = vec![0u32, 1, 1];
        let tgt = vec![1u32];
        let msk = vec![edge_kind::ASSIGNMENT];
        assert_eq!(cpu_ref(2, &off, &tgt, &msk, &[0b01], &[0b10]), 1);
    }

    #[test]
    fn no_label_hit_returns_zero() {
        let off = vec![0u32, 1, 1];
        let tgt = vec![1u32];
        let msk = vec![edge_kind::ASSIGNMENT];
        assert_eq!(cpu_ref(2, &off, &tgt, &msk, &[0b01], &[0]), 0);
    }

    #[test]
    fn empty_source_returns_zero() {
        let off = vec![0u32, 1, 1];
        let tgt = vec![1u32];
        let msk = vec![edge_kind::ASSIGNMENT];
        assert_eq!(cpu_ref(2, &off, &tgt, &msk, &[0], &[0xFFFF]), 0);
    }

    #[test]
    fn unreachable_label_returns_zero() {
        // 0 -> 1, label = {0}  -  source 0 doesn't taint itself.
        let off = vec![0u32, 1, 1];
        let tgt = vec![1u32];
        let msk = vec![edge_kind::ASSIGNMENT];
        assert_eq!(cpu_ref(2, &off, &tgt, &msk, &[0b01], &[0b01]), 0);
    }
}
