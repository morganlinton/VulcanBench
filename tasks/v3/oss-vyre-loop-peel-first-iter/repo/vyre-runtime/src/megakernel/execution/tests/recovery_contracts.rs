use super::*;
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

/// A backend whose compiled pipeline id changes on every `compile_native` call.
/// Used to verify that `pipeline_id()` reflects the REBUILT pipeline's id after
/// `recover_after_device_loss`, not the stale id from construction.
struct RotatingIdBackend {
    compile_count: Arc<AtomicU32>,
}

impl vyre_driver::backend::private::Sealed for RotatingIdBackend {}

impl VyreBackend for RotatingIdBackend {
    fn id(&self) -> &'static str {
        "rotating-id"
    }

    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(inputs.to_vec())
    }

    fn compile_native(
        &self,
        _program: &Program,
        _config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, vyre_driver::BackendError> {
        let n = self.compile_count.fetch_add(1, AtomicOrdering::SeqCst);
        Ok(Some(Arc::new(RotatingIdPipeline {
            id: format!("rotating-id:pipeline:gen{n}"),
        })))
    }
}

struct RotatingIdPipeline {
    id: String,
}

impl vyre_driver::backend::private::Sealed for RotatingIdPipeline {}

impl CompiledPipeline for RotatingIdPipeline {
    fn id(&self) -> &str {
        &self.id
    }

    fn dispatch(
        &self,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(vec![vec![]])
    }

    fn dispatch_borrowed_into(
        &self,
        _inputs: &[&[u8]],
        _config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        if outputs.is_empty() {
            outputs.push(Vec::new());
        }
        outputs[0].clear();
        Ok(())
    }
}

/// Regression: `pipeline_id` was a plain `String` field set at construction.
/// `recover_after_device_loss` installed a new compiled pipeline via
/// `ArcSwap::store` but never updated `pipeline_id`, so `pipeline_id()` returned
/// the pre-recovery ID even after recovery — a stale identity string.
///
/// After the fix, `pipeline_id` is `ArcSwap<String>` and
/// `recover_after_device_loss` atomically stores the rebuilt pipeline's id.
/// This test asserts that `pipeline_id()` changes to reflect the rebuilt
/// pipeline's id after recovery.
///
/// The `RotatingIdBackend` produces a different compiled-pipeline id on each
/// `compile_native` call (generation 0, 1, 2, ...). Without the fix, the
/// first id (`gen0`) stays cached in the plain `String` field forever. With
/// the fix, after recovery the id becomes `gen1`.
#[test]
fn pipeline_id_reflects_rebuilt_pipeline_after_recovery() {
    let compile_count = Arc::new(AtomicU32::new(0));
    let backend = Arc::new(RotatingIdBackend {
        compile_count: Arc::clone(&compile_count),
    });
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: rotating-id backend must bootstrap megakernel");

    // After construction, compile_count == 1 (one compile_native for gen0).
    let id_before = kernel.pipeline_id();
    assert_eq!(
        id_before, "rotating-id:pipeline:gen0",
        "Fix: pipeline_id at construction must be gen0, got: {id_before}"
    );

    kernel
        .recover_after_device_loss()
        .expect("Fix: rotating-id backend must recompile the megakernel during recovery");

    // After recovery, compile_count == 2 (one more compile_native for gen1).
    let id_after = kernel.pipeline_id();
    assert_eq!(
        id_after, "rotating-id:pipeline:gen1",
        "Fix: pipeline_id after recovery must be gen1 (the rebuilt pipeline's id), \
         not the stale gen0 id from construction. Got: {id_after}"
    );
    assert_ne!(
        id_before, id_after,
        "Fix: pipeline_id must change after recovery when the rebuilt pipeline has a different id"
    );
}

#[test]
fn recovery_retry_preserves_caller_output_slots() {
    let mut outputs = vec![Vec::with_capacity(8)];
    let outputs_addr = outputs.as_ptr() as usize;
    let slot_addr = outputs[0].as_ptr() as usize;
    let dispatch_calls = Arc::new(AtomicUsize::new(0));
    let backend = Arc::new(RecoveringBackend {
        dispatch_calls: Arc::clone(&dispatch_calls),
        expected_outputs_addr: outputs_addr,
        expected_slot_addr: slot_addr,
    });
    let kernel = Megakernel::bootstrap_sharded(backend, 1, 1, Vec::new())
        .expect("Fix: recovering backend must compile megakernel");
    let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
    let ring = Megakernel::try_encode_empty_ring(1).unwrap();
    let debug = Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
    let io_queue = io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap();

    let stats = kernel
        .dispatch_with_io_queue_borrowed_into(&control, &ring, &debug, &io_queue, &mut outputs)
        .expect("Fix: recovery retry must reuse caller-owned output storage");

    assert!(stats.recovered_after_device_loss);
    assert_eq!(dispatch_calls.load(Ordering::SeqCst), 2);
    assert_eq!(outputs, vec![vec![9, 8, 7, 6]]);
    assert_eq!(outputs.as_ptr() as usize, outputs_addr);
    assert_eq!(outputs[0].as_ptr() as usize, slot_addr);
}

