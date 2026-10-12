//! Backend-neutral compiled-pipeline construction.

use super::{
    dispatch_policy_cache_string, try_normalized_program_cache_digest, CompiledPipelineBuild,
    PipelineCacheSnapshot, PipelinePrewarmReport, PipelineReproManifest,
};
use crate::backend::{
    BackendError, CompiledPipeline, DispatchConfig, OutputBuffers, TimedDispatchResult, VyreBackend,
};
use std::sync::Arc;
use vyre_foundation::ir::Program;

/// Compile a borrowed program into a reusable backend pipeline.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile(
    backend: Arc<dyn VyreBackend>,
    program: &Program,
    config: &DispatchConfig,
) -> Result<Arc<dyn CompiledPipeline>, BackendError> {
    compile_shared(backend, Arc::new(program.clone()), config)
}

/// Compile an owned program into a reusable backend pipeline without cloning
/// the IR.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile_owned(
    backend: Arc<dyn VyreBackend>,
    program: Program,
    config: &DispatchConfig,
) -> Result<Arc<dyn CompiledPipeline>, BackendError> {
    compile_shared(backend, Arc::new(program), config)
}

/// Compile an already shared program into a reusable backend pipeline.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile_shared(
    backend: Arc<dyn VyreBackend>,
    program: Arc<Program>,
    config: &DispatchConfig,
) -> Result<Arc<dyn CompiledPipeline>, BackendError> {
    if let Some(message) = program.top_level_region_violation() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: megakernel/runtime admission requires a top-level Region-wrapped Program. {message}"
            ),
        });
    }
    crate::validation::validate_program_for_backend(backend.as_ref(), &program, config)?;

    if let Some(native) = backend.compile_native_shared(Arc::clone(&program), config)? {
        return Ok(native);
    }
    Ok(Arc::new(PassthroughPipeline {
        id: format!("{}:passthrough", backend.id()),
        backend,
        program,
        compile_config: config.clone(),
    }))
}

/// Compile a borrowed program and report backend cache telemetry.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile_with_telemetry(
    backend: Arc<dyn VyreBackend>,
    program: &Program,
    config: &DispatchConfig,
) -> Result<CompiledPipelineBuild, BackendError> {
    compile_shared_with_telemetry(backend, Arc::new(program.clone()), config)
}

/// Compile an owned program and report backend cache telemetry without cloning
/// the IR.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile_owned_with_telemetry(
    backend: Arc<dyn VyreBackend>,
    program: Program,
    config: &DispatchConfig,
) -> Result<CompiledPipelineBuild, BackendError> {
    compile_shared_with_telemetry(backend, Arc::new(program), config)
}

/// Compile an already shared program and report backend cache telemetry.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn compile_shared_with_telemetry(
    backend: Arc<dyn VyreBackend>,
    program: Arc<Program>,
    config: &DispatchConfig,
) -> Result<CompiledPipelineBuild, BackendError> {
    let backend_id = backend.id().to_owned();
    let program_digest = try_normalized_program_cache_digest(&program).map_err(|error| {
        BackendError::new(format!("compiled-pipeline cache digest failed: {error}"))
    })?;
    let dispatch_policy = dispatch_policy_cache_string(config);
    let before = backend.pipeline_cache_snapshot();
    let pipeline = compile_shared(Arc::clone(&backend), program, config)?;
    let after = backend.pipeline_cache_snapshot();
    let cache_hit = cache_status_from_snapshots(before, after);
    let manifest = PipelineReproManifest::new(
        backend_id,
        pipeline.id().to_owned(),
        program_digest,
        dispatch_policy,
        cache_hit,
    );
    Ok(CompiledPipelineBuild {
        pipeline,
        cache_hit,
        manifest,
    })
}

/// Prewarm a borrowed program into the backend pipeline cache.
///
/// This is the explicit first-dispatch removal path: it validates the program,
/// runs the backend's native compile/reflection/cache path, records cache
/// telemetry, and drops the returned pipeline handle after the cache has been
/// populated. Callers that want to keep the handle should use
/// [`compile_with_telemetry`] instead.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn prewarm(
    backend: Arc<dyn VyreBackend>,
    program: &Program,
    config: &DispatchConfig,
) -> Result<PipelinePrewarmReport, BackendError> {
    prewarm_shared(backend, Arc::new(program.clone()), config)
}

/// Prewarm an owned program without cloning the IR.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn prewarm_owned(
    backend: Arc<dyn VyreBackend>,
    program: Program,
    config: &DispatchConfig,
) -> Result<PipelinePrewarmReport, BackendError> {
    prewarm_shared(backend, Arc::new(program), config)
}

/// Prewarm an already shared program allocation.
///
/// # Errors
///
/// Returns when program validation fails or the backend cannot compile the
/// program for the supplied dispatch policy.
pub fn prewarm_shared(
    backend: Arc<dyn VyreBackend>,
    program: Arc<Program>,
    config: &DispatchConfig,
) -> Result<PipelinePrewarmReport, BackendError> {
    let build = compile_shared_with_telemetry(backend, program, config)?;
    Ok(PipelinePrewarmReport {
        pipeline_id: build.pipeline.id().to_owned(),
        cache_hit: build.cache_hit,
        manifest: build.manifest,
    })
}

fn cache_status_from_snapshots(
    before: Option<PipelineCacheSnapshot>,
    after: Option<PipelineCacheSnapshot>,
) -> Option<bool> {
    let (before, after) = (before?, after?);
    let hits = after.hits.saturating_sub(before.hits);
    let misses = after.misses.saturating_sub(before.misses);
    if hits > 0 {
        Some(true)
    } else if misses > 0 {
        Some(false)
    } else {
        None
    }
}

struct PassthroughPipeline {
    id: String,
    backend: Arc<dyn VyreBackend>,
    program: Arc<Program>,
    compile_config: DispatchConfig,
}

impl crate::backend::private::Sealed for PassthroughPipeline {}

impl CompiledPipeline for PassthroughPipeline {
    fn id(&self) -> &str {
        &self.id
    }

    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let effective = if *config == DispatchConfig::default() {
            &self.compile_config
        } else {
            config
        };
        self.backend.dispatch(&self.program, inputs, effective)
    }

    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let effective = if *config == DispatchConfig::default() {
            &self.compile_config
        } else {
            config
        };
        self.backend
            .dispatch_borrowed(&self.program, inputs, effective)
    }

    fn dispatch_borrowed_timed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<TimedDispatchResult, BackendError> {
        let effective = if *config == DispatchConfig::default() {
            &self.compile_config
        } else {
            config
        };
        self.backend
            .dispatch_borrowed_timed(&self.program, inputs, effective)
    }

    fn dispatch_borrowed_into(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let effective = if *config == DispatchConfig::default() {
            &self.compile_config
        } else {
            config
        };
        self.backend
            .dispatch_borrowed_into(&self.program, inputs, effective, outputs)
    }
}
