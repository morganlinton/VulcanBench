use super::{Megakernel, MegakernelDispatchStats};
use crate::megakernel::readback::MegakernelReadback;
use crate::PipelineError;
use vyre_driver::backend::OutputBuffers;

impl Megakernel {
    /// Dispatch with a caller-supplied IO queue and decode the strict
    /// megakernel readback ABI.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch fails or returned buffers do not
    /// match the compiled megakernel ABI.
    pub fn dispatch_with_io_queue_readback(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
        io_queue_bytes: Vec<u8>,
    ) -> Result<MegakernelReadback, PipelineError> {
        let outputs = self.dispatch_with_io_queue_borrowed(
            &control_bytes,
            &ring_bytes,
            &debug_log_bytes,
            &io_queue_bytes,
        )?;
        MegakernelReadback::from_outputs(outputs, self.slot_count)
    }

    /// Dispatch owned buffers with a caller-supplied IO queue and decode the
    /// strict megakernel readback ABI into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch_with_io_queue_readback_into(
        &self,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
        io_queue_bytes: Vec<u8>,
        readback: &mut MegakernelReadback,
        outputs: &mut OutputBuffers,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        self.dispatch_with_io_queue_readback_borrowed_into(
            &control_bytes,
            &ring_bytes,
            &debug_log_bytes,
            &io_queue_bytes,
            readback,
            outputs,
        )
    }

    /// Dispatch borrowed buffers with a caller-supplied IO queue and decode
    /// the strict megakernel readback ABI.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue_readback`].
    pub fn dispatch_with_io_queue_readback_borrowed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
    ) -> Result<MegakernelReadback, PipelineError> {
        let outputs = self.dispatch_with_io_queue_borrowed(
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
        )?;
        MegakernelReadback::from_outputs(outputs, self.slot_count)
    }

    /// Dispatch borrowed buffers with a caller-supplied IO queue, decode the
    /// strict megakernel readback ABI, and return dispatch instrumentation.
    ///
    /// # Errors
    ///
    /// See [`Megakernel::dispatch_with_io_queue_readback_borrowed`].
    pub fn dispatch_with_io_queue_readback_borrowed_observed(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
    ) -> Result<(MegakernelReadback, MegakernelDispatchStats), PipelineError> {
        let output = self.dispatch_with_io_queue_borrowed_observed(
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
        )?;
        let stats = output.stats;
        let readback = MegakernelReadback::from_outputs(output.buffers, self.slot_count)?;
        Ok((readback, stats))
    }

    /// Dispatch borrowed buffers with a caller-supplied IO queue, decode the
    /// strict megakernel readback ABI into caller-owned storage, and return
    /// dispatch instrumentation.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch_with_io_queue_readback_borrowed_into(
        &self,
        control_bytes: &[u8],
        ring_bytes: &[u8],
        debug_log_bytes: &[u8],
        io_queue_bytes: &[u8],
        readback: &mut MegakernelReadback,
        outputs: &mut OutputBuffers,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        let stats = self.dispatch_with_io_queue_borrowed_into(
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
            outputs,
        )?;
        MegakernelReadback::drain_outputs_into(outputs, self.slot_count, readback)?;
        Ok(stats)
    }
}
