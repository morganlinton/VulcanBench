use super::*;

pub(crate) fn dispatch_borrowed_cached_into(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), vyre::BackendError> {
    #[allow(clippy::type_complexity)]
    static PIPELINES: OnceLock<
        Mutex<BoundedPipelineCache<BackendProgramPipelineCacheKey, Arc<dyn CompiledPipeline>>>,
    > = OnceLock::new();
    let key = (
        backend_pipeline_cache_key(backend.id()),
        vyre_foundation::optimizer::pipeline_fingerprint_bytes(program),
    );
    let cache = PIPELINES.get_or_init(|| Mutex::new(BoundedPipelineCache::default()));
    if let Some(pipeline) = cache
        .lock()
        .map_err(|error| vyre::BackendError::DispatchFailed {
            code: None,
            message: format!("frontend C pipeline cache lock poisoned: {error}"),
        })?
        .get_cloned(&key)
    {
        return pipeline.dispatch_borrowed_into(inputs, config, outputs);
    }

    let Some(pipeline) = backend.compile_native(program, config)? else {
        if backend.id() == "cuda" && vyre_driver::grid_sync::contains_grid_sync(program) {
            return backend.dispatch_borrowed_into(program, inputs, config, outputs);
        }
        return Err(vyre::BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{} backend did not return a compiled native pipeline for `{}`. Fix: repair native compilation/cache integration; the C frontend must not silently downgrade to uncached dispatch.",
                backend.id(),
                program.entry_op_id.as_deref().unwrap_or("<anonymous>")
            ),
        });
    };
    let mut guard = cache
        .lock()
        .map_err(|error| vyre::BackendError::DispatchFailed {
            code: None,
            message: format!("frontend C pipeline cache lock poisoned while inserting: {error}"),
        })?;
    let cache_bytes = compiled_pipeline_cache_estimated_bytes(program);
    guard.insert_with_cost(
        key,
        Arc::clone(&pipeline),
        COMPILED_PIPELINE_CACHE_MAX_ENTRIES,
        cache_bytes,
        COMPILED_PIPELINE_CACHE_MAX_BYTES,
    );
    pipeline.dispatch_borrowed_into(inputs, config, outputs)
}

pub(crate) fn stage_pipeline_cache_key(stage: &str, params: &[u64]) -> StagePipelineCacheKey {
    let mut hash = blake3::Hasher::new();
    blake3_128_update_len_prefixed(&mut hash, stage.as_bytes());
    hash.update(&(params.len() as u64).to_le_bytes());
    for param in params {
        hash.update(&param.to_le_bytes());
    }
    blake3_128_from_hasher(&hash)
}

pub(super) fn stage_pipeline_cache_key_hex(key: StagePipelineCacheKey) -> String {
    let mut out = String::with_capacity(32);
    for byte in key {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub(crate) fn dispatch_borrowed_stage_cached_into<F>(
    backend: &dyn VyreBackend,
    stage_key: StagePipelineCacheKey,
    build_program: F,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), vyre::BackendError>
where
    F: FnOnce() -> Result<Program, String>,
{
    if backend.id() == "cuda" {
        let program = build_program().map_err(|message| vyre::BackendError::DispatchFailed {
            code: None,
            message,
        })?;
        return backend.dispatch_borrowed_into(&program, inputs, config, outputs);
    }

    #[allow(clippy::type_complexity)]
    static PIPELINES: OnceLock<
        Mutex<BoundedPipelineCache<BackendStagePipelineCacheKey, Arc<dyn CompiledPipeline>>>,
    > = OnceLock::new();
    let key = (backend_pipeline_cache_key(backend.id()), stage_key);
    let cache = PIPELINES.get_or_init(|| Mutex::new(BoundedPipelineCache::default()));
    if let Some(pipeline) = cache
        .lock()
        .map_err(|error| vyre::BackendError::DispatchFailed {
            code: None,
            message: format!("frontend C stage pipeline cache lock poisoned: {error}"),
        })?
        .get_cloned(&key)
    {
        return match pipeline.dispatch_borrowed_into(inputs, config, outputs) {
            Ok(()) => Ok(()),
            Err(error) if should_retry_stage_as_direct_cuda_dispatch(backend, &error) => {
                outputs.clear();
                let program =
                    build_program().map_err(|message| vyre::BackendError::DispatchFailed {
                        code: None,
                        message,
                    })?;
                backend.dispatch_borrowed_into(&program, inputs, config, outputs)
            }
            Err(error) => Err(error),
        };
    }

    let program = build_program().map_err(|message| vyre::BackendError::DispatchFailed {
        code: None,
        message,
    })?;
    let Some(pipeline) = backend.compile_native(&program, config)? else {
        if backend.id() == "cuda" && vyre_driver::grid_sync::contains_grid_sync(&program) {
            return backend.dispatch_borrowed_into(&program, inputs, config, outputs);
        }
        return Err(vyre::BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{} backend did not return a compiled native pipeline for `{}`. Fix: repair native compilation/cache integration; the C frontend must not silently downgrade to uncached dispatch.",
                backend.id(),
                program.entry_op_id.as_deref().unwrap_or("<anonymous>")
            ),
        });
    };
    match pipeline.dispatch_borrowed_into(inputs, config, outputs) {
        Ok(()) => {
            let mut guard = cache
                .lock()
                .map_err(|error| vyre::BackendError::DispatchFailed {
                    code: None,
                    message: format!(
                        "frontend C stage pipeline cache lock poisoned while inserting: {error}"
                    ),
                })?;
            let cache_bytes = compiled_pipeline_cache_estimated_bytes(&program);
            guard.insert_with_cost(
                key,
                Arc::clone(&pipeline),
                COMPILED_PIPELINE_CACHE_MAX_ENTRIES,
                cache_bytes,
                COMPILED_PIPELINE_CACHE_MAX_BYTES,
            );
            Ok(())
        }
        Err(error) if should_retry_stage_as_direct_cuda_dispatch(backend, &error) => {
            outputs.clear();
            backend.dispatch_borrowed_into(&program, inputs, config, outputs)
        }
        Err(error) => Err(error),
    }
}

