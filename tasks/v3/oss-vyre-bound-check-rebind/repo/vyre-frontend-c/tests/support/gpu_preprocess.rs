use std::path::{Path, PathBuf};

use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    GpuDispatcher, IncludeEventResidency, IncludeLoader,
};

pub(crate) struct ReferenceDispatcher;

impl GpuDispatcher for ReferenceDispatcher {
    fn dispatch(
        &self,
        program: &vyre::ir::Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, String> {
        let value_inputs: Vec<vyre_reference::value::Value> =
            inputs.iter().cloned().map(Into::into).collect();
        let outs = vyre_reference::reference_eval(program, &value_inputs)
            .map_err(|error| format!("reference_eval: {error}"))?;
        Ok(outs.into_iter().map(|value| value.to_bytes()).collect())
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

pub(crate) struct NullLoader;

impl IncludeLoader for NullLoader {
    fn load(
        &self,
        _path: &[u8],
        _is_system: bool,
        _is_next: bool,
        _from: &Path,
    ) -> Result<Option<(PathBuf, std::sync::Arc<[u8]>)>, String> {
        Ok(None)
    }
}

pub(crate) trait IncludeEventResidencyAssert {
    fn is_gpu_resident_request(&self) -> bool;
}

impl IncludeEventResidencyAssert for IncludeEventResidency {
    fn is_gpu_resident_request(&self) -> bool {
        matches!(self, IncludeEventResidency::GpuResidentRequest)
    }
}
