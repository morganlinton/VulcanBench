use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

use crate::backend::dispatch::CudaBackend;
use crate::backend::resident::CudaResidentBuffer;

impl CudaBackend {
    /// Dispatch a Program using caller-provided CUDA-resident buffers.
    pub fn dispatch_resident(
        &self,
        program: &Program,
        handles: &[CudaResidentBuffer],
        config: &DispatchConfig,
    ) -> Result<(), BackendError> {
        if crate::instrumentation::cuda_resident_borrowed_fallback_enabled() {
            return self
                .dispatch_resident_via_borrowed(program, handles, config)
                .map(|_| ());
        }
        {
            let prepared = self.prepare_resident_dispatch(program, handles, config)?;
            let (ptx_src, ptx_source_key) =
                self.ptx_for_program_cached_with_key(program, config)?;
            let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
            self.dispatch_resident_async_concrete_with_ptx_key(
                program, handles, config, &ptx_src, module_key, false, None, false, &prepared,
            )?;
            return Ok(());
        }
    }
}
