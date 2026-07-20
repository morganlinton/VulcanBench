use super::*;

#[test]
fn stable_recommendation_holds_sparse_topology_inside_frontier_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest {
        queue_len: 8_192,
        requested_worker_groups: 128,
        max_workgroup_size_x: 256,
        graph_node_count: 100_000,
        graph_edge_count: 250_000,
        frontier_density_bps: policy.sparse_frontier_threshold_bps + 125,
        ..MegakernelLaunchRequest::direct(8_192, 128, 256)
    };
    let stateless = policy
        .recommend(request)
        .expect("Fix: stateless launch recommendation should accept valid adapter limits");
    let stable = policy
        .recommend_with_previous_topology(request, MegakernelDispatchTopology::SparseFrontier)
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(
        stateless.topology,
        MegakernelDispatchTopology::HybridFrontier
    );
    assert_eq!(stable.topology, MegakernelDispatchTopology::SparseFrontier);
}

#[test]
fn stable_recommendation_releases_sparse_topology_outside_frontier_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend_with_previous_topology(
            MegakernelLaunchRequest {
                queue_len: 8_192,
                requested_worker_groups: 128,
                max_workgroup_size_x: 256,
                graph_node_count: 100_000,
                graph_edge_count: 250_000,
                frontier_density_bps: policy.sparse_frontier_threshold_bps + 300,
                ..MegakernelLaunchRequest::direct(8_192, 128, 256)
            },
            MegakernelDispatchTopology::SparseFrontier,
        )
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::HybridFrontier);
}

#[test]
fn stable_recommendation_holds_hybrid_topology_inside_sparse_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend_with_previous_topology(
            MegakernelLaunchRequest {
                queue_len: 8_192,
                requested_worker_groups: 128,
                max_workgroup_size_x: 256,
                graph_node_count: 100_000,
                graph_edge_count: 250_000,
                frontier_density_bps: policy.sparse_frontier_threshold_bps - 125,
                ..MegakernelLaunchRequest::direct(8_192, 128, 256)
            },
            MegakernelDispatchTopology::HybridFrontier,
        )
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::HybridFrontier);
}

#[test]
fn stable_recommendation_holds_hybrid_topology_inside_dense_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend_with_previous_topology(
            MegakernelLaunchRequest {
                queue_len: 16_384,
                requested_worker_groups: 128,
                max_workgroup_size_x: 256,
                graph_node_count: 16_384,
                graph_edge_count: 250_000,
                frontier_density_bps: policy.dense_frontier_threshold_bps + 125,
                ..MegakernelLaunchRequest::direct(16_384, 128, 256)
            },
            MegakernelDispatchTopology::HybridFrontier,
        )
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::HybridFrontier);
}

#[test]
fn stable_recommendation_holds_dense_topology_inside_frontier_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest {
        queue_len: 16_384,
        requested_worker_groups: 128,
        max_workgroup_size_x: 256,
        graph_node_count: 16_384,
        graph_edge_count: 250_000,
        frontier_density_bps: policy.dense_frontier_threshold_bps - 125,
        ..MegakernelLaunchRequest::direct(16_384, 128, 256)
    };
    let stateless = policy
        .recommend(request)
        .expect("Fix: stateless launch recommendation should accept valid adapter limits");
    let stable = policy
        .recommend_with_previous_topology(request, MegakernelDispatchTopology::DenseFrontier)
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(
        stateless.topology,
        MegakernelDispatchTopology::HybridFrontier
    );
    assert_eq!(stable.topology, MegakernelDispatchTopology::DenseFrontier);
}

#[test]
fn stable_recommendation_preserves_fused_dense_when_hot_graph_stays_near_dense() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend_with_previous_topology(
            MegakernelLaunchRequest {
                queue_len: 131_072,
                requested_worker_groups: 256,
                max_workgroup_size_x: 256,
                graph_node_count: 32_768,
                graph_edge_count: 500_000,
                frontier_density_bps: policy.dense_frontier_threshold_bps - 125,
                hot_window_count: policy.hot_window_threshold,
                ..MegakernelLaunchRequest::direct(131_072, 256, 256)
            },
            MegakernelDispatchTopology::FusedDense,
        )
        .expect("Fix: stable fused dense recommendation should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
}

#[test]
fn stable_recommendation_holds_memory_constrained_topology_inside_pressure_hysteresis() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest {
        queue_len: 16_384,
        requested_worker_groups: 128,
        max_workgroup_size_x: 256,
        graph_node_count: 16_384,
        graph_edge_count: 250_000,
        frontier_density_bps: 9_000,
        memory_pressure_bps: policy.memory_pressure_threshold_bps - 125,
        ..MegakernelLaunchRequest::direct(16_384, 128, 256)
    };
    let stateless = policy
        .recommend(request)
        .expect("Fix: stateless launch recommendation should accept valid adapter limits");
    let stable = policy
        .recommend_with_previous_topology(request, MegakernelDispatchTopology::MemoryConstrained)
        .expect("Fix: stable launch recommendation should accept valid adapter limits");

    assert_eq!(
        stateless.topology,
        MegakernelDispatchTopology::DenseFrontier
    );
    assert_eq!(
        stable.topology,
        MegakernelDispatchTopology::MemoryConstrained
    );
    assert!(
        stable.worker_groups < stateless.worker_groups,
        "stable memory-constrained topology must preserve worker shedding near pressure threshold"
    );
}
