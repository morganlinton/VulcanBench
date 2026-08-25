use super::*;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use vyre_foundation::ir::Program;

#[test]
fn program_cache_reuses_same_key_and_rebuilds_on_shape_change() {
    let mut cache = ProgramCache::default();

    assert_eq!(*cache.get_or_insert_with(7_u32, || 11_u32), 11);
    assert_eq!(*cache.get_or_insert_with(7_u32, || 99_u32), 11);
    assert_eq!(cache.builds(), 1);

    assert_eq!(*cache.get_or_insert_with(8_u32, || 22_u32), 22);
    assert_eq!(cache.builds(), 2);
}

#[test]
fn dispatch_input_writer_encodes_zero_and_little_endian_slots_in_place() {
    let mut slot = Vec::with_capacity(32);

    write_dispatch_input(
        &mut slot,
        DispatchInput::u32_slice_or_zero_words(&[], 3, "zero words"),
    )
    .expect("Fix: zero-word dispatch input should encode");
    assert_eq!(slot, vec![0; 12]);

    write_dispatch_input(&mut slot, DispatchInput::u32_slice(&[1, 0xAABB_CCDD]))
        .expect("Fix: u32 dispatch input should encode little-endian bytes");
    assert_eq!(slot, vec![1, 0, 0, 0, 0xDD, 0xCC, 0xBB, 0xAA]);
}

#[test]
fn input_slot_shell_drops_stale_dispatch_slots_on_shape_shrink() {
    let mut inputs = vec![vec![0xAA; 4], vec![0xBB; 8], vec![0xCC; 12], vec![0xDD; 16]];

    crate::dispatch_buffers::ensure_input_slots(&mut inputs, 2);

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0], vec![0xAA; 4]);
    assert_eq!(inputs[1], vec![0xBB; 8]);
}

#[test]
fn prepared_dispatch_inputs_never_forward_stale_slots_after_shrink() {
    let mut inputs = vec![vec![0xAA; 4], vec![0xBB; 8], vec![0xCC; 12], vec![0xDD; 16]];

    super::inputs::prepare_dispatch_inputs(&mut inputs, &[DispatchInput::u32_slice(&[9])])
        .expect("Fix: prepared dispatch input shrink should encode");

    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0], vec![9, 0, 0, 0]);
}

#[test]
fn u32_slice_fingerprint_tracks_width_order_and_content() {
    let base = fingerprint_u32_slice(&[1, 2, 3, 4]);

    assert_eq!(base, fingerprint_u32_slice(&[1, 2, 3, 4]));
    assert_ne!(base, fingerprint_u32_slice(&[4, 3, 2, 1]));
    assert_ne!(base, fingerprint_u32_slice(&[1, 2, 3, 4, 0]));
    assert_ne!(base, fingerprint_u32_slice(&[1, 2, 3, 5]));
}

#[test]
fn keyed_dispatch_refresh_reuses_static_slots_and_updates_mutable_slots() {
    let mut inputs = Vec::new();
    let mut key = None;

    refresh_keyed_dispatch_inputs(
        &mut inputs,
        &mut key,
        7_u32,
        &[
            DispatchInput::u32_slice(&[10, 11]),
            DispatchInput::u32_slice(&[20]),
            DispatchInput::zero_u32_words(2, "mutable out"),
        ],
        &[(2, DispatchInput::zero_u32_words(2, "mutable out"))],
    )
    .expect("Fix: first keyed dispatch refresh should stage every slot");
    assert_eq!(inputs[0], vec![10, 0, 0, 0, 11, 0, 0, 0]);
    assert_eq!(inputs[1], vec![20, 0, 0, 0]);
    inputs[2].fill(0xA5);

    refresh_keyed_dispatch_inputs(
        &mut inputs,
        &mut key,
        7_u32,
        &[
            DispatchInput::u32_slice(&[99, 100]),
            DispatchInput::u32_slice(&[88]),
            DispatchInput::zero_u32_words(2, "mutable out"),
        ],
        &[(2, DispatchInput::zero_u32_words(2, "mutable out"))],
    )
    .expect("Fix: same-key refresh should only rewrite mutable slots");
    assert_eq!(inputs[0], vec![10, 0, 0, 0, 11, 0, 0, 0]);
    assert_eq!(inputs[1], vec![20, 0, 0, 0]);
    assert_eq!(inputs[2], vec![0; 8]);

    refresh_keyed_dispatch_inputs(
        &mut inputs,
        &mut key,
        8_u32,
        &[
            DispatchInput::u32_slice(&[99, 100]),
            DispatchInput::u32_slice(&[88]),
            DispatchInput::zero_u32_words(2, "mutable out"),
        ],
        &[(2, DispatchInput::zero_u32_words(2, "mutable out"))],
    )
    .expect("Fix: changed key should restage every slot");
    assert_eq!(inputs[0], vec![99, 0, 0, 0, 100, 0, 0, 0]);
    assert_eq!(inputs[1], vec![88, 0, 0, 0]);
    assert_eq!(inputs[2], vec![0; 8]);
}

