use super::*;

#[test]
fn policy_recommends_padded_geometry_and_hit_capacity() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 300,
            requested_worker_groups: 64,
            max_workgroup_size_x: 256,
            requested_hit_capacity: 0,
            expected_hits_per_item: 3,
            ..MegakernelLaunchRequest::direct(300, 64, 256)
        })
        .expect("Fix: policy should accept non-zero adapter limits");
    assert_eq!(rec.geometry.workgroup_size_x, 64);
    assert_eq!(rec.geometry.slot_count, 320);
    assert_eq!(rec.geometry.dispatch_grid, [5, 1, 1]);
    assert_eq!(rec.hit_capacity, 1800);
    assert_eq!(rec.estimated_peak_device_bytes, 28_800);
    assert_eq!(rec.device_memory_budget_bytes, 0);
    assert_eq!(rec.topology, MegakernelDispatchTopology::SparseFrontier);
}

#[test]
fn telemetry_pressure_selects_jit_and_priority_aging() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 8192,
            requested_worker_groups: 64,
            max_workgroup_size_x: 256,
            hot_opcode_count: 8,
            requeue_count: 1,
            max_priority_age: 64,
            ..MegakernelLaunchRequest::direct(8192, 64, 256)
        })
        .expect("Fix: policy should accept non-zero adapter limits");
    assert_eq!(rec.pressure, MegakernelQueuePressure::Saturated);
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
    assert_eq!(rec.topology, MegakernelDispatchTopology::SparseFrontier);
    assert!(rec.promote_hot_opcodes);
    assert!(rec.age_priority_work);
}

#[test]
fn dense_large_hot_graph_selects_fused_dense_topology() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 131_072,
            requested_worker_groups: 256,
            max_workgroup_size_x: 256,
            graph_node_count: 32_768,
            graph_edge_count: 500_000,
            frontier_density_bps: 7_500,
            hot_window_count: policy.hot_window_threshold,
            ..MegakernelLaunchRequest::direct(131_072, 256, 256)
        })
        .expect("Fix: fused dense topology should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
}

#[test]
fn topology_evidence_reports_graphblas_switch_inputs_and_parity_contract() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest {
        queue_len: 131_072,
        requested_worker_groups: 256,
        max_workgroup_size_x: 256,
        graph_node_count: 32_768,
        graph_edge_count: 500_000,
        frontier_density_bps: 7_500,
        hot_window_count: policy.hot_window_threshold,
        resident_device_bytes: 64 * 1024 * 1024,
        ..MegakernelLaunchRequest::direct(131_072, 256, 256)
    };
    let (rec, evidence) = policy
        .recommend_with_topology_evidence(request)
        .expect("Fix: topology evidence should be emitted for valid launch telemetry");

    assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
    assert_eq!(evidence.schema_version, TOPOLOGY_EVIDENCE_SCHEMA_VERSION);
    assert_eq!(evidence.selected_topology, rec.topology);
    assert_eq!(evidence.queue_pressure, rec.pressure);
    assert_eq!(evidence.frontier_density_bps, 7_500);
    assert_eq!(evidence.semiring_frontier_density_bps, 7_500);
    assert_eq!(
        evidence.graphblas_switch_class,
        MegakernelGraphBlasSwitchClass::Dense
    );
    assert_eq!(evidence.resident_device_bytes, 64 * 1024 * 1024);
    assert_eq!(
        evidence.estimated_peak_device_bytes,
        rec.estimated_peak_device_bytes
    );
    assert!(evidence.output_parity_required);
    assert!(evidence.is_complete());
}

#[test]
fn promotion_evidence_reports_fused_window_lowerer_contract() {
    let policy = MegakernelLaunchPolicy::standard();
    let request = MegakernelLaunchRequest {
        queue_len: 1024,
        requested_worker_groups: 64,
        max_workgroup_size_x: 256,
        hot_window_count: policy.hot_window_threshold,
        ..MegakernelLaunchRequest::direct(1024, 64, 256)
    };
    let (rec, evidence) = policy
        .recommend_with_promotion_evidence(request)
        .expect("Fix: promotion evidence should be emitted for valid hot-window telemetry");

    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
    assert!(rec.promote_hot_windows);
    assert_eq!(
        evidence.schema_version,
        HOT_WINDOW_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    assert_eq!(evidence.queue_len, 1024);
    assert_eq!(evidence.hot_window_count, policy.hot_window_threshold);
    assert_eq!(evidence.hot_window_threshold, policy.hot_window_threshold);
    assert_eq!(evidence.hot_opcode_count, 0);
    assert_eq!(evidence.hot_opcode_threshold, policy.hot_opcode_threshold);
    assert_eq!(evidence.execution_mode, MegakernelExecutionMode::Jit);
    assert_eq!(evidence.promotion_route, MegakernelPromotionRoute::WindowJit);
    assert!(evidence.promote_hot_windows);
    assert!(!evidence.promote_hot_opcodes);
    assert!(evidence.fused_descriptor_window_required);
    assert!(evidence.output_parity_required);
    assert!(evidence.is_complete());
}

#[test]
fn high_memory_pressure_overrides_dense_frontier() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 16_384,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            graph_node_count: 16_384,
            graph_edge_count: 250_000,
            frontier_density_bps: 9_000,
            memory_pressure_bps: policy.memory_pressure_threshold_bps,
            ..MegakernelLaunchRequest::direct(16_384, 128, 256)
        })
        .expect("Fix: memory-constrained topology should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::MemoryConstrained);
    assert!(
        rec.worker_groups < 128,
        "memory-constrained topology must lower worker-group pressure, got {}",
        rec.worker_groups
    );
    assert_eq!(
        rec.hit_capacity, 16_384,
        "memory-constrained topology must avoid the normal sparse-hit over-allocation multiplier"
    );
}

