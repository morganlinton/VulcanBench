use smallvec::SmallVec;

use super::*;

const RESIDENT_STAGE_INLINE_BINDINGS: usize = 8;

#[derive(Clone)]
struct ResidentCachedPipeline {
    pipeline: Arc<dyn CompiledPipeline>,
    plan: Arc<BindingPlan>,
}

#[derive(Clone, Debug)]
pub(crate) struct ResidentBlob {
    pub(crate) resource: Resource,
    pub(crate) byte_len: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ResidentStageInput<'a> {
    Host(&'a [u8]),
    Resident(&'a ResidentBlob),
}

impl ResidentStageInput<'_> {
    fn byte_len(self) -> usize {
        match self {
            Self::Host(bytes) => bytes.len(),
            Self::Resident(blob) => blob.byte_len,
        }
    }
}

pub(crate) fn free_resident_blobs(
    backend: &dyn VyreBackend,
    blobs: impl IntoIterator<Item = ResidentBlob>,
) -> Result<(), vyre::BackendError> {
    let mut seen: SmallVec<[u64; RESIDENT_STAGE_INLINE_BINDINGS]> = SmallVec::new();
    for blob in blobs {
        match blob.resource {
            Resource::Resident(id) if !seen.contains(&id) => {
                seen.push(id);
                backend.free_resident(Resource::Resident(id))?;
            }
            Resource::Resident(_) | Resource::Borrowed(_) => {}
        }
    }
    Ok(())
}

fn output_resources_contain(output_resources: &[Resource], candidate: &Resource) -> bool {
    match candidate {
        Resource::Resident(candidate_id) => output_resources.iter().any(|resource| {
            matches!(resource, Resource::Resident(resource_id) if resource_id == candidate_id)
        }),
        Resource::Borrowed(_) => false,
    }
}

struct ResidentStageResources {
    resources: SmallVec<[Resource; RESIDENT_STAGE_INLINE_BINDINGS]>,
    allocated: SmallVec<[Resource; RESIDENT_STAGE_INLINE_BINDINGS]>,
    output_lengths: SmallVec<[(usize, usize); RESIDENT_STAGE_INLINE_BINDINGS]>,
}

fn resident_input_lengths(
    inputs: &[ResidentStageInput<'_>],
) -> SmallVec<[usize; RESIDENT_STAGE_INLINE_BINDINGS]> {
    inputs
        .iter()
        .copied()
        .map(ResidentStageInput::byte_len)
        .collect()
}

fn resident_cached_pipeline<F>(
    backend: &dyn VyreBackend,
    stage_key: StagePipelineCacheKey,
    build_program: F,
    input_lengths: &[usize],
    config: &DispatchConfig,
    stage_label: &str,
    trace: bool,
    started: std::time::Instant,
) -> Result<ResidentCachedPipeline, vyre::BackendError>
where
    F: FnOnce() -> Result<Program, String>,
{
    #[allow(clippy::type_complexity)]
    static PIPELINES: OnceLock<
        Mutex<BoundedPipelineCache<BackendStagePipelineCacheKey, ResidentCachedPipeline>>,
    > = OnceLock::new();

    let key = (backend_pipeline_cache_key(backend.id()), stage_key);
    let cache = PIPELINES.get_or_init(|| Mutex::new(BoundedPipelineCache::default()));
    let cached = {
        let mut guard = cache
            .lock()
            .map_err(|error| vyre::BackendError::DispatchFailed {
                code: None,
                message: format!("frontend C resident pipeline cache lock poisoned: {error}"),
            })?;
        guard.get_cloned(&key)
    };
    if let Some(entry) = cached {
        entry.plan.validate_input_byte_lengths(input_lengths)?;
        return Ok(entry);
    }

    let program = build_program().map_err(|message| vyre::BackendError::DispatchFailed {
        code: None,
        message,
    })?;
    if vyre_driver::grid_sync::contains_grid_sync(&program) {
        return Err(vyre::BackendError::UnsupportedFeature {
            name: format!("{stage_label} grid-sync split dispatch"),
            backend: backend.id().to_string(),
        });
    }
    let plan = Arc::new(BindingPlan::from_input_lengths(&program, input_lengths)?);
    if trace {
        eprintln!(
            "[resident-stage] +{}ms planned entry={} bindings={} outputs={}",
            started.elapsed().as_millis(),
            program.entry_op_id.as_deref().unwrap_or("<anonymous>"),
            plan.bindings.len(),
            plan.output_indices.len()
        );
        eprintln!(
            "[resident-stage] +{}ms compile_native entry={}",
            started.elapsed().as_millis(),
            program.entry_op_id.as_deref().unwrap_or("<anonymous>")
        );
    }
    let Some(pipeline) = backend.compile_native(&program, config)? else {
        return Err(vyre::BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{} backend did not return a compiled native pipeline for {stage_label} `{}`. Fix: repair native resident pipeline compilation; parser stages must not downgrade through host readbacks.",
                backend.id(),
                program.entry_op_id.as_deref().unwrap_or("<anonymous>")
            ),
        });
    };
    let entry = ResidentCachedPipeline { pipeline, plan };
    let mut guard = cache
        .lock()
        .map_err(|error| vyre::BackendError::DispatchFailed {
            code: None,
            message: format!(
                "frontend C resident pipeline cache lock poisoned while inserting: {error}"
            ),
        })?;
    let cache_bytes = compiled_pipeline_cache_estimated_bytes(&program);
    guard.insert_with_cost(
        key,
        entry.clone(),
        COMPILED_PIPELINE_CACHE_MAX_ENTRIES,
        cache_bytes,
        COMPILED_PIPELINE_CACHE_MAX_BYTES,
    );
    Ok(entry)
}

