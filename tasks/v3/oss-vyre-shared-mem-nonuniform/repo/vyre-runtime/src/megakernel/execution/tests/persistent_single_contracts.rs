use super::*;

#[test]
fn persistent_handle_dispatch_never_reenters_host_byte_path() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let calls = Arc::clone(&backend.calls);
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");

    let output = kernel
        .dispatch_persistent_handles_observed(MegakernelResidentHandles::new(11, 12, 13, 14))
        .expect("Fix: persistent-handle dispatch must call the compiled pipeline handle API");

    assert_eq!(output.buffers, vec![vec![1, 2, 3, 4]]);
    assert_eq!(output.stats.input_bytes, 0);
    assert_eq!(output.stats.readback_bytes, 4);
    assert_eq!(output.stats.bytes_moved, 4);
    assert_eq!(output.stats.resident_resource_rows, 1);
    assert_eq!(output.stats.resident_resource_handles, 4);
    assert_eq!(
        output.stats.device_allocation_bytes, 0,
        "Fix: resident-handle dispatch must not report fresh host-visible device allocation"
    );
    assert_eq!(
        output.stats.device_allocation_events, 0,
        "Fix: resident-handle dispatch must not report fresh host-visible device allocation events"
    );
    assert_eq!(
        calls
            .lock()
            .expect("Fix: persistent-handle recording mutex must not be poisoned")
            .as_slice(),
        &[[11, 12, 13, 14]]
    );
}

#[test]

fn persistent_handle_dispatch_into_reuses_caller_output_storage() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");
    let mut outputs = vec![Vec::with_capacity(16)];
    let output_shell = outputs.as_ptr() as usize;
    let first_slot = outputs[0].as_ptr() as usize;

    let stats = kernel
        .dispatch_persistent_handles_into(
            MegakernelResidentHandles::new(11, 12, 13, 14),
            &mut outputs,
        )
        .expect("Fix: persistent-handle dispatch_into must call the compiled pipeline handle API");

    assert_eq!(outputs, vec![vec![1, 2, 3, 4]]);
    assert_eq!(outputs.as_ptr() as usize, output_shell);
    assert_eq!(outputs[0].as_ptr() as usize, first_slot);
    assert_eq!(stats.input_bytes, 0);
    assert_eq!(stats.output_bytes, 4);
    assert_eq!(stats.readback_bytes, 4);
    assert_eq!(stats.bytes_moved, 4);
    assert_eq!(stats.resident_resource_rows, 1);
    assert_eq!(stats.resident_resource_handles, 4);
    assert_eq!(stats.device_allocation_bytes, 0);
    assert_eq!(stats.device_allocation_events, 0);
}

#[test]
fn persistent_handle_observed_preallocates_abi_output_shell() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");

    let observed = kernel
        .dispatch_persistent_handles_observed(MegakernelResidentHandles::new(11, 12, 13, 14))
        .expect(
            "Fix: observed persistent-handle dispatch must call the compiled pipeline handle API",
        );

    assert_eq!(observed.buffers, vec![vec![1, 2, 3, 4]]);
    assert!(
        observed.buffers.capacity() >= MegakernelResidentHandles::ABI_RESOURCE_COUNT,
        "Fix: observed persistent-handle dispatch must preallocate the megakernel ABI output shell."
    );
    assert_eq!(observed.stats.output_bytes, 4);
    assert_eq!(observed.stats.output_buffers, 1);
}

