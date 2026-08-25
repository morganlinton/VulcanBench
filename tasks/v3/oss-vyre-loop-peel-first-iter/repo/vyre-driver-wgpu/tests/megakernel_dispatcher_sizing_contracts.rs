//! Dispatcher sizing invariants, batch capacity checks, and no-silent-CPU-fallback
//! contracts for the megakernel runtime.
//!
//! Covers:
//! - worker group sizing invariants
//! - publish/batch capacity checks
//! - no silent CPU fallback assumptions

use std::sync::Arc;
use vyre_runtime::megakernel::{protocol, Megakernel};
use vyre_runtime::PipelineError;

use vyre_driver_wgpu::WgpuBackend;

#[cfg(feature = "megakernel-batch")]
use std::time::Duration;
#[cfg(feature = "megakernel-batch")]
use vyre_driver_wgpu::megakernel::{BatchDispatchConfig, BatchDispatcher, BatchFile, FileBatch};
#[cfg(feature = "megakernel-batch")]
use vyre_runtime::megakernel::{MegakernelDispatchTopology, MegakernelLaunchPolicy};

fn assert_no_cpu_fallback_wording(err: &PipelineError) {
    let msg = err.to_string().to_lowercase();
    assert!(!msg.contains("cpu"), "error must never mention CPU: {msg}");
    assert!(
        !msg.contains("fallback"),
        "error must never mention fallback: {msg}"
    );
    assert!(
        !msg.contains("software"),
        "error must never imply software emulation: {msg}"
    );
}

fn require_backend() -> WgpuBackend {
    WgpuBackend::new().expect(
        "Fix: WGPU backend required for megakernel dispatcher sizing contracts; missing GPU is a configuration bug.",
    )
}

// ---------------------------------------------------------------------------
// 1. Worker group sizing invariants
// ---------------------------------------------------------------------------

#[test]
fn bootstrap_sharded_rejects_zero_slot_count() {
    let result = Megakernel::bootstrap_sharded(Arc::new(require_backend()), 0, 256, vec![]);
    let Err(err) = result else {
        panic!("zero slot count must fail")
    };
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    assert_no_cpu_fallback_wording(&err);
}

#[test]
fn bootstrap_sharded_rejects_zero_workgroup_size() {
    let result = Megakernel::bootstrap_sharded(Arc::new(require_backend()), 256, 0, vec![]);
    let Err(err) = result else {
        panic!("zero workgroup size must fail")
    };
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    assert_no_cpu_fallback_wording(&err);
}

#[test]
fn bootstrap_sharded_rejects_non_multiple_slot_count() {
    let result = Megakernel::bootstrap_sharded(Arc::new(require_backend()), 257, 256, vec![]);
    let Err(err) = result else {
        panic!("non-multiple slot count must fail")
    };
    assert!(matches!(err, PipelineError::QueueFull { .. }));
}

#[test]
fn worker_groups_computed_from_bootstrap_geometry() {
    assert_eq!(
        Megakernel::worker_groups_for_geometry(512, 64).expect("valid geometry must divide"),
        8,
        "worker_groups must be slot_count / workgroup_size_x"
    );
}

// ---------------------------------------------------------------------------
// 2. Publish/batch capacity checks
// ---------------------------------------------------------------------------

#[test]
fn batch_publish_empty_items_consumes_only_fence_slot() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    let items: &[(u32, &[u32])] = &[];
    let consumed = Megakernel::batch_publish(&mut ring, 0, 0, items, 0xABCD).unwrap();
    assert_eq!(
        consumed, 1,
        "empty batch must consume exactly the fence slot"
    );
    let fence_op = u32::from_le_bytes(ring[4..8].try_into().unwrap());
    let fence_tag = u32::from_le_bytes(ring[20..24].try_into().unwrap());
    assert_eq!(fence_op, protocol::opcode::BATCH_FENCE);
    assert_eq!(fence_tag, 0xABCD);
}

