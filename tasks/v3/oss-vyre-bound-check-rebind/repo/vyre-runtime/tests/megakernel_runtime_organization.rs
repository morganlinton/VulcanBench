//! Runtime organization contracts for megakernel resident/readback/recovery paths.

use vyre_driver::BackendError;
use vyre_runtime::megakernel::{
    backend_error_indicates_device_loss, protocol, Megakernel, MegakernelDispatchStats,
    MegakernelReadback, MegakernelResidentBuffers,
};

#[test]
fn dispatch_stats_expose_latency_and_throughput_without_overflow() {
    let stats = MegakernelDispatchStats {
        input_bytes: 128,
        output_bytes: 4096,
        readback_bytes: 4096,
        bytes_moved: 4224,
        device_allocation_bytes: 4224,
        device_allocation_events: 8,
        latency_ns: 1_000,
        output_buffers: 4,
        resident_resource_rows: 0,
        resident_resource_handles: 0,
        kernel_launches: 1,
        sync_points: 1,
        recovered_after_device_loss: false,
    };

    assert_eq!(stats.output_bytes_per_second(), 4_096_000_000);
    assert_eq!(stats.readback_bytes_per_second(), 4_096_000_000);
    assert_eq!(stats.bytes_moved_per_second(), 4_224_000_000);
    assert_eq!(stats.device_allocation_bytes, 4224);
    assert_eq!(stats.device_allocation_bytes_per_second(), 4_224_000_000);
    assert_eq!(stats.device_allocation_events, 8);
    assert_eq!(stats.kernel_launches, 1);
    assert_eq!(stats.sync_points, 1);

    let saturated = MegakernelDispatchStats {
        output_bytes: u64::MAX,
        latency_ns: 1,
        ..stats
    };
    assert_eq!(saturated.output_bytes_per_second(), u64::MAX);
    let saturated_alloc = MegakernelDispatchStats {
        device_allocation_bytes: u64::MAX,
        latency_ns: 1,
        ..stats
    };
    assert_eq!(
        saturated_alloc.device_allocation_bytes_per_second(),
        u64::MAX
    );
    let saturated_moved = MegakernelDispatchStats {
        bytes_moved: u64::MAX,
        latency_ns: 1,
        ..stats
    };
    assert_eq!(saturated_moved.bytes_moved_per_second(), u64::MAX);
}

#[test]
fn readback_rejects_truncated_ring_before_telemetry() {
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = vec![0_u8; 4];
    let debug = Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    let io = vyre_runtime::megakernel::io::try_encode_empty_io_queue(
        vyre_runtime::megakernel::io::IO_SLOT_COUNT,
    )
    .unwrap();

    let error = MegakernelReadback::from_outputs(vec![control, ring, debug, io], 2)
        .expect_err("truncated ring readback must fail before telemetry decode");
    assert!(error.to_string().contains("readback ring"));
}

#[test]
fn resident_buffers_preserve_multitenant_slot_headers() {
    let mut resident = MegakernelResidentBuffers::new(4, 8, 2).unwrap();
    resident
        .publish_slot(0, 3, protocol::opcode::STORE_U32, &[11, 12])
        .unwrap();
    resident
        .publish_slot(1, 7, protocol::opcode::ATOMIC_ADD, &[13, 14])
        .unwrap();

    let slot_bytes = protocol::SLOT_WORDS as usize * 4;
    let tenant0 = u32::from_le_bytes(
        resident.ring_bytes()
            [protocol::TENANT_WORD as usize * 4..protocol::TENANT_WORD as usize * 4 + 4]
            .try_into()
            .unwrap(),
    );
    let slot1_base = slot_bytes;
    let tenant1 = u32::from_le_bytes(
        resident.ring_bytes()[slot1_base + protocol::TENANT_WORD as usize * 4
            ..slot1_base + protocol::TENANT_WORD as usize * 4 + 4]
            .try_into()
            .unwrap(),
    );

    assert_eq!(tenant0, 3);
    assert_eq!(tenant1, 7);
}

#[test]
fn device_loss_classifier_drives_fault_recovery_only_for_device_loss() {
    let device_lost = BackendError::DispatchFailed {
        code: None,
        message: "adapter lost after GPU reset".to_string(),
    };
    let queue_error = BackendError::DispatchFailed {
        code: None,
        message: "queue rejected mismatched binding count".to_string(),
    };

    assert!(backend_error_indicates_device_loss(&device_lost));
    assert!(!backend_error_indicates_device_loss(&queue_error));
}
