use super::*;

#[test]
fn persistent_handle_many_dispatch_uses_backend_batch_contract_once() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let calls = Arc::clone(&backend.calls);
    let row_batch_calls = Arc::clone(&backend.row_batch_calls);
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");

    let output = kernel
        .dispatch_persistent_handles_many_observed(&[
            MegakernelResidentHandles::new(21, 22, 23, 24),
            MegakernelResidentHandles::new(31, 32, 33, 34),
        ])
        .expect("Fix: batched persistent-handle dispatch must use the compiled pipeline batch API");

    assert_eq!(output.batches, vec![vec![vec![0]], vec![vec![1]]]);
    assert_eq!(output.stats.input_bytes, 0);
    assert_eq!(output.stats.readback_bytes, 2);
    assert_eq!(output.stats.bytes_moved, 2);
    assert_eq!(output.stats.resident_resource_rows, 2);
    assert_eq!(output.stats.resident_resource_handles, 8);
    assert_eq!(
        output.stats.device_allocation_bytes, 0,
        "Fix: batched resident-handle dispatch must not report fresh host-visible device allocation"
    );
    assert_eq!(output.stats.device_allocation_events, 0);
    assert_eq!(output.stats.output_buffers, 2);
    assert_eq!(
        calls
            .lock()
            .expect("Fix: persistent-handle recording mutex must not be poisoned")
            .as_slice(),
        &[[21, 22, 23, 24], [31, 32, 33, 34]]
    );
    assert_eq!(
        row_batch_calls.load(Ordering::SeqCst),
        1,
        "Fix: megakernel resident batches must use fixed ABI resource rows directly, not rebuild transient &[Resource] slice lists"
    );
}

#[test]
fn persistent_handle_many_into_reuses_nested_output_storage() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");
    let mut batches = vec![vec![Vec::with_capacity(8)], vec![Vec::with_capacity(8)]];
    let outer_ptr = batches.as_ptr() as usize;
    let first_row_ptr = batches[0].as_ptr() as usize;
    let second_row_ptr = batches[1].as_ptr() as usize;
    let first_slot_ptr = batches[0][0].as_ptr() as usize;
    let second_slot_ptr = batches[1][0].as_ptr() as usize;

    let stats = kernel
        .dispatch_persistent_handles_many_into(
            &[
                MegakernelResidentHandles::new(21, 22, 23, 24),
                MegakernelResidentHandles::new(31, 32, 33, 34),
            ],
            &mut batches,
        )
        .expect("Fix: batched persistent-handle dispatch must fill caller-owned output storage");

    assert_eq!(batches, vec![vec![vec![0]], vec![vec![1]]]);
    assert_eq!(batches.as_ptr() as usize, outer_ptr);
    assert_eq!(batches[0].as_ptr() as usize, first_row_ptr);
    assert_eq!(batches[1].as_ptr() as usize, second_row_ptr);
    assert_eq!(batches[0][0].as_ptr() as usize, first_slot_ptr);
    assert_eq!(batches[1][0].as_ptr() as usize, second_slot_ptr);
    assert_eq!(stats.output_buffers, 2);
    assert_eq!(stats.resident_resource_rows, 2);
    assert_eq!(stats.resident_resource_handles, 8);
    assert_eq!(stats.device_allocation_events, 0);
}

#[test]
fn persistent_handle_many_scratch_reuses_resource_rows_and_outputs() {
    let backend = Arc::new(PersistentHandleBackend::default());
    let row_batch_calls = Arc::clone(&backend.row_batch_calls);
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: persistent-handle backend must bootstrap");
    let mut scratch = MegakernelResidentBatchScratch::with_capacity(2, 1)
        .expect("Fix: scratch preallocation must succeed in test environment; reduce batch count or slot count");

    let first_stats = kernel
        .dispatch_persistent_handles_many_with_scratch(
            &[
                MegakernelResidentHandles::new(21, 22, 23, 24),
                MegakernelResidentHandles::new(31, 32, 33, 34),
            ],
            &mut scratch,
        )
        .expect("Fix: scratch-backed batched persistent dispatch must run");
    let resource_ptr = scratch.resources.as_ptr() as usize;
    let resource_capacity = scratch.resource_capacity();
    let batch_ptr = scratch.batches.as_ptr() as usize;
    let first_row_ptr = scratch.batches[0].as_ptr() as usize;
    let second_row_ptr = scratch.batches[1].as_ptr() as usize;
    let first_slot_ptr = scratch.batches[0][0].as_ptr() as usize;
    let second_slot_ptr = scratch.batches[1][0].as_ptr() as usize;

    let second_stats = kernel
        .dispatch_persistent_handles_many_with_scratch(
            &[
                MegakernelResidentHandles::new(41, 42, 43, 44),
                MegakernelResidentHandles::new(51, 52, 53, 54),
            ],
            &mut scratch,
        )
        .expect("Fix: second scratch-backed dispatch must reuse retained storage");

    assert_eq!(scratch.batches(), &[vec![vec![0]], vec![vec![1]]]);
    assert_eq!(first_stats.output_buffers, 2);
    assert_eq!(second_stats.output_buffers, 2);
    assert_eq!(first_stats.resident_resource_rows, 2);
    assert_eq!(second_stats.resident_resource_rows, 2);
    assert_eq!(first_stats.resident_resource_handles, 8);
    assert_eq!(second_stats.resident_resource_handles, 8);
    assert_eq!(scratch.resources.as_ptr() as usize, resource_ptr);
    assert_eq!(scratch.resource_capacity(), resource_capacity);
    assert_eq!(scratch.batches.as_ptr() as usize, batch_ptr);
    assert_eq!(scratch.batches[0].as_ptr() as usize, first_row_ptr);
    assert_eq!(scratch.batches[1].as_ptr() as usize, second_row_ptr);
    assert_eq!(scratch.batches[0][0].as_ptr() as usize, first_slot_ptr);
    assert_eq!(scratch.batches[1][0].as_ptr() as usize, second_slot_ptr);
    assert_eq!(
        row_batch_calls.load(Ordering::SeqCst),
        2,
        "Fix: scratch-backed resident batches must keep using fixed resource rows across repeated submissions"
    );
}

