//! CUDA runtime telemetry contracts.

mod common;
use common::{bytes_u32, u32_bytes};
use vyre_driver::DispatchConfig;
use vyre_driver_cuda::{CudaBackend, CudaMegakernelScheduleSample};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn add_one_program(count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(count),
            BufferDecl::output("out", 1, DataType::U32).with_count(count),
        ],
        [128, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(1)),
        )],
    )
}

fn cuda_pool_bucket(bytes: usize) -> u64 {
    bytes.max(1).next_power_of_two() as u64
}

#[test]
fn cuda_direct_dispatch_updates_runtime_telemetry() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    backend.reset_telemetry();
    assert_eq!(backend.telemetry_snapshot(), Default::default());

    let input = u32_bytes(&[0, 1, 2, 3, 4, 5, 6, 7]);
    let outputs = backend
        .dispatch(
            &add_one_program(8),
            &[input.clone()],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA direct dispatch must succeed before telemetry can be trusted.");

    assert_eq!(bytes_u32(&outputs[0]), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let snapshot = backend.telemetry_snapshot();
    assert_eq!(
        snapshot.kernel_launches, 1,
        "Fix: one direct dispatch must record exactly one CUDA kernel launch."
    );
    assert_eq!(
        snapshot.cuda_graph_launches, 0,
        "Fix: direct dispatch must not be counted as cudaGraph replay."
    );
    assert!(
        snapshot.host_to_device_bytes >= input.len() as u64,
        "Fix: telemetry must include at least the direct input H2D bytes."
    );
    assert!(
        snapshot.device_to_host_bytes >= outputs[0].len() as u64,
        "Fix: telemetry must include the direct output D2H bytes."
    );
    assert!(
        snapshot.readback_bytes >= outputs[0].len() as u64,
        "Fix: output readback bytes must be visible separately from total D2H bytes."
    );
    assert!(
        snapshot.transient_allocation_bytes_requested
            >= input.len().saturating_add(outputs[0].len()) as u64,
        "Fix: dispatch allocation telemetry must expose transient device pressure."
    );
    assert!(
        snapshot.param_upload_bytes > 0,
        "Fix: kernel parameter upload bytes must be visible for dispatch overhead analysis."
    );
    assert!(
        snapshot.sync_points >= 1,
        "Fix: awaiting direct dispatch must record at least one CUDA synchronization point."
    );
    assert!(
        snapshot.host_upload_operations >= 1,
        "Fix: direct dispatch must count non-empty H2D upload operations."
    );
    assert!(
        snapshot.device_readback_operations >= 1,
        "Fix: direct dispatch must count non-empty D2H readback operations."
    );
    assert!(
        snapshot.launched_elements >= 8,
        "Fix: launch telemetry must expose logical launched element count."
    );
    assert!(
        snapshot.scheduled_thread_slots >= snapshot.launched_elements,
        "Fix: launch telemetry must expose scheduled CUDA thread slots without assuming a stale fixed workgroup floor."
    );
    assert!(
        snapshot.logical_thread_utilization_bps > 0,
        "Fix: launch telemetry must expose a non-zero occupancy/utilization proxy."
    );
    assert!(
        snapshot.logical_thread_utilization_bps <= 10_000,
        "Fix: launch telemetry utilization proxy must be clamped to basis points."
    );
    assert!(
        snapshot.wasted_thread_slots <= snapshot.scheduled_thread_slots,
        "Fix: launch telemetry wasted slots must be bounded by scheduled CUDA thread slots."
    );
    assert!(
        snapshot.logical_thread_waste_bps <= 10_000,
        "Fix: launch telemetry waste proxy must be clamped to basis points."
    );
    assert!(
        snapshot.logical_elements_per_thread_slot_bps > 0,
        "Fix: launch telemetry must expose unclamped logical element density per scheduled CUDA thread slot."
    );
    let scheduler_sample = CudaMegakernelScheduleSample::from_telemetry_snapshot(snapshot, 1_000.0);
    assert_eq!(
        scheduler_sample.dispatch_cost_ns, 1_000.0,
        "Fix: scheduler sample must preserve externally measured dispatch cost."
    );
    assert_eq!(
        scheduler_sample.readback_bytes, snapshot.readback_bytes,
        "Fix: scheduler sample must carry real compact readback volume from CUDA telemetry."
    );
    assert!(
        scheduler_sample.frontier_density > 0.0 && scheduler_sample.frontier_density <= 1.0,
        "Fix: scheduler sample must derive bounded frontier-density proxy from real CUDA launch telemetry."
    );

    backend.reset_telemetry();
    assert_eq!(
        backend.telemetry_snapshot(),
        Default::default(),
        "Fix: reset_telemetry must clear runtime counters without requiring backend cleanup."
    );
}

#[test]
fn cuda_direct_dispatch_reports_bucketed_transient_allocation_pressure() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    backend.reset_telemetry();

    let input = u32_bytes(&[41, 42, 43]);
    let outputs = backend
        .dispatch(
            &add_one_program(3),
            &[input.clone()],
            &DispatchConfig::default(),
        )
        .expect("Fix: CUDA direct dispatch must succeed for bucketed telemetry.");

    assert_eq!(
        bytes_u32(&outputs[0]),
        vec![42, 43, 44],
        "Fix: non-power-of-two dispatch telemetry fixture must still prove kernel semantics."
    );
    let snapshot = backend.telemetry_snapshot();
    let expected_transient_floor = cuda_pool_bucket(input.len())
        + cuda_pool_bucket(outputs[0].len())
        + cuda_pool_bucket(snapshot.param_upload_bytes as usize);
    assert!(
        snapshot.transient_allocation_bytes_requested >= expected_transient_floor,
        "Fix: transient allocation telemetry must report CUDA pool bucket pressure, not only \
         logical request bytes. expected_at_least={} observed={}",
        expected_transient_floor,
        snapshot.transient_allocation_bytes_requested
    );
}