fn bind_resident_stage_resources(
    backend: &dyn VyreBackend,
    plan: &BindingPlan,
    inputs: &[ResidentStageInput<'_>],
    input_lengths: &[usize],
    stage_label: &str,
) -> Result<ResidentStageResources, vyre::BackendError> {
    let mut resources: SmallVec<[Resource; RESIDENT_STAGE_INLINE_BINDINGS]> = SmallVec::new();
    let mut allocated: SmallVec<[Resource; RESIDENT_STAGE_INLINE_BINDINGS]> = SmallVec::new();
    let mut output_lengths: SmallVec<[(usize, usize); RESIDENT_STAGE_INLINE_BINDINGS]> =
        SmallVec::new();
    let mut host_uploads: SmallVec<[(Resource, &[u8]); RESIDENT_STAGE_INLINE_BINDINGS]> =
        SmallVec::new();

    for binding in &plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let resource = if let Some(input_index) = binding.input_index {
            match inputs.get(input_index).copied().ok_or_else(|| {
                vyre::BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: {stage_label} input index {input_index} for `{}` was missing after binding validation.",
                        binding.name
                    ),
                }
            })? {
                ResidentStageInput::Host(bytes) => {
                    let resource = backend.allocate_resident(bytes.len())?;
                    allocated.push(resource.clone());
                    host_uploads.push((resource.clone(), bytes));
                    resource
                }
                ResidentStageInput::Resident(blob) => blob.resource.clone(),
            }
        } else {
            let byte_len = binding.static_byte_len.ok_or_else(|| {
                vyre::BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: {stage_label} output `{}` has dynamic size with no input-derived byte length; declare a static count before zero-copy frontend chaining.",
                        binding.name
                    ),
                }
            })?;
            let resource = backend.allocate_resident(byte_len)?;
            allocated.push(resource.clone());
            resource
        };

        if let Some(output_index) = binding.output_index {
            let byte_len = binding
                .static_byte_len
                .or_else(|| binding.input_index.map(|input_index| input_lengths[input_index]))
                .ok_or_else(|| vyre::BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: {stage_label} output `{}` needs a static or input-derived byte length for output-resource chaining.",
                        binding.name
                    ),
                })?;
            output_lengths.push((output_index, byte_len));
        }
        resources.push(resource);
    }
    if !host_uploads.is_empty() {
        let uploads: SmallVec<[(&Resource, &[u8]); RESIDENT_STAGE_INLINE_BINDINGS]> = host_uploads
            .iter()
            .map(|(resource, bytes)| (resource, *bytes))
            .collect();
        if let Err(error) = backend.upload_resident_many(&uploads) {
            for resource in allocated {
                let _ = backend.free_resident(resource);
            }
            return Err(error);
        }
    }

    Ok(ResidentStageResources {
        resources,
        allocated,
        output_lengths,
    })
}