#[test]
fn resident_batch_scratch_clear_retains_nested_allocations_but_hides_logical_batches() {
    let mut scratch = MegakernelResidentBatchScratch::with_capacity(2, 1)
        .expect("Fix: scratch preallocation must succeed in test environment; reduce batch count or slot count");
    scratch.batches[0][0].extend_from_slice(&[1, 2, 3]);
    scratch.batches[1][0].extend_from_slice(&[4, 5, 6]);
    scratch.active_batches = 2;
    let batch_ptr = scratch.batches.as_ptr() as usize;
    let first_row_ptr = scratch.batches[0].as_ptr() as usize;
    let second_row_ptr = scratch.batches[1].as_ptr() as usize;
    let first_slot_ptr = scratch.batches[0][0].as_ptr() as usize;
    let second_slot_ptr = scratch.batches[1][0].as_ptr() as usize;

    scratch.clear();

    assert!(scratch.batches().is_empty());
    assert_eq!(scratch.batches.as_ptr() as usize, batch_ptr);
    assert_eq!(scratch.batches[0].as_ptr() as usize, first_row_ptr);
    assert_eq!(scratch.batches[1].as_ptr() as usize, second_row_ptr);
    assert_eq!(scratch.batches[0][0].as_ptr() as usize, first_slot_ptr);
    assert_eq!(scratch.batches[1][0].as_ptr() as usize, second_slot_ptr);
    assert!(scratch.batches.iter().flatten().all(Vec::is_empty));
}

/// Regression: `MegakernelResidentBatchScratch::with_capacity` previously
/// swallowed allocation failures with `Err(_error) => Self::default()`, returning
/// empty scratch with no signal. The silent degrade voided the allocation-stability
/// guarantee that `with_capacity` is supposed to provide.
///
/// After the fix, `with_capacity` returns `Result<Self, PipelineError>` so
/// callers see the error. This test verifies:
/// 1. A valid call returns `Ok(scratch)` with the correct preallocated shape.
/// 2. The preallocated scratch has exactly `batch_count` rows each with
///    `output_slots_per_batch` output slots — NOT an empty default.
/// 3. `resource_capacity()` and `batch_capacity()` reflect the reservation,
///    not zero (the shape an empty default would have had).
#[test]
fn with_capacity_returns_result_and_preallocates_correct_shape() {
    let scratch = MegakernelResidentBatchScratch::with_capacity(3, 2)
        .expect("Fix: with_capacity must succeed for small in-bounds allocation");

    // resource capacity must reflect the 3-row reservation
    assert_eq!(
        scratch.resource_capacity(),
        3,
        "Fix: with_capacity must reserve resource rows, not return empty default"
    );
    // batch capacity must reflect the 3-row reservation
    assert_eq!(
        scratch.batch_capacity(),
        3,
        "Fix: with_capacity must reserve batch rows, not return empty default"
    );
    // each of the 3 rows must have exactly 2 pre-pushed (empty) output slots
    assert_eq!(
        scratch.batches.len(),
        3,
        "Fix: with_capacity must push batch rows into the Vec; silent default has 0 rows"
    );
    assert_eq!(
        scratch.batches[0].len(),
        2,
        "Fix: each batch row must have output_slots_per_batch slots pre-pushed"
    );
    assert_eq!(
        scratch.batches[2].len(),
        2,
        "Fix: every batch row must have output_slots_per_batch slots pre-pushed"
    );
    assert!(
        scratch.batches.iter().flatten().all(Vec::is_empty),
        "Fix: pre-pushed output slots must be empty Vecs, not garbage"
    );
}

