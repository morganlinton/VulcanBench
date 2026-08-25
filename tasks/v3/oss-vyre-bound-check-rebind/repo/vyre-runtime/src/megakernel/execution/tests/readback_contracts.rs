use super::*;

#[test]
fn readback_borrowed_into_decodes_into_caller_storage() {
    let backend = Arc::new(EchoBackend);
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: echo backend must compile megakernel");
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let debug = Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    let io_queue = io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap();
    let mut readback = MegakernelReadback::default();
    let mut outputs = Vec::with_capacity(4);

    let stats = kernel
        .dispatch_with_io_queue_readback_borrowed_into(
            &control,
            &ring,
            &debug,
            &io_queue,
            &mut readback,
            &mut outputs,
        )
        .expect("Fix: readback into caller storage must decode echoed ABI buffers");

    assert_eq!(outputs.len(), 4);
    assert!(
        outputs.iter().all(Vec::is_empty),
        "Fix: readback decode must leave reusable output slots empty after swapping bytes into MegakernelReadback."
    );
    assert!(
        outputs.capacity() >= 4,
        "Fix: readback decode must preserve caller output-vector capacity across dispatches."
    );
    assert_eq!(stats.output_buffers, 4);
    assert_eq!(stats.readback_bytes, stats.output_bytes);
    assert_eq!(
        stats.bytes_moved,
        stats.input_bytes.saturating_add(stats.readback_bytes)
    );
    assert_eq!(
        stats.device_allocation_bytes,
        stats.input_bytes.saturating_add(stats.output_bytes)
    );
    assert_eq!(stats.device_allocation_events, 8);
    assert_eq!(stats.kernel_launches, 1);
    assert_eq!(stats.sync_points, 1);
    assert_eq!(readback.control_bytes, control);
    assert_eq!(readback.ring_bytes, ring);
    assert_eq!(readback.debug_log_bytes, debug);
    assert_eq!(readback.io_queue_bytes, io_queue);
}

#[test]
fn readback_owned_into_uses_caller_storage() {
    let backend = Arc::new(EchoBackend);
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: echo backend must compile megakernel");
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let debug = Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    let io_queue = io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap();
    let mut readback = MegakernelReadback::default();
    let mut outputs = Vec::with_capacity(4);

    let stats = kernel
        .dispatch_with_io_queue_readback_into(
            control.clone(),
            ring.clone(),
            debug.clone(),
            io_queue.clone(),
            &mut readback,
            &mut outputs,
        )
        .expect("Fix: owned readback-into dispatch must decode echoed ABI buffers");

    assert_eq!(outputs.len(), 4);
    assert!(
        outputs.iter().all(Vec::is_empty),
        "Fix: owned readback-into dispatch must leave caller output slots reusable after decode."
    );
    assert!(
        outputs.capacity() >= 4,
        "Fix: owned readback-into dispatch must preserve caller output-vector capacity."
    );
    assert_eq!(stats.output_buffers, 4);
    assert_eq!(stats.kernel_launches, 1);
    assert_eq!(stats.sync_points, 1);
    assert_eq!(readback.control_bytes, control);
    assert_eq!(readback.ring_bytes, ring);
    assert_eq!(readback.debug_log_bytes, debug);
    assert_eq!(readback.io_queue_bytes, io_queue);
}


