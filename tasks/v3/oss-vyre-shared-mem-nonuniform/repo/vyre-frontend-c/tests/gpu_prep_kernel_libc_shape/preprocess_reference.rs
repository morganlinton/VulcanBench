use vyre_libs::parsing::c::preprocess::gpu_pipeline::GpuDispatcher;

pub(super) struct ReferenceDispatcher;

impl GpuDispatcher for ReferenceDispatcher {
    fn dispatch(
        &self,
        program: &vyre::ir::Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, String> {
        let value_inputs: Vec<vyre_reference::value::Value> =
            inputs.iter().cloned().map(Into::into).collect();
        let outs = vyre_reference::reference_eval(program, &value_inputs)
            .map_err(|e| format!("reference_eval: {e}"))?;
        Ok(outs.into_iter().map(|value| value.to_bytes()).collect())
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}
