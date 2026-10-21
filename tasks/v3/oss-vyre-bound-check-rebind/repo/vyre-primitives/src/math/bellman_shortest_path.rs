use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::bellman_shortest_path";

/// Build a fused Bellman-Ford shortest-path Program: relax edges
/// until convergence, all inside ONE GPU dispatch.
///
/// Composes `persistent_fixpoint` over an edge list to perform
/// graph distances without host round-trips.
///
/// Invalid dimensions lower to an explicit trap program.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn bellman_shortest_path(
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
    if n_nodes == 0 {
        return crate::invalid_output_program(
            OP_ID,
            dist,
            DataType::U32,
            format!("Fix: bellman_shortest_path requires n_nodes > 0, got {n_nodes}."),
        );
    }
    if max_iterations == 0 {
        return crate::invalid_output_program(
            OP_ID,
            dist,
            DataType::U32,
            format!(
                "Fix: bellman_shortest_path requires max_iterations > 0, got {max_iterations}."
            ),
        );
    }

    let t = Expr::InvocationId { axis: 0 };

    let transfer_body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n_edges)),
        vec![
            Node::let_bind("u", Expr::load(src, t.clone())),
            Node::let_bind("v", Expr::load(dst, t.clone())),
            Node::let_bind("w", Expr::load(weight, t.clone())),
            Node::let_bind("du", Expr::load(dist, Expr::var("u"))),
            Node::if_then(
                Expr::ne(Expr::var("du"), Expr::u32(u32::MAX)),
                vec![
                    Node::let_bind(
                        "alt",
                        Expr::select(
                            Expr::gt(
                                Expr::var("w"),
                                Expr::sub(Expr::u32(u32::MAX), Expr::var("du")),
                            ),
                            Expr::u32(u32::MAX),
                            Expr::add(Expr::var("du"), Expr::var("w")),
                        ),
                    ),
                    Node::let_bind(
                        "_relax",
                        Expr::atomic_min(next_dist, Expr::var("v"), Expr::var("alt")),
                    ),
                ],
            ),
        ],
    )];

    let inner = crate::fixpoint::persistent_fixpoint::persistent_fixpoint(
        transfer_body,
        dist,
        next_dist,
        changed,
        n_nodes,
        max_iterations,
    );

    let entry: Vec<Node> = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(inner.entry().to_vec()),
    }];

    Program::wrapped(
        vec![
            BufferDecl::storage(dist, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(next_dist, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_nodes),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(src, 3, BufferAccess::ReadOnly, DataType::U32).with_count(n_edges),
            BufferDecl::storage(dst, 4, BufferAccess::ReadOnly, DataType::U32).with_count(n_edges),
            BufferDecl::storage(weight, 5, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_edges),
        ],
        [256, 1, 1],
        entry,
    )
}

/// CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn cpu_ref(
    src: &[u32],
    dst: &[u32],
    weight: &[u32],
    dist: &[u32],
    n_nodes: u32,
    max_iterations: u32,
) -> (Vec<u32>, u32) {
    let mut current = Vec::new();
    let mut next = Vec::new();
    let iters = cpu_ref_into(
        src,
        dst,
        weight,
        dist,
        n_nodes,
        max_iterations,
        &mut current,
        &mut next,
    );
    (current, iters)
}