struct RecordingResidentUploadDispatcher {
    next_handle: AtomicU64,
    allocations: Mutex<Vec<usize>>,
    uploads: Mutex<Vec<(u64, Vec<u8>)>>,
    frees: Mutex<Vec<u64>>,
}

impl RecordingResidentUploadDispatcher {
    fn new(first_handle: u64) -> Self {
        Self {
            next_handle: AtomicU64::new(first_handle),
            allocations: Mutex::new(Vec::new()),
            uploads: Mutex::new(Vec::new()),
            frees: Mutex::new(Vec::new()),
        }
    }
}

impl OptimizerDispatcher for RecordingResidentUploadDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Ok(Vec::new())
    }

    fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
        self.allocations
            .lock()
            .expect("Fix: resident upload allocation recorder lock should not be poisoned")
            .push(byte_len);
        Ok(self.next_handle.fetch_add(1, Ordering::SeqCst))
    }

    fn upload_resident_many(&self, uploads: &[(u64, &[u8])]) -> Result<(), DispatchError> {
        self.uploads
            .lock()
            .expect("Fix: resident upload recorder lock should not be poisoned")
            .extend(
                uploads
                    .iter()
                    .map(|(handle, bytes)| (*handle, bytes.to_vec())),
            );
        Ok(())
    }

    fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
        self.frees
            .lock()
            .expect("Fix: resident free recorder lock should not be poisoned")
            .push(handle);
        Ok(())
    }
}

#[test]
fn resident_dispatch_input_upload_helper_prepares_and_uploads_encoded_slots() {
    let dispatcher = RecordingResidentUploadDispatcher::new(40);
    let mut staging = Vec::new();

    let handles = upload_resident_dispatch_inputs(
        &dispatcher,
        &mut staging,
        [
            DispatchInput::u32_slice(&[1, 0xAABB_CCDD]),
            DispatchInput::zero_u32_words(2, "resident zeros"),
        ],
    )
    .expect("Fix: resident dispatch input helper should encode and upload slots");

    assert_eq!(handles, [40, 41]);
    assert_eq!(
        *dispatcher
            .allocations
            .lock()
            .expect("Fix: allocation recorder lock should not be poisoned"),
        vec![8, 8]
    );
    assert_eq!(
        *dispatcher
            .uploads
            .lock()
            .expect("Fix: upload recorder lock should not be poisoned"),
        vec![
            (40, vec![1, 0, 0, 0, 0xDD, 0xCC, 0xBB, 0xAA]),
            (41, vec![0; 8]),
        ]
    );
    assert!(
        dispatcher
            .frees
            .lock()
            .expect("Fix: free recorder lock should not be poisoned")
            .is_empty(),
        "successful resident upload should not free live handles"
    );
}

struct FailingResidentAllocDispatcher {
    next_handle: AtomicU64,
    fail_at_call: usize,
    allocations: Mutex<Vec<usize>>,
    frees: Mutex<Vec<u64>>,
}

impl FailingResidentAllocDispatcher {
    fn new(first_handle: u64, fail_at_call: usize) -> Self {
        Self {
            next_handle: AtomicU64::new(first_handle),
            fail_at_call,
            allocations: Mutex::new(Vec::new()),
            frees: Mutex::new(Vec::new()),
        }
    }
}

impl OptimizerDispatcher for FailingResidentAllocDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Ok(Vec::new())
    }

    fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
        let mut allocations = self
            .allocations
            .lock()
            .expect("Fix: failing allocation recorder lock should not be poisoned");
        let call = allocations.len();
        allocations.push(byte_len);
        if call == self.fail_at_call {
            return Err(DispatchError::BackendError(
                "Fix: injected resident allocation failure".to_string(),
            ));
        }
        Ok(self.next_handle.fetch_add(1, Ordering::SeqCst))
    }

    fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
        self.frees
            .lock()
            .expect("Fix: failing allocator free recorder lock should not be poisoned")
            .push(handle);
        Ok(())
    }
}

#[test]
fn resident_buffer_allocator_rolls_back_partial_allocations() {
    let dispatcher = FailingResidentAllocDispatcher::new(70, 2);

    let err = alloc_resident_buffers(&dispatcher, [4, 8, 12], "test resident group")
        .expect_err("Fix: injected allocation failure should surface");

    assert!(
        matches!(err, DispatchError::BackendError(message) if message.contains("injected resident allocation failure"))
    );
    assert_eq!(
        *dispatcher
            .allocations
            .lock()
            .expect("Fix: allocation recorder lock should not be poisoned"),
        vec![4, 8, 12]
    );
    assert_eq!(
        *dispatcher
            .frees
            .lock()
            .expect("Fix: free recorder lock should not be poisoned"),
        vec![70, 71],
        "grouped resident allocation must free every successfully allocated handle on failure"
    );
}
