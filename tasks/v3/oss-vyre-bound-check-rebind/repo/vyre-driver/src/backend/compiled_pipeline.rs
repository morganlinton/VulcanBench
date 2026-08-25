//! Pre-compiled pipeline trait.

use crate::backend::{
    private, BackendError, DispatchConfig, OutputBuffers, Resource, TimedDispatchResult,
};

/// A program that has been pre-compiled by a backend, ready for repeated
/// dispatch with new inputs without paying compilation cost on each call.
///
/// Build one with [`crate::pipeline::compile`]. Backends that override
/// [`crate::backend::VyreBackend::compile_native`] return a cached pipeline (skipping
/// shader compilation, pipeline-layout creation, and bind-group-layout
/// creation on every dispatch); backends that don't get a transparent
/// passthrough whose semantics are identical to repeated [`crate::backend::VyreBackend::dispatch`].
///
/// `CompiledPipeline::dispatch` MUST be bit-identical to
/// `VyreBackend::dispatch(program, inputs, config)` for the program this
/// pipeline was compiled from. Any divergence is a backend bug.
pub trait CompiledPipeline: private::Sealed + Send + Sync {
    /// Stable identifier for this pipeline (typically `<backend>:<program-fingerprint>`).
    ///
    /// Used by certificates and debugging to confirm a particular cached
    /// pipeline was reused vs recompiled.
    fn id(&self) -> &str;

    /// Dispatch the precompiled pipeline with new inputs.
    ///
    /// Bit-identical to `VyreBackend::dispatch(self.program, inputs, config)`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    /// The error message always includes a `Fix: ` remediation section.
    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError>;

