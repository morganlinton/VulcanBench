//! Provenance closure tracking up to W·32 source rules.
//!
//! Extends `#39 scallop_provenance` from a 32-rule (single u32) capacity to
//! up to 256 rules (`W=8`). Uses the wide-lineage variant of `scallop_join`.
//!
//! Dispatches the `vyre_primitives::math::scallop_join_wide` primitive.

use vyre_foundation::ir::Program;
use vyre_primitives::math::scallop_join_wide::scallop_join_wide;

/// Stable op identifier for the wide-lineage Scallop provenance closure.
pub const OP_ID: &str = "vyre-libs::self_substrate::scallop_provenance_wide";

/// Compile a Program that tracks provenance via Datalog fixpoint.
#[must_use]
pub fn scallop_provenance_wide_program(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    w: u32,
    max_iterations: u32,
) -> Program {
    use crate::observability::{bump, scallop_provenance_wide_calls};
    bump(&scallop_provenance_wide_calls);
    scallop_join_wide(state, next, join_rules, changed, n, w, max_iterations)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::identity_op, clippy::erasing_op)]
    use super::*;

    #[test]
    fn test_scallop_provenance_wide_program() {
        let p = scallop_provenance_wide_program("s", "n", "j", "c", 10, 4, 100);
        assert_eq!(p.buffers().len(), 4);
    }

    #[test]
    fn test_multi_region_provenance() {
        let p1 = scallop_provenance_wide_program("s1", "n1", "j1", "c1", 4, 1, 5);
        let p2 = scallop_provenance_wide_program("s2", "n2", "j2", "c2", 4, 1, 5);
        let p3 = scallop_provenance_wide_program("s3", "n3", "j3", "c3", 4, 1, 5);

        let final_p = crate::test_support::wrap_program_sequence(&[&p1, &p2, &p3], [256, 1, 1]);
        let region_count = final_p
            .entry()
            .iter()
            .filter(|n| matches!(n, vyre_foundation::ir::Node::Region { .. }))
            .count();
        assert!(region_count >= 3);
    }

    #[test]
    fn test_end_to_end_provenance_parity() {
        let n = 2;
        let w = 1;
        let mut state_init = vec![0; 4];
        state_init[0 * 2 + 1] = 0b01;
        let mut join_rules = vec![0; 4];
        join_rules[1 * 2 + 0] = 0b10;

        let p = scallop_provenance_wide_program("s", "nx", "j", "c", n, w, 2);

        use std::sync::Arc;
        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[u32]| {
            let bytes = vyre_primitives::wire::pack_u32_slice(data);
            Value::Bytes(Arc::from(bytes))
        };

        // Buffer order matches `scallop_join_wide` `Program::wrapped`: state, next, changed, join_rules.
        let inputs = vec![
            to_value(&state_init),
            to_value(&[0_u32; 4]),
            to_value(&[0_u32]),
            to_value(&join_rules),
        ];

        let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
        let actual_bytes = results[0].to_bytes();
        let actual_out: Vec<u32> = actual_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        // (0,1) * (1,0) -> (0,0) becomes 0b11
        assert_eq!(actual_out[0], 0b11);
    }
}