fn should_retry_stage_as_direct_cuda_dispatch(
    backend: &dyn VyreBackend,
    error: &vyre::BackendError,
) -> bool {
    backend.id() == "cuda" && cuda_stage_error_should_retry_direct_dispatch(error)
}

fn cuda_stage_error_should_retry_direct_dispatch(error: &vyre::BackendError) -> bool {
    matches!(
        error,
        vyre::BackendError::DispatchFailed {
            code: Some(701),
            message
        } if message.contains("cuGraphInstantiate")
            || message.contains("CUDA_ERROR_LAUNCH_OUT_OF_RESOURCES")
    ) || matches!(
        error,
        vyre::BackendError::DispatchFailed {
            code: Some(716),
            message
        } if message.contains("cuda_graph fallback")
            && message.contains("CUDA_ERROR_MISALIGNED_ADDRESS")
    )
}

#[cfg(test)]
mod tests {
    use super::{
        backend_pipeline_cache_key, cuda_stage_error_should_retry_direct_dispatch,
        stage_pipeline_cache_key,
    };

    #[test]
    fn stage_pipeline_cache_key_uses_128_bit_stage_and_param_identity() {
        let key = stage_pipeline_cache_key("stage", &[1, 2, 3]);
        assert_eq!(key.len(), 16);
        assert_ne!(key, stage_pipeline_cache_key("stage", &[1, 23]));
        assert_ne!(key, stage_pipeline_cache_key("other-stage", &[1, 2, 3]));
    }

    #[test]
    fn backend_pipeline_cache_key_uses_128_bit_backend_identity() {
        let cuda = backend_pipeline_cache_key("cuda");
        let wgpu = backend_pipeline_cache_key("wgpu");
        assert_eq!(cuda.len(), 16);
        assert_eq!(cuda, backend_pipeline_cache_key("cuda"));
        assert_ne!(cuda, wgpu);
    }

    #[test]
    fn cuda_stage_dispatch_bypasses_graph_cache_before_launch() {
        let source = include_str!("borrowed_cache.rs");
        assert!(
            source.contains("if backend.id() == \"cuda\""),
            "Fix: C frontend CUDA stage dispatch must bypass cached CUDA graph launch before a graph fault can poison the CUDA context."
        );
        assert!(
            source.contains(
                "return backend.dispatch_borrowed_into(&program, inputs, config, outputs);"
            ),
            "Fix: CUDA stage bypass must execute direct borrowed dispatch."
        );
    }

    #[test]
    fn cuda_stage_retry_covers_graph_fallback_misaligned_address() {
        let error = vyre::BackendError::DispatchFailed {
            code: Some(716),
            message: "cuStreamSynchronize (cuda_graph fallback) failed with CUDA_ERROR_MISALIGNED_ADDRESS".to_string(),
        };

        assert!(cuda_stage_error_should_retry_direct_dispatch(&error));
    }
}
