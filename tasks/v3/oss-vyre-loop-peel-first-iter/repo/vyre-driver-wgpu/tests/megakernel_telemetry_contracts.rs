//! Megakernel telemetry and hot-plan cache contracts for the WGPU backend.

#![cfg(feature = "megakernel-batch")]

use std::time::Duration;
use vyre_driver_wgpu::megakernel::{
    BatchDispatchConfig, BatchDispatchTelemetry, BatchDispatcher, BatchFile, FileBatch,
};
use vyre_driver_wgpu::WgpuBackend;
use vyre_runtime::megakernel::{BatchRuleProgram, MegakernelDispatchTopology};

#[test]
fn megakernel_dispatch_telemetry_is_public_and_complete() {
    let telemetry = BatchDispatchTelemetry::default();
    assert_eq!(telemetry.bytes_uploaded, 0);
    assert_eq!(telemetry.bytes_read_back, 0);
    assert_eq!(telemetry.bytes_moved, 0);
    assert_eq!(telemetry.resident_allocations, 0);
    assert_eq!(telemetry.kernel_launches, 0);
    assert_eq!(telemetry.sync_points, 0);
    assert_eq!(telemetry.occupancy_proxy_bps, 0);
    assert_eq!(telemetry.frontier_density_bps, 0);
    assert_eq!(telemetry.queue_state_readback_bytes, 0);
    assert_eq!(telemetry.hit_readback_bytes, 0);
    assert!(!telemetry.dispatch_plan_cache_hit);
    assert_eq!(telemetry.dispatch_plan_cache_entries, 0);
}

#[test]
fn megakernel_dispatcher_populates_release_telemetry_fields() {
    let src = include_str!("../src/megakernel/dispatcher.rs");
    let prod_src = src.split("#[cfg(test)]").next().unwrap_or(src);
    for required in [
        "bytes_uploaded: rule_update.uploaded_bytes",
        "bytes_read_back",
        "bytes_moved",
        "resident_allocations: rule_update.resident_allocations",
        "kernel_launches: 1",
        "sync_points: 2",
        "occupancy_proxy_bps(",
        "frontier_density_bps: self.config.frontier_density_bps",
        "queue_state_readback_bytes",
        "hit_readback_bytes",
        "topology: dynamic_plan.plan.topology",
        "dispatch_plan_cache_hit: dynamic_plan.cache_hit",
        "dispatch_plan_cache_entries: dynamic_plan.cache_entries",
    ] {
        assert!(
            prod_src.contains(required),
            "megakernel dispatch report must populate `{required}` for release performance telemetry"
        );
    }
}

