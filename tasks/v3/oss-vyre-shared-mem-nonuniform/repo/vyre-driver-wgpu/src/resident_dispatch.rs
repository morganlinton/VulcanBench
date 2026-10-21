//! Resident dispatch helpers for WGPU backend resources.

use crate::WgpuBackend;
use std::time::Instant;
use vyre_driver::CompiledPipeline;
use vyre_foundation::ir::Program;

/// Dispatch a program with backend-resident resources and return timing.
pub(crate) fn dispatch_resident_timed(
    backend: &WgpuBackend,
    program: &Program,
    resources: &[vyre_driver::Resource],
    config: &vyre_driver::DispatchConfig,
) -> Result<vyre_driver::TimedDispatchResult, vyre_driver::BackendError> {
    let started = Instant::now();
    let pipeline = backend.compile_resident_pipeline_cached(program, config)?;
    let timed = pipeline.dispatch_persistent_handles_timed(resources, config)?;
    Ok(vyre_driver::TimedDispatchResult {
        outputs: timed.outputs,
        wall_ns: elapsed_nanos_u64(started, "resident timed dispatch")?,
        device_ns: timed.device_ns,
        enqueue_ns: timed.enqueue_ns,
        wait_ns: timed.wait_ns,
    })
}

fn elapsed_nanos_u64(start: Instant, label: &str) -> Result<u64, vyre_driver::BackendError> {
    u64::try_from(start.elapsed().as_nanos()).map_err(|source| {
        vyre_driver::BackendError::new(format!(
            "{label} elapsed time cannot fit u64 nanoseconds: {source}. Fix: split or timeout the dispatch before telemetry overflows."
        ))
    })
}