    /// Dispatch the precompiled pipeline with borrowed input buffers.
    ///
    /// Backends may override this to bind caller-owned byte slices directly.
    /// The default allocates the owned input vector once, preserving the
    /// existing [`CompiledPipeline::dispatch`] contract for current backends.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let owned = crate::backend::clone_borrowed_inputs_for_dispatch(
            inputs,
            "compiled pipeline input staging",
        )?;
        let outputs = self.dispatch(&owned, config)?;
        crate::observability::record_dispatch_io(inputs, &outputs);
        Ok(outputs)
    }

    /// Dispatch with backend-owned timing.
    ///
    /// Default timing is host wall time. Native pipeline implementations may
    /// attach device elapsed time without exposing driver APIs to callers.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed_timed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = std::time::Instant::now();
        let outputs = self.dispatch_borrowed(inputs, config)?;
        Ok(TimedDispatchResult {
            outputs,
            wall_ns: crate::backend::checked_elapsed_wall_ns(
                started,
                "compiled pipeline dispatch",
            )?,
            device_ns: None,
            enqueue_ns: None,
            wait_ns: None,
        })
    }

    /// Dispatch the precompiled pipeline with borrowed inputs and write
    /// outputs into caller-owned storage.
    ///
    /// Backends may override this to reuse output buffers across repeated
    /// dispatches. The default preserves the existing return-value contract and
    /// copies returned bytes into existing output slots where possible.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_borrowed_into(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let result = self.dispatch_borrowed(inputs, config)?;
        let stats = crate::backend::replace_output_buffers_preserving_slots_with_memory_stats(
            result, outputs,
        );
        crate::observability::record_output_replacement_stats(stats);
        Ok(())
    }

    /// Dispatch several independent borrowed-input submissions for the same
    /// compiled program.
    ///
    /// Backends with native queues/streams should override this to enqueue the
    /// whole batch before waiting for readback. The default is intentionally
    /// semantic, not fast: it preserves bit-identical behavior for backends
    /// that only implement the single-dispatch path.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any item cannot complete dispatch.
    fn dispatch_borrowed_batched(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = crate::backend::reserved_batch_output_slots(
            batches.len(),
            "compiled borrowed batch outputs",
        )?;
        self.dispatch_borrowed_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    /// Dispatch several borrowed-input submissions and write every item's
    /// outputs into caller-owned storage.
    ///
    /// The outer vector is one entry per batch item. Each inner
    /// [`OutputBuffers`] preserves already-allocated output slots where the
    /// backend can collect directly into caller storage. This is the hot
    /// repeated-dispatch contract: callers can keep one output arena per batch
    /// lane instead of rebuilding `Vec<Vec<u8>>` shells after every launch.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any item cannot complete dispatch.
    fn dispatch_borrowed_batched_into(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        crate::backend::resize_batch_output_slots(
            outputs,
            batches.len(),
            "compiled borrowed batch outputs",
        )?;
        for (batch, slot) in batches.iter().zip(outputs.iter_mut()) {
            self.dispatch_borrowed_into(batch, config, slot)?;
        }
        Ok(())
    }

    /// Dispatch the precompiled pipeline with mixed host/resident handles.
    ///
    /// This is the P-41 contract: keep control, ring, IO, and debug buffers
    /// GPU-resident across launches.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_persistent_handles(
        &self,
        _inputs: &[Resource],
        _config: &DispatchConfig,
    ) -> Result<OutputBuffers, BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "persistent handle dispatch".to_string(),
            backend: "unspecified".to_string(),
        })
    }

    /// Dispatch the precompiled pipeline with resident handles and backend-owned timing.
    ///
    /// Default timing is host wall time around the resident-handle dispatch.
    /// Native backends should override this when they can expose device event
    /// timing for resident compiled launches.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_persistent_handles_timed(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let started = std::time::Instant::now();
        let outputs = self.dispatch_persistent_handles(inputs, config)?;
        Ok(TimedDispatchResult {
            outputs,
            wall_ns: crate::backend::checked_elapsed_wall_ns(
                started,
                "compiled persistent handle dispatch",
            )?,
            device_ns: None,
            enqueue_ns: None,
            wait_ns: None,
        })
    }

    /// Dispatch the precompiled pipeline with mixed host/resident handles and
    /// write readback bytes into caller-owned output storage.
    ///
    /// This is the single-submission resident reuse contract. Backends that
    /// still need host-visible results must fill existing output slots instead
    /// of forcing callers to rebuild `Vec<Vec<u8>>` shells on every resident
    /// dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch.
    fn dispatch_persistent_handles_into(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let result = self.dispatch_persistent_handles(inputs, config)?;
        crate::observability::record_dispatch_io(&[], &result);
        let stats = crate::backend::replace_output_buffers_preserving_slots_with_memory_stats(
            result, outputs,
        );
        crate::observability::record_output_replacement_stats(stats);
        Ok(())
    }

    /// Dispatch the precompiled pipeline with resident handles and return
    /// resident resources for its ordered outputs without host readback.
    ///
    /// This is the zero-copy chaining contract for multi-stage GPU pipelines:
    /// callers allocate resident resources for every non-shared binding, pass
    /// them in binding order, and receive the output subset in stable output
    /// order so those buffers can feed later kernels directly. The returned
    /// resources remain owned by the backend and must be freed by the caller
    /// through [`crate::backend::VyreBackend::free_resident`] when no longer
    /// needed.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the backend cannot complete dispatch or
    /// cannot preserve outputs as resident resources.
    fn dispatch_persistent_resource_outputs(
        &self,
        _inputs: &[Resource],
        _config: &DispatchConfig,
    ) -> Result<Vec<Resource>, BackendError> {
        Err(BackendError::UnsupportedFeature {
            name: "persistent resident output dispatch".to_string(),
            backend: "unspecified".to_string(),
        })
    }

    /// Dispatch several resident-handle submissions for the same compiled
    /// program.
    ///
    /// Native backends should override this to record/replay the batch through
    /// one device submission or graph replay. The default preserves semantics
    /// for backends that only implement the single-submission resident path.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any item cannot complete dispatch.
    fn dispatch_persistent_handles_batched(
        &self,
        batches: &[&[Resource]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = crate::backend::reserved_batch_output_slots(
            batches.len(),
            "compiled resident batch outputs",
        )?;
        self.dispatch_persistent_handles_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    /// Dispatch several resident-handle submissions and write readbacks into
    /// caller-owned batch output storage.
    ///
    /// This is the resident equivalent of
    /// [`CompiledPipeline::dispatch_borrowed_batched_into`]. It keeps repeated
    /// megakernel/dataflow evaluations from rebuilding host output shells when
    /// readback is still requested.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any item cannot complete dispatch.
    fn dispatch_persistent_handles_batched_into(
        &self,
        batches: &[&[Resource]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        crate::backend::resize_batch_output_slots(
            outputs,
            batches.len(),
            "compiled resident batch outputs",
        )?;
        for (batch, slot) in batches.iter().zip(outputs.iter_mut()) {
            self.dispatch_persistent_handles_into(batch, config, slot)?;
        }
        Ok(())
    }

    /// Dispatch several fixed megakernel ABI resident-resource rows directly.
    ///
    /// Megakernel resident dispatch always submits exactly four resources:
    /// control, ring, debug log, and IO queue. This hook lets native backends
    /// consume that fixed row shape without the runtime rebuilding a transient
    /// `Vec<&[Resource]>` around every hot batch. Backends that only implement
    /// the generic slice batch path inherit the semantic adapter below.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when any row cannot complete dispatch.
    fn dispatch_persistent_handle_rows_into(
        &self,
        rows: &[[Resource; 4]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let batches = borrowed_resource_rows(rows)?;
        self.dispatch_persistent_handles_batched_into(&batches, config, outputs)
    }
}

fn borrowed_resource_rows(rows: &[[Resource; 4]]) -> Result<Vec<&[Resource]>, BackendError> {
    let mut batches = Vec::new();
    batches
        .try_reserve_exact(rows.len())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {} fixed megakernel resident row view(s): {error}. Split the resident batch or override dispatch_persistent_handle_rows_into natively.",
                rows.len()
            ),
        })?;
    batches.extend(rows.iter().map(|row| row.as_slice()));
    Ok(batches)
}
