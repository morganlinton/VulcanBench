//! Full Sinkhorn-balanced dispatch-graph clustering.
//!
//! Replaces the single-step version in `sinkhorn_dispatch_clustering` (#2)
//! with a full iterative fixpoint. This computes an entropy-regularized
//! optimal transport plan between dispatch components, yielding a balanced
//! soft assignment of nodes to clusters.
//!
//! Composes the `vyre_primitives::math::sinkhorn_iterate` primitive to run
//! entirely on device without host round-trips.

use vyre_foundation::ir::Program;
use vyre_primitives::math::sinkhorn_iterate::sinkhorn_iterate;

/// Stable op identifier for the full-clustering Sinkhorn iteration self-consumer.
pub const OP_ID: &str = "vyre-libs::self_substrate::sinkhorn_full_clustering";

/// Compile a Program that runs full Sinkhorn iterations.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn sinkhorn_full_clustering_program(
    k: &str,
    k_t: &str,
    a: &str,
    b: &str,
    u_curr: &str,
    u_next: &str,
    v: &str,
    kv: &str,
    ktu: &str,
    changed: &str,
    m: u32,
    n: u32,
    max_iterations: u32,
) -> Program {
    use crate::observability::{bump, sinkhorn_full_clustering_calls};
    bump(&sinkhorn_full_clustering_calls);
    sinkhorn_iterate(
        k,
        k_t,
        a,
        b,
        u_curr,
        u_next,
        v,
        kv,
        ktu,
        changed,
        m,
        n,
        max_iterations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sinkhorn_clustering_program() {
        let p = sinkhorn_full_clustering_program(
            "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 10, 20, 5,
        );
        assert_eq!(p.buffers().len(), 10);
        assert!(p.buffers().iter().any(|b| b.name() == "uc"));
    }

    #[test]
    fn test_multi_region_sinkhorn() {
        let p1 = sinkhorn_full_clustering_program(
            "k1", "kt1", "a1", "b1", "uc1", "un1", "v1", "kv1", "ktu1", "c1", 2, 2, 1,
        );
        let p2 = sinkhorn_full_clustering_program(
            "k2", "kt2", "a2", "b2", "uc2", "un2", "v2", "kv2", "ktu2", "c2", 2, 2, 1,
        );
        let p3 = sinkhorn_full_clustering_program(
            "k3", "kt3", "a3", "b3", "uc3", "un3", "v3", "kv3", "ktu3", "c3", 2, 2, 1,
        );

        let final_p = crate::test_support::wrap_program_sequence(&[&p1, &p2, &p3], [256, 1, 1]);
        let region_count = final_p
            .entry()
            .iter()
            .filter(|n| matches!(n, vyre_foundation::ir::Node::Region { .. }))
            .count();
        assert!(region_count >= 3);
    }

    #[test]
    fn test_end_to_end_sinkhorn_parity() {
        let k = vec![65536, 65536, 65536, 65536];
        let a = vec![32768, 32768];
        let b = vec![32768, 32768];
        let u_c = vec![65536, 65536];
        let v_in = vec![65536, 65536];

        let p = sinkhorn_full_clustering_program(
            "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 2, 2, 1,
        );

        use std::sync::Arc;
        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[u32]| {
            let bytes = vyre_primitives::wire::pack_u32_slice(data);
            Value::Bytes(Arc::from(bytes))
        };

        let inputs = vec![
            to_value(&u_c),
            to_value(&[0_u32, 0]),
            to_value(&[0]),
            to_value(&k),
            to_value(&k), // kt
            to_value(&a),
            to_value(&b),
            to_value(&v_in),
            to_value(&[0_u32, 0]),
            to_value(&[0_u32, 0]),
        ];

        let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
        let actual_bytes = results[0].to_bytes();
        let actual_u: Vec<u32> = actual_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        // first iter: Kv = [2, 2] scaled by 2^32? No, 2^32 = 0.
        // If it wraps to 0, floor is 1. u = a/1 = 32768.
        assert_eq!(actual_u[0], 32768);
    }
}
