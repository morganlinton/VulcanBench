use super::*;

#[test]
fn borrowed_dispatch_uses_grid_sync_splitter_when_backend_lacks_native_barrier() {
    let backend = Arc::new(GridSyncBackend::default());
    let kernel = Megakernel::compile_bootstrap(backend.clone(), 1, 1, grid_sync_program())
        .expect("Fix: grid-sync test megakernel must compile");
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let debug = Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    let io_queue = io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap();

    kernel
        .dispatch_with_io_queue_borrowed(&control, &ring, &debug, &io_queue)
        .expect("Fix: grid-sync split dispatch must succeed through borrowed buffers");

    let segment_lengths = backend
        .segment_lengths
        .lock()
        .expect("Fix: grid-sync recording mutex must not be poisoned")
        .clone();
    assert_eq!(segment_lengths, vec![0, 1, 1]);
}


