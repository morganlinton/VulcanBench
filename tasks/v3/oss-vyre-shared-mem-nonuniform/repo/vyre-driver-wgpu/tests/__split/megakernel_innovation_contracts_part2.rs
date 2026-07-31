use super::*;

#[test]
fn batch_dispatcher_isolates_invalid_rules_and_runs_rest() {
    let backend = vyre_driver_wgpu::WgpuBackend::new()
        .expect("live GPU backend must initialize for megakernel batch contract");
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 4,
        hit_capacity: 1024,
        timeout: Duration::from_secs(10),
        ..Default::default()
    };
    let mut dispatcher = BatchDispatcher::new(backend.clone(), config)
        .expect("BatchDispatcher construction must compile the live batch megakernel");

    let valid = BatchRuleProgram::new(0, vec![0; 256], vec![0], 1).unwrap();

    // Duplicate of rule_idx 0
    let duplicate = BatchRuleProgram {
        rule_idx: 0,
        transitions: vec![0; 256],
        accept: vec![0],
        state_count: 1,
    };

    // Out of range
    let out_of_range = BatchRuleProgram {
        rule_idx: 999_999,
        transitions: vec![0; 256],
        accept: vec![0],
        state_count: 1,
    };

    // Malformed shape (transitions should be 256 words for state_count=1)
    let invalid_shape = BatchRuleProgram {
        rule_idx: 1,
        transitions: vec![0; 128],
        accept: vec![0; 1],
        state_count: 1,
    };

    let files = vec![BatchFile::new(1, 0, b"scanme".to_vec())];
    let batch = FileBatch::upload(backend.device_queue(), &files, 4, 1024)
        .expect("FileBatch must upload on live adapter");

    let report = dispatcher
        .dispatch(&batch, &[valid, duplicate, out_of_range, invalid_shape])
        .expect("dispatch must complete without backend panic");

    // Collect rejection reasons
    let reasons: Vec<String> = report
        .rejected_rules
        .iter()
        .map(|r| r.reason.clone())
        .collect();

    assert!(
        reasons.iter().any(|r| r.contains("duplicate")),
        "duplicate rule must be rejected: {reasons:?}"
    );
    assert!(
        reasons.iter().any(|r| r.contains("outside")),
        "out-of-range rule must be rejected: {reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("transition") || r.contains("accept")),
        "invalid shape must be rejected: {reasons:?}"
    );

    // The dispatch itself must have completed and measured wall time.
    assert!(
        report.wall_time > Duration::ZERO,
        "dispatch must have consumed non-zero wall time"
    );
}

// ---------------------------------------------------------------------------
// 6. Adaptive launch consumes backend limits
// ---------------------------------------------------------------------------

#[test]
fn adaptive_launch_derives_worker_groups_from_limits_when_zero() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 100,
            requested_worker_groups: 0, // sentinel: derive from limits
            max_workgroup_size_x: 256,
            max_compute_workgroups_per_dimension: 16,
            max_compute_invocations_per_workgroup: 256,
            requested_hit_capacity: 0,
            expected_hits_per_item: 1,
            hot_opcode_count: 0,
            hot_window_count: 0,
            requeue_count: 0,
            max_priority_age: 0,
            graph_node_count: 0,
            graph_edge_count: 0,
            frontier_density_bps: 0,
            memory_pressure_bps: 0,
            resident_device_bytes: 0,
            device_memory_budget_bytes: 0,
        })
        .expect("must produce recommendation for zero worker_groups");
    assert!(
        rec.worker_groups > 0,
        "policy must derive positive worker_groups from adapter limits"
    );
    assert!(
        rec.worker_groups <= 16,
        "derived worker_groups {} must not exceed adapter limit 16",
        rec.worker_groups
    );
}

#[test]
fn adaptive_launch_dispatch_grid_respects_adapter_limit() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 10_000,
            requested_worker_groups: 128,
            max_workgroup_size_x: 256,
            max_compute_workgroups_per_dimension: 4,
            max_compute_invocations_per_workgroup: 256,
            requested_hit_capacity: 0,
            expected_hits_per_item: 1,
            hot_opcode_count: 0,
            hot_window_count: 0,
            requeue_count: 0,
            max_priority_age: 0,
            graph_node_count: 0,
            graph_edge_count: 0,
            frontier_density_bps: 0,
            memory_pressure_bps: 0,
            resident_device_bytes: 0,
            device_memory_budget_bytes: 0,
        })
        .expect("must produce recommendation");

    // The dispatch grid X must never exceed the adapter's per-dimension limit.
    assert!(
        rec.geometry.dispatch_grid[0] <= 4,
        "dispatch_grid[0] = {} must respect max_compute_workgroups_per_dimension = 4",
        rec.geometry.dispatch_grid[0]
    );
}

#[test]
fn adaptive_launch_selects_jit_for_large_queues_or_hot_opcodes() {
    let policy = MegakernelLaunchPolicy::standard();
    let rec = policy
        .recommend(MegakernelLaunchRequest {
            queue_len: 8192,
            requested_worker_groups: 64,
            max_workgroup_size_x: 256,
            max_compute_workgroups_per_dimension: 65_536,
            max_compute_invocations_per_workgroup: 256,
            requested_hit_capacity: 0,
            expected_hits_per_item: 1,
            hot_opcode_count: 8,
            hot_window_count: 0,
            requeue_count: 0,
            max_priority_age: 0,
            graph_node_count: 0,
            graph_edge_count: 0,
            frontier_density_bps: 0,
            memory_pressure_bps: 0,
            resident_device_bytes: 0,
            device_memory_budget_bytes: 0,
        })
        .expect("must produce recommendation");
    assert_eq!(
        rec.execution_mode,
        MegakernelExecutionMode::Jit,
        "large queue with hot opcodes must select JIT execution mode"
    );
}

#[test]
fn batch_dispatcher_new_consumes_launch_recommendation_in_source() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/megakernel/dispatcher.rs"
    ))
    .expect("dispatcher source must be readable");
    let prod = src.split("#[cfg(test)]").next().unwrap_or(&src);

    assert!(
        prod.contains("config.worker_groups = launch.worker_groups"),
        "BatchDispatcher::new must consume launch.worker_groups into config"
    );
    assert!(
        prod.contains("config.hit_capacity = launch.hit_capacity"),
        "BatchDispatcher::new must consume launch.hit_capacity into config"
    );
}

#[test]
fn batch_dispatch_config_launch_recommendation_produces_valid_geometry() {
    let limits = wgpu::Limits::default();
    let config = BatchDispatchConfig::default();
    let rec = config
        .launch_recommendation(&limits, 128)
        .expect("default config must produce valid recommendation against default limits");
    assert!(rec.worker_groups >= 1);
    assert!(rec.hit_capacity >= 1024);
    assert!(rec.geometry.workgroup_size_x >= 1);
    assert!(rec.geometry.slot_count >= rec.geometry.workgroup_size_x);
}