#[test]
fn explicit_hit_capacity_survives_memory_constrained_worker_shedding() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 16_384,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            requested_hit_capacity: 65_536,
            memory_pressure_bps: 10_000,
            ..MegakernelLaunchRequest::direct(16_384, 128, 256)
        })
        .expect(
            "Fix: memory-constrained explicit-capacity launch should accept valid adapter limits",
        );

    assert_eq!(rec.topology, MegakernelDispatchTopology::MemoryConstrained);
    assert_eq!(rec.hit_capacity, 65_536);
    assert_eq!(rec.worker_groups, 64);
}

#[test]
fn device_memory_budget_rejects_oversized_hit_plan_before_allocation() {
    let policy = MegakernelLaunchPolicy::standard();
    let err = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 1024,
            requested_worker_groups: 64,
            max_workgroup_size_x: 256,
            expected_hits_per_item: 4,
            resident_device_bytes: 1024,
            device_memory_budget_bytes: 64 * 1024,
            ..MegakernelLaunchRequest::direct(1024, 64, 256)
        })
        .expect_err("Fix: launch policy must reject plans that exceed explicit device budget");

    match err {
        vyre_driver::backend::BackendError::DeviceOutOfMemory {
            requested,
            available,
        } => {
            assert_eq!(requested, 132_096);
            assert_eq!(available, 64 * 1024);
        }
        other => panic!("expected DeviceOutOfMemory for budget overflow, got {other:?}"),
    }
}

#[test]
fn device_memory_budget_infers_pressure_without_manual_bps() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 1024,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            resident_device_bytes: 900_000,
            device_memory_budget_bytes: 1_000_000,
            ..MegakernelLaunchRequest::direct(1024, 128, 256)
        })
        .expect("Fix: budget-aware policy should accept launches under the byte budget");

    assert_eq!(rec.topology, MegakernelDispatchTopology::MemoryConstrained);
    assert!(
        rec.worker_groups < 128,
        "inferred memory pressure must shed worker groups before launch"
    );
    assert_eq!(rec.estimated_peak_device_bytes, 916_384);
    assert_eq!(rec.device_memory_budget_bytes, 1_000_000);
}

#[test]
fn dense_frontier_without_hot_fusion_stays_dense() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 16_384,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            graph_node_count: 16_384,
            graph_edge_count: 250_000,
            frontier_density_bps: policy.dense_frontier_threshold_bps,
            ..MegakernelLaunchRequest::direct(16_384, 128, 256)
        })
        .expect("Fix: dense topology should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::DenseFrontier);
}

#[test]
fn mid_density_frontier_selects_hybrid_topology() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 8192,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            graph_node_count: 8192,
            graph_edge_count: 32_768,
            frontier_density_bps: policy.sparse_frontier_threshold_bps + 1,
            ..MegakernelLaunchRequest::direct(8192, 128, 256)
        })
        .expect("Fix: hybrid topology should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::HybridFrontier);
}

#[test]
fn missing_frontier_telemetry_infers_density_from_queue_and_graph_scale() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 90_000,
            requested_worker_groups: 256,
            max_workgroup_size_x: 256,
            graph_node_count: 100_000,
            graph_edge_count: 750_000,
            hot_opcode_count: policy.hot_opcode_threshold,
            frontier_density_bps: 0,
            ..MegakernelLaunchRequest::direct(90_000, 256, 256)
        })
        .expect("Fix: inferred-density topology should accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
    assert_eq!(rec.execution_mode, MegakernelExecutionMode::Jit);
}

#[test]
fn sparse_frontier_density_sheds_worker_pressure_without_losing_warp_floor() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 100_000,
            requested_worker_groups: 256,
            max_workgroup_size_x: 256,
            graph_node_count: 1_000_000,
            graph_edge_count: 4_000_000,
            frontier_density_bps: 100,
            ..MegakernelLaunchRequest::direct(100_000, 256, 256)
        })
        .expect("Fix: sparse density worker shedding must accept valid adapter limits");

    assert_eq!(rec.topology, MegakernelDispatchTopology::SparseFrontier);
    assert_eq!(rec.worker_groups, 51);
    assert_eq!(rec.geometry.workgroup_size_x, 51);
    assert_eq!(rec.geometry.dispatch_grid, [51, 1, 1]);
}

#[test]
fn sparse_frontier_worker_shedding_preserves_warp_floor_for_tiny_density() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 1_000,
            requested_worker_groups: 256,
            max_workgroup_size_x: 256,
            graph_node_count: 1_000_000,
            graph_edge_count: 4_000_000,
            frontier_density_bps: 1,
            ..MegakernelLaunchRequest::direct(1_000, 256, 256)
        })
        .expect("Fix: sparse density worker shedding must retain a useful GPU width");

    assert_eq!(rec.topology, MegakernelDispatchTopology::SparseFrontier);
    assert_eq!(rec.worker_groups, 32);
    assert_eq!(rec.geometry.workgroup_size_x, 32);
}
