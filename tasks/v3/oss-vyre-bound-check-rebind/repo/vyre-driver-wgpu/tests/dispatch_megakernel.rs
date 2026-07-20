//! End-to-end test for `MegakernelDispatch for WgpuBackend`.
//!
//! Exercises the `MegakernelWorkItem` → ring-slot translation path on the 5090
//! via a SHUTDOWN-only work queue so the kernel exits on its first
//! iteration (same pattern as megakernel_emit.rs).
//!
//! Why SHUTDOWN-only: the legacy `MegakernelWorkItem` opcode space is
//! caller-defined, but `megakernel::opcode::SHUTDOWN` (u32::MAX) is
//! handled by the default interpreted body. Publishing one SHUTDOWN
//! item gives us a full end-to-end round trip (readback +
//! `done_count`) without coupling this test to application opcode
//! handlers outside the default interpreted body.

use std::time::Duration;

use vyre_driver_wgpu::{megakernel::WgpuMegakernelDispatcher, WgpuBackend};
use vyre_runtime::megakernel::protocol::opcode;
use vyre_runtime::megakernel::{MegakernelConfig, MegakernelDispatch, MegakernelWorkItem};

#[test]
fn dispatch_megakernel_runs_shutdown_item_and_reports() {
    let backend = WgpuBackend::acquire().expect(
        "Fix: GPU adapter required for dispatch_megakernel end-to-end; missing adapter is a configuration bug, not graceful fallback.",
    );

    // One MegakernelWorkItem carrying the SHUTDOWN opcode. The legacy
    // MegakernelWorkItem { op_handle, input_handle, output_handle, param } maps
    // onto the ring slot as (opcode, arg0, arg1, arg2).
    let items = vec![MegakernelWorkItem {
        op_handle: opcode::SHUTDOWN,
        input_handle: 0,
        output_handle: 0,
        param: 0,
    }];

    let config = MegakernelConfig {
        worker_count: 64,
        max_wall_time: Duration::from_secs(10),
        expected_items_per_worker: 1,
        ..MegakernelConfig::default()
    };

    let dispatcher = WgpuMegakernelDispatcher::new(&backend);
    let report = MegakernelDispatch::dispatch_megakernel(&dispatcher, &items, &config)
        .expect("Fix: dispatch_megakernel must run a single SHUTDOWN work item end to end");
    let cached_report = MegakernelDispatch::dispatch_megakernel(&dispatcher, &items, &config)
        .expect("Fix: repeated dispatch_megakernel must reuse the compiled hot path");

    // SHUTDOWN arrives via a ring slot, which the kernel claims and
    // executes (atomic_exchange into control[SHUTDOWN]). Claiming +
    // executing increments DONE_COUNT, so we expect at least one
    // processed item (even though SHUTDOWN then terminates the loop
    // on the next iteration).
    assert!(
        report.items_processed >= 1,
        "expected the SHUTDOWN item to be claimed & counted; got items_processed={}",
        report.items_processed
    );
    assert!(
        report.wall_time > Duration::ZERO,
        "wall_time must be non-zero; got {:?}",
        report.wall_time
    );
    assert!(
        !report.telemetry.compiled_pipeline_cache_hit,
        "first direct megakernel dispatch should compile the initial geometry, not report a cache hit"
    );
    assert!(
        cached_report.telemetry.compiled_pipeline_cache_hit,
        "Fix: repeated direct megakernel dispatch must report compiled-pipeline cache reuse."
    );
    assert_eq!(
        cached_report.telemetry.kernel_launches, 1,
        "Fix: compiled-cache reuse must not add extra logical kernel launches."
    );
}

#[test]
fn dispatch_megakernel_rejects_misaligned_queue() {
    let backend = WgpuBackend::acquire().expect("Fix: GPU adapter required.");

    // 15 bytes is not a multiple of sizeof(MegakernelWorkItem)=16  -  must reject.
    let bytes = vec![0u8; 15];

    let config = MegakernelConfig::default();
    let err = WgpuMegakernelDispatcher::new(&backend)
        .dispatch_megakernel_bytes(&bytes, &config)
        .expect_err("misaligned work_queue must reject");
    assert!(
        format!("{err}").contains("sizeof(MegakernelWorkItem)"),
        "error must mention MegakernelWorkItem size; got: {err}"
    );
}