#[test]
fn cuda_compile_native_reports_static_param_allocation_pressure() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    backend.reset_telemetry();

    let _pipeline = backend
        .compile_native(&add_one_program(8), &DispatchConfig::default())
        .expect("Fix: CUDA native pipeline compilation must succeed before telemetry is trusted.");

    let snapshot = backend.telemetry_snapshot();
    assert!(
        snapshot.param_upload_bytes > 0,
        "Fix: compile_native uploads static launch parameters and must expose those H2D bytes."
    );
    let live_transient_bytes = backend
        .allocated_transient_allocation_bytes()
        .expect("Fix: CUDA live transient allocation accounting must be readable.");
    assert!(
        snapshot.transient_allocation_bytes_requested
            >= cuda_pool_bucket(snapshot.param_upload_bytes as usize),
        "Fix: compile_native static parameter allocation must be visible as bucketed CUDA \
         transient allocation pressure. param_bytes={} transient_bytes={}",
        snapshot.param_upload_bytes,
        snapshot.transient_allocation_bytes_requested
    );
    assert!(
        live_transient_bytes >= cuda_pool_bucket(snapshot.param_upload_bytes as usize) as usize,
        "Fix: live transient allocation accounting must include compiled-pipeline static \
         parameter buffers. param_bytes={} live_transient_bytes={}",
        snapshot.param_upload_bytes,
        live_transient_bytes
    );
}

#[test]
fn cuda_graph_replay_updates_graph_telemetry_after_reset() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let program = add_one_program(8);
    let inputs = vec![u32_bytes(&[10, 11, 12, 13, 14, 15, 16, 17])];
    let config = DispatchConfig::default();
    let mut cached = backend
        .record_cuda_graph(&program, &inputs, &config)
        .expect("Fix: cudaGraph recording must succeed on the release CUDA backend.");

    backend.reset_telemetry();
    let input_refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
    let outputs = backend
        .dispatch_via_cuda_graph(&mut cached, &input_refs)
        .expect("Fix: cudaGraph replay must succeed before telemetry can be trusted.");

    assert_eq!(bytes_u32(&outputs[0]), vec![11, 12, 13, 14, 15, 16, 17, 18]);
    let snapshot = backend.telemetry_snapshot();
    assert_eq!(
        snapshot.cuda_graph_launches, 1,
        "Fix: one cudaGraph replay must record exactly one graph launch."
    );
    assert_eq!(
        snapshot.kernel_launches, 0,
        "Fix: graph replay telemetry must not double-count as a direct kernel launch."
    );
    assert_eq!(
        snapshot.host_to_device_bytes,
        inputs[0].len() as u64,
        "Fix: graph replay telemetry must expose replay input byte volume."
    );
    assert_eq!(
        snapshot.device_to_host_bytes,
        outputs[0].len() as u64,
        "Fix: graph replay telemetry must expose replay output byte volume."
    );
    assert_eq!(
        snapshot.readback_bytes,
        outputs[0].len() as u64,
        "Fix: graph replay output bytes must be counted as readback bytes."
    );
    assert_eq!(
        snapshot.host_upload_operations, 1,
        "Fix: graph replay telemetry must count the non-empty replay H2D operation."
    );
    assert_eq!(
        snapshot.device_readback_operations, 1,
        "Fix: graph replay telemetry must count the non-empty replay D2H operation."
    );
    assert_eq!(
        snapshot.sync_points, 1,
        "Fix: graph replay must record the stream synchronization used to publish outputs."
    );
}
