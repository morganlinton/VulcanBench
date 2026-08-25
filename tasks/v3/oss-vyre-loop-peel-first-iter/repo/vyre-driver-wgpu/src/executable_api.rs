use crate::{pipeline, WgpuBackend};

/// Progressive staging: `Program -> WgpuIR -> WGSL -> pipeline`.
///
/// `WgpuIR` is the intermediate artifact returned by
/// [`WgpuBackend::compile`]. Each downstream stage (WGSL emission,
/// pipeline creation) is independently cacheable and testable.
pub struct WgpuIR {
    /// Cached pipeline that already embeds the naga::Module, WGSL
    /// shader source, bind-group layout, and workgroup size.
    pub pipeline: pipeline::WgpuPipeline,
}

impl vyre_driver::Executable for WgpuBackend {
    fn dispatch(
        &self,
        program: &vyre_foundation::ir::Program,
        inputs: &[vyre_driver::MemoryRef<'_>],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Vec<vyre_driver::Memory>, vyre_driver::BackendError> {
        <Self as vyre_driver::VyreBackend>::dispatch_borrowed(self, program, inputs, config)
    }
}

impl WgpuBackend {
    /// Compile a program once for repeated dispatch.
    pub fn compile(
        &self,
        program: &vyre_foundation::ir::Program,
    ) -> Result<WgpuIR, vyre_driver::BackendError> {
        let config = vyre_driver::DispatchConfig::default();
        self.validate_with_cache(program)?;
        let pipeline = crate::pipeline::WgpuPipeline::compile_with_device_queue(
            program,
            &config,
            self.adapter_info.clone(),
            self.enabled_features,
            self.current_device_queue(),
            self.dispatch_arena_snapshot(),
            self.current_persistent_pool(),
            self.pipeline_cache.clone(),
            self.bind_group_layout_cache.clone(),
        )?;
        Ok(WgpuIR {
            pipeline: (*pipeline).clone(),
        })
    }

    /// Dispatch a previously compiled program artifact.
    pub fn dispatch_compiled(
        &self,
        compiled: &WgpuIR,
        inputs: &[vyre_driver::MemoryRef<'_>],
        config: &vyre_driver::DispatchConfig,
    ) -> Result<Vec<vyre_driver::Memory>, vyre_driver::BackendError> {
        vyre_driver::CompiledPipeline::dispatch_borrowed(&compiled.pipeline, inputs, config)
    }
}