pub(crate) fn dispatch_resident_stage_cached<F>(
    backend: &dyn VyreBackend,
    stage_key: StagePipelineCacheKey,
    build_program: F,
    inputs: &[ResidentStageInput<'_>],
    config: &DispatchConfig,
) -> Result<Vec<ResidentBlob>, vyre::BackendError>
where
    F: FnOnce() -> Result<Program, String>,
{
    let trace = std::env::var_os("VYRE_STAGE_TRACE").is_some()
        || std::env::var_os("VYRE_CUDA_STAGE_TRACE").is_some();
    let started = std::time::Instant::now();
    if trace {
        eprintln!(
            "[resident-stage] start backend={} key={} inputs={}",
            backend.id(),
            stage_pipeline_cache_key_hex(stage_key),
            inputs.len()
        );
    }
    let input_lengths = resident_input_lengths(inputs);
    let ResidentCachedPipeline { pipeline, plan } = resident_cached_pipeline(
        backend,
        stage_key,
        build_program,
        &input_lengths,
        config,
        "resident stage",
        trace,
        started,
    )?;
    if trace {
        eprintln!(
            "[resident-stage] +{}ms pipeline ready",
            started.elapsed().as_millis()
        );
    }

    let ResidentStageResources {
        resources,
        allocated,
        mut output_lengths,
    } = bind_resident_stage_resources(backend, &plan, inputs, &input_lengths, "resident stage")?;
    if trace {
        eprintln!(
            "[resident-stage] +{}ms resources ready count={} allocated={}",
            started.elapsed().as_millis(),
            resources.len(),
            allocated.len()
        );
    }

    output_lengths.sort_unstable_by_key(|(output_index, _)| *output_index);
    let output_resources = match pipeline.dispatch_persistent_resource_outputs(&resources, config) {
        Ok(resources) => resources,
        Err(error) => {
            for resource in allocated {
                let _ = backend.free_resident(resource);
            }
            return Err(error);
        }
    };
    if trace {
        eprintln!(
            "[resident-stage] +{}ms resource outputs={}",
            started.elapsed().as_millis(),
            output_resources.len()
        );
    }
    for resource in allocated {
        if !output_resources_contain(&output_resources, &resource) {
            backend.free_resident(resource)?;
        }
    }
    if output_resources.len() != output_lengths.len() {
        return Err(vyre::BackendError::InvalidProgram {
            fix: format!(
                "Fix: resident stage returned {} output resource(s) but binding plan expected {}.",
                output_resources.len(),
                output_lengths.len()
            ),
        });
    }
    Ok(output_resources
        .into_iter()
        .zip(output_lengths)
        .map(|(resource, (_, byte_len))| ResidentBlob { resource, byte_len })
        .collect())
}

pub(crate) fn dispatch_resident_stage_readback_cached_into<F>(
    backend: &dyn VyreBackend,
    stage_key: StagePipelineCacheKey,
    build_program: F,
    inputs: &[ResidentStageInput<'_>],
    config: &DispatchConfig,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), vyre::BackendError>
where
    F: FnOnce() -> Result<Program, String>,
{
    let input_lengths = resident_input_lengths(inputs);
    let ResidentCachedPipeline { pipeline, plan } = resident_cached_pipeline(
        backend,
        stage_key,
        build_program,
        &input_lengths,
        config,
        "resident terminal stage",
        false,
        std::time::Instant::now(),
    )?;
    let ResidentStageResources {
        resources,
        allocated,
        ..
    } = bind_resident_stage_resources(
        backend,
        &plan,
        inputs,
        &input_lengths,
        "resident terminal stage",
    )?;

    outputs.reserve(plan.output_indices.len().saturating_sub(outputs.len()));
    if let Err(error) = pipeline.dispatch_persistent_handles_into(&resources, config, outputs) {
        for resource in &allocated {
            let _ = backend.free_resident(resource.clone());
        }
        return Err(error);
    }
    for resource in allocated {
        backend.free_resident(resource)?;
    }
    Ok(())
}
