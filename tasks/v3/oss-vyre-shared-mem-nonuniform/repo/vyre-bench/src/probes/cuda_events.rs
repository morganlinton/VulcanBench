use crate::api::case::{BenchContext, BenchError};
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::Program;

/// Result of a dispatch that includes backend device timing when available.
pub struct CudaEventResult {
    /// The outputs of the dispatch.
    pub outputs: Vec<Vec<u8>>,
    /// Host wall clock time.
    pub wall_ns: u64,
    /// Time elapsed on the device.
    pub device_ns: Option<u64>,
    /// Time spent submitting the kernel to the queue.
    pub kernel_queue_submit_ns: Option<u64>,
    /// Time spent executing the kernel on the device.
    pub kernel_execute_ns: Option<u64>,
    /// Time spent syncing with the device.
    pub device_sync_ns: Option<u64>,
}

/// Dispatch a program and preserve backend device timing when available.
pub fn dispatch_with_events(
    ctx: &BenchContext,
    prog: &Program,
    inputs: &[Vec<u8>],
    config: &DispatchConfig,
) -> Result<CudaEventResult, BenchError> {
    let timed = ctx
        .dispatch_timed(prog, inputs, config)
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;

    Ok(CudaEventResult {
        outputs: timed.outputs,
        wall_ns: timed.wall_ns,
        device_ns: timed.device_ns,
        kernel_queue_submit_ns: timed.enqueue_ns,
        kernel_execute_ns: timed.device_ns,
        device_sync_ns: timed.wait_ns,
    })
}
