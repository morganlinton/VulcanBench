use super::*;

pub(super) fn resident_preprocessor_backend(
) -> Result<std::sync::Arc<dyn vyre::VyreBackend>, String> {
    crate::pipeline::shared_dispatch_backend()
        .map_err(|error| format!("vyre-frontend-c: backend unavailable: {error}"))
}

pub(super) struct CachedResidentDispatcher<'a>(pub(super) &'a dyn vyre::VyreBackend);

impl gpu_pipeline::GpuDispatcher for CachedResidentDispatcher<'_> {
    fn dispatch(
        &self,
        program: &vyre::ir::Program,
        inputs: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>, String> {
        let refs: Vec<&[u8]> = inputs.iter().map(Vec::as_slice).collect();
        self.dispatch_refs(program, &refs)
    }

    fn dispatch_borrowed(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        let mut outputs = Vec::new();
        self.dispatch_refs_into(program, inputs, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_borrowed_into(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        self.dispatch_refs_into(program, inputs, outputs)
    }
}

impl CachedResidentDispatcher<'_> {
    fn dispatch_refs(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        let mut outputs = Vec::new();
        self.dispatch_refs_into(program, inputs, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_refs_into(
        &self,
        program: &vyre::ir::Program,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        let mut params = Vec::with_capacity(program.buffers().len());
        for buffer in program.buffers() {
            params.push(buffer.count as u64);
        }
        let stage = program.entry_op_id.as_deref().unwrap_or("<anonymous>");
        let key = crate::pipeline::stage_pipeline_cache_key(stage, &params);
        crate::pipeline::dispatch_borrowed_stage_cached_into(
            self.0,
            key,
            || Ok(program.clone()),
            inputs,
            &vyre::DispatchConfig::default(),
            outputs,
        )
        .map_err(|error| format!("backend dispatch: {error}"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_resident_dispatcher_overrides_borrowed_dispatch() {
        let source = include_str!("resident_backend.rs");
        assert!(
            source.contains("fn dispatch_borrowed"),
            "Fix: CachedResidentDispatcher must override dispatch_borrowed so GPU preprocessing does not copy borrowed inputs into owned Vec<Vec<u8>> before resident dispatch."
        );
        assert!(
            source.contains("fn dispatch_borrowed_into"),
            "Fix: CachedResidentDispatcher must override dispatch_borrowed_into so GPU preprocessing reuses caller-owned output slots."
        );
        assert!(
            source.contains("self.dispatch_refs_into(program, inputs, outputs)"),
            "Fix: borrowed resident dispatch must route slices and caller-owned outputs directly into the cached backend helper."
        );
    }
}
