//! Handle to a dispatch in flight.

use crate::backend::{private, BackendError, OutputBuffers};

/// Handle to a dispatch in flight. Returned by
/// [`crate::backend::VyreBackend::dispatch_async`].
///
/// Consumer shape:
///
/// ```no_run
/// # use std::sync::Arc;
/// # use vyre::{Program, VyreBackend, DispatchConfig};
/// # fn run(backend: Arc<dyn VyreBackend>, program: &Program) -> Result<(), vyre::BackendError> {
/// let pending = backend.dispatch_async(program, &[vec![0u8; 64]], &DispatchConfig::default())?;
/// while !pending.is_ready() {
///     // Host-side work overlaps with the GPU dispatch.
/// }
/// let _outputs = pending.await_result()?;
/// # Ok(())
/// # }
/// ```
///
/// Backends that do not overlap host and device work return a
/// trivially-ready handle built by the default
/// [`crate::backend::VyreBackend::dispatch_async`] implementation  -  the consumer code
/// above still works, just without the overlap.
pub trait PendingDispatch: private::Sealed + Send + Sync {
    /// Non-blocking probe. Returns `true` when
    /// [`PendingDispatch::await_result`] would complete without
    /// blocking the caller thread.
    ///
    /// Backends that cannot probe without cost (no map_async
    /// equivalent) return `true` unconditionally; consumers will
    /// simply block inside `await_result`.
    fn is_ready(&self) -> bool;

    /// Consume the handle and return the dispatch's output buffers.
    ///
    /// Blocks the caller thread until the dispatch completes. Calling
    /// this on a handle whose `is_ready` reports `true` does not
    /// block.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the dispatch failed on the device.
    fn await_result(self: Box<Self>) -> Result<Vec<Vec<u8>>, BackendError>;

    /// Consume the handle and write output buffers into caller-owned storage.
    ///
    /// The default preserves the return-value contract while reusing existing
    /// output slots where possible. Backends with host readback staging should
    /// override this to collect directly into `outputs`.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the dispatch failed on the device.
    fn await_result_into(self: Box<Self>, outputs: &mut OutputBuffers) -> Result<(), BackendError> {
        let result = self.await_result()?;
        crate::backend::dispatch_result::replace_output_buffers_preserving_slots(result, outputs);
        Ok(())
    }

    /// Async variant of [`PendingDispatch::await_result`].
    ///
    /// Default implementation delegates to the synchronous
    /// [`PendingDispatch::await_result`]; backends that overlap host
    /// and device work should override this with a non-blocking await.
    ///
    /// `where Self: Sized` keeps `dyn PendingDispatch` object-safe;
    /// call this on concrete pending-dispatch types.
    fn await_result_async(
        self: Box<Self>,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<u8>>, BackendError>> + Send
    where
        Self: Sized,
    {
        async { self.await_result() }
    }
}

/// Default [`PendingDispatch`] adapter used by the synchronous
/// [`VyreBackend::dispatch_async`] default.
///
/// Holds the already-computed output buffers; `is_ready` is always
/// `true` and `await_result` returns the buffers verbatim.
pub(crate) struct ReadyPending {
    pub(crate) outputs: Vec<Vec<u8>>,
}

impl private::Sealed for ReadyPending {}

impl PendingDispatch for ReadyPending {
    fn is_ready(&self) -> bool {
        true
    }
    fn await_result(self: Box<Self>) -> Result<Vec<Vec<u8>>, BackendError> {
        Ok(self.outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_dispatch_default_into_preserves_output_slots() {
        let mut outputs = vec![Vec::with_capacity(8)];
        let outputs_addr = outputs.as_ptr() as usize;
        let slot_addr = outputs[0].as_ptr() as usize;

        Box::new(ReadyPending {
            outputs: vec![vec![1, 2, 3]],
        })
        .await_result_into(&mut outputs)
        .expect("Fix: ready pending output should write into caller storage");

        assert_eq!(outputs, vec![vec![1, 2, 3]]);
        assert_eq!(outputs.as_ptr() as usize, outputs_addr);
        assert_eq!(outputs[0].as_ptr() as usize, slot_addr);
    }
}