#[test]
fn live_megakernel_dispatch_reports_transfer_and_reuse_telemetry() {
    let backend = WgpuBackend::new().expect(
        "Fix: live WGPU backend required for megakernel telemetry contracts; missing GPU is a configuration bug.",
    );
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 4,
        hit_capacity: 1024,
        timeout: Duration::from_secs(10),
        frontier_density_bps: 625,
        ..Default::default()
    };
    let mut dispatcher = BatchDispatcher::new(backend.clone(), config)
        .expect("BatchDispatcher construction must compile the live batch megakernel");
    let files = vec![BatchFile::new(1, 0, b"telemetry".to_vec())];
    let batch =
        FileBatch::upload(backend.device_queue(), &files, 4, 1024).expect("FileBatch must upload");
    let rules = vec![BatchRuleProgram::new(0, vec![0; 256], vec![1], 1)
        .expect("single-state accepting DFA rule must be valid")];
    let mut hits = Vec::with_capacity(64);
    let hit_storage = hits.as_ptr();

    let first = dispatcher
        .dispatch_into(&batch, &rules, &mut hits)
        .expect("first live dispatch must complete");

    assert_eq!(first.telemetry.kernel_launches, 1);
    assert_eq!(first.telemetry.sync_points, 2);
    assert_eq!(first.telemetry.frontier_density_bps, 625);
    assert!(first.items_processed > 0, "live dispatch must process work");
    assert!(
        first.telemetry.occupancy_proxy_bps > 0,
        "non-empty live dispatch must expose a non-zero occupancy proxy"
    );
    assert!(
        first.telemetry.bytes_uploaded > 0,
        "first dispatch must account for resident rule-catalog upload bytes"
    );
    assert_eq!(
        first.telemetry.resident_allocations, 4,
        "first dispatch must account for rule-meta, transition, accept, and byte-class-map resident buffers"
    );
    assert!(
        first.telemetry.queue_state_readback_bytes > 0,
        "queue-state readback bytes must be counted"
    );
    assert_eq!(
        first.telemetry.bytes_read_back,
        first
            .telemetry
            .queue_state_readback_bytes
            .saturating_add(first.telemetry.hit_readback_bytes)
    );
    assert_eq!(
        first.telemetry.bytes_moved,
        first
            .telemetry
            .bytes_uploaded
            .saturating_add(first.telemetry.bytes_read_back)
    );
    assert_eq!(
        hits.as_ptr(),
        hit_storage,
        "caller-owned hit scratch must be reused when capacity is sufficient"
    );
    assert!(
        !first.telemetry.dispatch_plan_cache_hit,
        "first fixed-batch dispatch must miss the dispatcher-local plan cache"
    );
    assert_eq!(
        first.telemetry.dispatch_plan_cache_entries, 1,
        "first fixed-batch dispatch must seed exactly one cached launch plan"
    );

    let second = dispatcher
        .dispatch_into(&batch, &rules, &mut hits)
        .expect("second live dispatch must reuse resident rule buffers");

    assert_eq!(
        second.telemetry.bytes_uploaded, 0,
        "resident rule buffers must not be re-uploaded for an unchanged catalog"
    );
    assert_eq!(
        second.telemetry.resident_allocations, 0,
        "resident rule buffers must be reused for an unchanged catalog"
    );
    assert_eq!(second.telemetry.kernel_launches, 1);
    assert!(
        second.telemetry.bytes_read_back > 0,
        "cached-rule dispatch still must count readback volume"
    );
    assert_eq!(
        second.telemetry.bytes_moved,
        second.telemetry.bytes_read_back
    );
    assert_eq!(
        hits.as_ptr(),
        hit_storage,
        "cached dispatch must keep using caller-owned hit scratch"
    );
    assert!(
        second.telemetry.dispatch_plan_cache_hit,
        "second fixed-batch dispatch must reuse dispatcher-local launch metadata"
    );
    assert_eq!(
        second.telemetry.dispatch_plan_cache_entries, 1,
        "fixed-batch plan reuse must not grow cache entries"
    );
}

#[test]
fn live_megakernel_dispatch_telemetry_uses_actual_batch_topology() {
    let backend = WgpuBackend::new().expect(
        "Fix: live WGPU backend required for megakernel topology telemetry; missing GPU is a configuration bug.",
    );
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 4,
        hit_capacity: 1024,
        timeout: Duration::from_secs(10),
        frontier_density_bps: 9_000,
        ..Default::default()
    };
    let mut dispatcher = BatchDispatcher::new(backend.clone(), config)
        .expect("BatchDispatcher construction must compile the live batch megakernel");
    let batch =
        FileBatch::upload(backend.device_queue(), &[], 1, 1024).expect("empty batch must upload");
    let rules = vec![BatchRuleProgram::new(0, vec![0; 256], vec![1], 1)
        .expect("single-state accepting DFA rule must be valid")];
    let mut hits = Vec::new();

    let report = dispatcher
        .dispatch_into(&batch, &rules, &mut hits)
        .expect("empty live batch dispatch must complete");

    assert_eq!(report.items_processed, 0);
    assert_eq!(
        report.telemetry.topology,
        MegakernelDispatchTopology::Empty,
        "telemetry topology must be derived from actual batch queue length, not constructor seed geometry"
    );
}