#[test]
fn batch_publish_empty_items_fence_item_count_is_zero() {
    let mut ring = Megakernel::encode_empty_ring(2).unwrap();
    let items: &[(u32, &[u32])] = &[];
    Megakernel::batch_publish(&mut ring, 0, 0, items, 0).unwrap();
    let item_count = u32::from_le_bytes(
        ring[(protocol::ARG0_WORD as usize) * 4..(protocol::ARG0_WORD as usize) * 4 + 4]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        item_count, 0,
        "fence item_count must be zero for empty batch"
    );
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_default_sentinels() {
    let config = BatchDispatchConfig::default();
    assert_eq!(config.workgroup_size_x, 64);
    assert_eq!(
        config.worker_groups, 0,
        "default worker_groups must be 0 sentinel"
    );
    assert_eq!(config.hit_capacity, 65_536);
    assert_eq!(config.timeout, Duration::from_secs(30));
    assert_eq!(config.graph_node_count, 0);
    assert_eq!(config.graph_edge_count, 0);
    assert_eq!(config.frontier_density_bps, 0);
    assert_eq!(config.memory_pressure_bps, 0);
    assert_eq!(config.resident_device_bytes, 0);
    assert_eq!(config.device_memory_budget_bytes, 0);
    assert_eq!(config.hot_opcode_count, 0);
    assert_eq!(config.hot_window_count, 0);
    assert_eq!(config.requeue_count, 0);
    assert_eq!(config.max_priority_age, 0);
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_launch_recommendation_zero_queue_len_succeeds() {
    let config = BatchDispatchConfig::default();
    let limits = wgpu::Limits::default();
    let rec = config
        .launch_recommendation(&limits, 0)
        .expect("zero queue_len must produce valid recommendation");
    assert!(rec.worker_groups >= 1);
    assert!(rec.hit_capacity >= 1024);
    assert!(
        rec.estimated_peak_device_bytes >= 20,
        "launch recommendation must include fixed queue-state resident overhead in device budget telemetry"
    );
    assert_eq!(
        rec.pressure,
        vyre_runtime::megakernel::MegakernelQueuePressure::Empty
    );
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_explicit_graph_hints_select_topology() {
    let config = BatchDispatchConfig::default()
        .with_graph_hints(8192, 131_072, 9_000, 0)
        .with_execution_hints(8, 0, 0, 0);
    let rec = config
        .launch_recommendation(&wgpu::Limits::default(), 8192)
        .expect("explicit graph hints must produce a valid recommendation");

    assert_eq!(rec.topology, MegakernelDispatchTopology::FusedDense);
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_device_budget_rejects_oversized_plan() {
    let err = BatchDispatchConfig::default()
        .with_device_memory_budget(1024, 64 * 1024)
        .launch_recommendation(&wgpu::Limits::default(), 8192)
        .expect_err("device budget must reject oversized sparse-hit plan before allocation");

    let msg = err.to_string();
    assert!(
        msg.contains("device out of memory"),
        "budget rejection must surface as actionable OOM, got: {msg}"
    );
    assert_no_cpu_fallback_wording(&err);
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_repeated_recommendation_hits_launch_cache() {
    MegakernelLaunchPolicy::reset_launch_cache_for_thread();
    let config = BatchDispatchConfig::default()
        .with_graph_hints(8192, 131_072, 9_000, 0)
        .with_execution_hints(8, 0, 0, 0);
    let limits = wgpu::Limits::default();

    let first = config
        .launch_recommendation(&limits, 8192)
        .expect("first batch recommendation must compute");
    let second = config
        .launch_recommendation(&limits, 8192)
        .expect("second batch recommendation must hit cache");
    let stats = MegakernelLaunchPolicy::launch_cache_stats();

    assert_eq!(first, second);
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 1);
    MegakernelLaunchPolicy::reset_launch_cache_for_thread();
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_default_does_not_invent_dense_frontier() {
    let rec = BatchDispatchConfig::default()
        .launch_recommendation(&wgpu::Limits::default(), 8192)
        .expect("default graph hints must produce a valid recommendation");

    assert_ne!(rec.topology, MegakernelDispatchTopology::FusedDense);
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatcher_new_rejects_zero_workgroup_size() {
    let config = BatchDispatchConfig {
        workgroup_size_x: 0,
        ..Default::default()
    };
    let err = BatchDispatcher::new(require_backend(), config)
        .expect_err("zero workgroup_size_x must fail before compilation");
    assert!(matches!(err, PipelineError::QueueFull { .. }));
    assert_no_cpu_fallback_wording(&err);
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatcher_dispatch_empty_rules_returns_zeroed_report() {
    let backend = require_backend();
    let config = BatchDispatchConfig {
        workgroup_size_x: 64,
        worker_groups: 4,
        hit_capacity: 1024,
        timeout: Duration::from_secs(10),
        ..Default::default()
    };
    let mut dispatcher = BatchDispatcher::new(backend.clone(), config)
        .expect("BatchDispatcher must construct on live adapter");

    let files = vec![BatchFile::new(1, 0, b"scan".to_vec())];
    let batch =
        FileBatch::upload(backend.device_queue(), &files, 1, 1024).expect("FileBatch must upload");

    let report = dispatcher
        .dispatch(&batch, &[])
        .expect("empty-rules dispatch must succeed without backend interaction");
    assert_eq!(report.hit_count, 0);
    assert!(report.hits.is_empty());
    assert_eq!(report.items_processed, 0);
    assert!(report.wall_time.is_zero());
    assert!(report.rejected_rules.is_empty());
}

// ---------------------------------------------------------------------------
// 3. No silent CPU fallback assumptions
// ---------------------------------------------------------------------------

#[test]
fn bootstrap_errors_contain_no_cpu_fallback_wording() {
    let backend = require_backend();
    for (slots, wg_size, desc) in [(0, 256, "zero slots"), (256, 0, "zero workgroup")] {
        let result =
            Megakernel::bootstrap_sharded(Arc::new(backend.clone()), slots, wg_size, vec![]);
        let Err(err) = result else {
            panic!("{desc} must fail")
        };
        assert_no_cpu_fallback_wording(&err);
    }
}

#[cfg(feature = "megakernel-batch")]
#[test]
fn batch_dispatch_config_errors_contain_no_cpu_fallback_wording() {
    let config = BatchDispatchConfig {
        workgroup_size_x: 0,
        ..Default::default()
    };
    let limits = wgpu::Limits::default();
    let err = config
        .launch_recommendation(&limits, 64)
        .expect_err("zero workgroup_size_x must fail");
    assert_no_cpu_fallback_wording(&err);
}