/// CPU reference using caller-owned current and next-distance buffers.
///
/// `current` is overwritten with the final distance vector. `next` is retained
/// as monotone relaxation scratch so repeated parity checks do not allocate
/// fresh `Vec`s or clone the initial distance vector.
#[cfg(any(test, feature = "cpu-parity"))]
#[allow(clippy::too_many_arguments)]
pub fn cpu_ref_into(
    src: &[u32],
    dst: &[u32],
    weight: &[u32],
    dist: &[u32],
    n_nodes: u32,
    max_iterations: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> u32 {
    let n = n_nodes as usize;
    let edge_count = src.len().min(dst.len()).min(weight.len());
    current.clear();
    current.resize(n, u32::MAX);
    for (out, &value) in current.iter_mut().zip(dist.iter()) {
        *out = value;
    }
    next.clear();
    next.extend_from_slice(current);
    for iter in 0..max_iterations {
        for i in 0..edge_count {
            let u = src[i] as usize;
            let v = dst[i] as usize;
            if u >= n || v >= n {
                continue;
            }
            let w = weight[i];
            let du = current[u];
            if du != u32::MAX {
                let alt = du.saturating_add(w);
                next[v] = next[v].min(alt);
            }
        }
        if next.as_slice() == current.as_slice() {
            return iter;
        }
        current.copy_from_slice(&next);
    }
    max_iterations
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bellman_shortest_path("src", "dst", "weight", "dist", "next_dist", "changed", 4, 4, 10),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, u32::MAX, u32::MAX, u32::MAX]), // dist
                to_bytes(&[0, u32::MAX, u32::MAX, u32::MAX]), // next_dist
                to_bytes(&[0]), // changed
                to_bytes(&[0, 1, 2, 0]), // src
                to_bytes(&[1, 2, 3, 3]), // dst
                to_bytes(&[10, 20, 30, 100]), // weight
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 10, 30, 60]), // dist
                to_bytes(&[0, 10, 30, 60]), // next_dist
                to_bytes(&[0]),             // changed
            ]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_ref_trivial() {
        let src = vec![0];
        let dst = vec![1];
        let weight = vec![5];
        let dist = vec![0, u32::MAX];
        let (final_dist, iters) = cpu_ref(&src, &dst, &weight, &dist, 2, 10);
        assert_eq!(final_dist, vec![0, 5]);
        assert_eq!(iters, 1);
    }

    #[test]
    fn test_cpu_ref_single_node() {
        let dist = vec![0];
        let (final_dist, iters) = cpu_ref(&[], &[], &[], &dist, 1, 10);
        assert_eq!(final_dist, vec![0]);
        assert_eq!(iters, 0);
    }

    #[test]
    fn test_cpu_ref_cycle() {
        let src = vec![0, 1, 2];
        let dst = vec![1, 2, 0];
        let weight = vec![10, 10, 10];
        let dist = vec![0, u32::MAX, u32::MAX];
        let (final_dist, _) = cpu_ref(&src, &dst, &weight, &dist, 3, 10);
        assert_eq!(final_dist, vec![0, 10, 20]);
    }

    #[test]
    fn test_cpu_ref_large_line() {
        let n = 50;
        let mut src = Vec::new();
        let mut dst = Vec::new();
        let mut weight = Vec::new();
        for i in 0..n - 1 {
            src.push(i as u32);
            dst.push((i + 1) as u32);
            weight.push(1);
        }
        let mut dist = vec![u32::MAX; n];
        dist[0] = 0;
        let (final_dist, iters) = cpu_ref(&src, &dst, &weight, &dist, n as u32, n as u32 * 2);
        assert_eq!(final_dist[n - 1], (n - 1) as u32);
        assert_eq!(iters, (n - 1) as u32);
    }

    #[test]
    fn test_cpu_ref_asymmetric() {
        let src = vec![0, 0, 1, 2];
        let dst = vec![1, 3, 3, 3];
        let weight = vec![10, 100, 20, 5];
        let dist = vec![0, u32::MAX, u32::MAX, u32::MAX];
        // 0->3 is 100
        // 0->1->3 is 10+20=30
        let (final_dist, _) = cpu_ref(&src, &dst, &weight, &dist, 4, 10);
        assert_eq!(final_dist[3], 30);
    }

    #[test]
    fn test_cpu_ref_ignores_malformed_edges_and_pads_distances() {
        let src = vec![0, 9, 1];
        let dst = vec![1, 2];
        let weight = vec![5, 99, 7];
        let (final_dist, _) = cpu_ref(&src, &dst, &weight, &[0], 3, 10);
        assert_eq!(final_dist, vec![0, 5, u32::MAX]);
    }

    #[test]
    fn cpu_ref_into_reuses_current_and_next_buffers() {
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist = vec![0, u32::MAX, u32::MAX, u32::MAX];
        let mut current = Vec::with_capacity(16);
        let mut next = Vec::with_capacity(16);
        current.extend_from_slice(&[99, 98, 97, 96, 95, 94]);
        next.extend_from_slice(&[77, 76, 75, 74, 73, 72]);
        let current_capacity = current.capacity();
        let next_capacity = next.capacity();

        let iters = cpu_ref_into(&src, &dst, &weight, &dist, 4, 10, &mut current, &mut next);

        assert_eq!(current, vec![0, 10, 30, 60]);
        assert!(iters <= 4);
        assert_eq!(current.capacity(), current_capacity);
        assert_eq!(next.capacity(), next_capacity);

        let iters = cpu_ref_into(&[], &[], &[], &[0], 1, 10, &mut current, &mut next);
        assert_eq!(current, vec![0]);
        assert_eq!(next, vec![0]);
        assert_eq!(iters, 0);
        assert_eq!(current.capacity(), current_capacity);
        assert_eq!(next.capacity(), next_capacity);
    }

    #[test]
    fn test_parity_small_graph() {
        let src = vec![0, 1, 2, 0];
        let dst = vec![1, 2, 3, 3];
        let weight = vec![10, 20, 30, 100];
        let dist_init = vec![0, u32::MAX, u32::MAX, u32::MAX];

        let p = bellman_shortest_path(
            "src",
            "dst",
            "weight",
            "dist",
            "next_dist",
            "changed",
            4,
            4,
            10,
        );

        let (expected_dist, _) = cpu_ref(&src, &dst, &weight, &dist_init, 4, 10);

        use vyre_reference::reference_eval;
        use vyre_reference::value::Value;

        let to_value = |data: &[u32]| {
            let bytes = crate::wire::pack_u32_slice(data);
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
    fn program_declares_six_buffers() {
        let p = bellman_shortest_path("s", "d", "w", "di", "nd", "c", 4, 4, 10);
        assert_eq!(p.buffers().len(), 6);
    }

    #[test]
    fn rejects_zero_nodes_with_trap() {
        let p = bellman_shortest_path("s", "d", "w", "di", "nd", "c", 0, 4, 10);
        assert!(p.stats().trap());
    }

    #[test]
    fn rejects_zero_max_iterations_with_trap() {
        let p = bellman_shortest_path("s", "d", "w", "di", "nd", "c", 4, 4, 0);
        assert!(p.stats().trap());
    }
}
