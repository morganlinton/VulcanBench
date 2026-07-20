//! Tensor-network contraction order via shortest-path on the contraction-cost graph.
//!
//! Extends `tensor_network_fusion_order` (#35). Instead of a greedy heuristic,
//! we frame the search for the optimal contraction order of a Region chain as
//! finding the shortest path in a state graph where:
//! - Node = subset of contracted tensors (represented as an integer bitset or ID).
//! - Edge = contracting two adjacent sub-networks.
//! - Weight = FLOP cost of that specific contraction step.
//!
//! We dispatch `vyre_primitives::math::bellman_shortest_path` to find the
//! globally optimal sequence of pairwise fusions.

use vyre_foundation::ir::Program;
use vyre_primitives::math::bellman_shortest_path::bellman_shortest_path;

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Canonical self-substrate op ID for the Bellman TN order.
pub const OP_ID: &str = "vyre-libs::self_substrate::bellman_tn_order";

/// Caller-owned GPU dispatch scratch for Bellman tensor-network ordering.
#[derive(Debug, Default)]
pub struct BellmanTnOrderGpuScratch {
    inputs: Vec<Vec<u8>>,
    changed: [u32; 1],
}

/// Compile a Program that finds the optimal tensor-network contraction
/// order by running Bellman-Ford over the state space of contractions.
///
/// `n_nodes` is the number of possible contraction states (e.g. `2^N` for N tensors).
/// `n_edges` is the number of valid contraction transitions.
/// The output `dist` buffer will contain the minimum cost to reach each state.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn bellman_tn_order_program(
    src: &str,
    dst: &str,
    weight: &str,
    dist: &str,
    next_dist: &str,
    changed: &str,
    n_nodes: u32,
    n_edges: u32,
    max_iterations: u32,
) -> Program {
    use crate::observability::{bellman_tn_order_calls, bump};
    bump(&bellman_tn_order_calls);
    // Composes the tier-2.5 primitive directly.
    bellman_shortest_path(
        src,
        dst,
        weight,
        dist,
        next_dist,
        changed,
        n_nodes,
        n_edges,
        max_iterations,
    )
}

/// GPU dispatch wrapper for the Bellman-Ford-based contraction-order
/// solver. Returns the converged minimum-distance vector.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed edge or distance
/// buffers.
#[allow(clippy::too_many_arguments)]
pub fn bellman_tn_order_via(
    dispatcher: &dyn OptimizerDispatcher,
    src: &[u32],
    dst: &[u32],
    weight: &[u32],
    dist_init: &[u32],
    n_nodes: u32,
    max_iterations: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    bellman_tn_order_via_into(
        dispatcher,
        src,
        dst,
        weight,
        dist_init,
        n_nodes,
        max_iterations,
        &mut out,
    )?;
    Ok(out)
}

/// GPU dispatch wrapper for the Bellman-Ford contraction-order solver into
/// caller-owned output storage.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed edge or distance buffers.
#[allow(clippy::too_many_arguments)]
pub fn bellman_tn_order_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    src: &[u32],
    dst: &[u32],
    weight: &[u32],
    dist_init: &[u32],
    n_nodes: u32,
    max_iterations: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = BellmanTnOrderGpuScratch::default();
    bellman_tn_order_via_with_scratch_into(
        dispatcher,
        src,
        dst,
        weight,
        dist_init,
        n_nodes,
        max_iterations,
        &mut scratch,
        out,
    )
}

/// GPU dispatch wrapper for the Bellman-Ford contraction-order solver into
/// caller-owned dispatch and output storage.
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed edge or distance buffers.
#[allow(clippy::too_many_arguments)]
pub fn bellman_tn_order_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    src: &[u32],
    dst: &[u32],
    weight: &[u32],
    dist_init: &[u32],
    n_nodes: u32,
    max_iterations: u32,
    scratch: &mut BellmanTnOrderGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if n_nodes == 0 {
        if !dist_init.is_empty() {
            return Err(DispatchError::BadInputs(format!(
                "Fix: bellman_tn_order_via n_nodes=0 requires empty dist_init, got {} entries.",
                dist_init.len()
            )));
        }
        out.clear();
        return Ok(());
    }
    if max_iterations == 0 {
        if dist_init.len() != n_nodes as usize {
            return Err(DispatchError::BadInputs(format!(
                "Fix: bellman_tn_order_via expected dist_init length {n_nodes}, got {}.",
                dist_init.len()
            )));
        }
        out.clear();
        out.extend_from_slice(dist_init);
        return Ok(());
    }
    if src.len() != dst.len() || src.len() != weight.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: bellman_tn_order_via requires equal edge buffer lengths, got src={}, dst={}, weight={}.",
            src.len(),
            dst.len(),
            weight.len()
        )));
    }
    if dist_init.len() != n_nodes as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: bellman_tn_order_via expected dist_init length {n_nodes}, got {}.",
            dist_init.len()
        )));
    }
    for (idx, (&u, &v)) in src.iter().zip(dst.iter()).enumerate() {
        if u >= n_nodes || v >= n_nodes {
            return Err(DispatchError::BadInputs(format!(
                "Fix: bellman_tn_order_via edge {idx} has endpoint ({u}->{v}) outside n_nodes {n_nodes}."
            )));
        }
    }
    let n_edges = u32::try_from(src.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: bellman_tn_order_via edge count {} exceeds u32 index space.",
            src.len()
        ))
    })?;
    if n_edges == 0 {
        out.clear();
        out.extend_from_slice(dist_init);
        return Ok(());
    }
    let program = bellman_tn_order_program(
        "src",
        "dst",
        "weight",
        "dist",
        "next_dist",
        "changed",
        n_nodes,
        n_edges,
        max_iterations,
    );
    scratch.changed[0] = 0;
    ensure_input_slots(&mut scratch.inputs, 6);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], dist_init);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], dist_init);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.changed);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], src);
    write_u32_slice_le_bytes(&mut scratch.inputs[4], dst);
    write_u32_slice_le_bytes(&mut scratch.inputs[5], weight);
    let grid_x = ceil_div_u32(n_edges, 256);
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: bellman_tn_order_via expected at least the dist output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], n_nodes as usize, "bellman_tn_order_via", out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_primitives::math::bellman_shortest_path::cpu_ref;

    struct BellmanDispatcher;

    impl OptimizerDispatcher for BellmanDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 6);
            let dist = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let next_dist = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let changed = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
            let src = crate::hardware::dispatch_buffers::read_u32s(&inputs[3]);
            let dst = crate::hardware::dispatch_buffers::read_u32s(&inputs[4]);
            let weight = crate::hardware::dispatch_buffers::read_u32s(&inputs[5]);
            assert_eq!(dist, next_dist);
            assert_eq!(changed, vec![0]);
            let (out, _) = cpu_ref(&src, &dst, &weight, &dist, dist.len() as u32, 10);
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn test_tn_order_program_structure() {
        let p = bellman_tn_order_program("s", "d", "w", "dist", "nd", "c", 8, 12, 5);
        assert_eq!(
            p.buffers().len(),
            6,
            "Must expose 6 buffers for Bellman-Ford"
        );
        assert!(p.buffers().iter().any(|b| b.name() == "dist"));
    }

    #[test]
    fn test_tn_contraction_cost_graph_parity() {
        // Non-trivial vyre IR shape: A chain of 3 tensors (A, B, C)
        // dimensions: A(10x20), B(20x30), C(30x40).
        // States:
        // 0: [A, B, C]
        // 1: [(AB), C]  cost = 10*20*30 = 6000
        // 2: [A, (BC)]  cost = 20*30*40 = 24000
        // 3: [(ABC)] from 1: cost = 10*30*40 = 12000
        // 4: [(ABC)] from 2: cost = 10*20*40 = 8000

        // Edge list (src, dst, weight)
        let src = vec![0, 0, 1, 2];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![6000, 24000, 12000, 8000];

        // 4 nodes, start at 0
        let mut dist = vec![u32::MAX; 4];
        dist[0] = 0; // source is state 0

        let (final_dist, _) = cpu_ref(&src, &dst, &weight, &dist, 4, 10);

        // Optimal path to 3:
        // 0 -> 1 -> 3: 6000 + 12000 = 18000
        // 0 -> 2 -> 3: 24000 + 8000 = 32000
        // So final_dist[3] should be 18000.
        assert_eq!(final_dist[1], 6000);
        assert_eq!(final_dist[2], 24000);
        assert_eq!(final_dist[3], 18000);
    }

    #[test]
    fn test_tn_chain_4_tensors_optimal() {
        // 4 tensors, dimensions: 10, 20, 30, 40, 50
        // We'll mock a small DP graph for Matrix Chain Multiplication.
        // Let nodes be represented by intervals [i, j].
        // Node 0: start, Node 1: ends. Just some mock topology.
        let src = vec![0, 0, 0, 1, 2, 3];
        let dst = vec![1, 2, 3, 4, 4, 4];
        let weight = vec![100, 200, 300, 50, 40, 10]; // mock costs

        let mut dist = vec![u32::MAX; 5];
        dist[0] = 0;

        let (final_dist, _) = cpu_ref(&src, &dst, &weight, &dist, 5, 10);

        // 0->1->4 (150)
        // 0->2->4 (240)
        // 0->3->4 (310)
        assert_eq!(final_dist[4], 150);
    }

    #[test]
    fn test_multi_stage_order_refining() {
        // Build a Program with 3 separate Bellman regions.
        let p1 = bellman_tn_order_program("s", "d", "w", "dist1", "nd1", "c1", 4, 4, 5);
        let p2 = bellman_tn_order_program("s", "d", "w", "dist2", "nd2", "c2", 4, 4, 5);
        let p3 = bellman_tn_order_program("s", "d", "w", "dist3", "nd3", "c3", 4, 4, 5);

        let final_p = crate::test_support::wrap_program_sequence(&[&p1, &p2, &p3], [256, 1, 1]);
        // Assert we have at least 3 regions
        let region_count = final_p
            .entry()
            .iter()
            .filter(|n| matches!(n, vyre_foundation::ir::Node::Region { .. }))
            .count();
        assert!(region_count >= 3);
    }

    #[test]
    fn test_end_to_end_tn_parity() {
        // Same shape as `vyre_primitives::math::bellman_shortest_path::tests::test_parity_small_graph`.
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist_init = vec![0, u32::MAX, u32::MAX, u32::MAX];

        let p = bellman_tn_order_program("s", "d", "w", "dist", "nd", "c", 4, 4, 10);

        let (expected_dist, _) = cpu_ref(&src, &dst, &weight, &dist_init, 4, 10);

        use std::sync::Arc;
        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[u32]| {
            let bytes = vyre_primitives::wire::pack_u32_slice(data);
            Value::Bytes(Arc::from(bytes))
        };

        let inputs = vec![
            to_value(&dist_init),
            to_value(&dist_init),
            to_value(&[0]),
            to_value(&src),
            to_value(&dst),
            to_value(&weight),
        ];

        let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
        let actual_bytes = results[0].to_bytes();
        let actual_dist: Vec<u32> = actual_bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(actual_dist, expected_dist);
    }

    #[test]
    fn bellman_tn_order_via_dispatches_primitive() {
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist_init = vec![0, u32::MAX, u32::MAX, u32::MAX];

        let out = bellman_tn_order_via(&BellmanDispatcher, &src, &dst, &weight, &dist_init, 4, 10)
            .unwrap();

        assert_eq!(out, vec![0, 10, 30, 60]);
    }

    #[test]
    fn bellman_tn_order_via_into_reuses_output() {
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist_init = vec![0, u32::MAX, u32::MAX, u32::MAX];
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();

        bellman_tn_order_via_into(
            &BellmanDispatcher,
            &src,
            &dst,
            &weight,
            &dist_init,
            4,
            10,
            &mut out,
        )
        .unwrap();

        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out, vec![0, 10, 30, 60]);
    }

    #[test]
    fn bellman_tn_order_via_with_scratch_reuses_dispatch_and_output_storage() {
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist_init = vec![0, u32::MAX, u32::MAX, u32::MAX];
        let mut scratch = BellmanTnOrderGpuScratch::default();
        let mut out = Vec::with_capacity(4);

        bellman_tn_order_via_with_scratch_into(
            &BellmanDispatcher,
            &src,
            &dst,
            &weight,
            &dist_init,
            4,
            10,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let out_capacity = out.capacity();

        bellman_tn_order_via_with_scratch_into(
            &BellmanDispatcher,
            &src,
            &dst,
            &weight,
            &dist_init,
            4,
            10,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(out, vec![0, 10, 30, 60]);
    }

    #[test]
    fn bellman_tn_order_via_rejects_bad_edge_shape() {
        let err =
            bellman_tn_order_via(&BellmanDispatcher, &[0], &[], &[1], &[0], 1, 10).unwrap_err();

        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn bellman_tn_order_via_empty_edges_returns_initial_dist_without_dispatch() {
        struct NoDispatch;

        impl OptimizerDispatcher for NoDispatch {
            fn dispatch(
                &self,
                _program: &Program,
                _inputs: &[Vec<u8>],
                _grid_override: Option<[u32; 3]>,
            ) -> Result<Vec<Vec<u8>>, DispatchError> {
                panic!("Fix: empty Bellman edge set must not submit a zero-work GPU dispatch");
            }
        }

        let mut out = Vec::with_capacity(8);
        bellman_tn_order_via_into(&NoDispatch, &[], &[], &[], &[0, u32::MAX], 2, 10, &mut out)
            .expect("Fix: empty Bellman edge set must return the initial distances");
        assert_eq!(out, vec![0, u32::MAX]);
    }
}
