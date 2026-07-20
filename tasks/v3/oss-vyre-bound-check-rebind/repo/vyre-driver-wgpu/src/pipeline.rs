//! Native pipeline-mode implementation for the wgpu backend.
//!
//! P-6 from `docs/audits/ROADMAP_PERFORMANCE.md`. Pre-compiles WGSL,
//! compute pipeline, and bind-group layout once so subsequent dispatch
//! calls only pay buffer-allocation + execution + readback cost.
//!
//! Per the roadmap, this removes ~90% of per-call overhead  -  the WGSL
//! lowering and pipeline compilation costs dominate over the actual GPU
//! work for short programs run repeatedly.

use std::sync::Arc;
use std::time::Instant;

use rustc_hash::FxHashSet;
use smallvec::SmallVec;
use std::hash::BuildHasherDefault;
use vyre_driver::launch::resolve_launch_workgroup_for_mode;
#[cfg(test)]
pub(crate) use vyre_driver::program_walks::enforce_actual_output_budget;
pub(crate) use vyre_driver::program_walks::{element_size_bytes, OutputBindingLayout};
use vyre_driver::program_walks::{find_indirect_dispatch, infer_dispatch_grid_for_count};
pub use vyre_driver::program_walks::{output_layout_from_program, IndirectDispatch, OutputLayout};
use vyre_driver::tuner::Mode;
use vyre_driver::validation::LaunchGeometryLimits;
use vyre_driver::BackendLayoutFingerprint;
use vyre_driver::{BackendError, CompiledPipeline, DispatchConfig, OutputBuffers};
use vyre_foundation::execution_plan::{self, ExecutionPlan};
use vyre_foundation::ir::Program;
use vyre_foundation::validate::ValidationOptions;

pub use crate::buffer::BindGroupCacheStats;
use crate::buffer::{BindGroupCache, StagingBufferPool};
use crate::pipeline::disk_cache::{
    compiled_pipeline_cache_key, create_compiled_pipeline_cache, early_pipeline_cache_key,
    load_or_compile_disk_wgsl, persist_compiled_pipeline_cache,
};
pub use crate::pipeline::persistent::DispatchItem;
use crate::runtime;
use crate::staging_reserve::reserve_backend_vec;
use crate::DispatchArena;
use vyre_driver::allocation::reserve_hash_set_to_capacity;
use vyre_emit_naga::program::TrapTag;
use vyre_lower::{TRAP_SIDECAR_NAME, TRAP_SIDECAR_WORDS};

pub(crate) use self::descriptor_metadata::BufferBindingInfo;
use self::descriptor_metadata::{
    bind_group_layout_fingerprint, create_bind_group_layouts, descriptor_buffer_bindings,
    descriptor_trap_tags,
};

pub(crate) type BindGroupLayoutCache = dashmap::DashMap<
    BackendLayoutFingerprint,
    Arc<[Arc<wgpu::BindGroupLayout>]>,
    BuildHasherDefault<rustc_hash::FxHasher>,
>;

/// GPU pipeline + **all** per-program dispatch metadata co-located for
/// cache hits. A hit on [`early_pipeline_cache_key`] or the WGSL hash
/// key must skip `execution_plan::plan`, output-layout derivation,
/// and fresh [`StagingBufferPool::new`] (subagent: pipeline.rs compile
/// path  -  2026-04 orchestration sweep).
#[derive(Debug)]
pub(crate) struct CachedPipelineArtifact {
    id: String,
    pipeline: Arc<wgpu::ComputePipeline>,
    bind_group_layouts: Arc<[Arc<wgpu::BindGroupLayout>]>,
    bind_group_cache: Arc<BindGroupCache>,
    /// Shared across every [`WgpuPipeline`] built from this artifact.
    pub(crate) execution_plan: Arc<ExecutionPlan>,
    pub(crate) output_bindings: Arc<[OutputBindingLayout]>,
    pub(crate) buffer_bindings: Arc<[BufferBindingInfo]>,
    pub(crate) output: OutputLayout,
    pub(crate) output_word_count: usize,
    pub(crate) workgroup_shape: [u32; 3],
    pub(crate) workgroup_size: u32,
    pub(crate) indirect: Option<IndirectDispatch>,
    pub(crate) trap_tags: Arc<[TrapTag]>,
    /// Cloned per [`WgpuPipeline`]; all clones share the inner pool.
    pub(crate) staging_pool: StagingBufferPool,
}

impl CachedPipelineArtifact {
    pub(crate) fn cache_cost_bytes(&self) -> usize {
        let binding_names: usize = self
            .buffer_bindings
            .iter()
            .map(|binding| binding.name.len())
            .sum();
        let output_names: usize = self
            .output_bindings
            .iter()
            .map(|output| output.name.len())
            .sum();
        checked_cache_cost_sum(&[
            self.id.len(),
            binding_names,
            output_names,
            checked_cache_cost_product(
                self.bind_group_layouts.len(),
                std::mem::size_of::<Arc<wgpu::BindGroupLayout>>(),
            ),
            checked_cache_cost_product(
                self.buffer_bindings.len(),
                std::mem::size_of::<BufferBindingInfo>(),
            ),
            checked_cache_cost_product(
                self.output_bindings.len(),
                std::mem::size_of::<OutputBindingLayout>(),
            ),
            checked_cache_cost_product(self.trap_tags.len(), std::mem::size_of::<TrapTag>()),
            std::mem::size_of::<Self>(),
        ])
    }
}

fn checked_cache_cost_product(count: usize, element_size: usize) -> usize {
    count.saturating_mul(element_size)
}

fn checked_cache_cost_sum(parts: &[usize]) -> usize {
    let mut total = 0usize;
    for &part in parts {
        total = total.saturating_add(part);
    }
    total
}

fn wgpu_effective_dispatch_config(
    program: &Program,
    config: &DispatchConfig,
    device: &wgpu::Device,
) -> Result<DispatchConfig, BackendError> {
    wgpu_effective_dispatch_config_for_limits(
        program,
        config,
        wgpu_launch_limits(device),
        Mode::from_env(),
    )
}

fn wgpu_effective_dispatch_config_for_limits(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    mode: Mode,
) -> Result<DispatchConfig, BackendError> {
    let mut effective = config.clone();
    if effective.workgroup_override.is_some() {
        return Ok(effective);
    }
    let element_count = wgpu_launch_element_count_for_tuning(program)?;
    let selected =
        resolve_launch_workgroup_for_mode(program, &effective, limits, element_count, mode);
    if selected != program.workgroup_size() {
        effective.workgroup_override = Some(selected);
    }
    Ok(effective)
}

fn wgpu_launch_element_count_for_tuning(program: &Program) -> Result<u32, BackendError> {
    if program.output_buffer_indices().is_empty() {
        return Ok(0);
    }
    let layouts = vyre_driver::program_walks::output_binding_layouts(program)?;
    let word_count = layouts
        .first()
        .map(|layout| layout.word_count)
        .unwrap_or_default();
    u32::try_from(word_count).map_err(|error| {
        BackendError::new(format!(
            "wgpu natural-gradient launch tuning cannot represent {word_count} output word(s) as u32: {error}. Fix: split the dispatch or provide an explicit workgroup/grid override."
        ))
    })
}

fn wgpu_launch_limits(device: &wgpu::Device) -> LaunchGeometryLimits {
    let limits = device.limits();
    LaunchGeometryLimits {
        backend: "wgpu",
        max_threads_per_block: limits.max_compute_invocations_per_workgroup,
        max_block_dim: [
            limits.max_compute_workgroup_size_x,
            limits.max_compute_workgroup_size_y,
            limits.max_compute_workgroup_size_z,
        ],
        max_grid_dim: [limits.max_compute_workgroups_per_dimension; 3],
    }
}

/// In-memory pipeline cache (P-27 from `docs/audits/ROADMAP_PERFORMANCE.md`).
///
/// Keyed by a full program fingerprint (serialized IR + adapter fingerprint),
/// returned as `Arc` so multiple callers share one ComputePipeline.
/// `WgpuPipeline` is a thin wrapper around an `Arc<CachedPipeline>` plus
/// per-instance values (id, output_size).

/// Cached state for a vyre program on the wgpu backend.
///
/// Built by `WgpuBackend::compile_native`.
/// Holds the compiled compute pipeline and the bind-group layout (both
/// derived from the WGSL lowering) plus the geometry needed to size each
/// dispatch's input/output buffers.
#[derive(Clone)]
pub struct WgpuPipeline {
    pub(crate) id: String,
    pub(crate) pipeline: Arc<wgpu::ComputePipeline>,
    pub(crate) bind_group_layouts: Arc<[Arc<wgpu::BindGroupLayout>]>,
    pub(crate) bind_group_cache: Arc<BindGroupCache>,
    pub(crate) buffer_bindings: Arc<[BufferBindingInfo]>,
    pub(crate) output_bindings: Arc<[OutputBindingLayout]>,
    pub(crate) execution_plan: Arc<ExecutionPlan>,
    pub(crate) device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
    pub(crate) output: OutputLayout,
    pub(crate) output_word_count: usize,
    pub(crate) workgroup_shape: [u32; 3],
    pub(crate) workgroup_size: u32,
    pub(crate) indirect: Option<IndirectDispatch>,
    pub(crate) trap_tags: Arc<[TrapTag]>,
    /// Shared persistent GPU-handle pool (H1). The legacy dispatch
    /// path acquires handles from here so repeated dispatches reuse
    /// `wgpu::Buffer` allocations instead of churning the GPU
    /// allocator on every call.
    pub(crate) persistent_pool: crate::buffer::BufferPool,
    /// Staging buffer pool for readback. Hot dispatch paths reuse
    /// MAP_READ staging buffers instead of creating a fresh
    /// `wgpu::Buffer` on every readback.
    pub(crate) staging_pool: StagingBufferPool,
}

impl std::fmt::Debug for WgpuPipeline {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WgpuPipeline")
            .field("id", &self.id)
            .field("buffer_bindings", &self.buffer_bindings)
            .field("output_bindings", &self.output_bindings)
            .field("execution_tracks", &self.execution_plan.tracks)
            .field("output", &self.output)
            .field("output_word_count", &self.output_word_count)
            .field("workgroup_shape", &self.workgroup_shape)
            .field("workgroup_size", &self.workgroup_size)
            .field("indirect", &self.indirect)
            .field("trap_tags", &self.trap_tags)
            .finish_non_exhaustive()
    }
}

impl WgpuPipeline {
    fn from_cached_artifact(
        cached: &CachedPipelineArtifact,
        device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
        persistent_pool: crate::buffer::BufferPool,
    ) -> Self {
        Self {
            id: cached.id.clone(),
            pipeline: cached.pipeline.clone(),
            bind_group_layouts: cached.bind_group_layouts.clone(),
            bind_group_cache: cached.bind_group_cache.clone(),
            buffer_bindings: cached.buffer_bindings.clone(),
            output_bindings: cached.output_bindings.clone(),
            execution_plan: cached.execution_plan.clone(),
            device_queue,
            output: cached.output,
            output_word_count: cached.output_word_count,
            workgroup_shape: cached.workgroup_shape,
            workgroup_size: cached.workgroup_size,
            indirect: cached.indirect.clone(),
            trap_tags: cached.trap_tags.clone(),
            persistent_pool,
            staging_pool: cached.staging_pool.clone(),
        }
    }

    /// Pre-compile `program` into a reusable pipeline.
    ///
    /// First-call: performs WGSL lowering, ComputePipeline creation, and
    /// BindGroupLayout caching, then INSERTS the result in `PIPELINE_CACHE`
    /// keyed by the serialized IR + adapter fingerprint.
    ///
    /// Subsequent calls with the same Program on the same adapter skip
    /// the ComputePipeline / BindGroupLayout creation entirely  -  the cache
    /// returns the same `Arc<wgpu::ComputePipeline>` and the new
    /// `WgpuPipeline` instance just carries fresh metadata (output sizing).
    /// Per-Program metadata varies even when the WGSL doesn't, so it stays
    /// per-instance.
    pub fn compile(program: &Program) -> Result<Arc<Self>, BackendError> {
        Self::compile_with_config(program, &DispatchConfig::default())
    }

    /// Pre-compile `program` into a reusable pipeline using dispatch policy.
    ///
    /// # Errors
    ///
    /// Returns a backend error when lowering, cache access, or pipeline
    /// creation fails.
    pub fn compile_with_config(
        program: &Program,
        config: &DispatchConfig,
    ) -> Result<Arc<Self>, BackendError> {
        let ((device, queue), adapter_info, enabled_features) =
            runtime::init_device().map_err(|error| BackendError::new(error.to_string()))?;
        // Build a fresh pool tied to this call's device+queue. The
        // pool lives as long as the returned pipeline; consumers
        // that want cross-pipeline pool sharing go through
        // `WgpuBackend::acquire` instead (which owns one pool per
        // adapter).
        let pool = crate::buffer::BufferPool::new(device.clone(), queue.clone(), config);
        let (pipeline_cache_entries, pipeline_cache_bytes) =
            vyre_driver::pipeline::pipeline_cache_limits_from_env();
        Self::compile_with_device_queue(
            program,
            config,
            adapter_info,
            enabled_features,
            Arc::new((device.clone(), queue.clone())),
            Arc::new(DispatchArena::new(device.clone(), queue.clone(), config)),
            pool,
            Arc::new(runtime::cache::pipeline::LruPipelineCache::with_limits(
                pipeline_cache_entries,
                pipeline_cache_bytes,
            )),
            Arc::new(BindGroupLayoutCache::with_hasher(BuildHasherDefault::<
                rustc_hash::FxHasher,
            >::default())),
        )
    }

    /// Pre-compile `program` using the supplied backend-owned device and arena.
    ///
    /// # Errors
    ///
    /// Returns a backend error when lowering, cache access, or pipeline
    /// creation fails.
    pub(crate) fn compile_with_device_queue(
        program: &Program,
        config: &DispatchConfig,
        adapter_info: wgpu::AdapterInfo,
        enabled_features: crate::runtime::device::EnabledFeatures,
        device_queue: Arc<(wgpu::Device, wgpu::Queue)>,
        _dispatch_arena: Arc<DispatchArena>,
        persistent_pool: crate::buffer::BufferPool,
        pipeline_cache: Arc<runtime::cache::pipeline::LruPipelineCache>,
        bind_group_layout_cache: Arc<BindGroupLayoutCache>,
    ) -> Result<Arc<Self>, BackendError> {
        let compile_program = program;
        let effective_config =
            wgpu_effective_dispatch_config(compile_program, config, &device_queue.0)?;
        let config = &effective_config;
        // Cache-first: both keys are checked before `execution_plan::plan`
        // and before binding-metadata construction (orchestration sweep 2026-04).
        let early_key = early_pipeline_cache_key(compile_program, &adapter_info, config);
        if let Some(hit) = pipeline_cache.get(&early_key) {
            return Ok(Arc::new(Self::from_cached_artifact(
                hit.as_ref(),
                device_queue,
                persistent_pool,
            )));
        }

        let wgsl =
            load_or_compile_disk_wgsl(compile_program, &adapter_info, config, &enabled_features)?;
        let artifact_key = compiled_pipeline_cache_key(&adapter_info, &wgsl);

        let descriptor = crate::emit::descriptor_gate::validate_and_analyze(compile_program)
            .map_err(|error| {
                BackendError::new(format!(
                    "failed to derive KernelDescriptor for wgpu pipeline metadata: {error}. Fix: keep pipeline metadata on the same descriptor path as WGSL emission."
                ))
            })?;
        let staging_pool = StagingBufferPool::new();
        let trap_tags_vec = descriptor_trap_tags(&descriptor)?;
        if !trap_tags_vec.is_empty()
            && !descriptor
                .bindings
                .slots
                .iter()
                .any(|slot| slot.name == TRAP_SIDECAR_NAME)
        {
            return Err(BackendError::new(format!(
                "descriptor contains trap tags but no `{TRAP_SIDECAR_NAME}` binding. Fix: lower traps through vyre-lower so the sidecar binding is inserted."
            )));
        }
        let trap_tags: Arc<[TrapTag]> = trap_tags_vec.into();
        let validation_options = ValidationOptions::default().with_backend_capabilities(
            crate::runtime::adapter_caps_probe::from_backend_profile(
                &adapter_info,
                &device_queue.0.limits(),
                &enabled_features,
            )
            .validation_capabilities(),
        );
        let execution_plan = Arc::new(
            execution_plan::plan_with_options(compile_program, validation_options).map_err(
                |error| BackendError::InvalidProgram {
                    fix: format!("Fix: wgpu pipeline planning rejected the Program: {error}"),
                },
            )?,
        );
        let output_bindings: Arc<[OutputBindingLayout]> =
            if program.output_buffer_indices().is_empty() && !trap_tags.is_empty() {
                Arc::from([])
            } else {
                vyre_driver::program_walks::output_binding_layouts(program)?.into()
            };
        let (output, output_word_count) = output_bindings.first().map_or(
            (
                OutputLayout {
                    full_size: 0,
                    read_size: 0,
                    copy_offset: 0,
                    copy_size: 0,
                    trim_start: 0,
                },
                0,
            ),
            |primary_output| (primary_output.layout, primary_output.word_count),
        );
        // Preserve the original workgroup shape. Without program-level
        // logical extents, dispatch paths can only derive a safe default grid
        // for 1D kernels; 2D/3D kernels must provide `grid_override`.
        let effective_wg = config
            .workgroup_override
            .unwrap_or(compile_program.workgroup_size);
        let workgroup_shape = [
            effective_wg[0].max(1),
            effective_wg[1].max(1),
            effective_wg[2].max(1),
        ];
        let workgroup_size = workgroup_shape[0]
            .checked_mul(workgroup_shape[1])
            .and_then(|xy| xy.checked_mul(workgroup_shape[2]))
            .ok_or_else(|| {
                BackendError::new(format!(
                    "workgroup_size {:?} overflows u32 when flattened. Fix: lower to a valid WGPU workgroup shape instead of saturating launch metadata.",
                    workgroup_shape
                ))
            })?;
        let indirect = find_indirect_dispatch(compile_program)?;
        let mut public_output_bindings = FxHashSet::default();
        reserve_hash_set_to_capacity(
            &mut public_output_bindings,
            output_bindings.len(),
            "WGPU pipeline binding classification",
            "public output binding",
            "split the pipeline or reduce output binding fanout before compilation",
        )?;
        public_output_bindings.extend(output_bindings.iter().map(|output| output.binding));
        let buffers = program.buffers();
        let mut explicit_output_bindings = FxHashSet::default();
        reserve_hash_set_to_capacity(
            &mut explicit_output_bindings,
            buffers.len(),
            "WGPU pipeline binding classification",
            "explicit output binding",
            "split the pipeline or reduce output binding fanout before compilation",
        )?;
        let mut pipeline_live_out_bindings = FxHashSet::default();
        reserve_hash_set_to_capacity(
            &mut pipeline_live_out_bindings,
            buffers.len(),
            "WGPU pipeline binding classification",
            "pipeline live-out binding",
            "split the pipeline or reduce live-out binding fanout before compilation",
        )?;
        for buffer in buffers {
            if buffer.is_output() {
                explicit_output_bindings.insert(buffer.binding());
            }
            if buffer.is_pipeline_live_out() {
                pipeline_live_out_bindings.insert(buffer.binding());
            }
        }

        let buffer_bindings: Arc<[BufferBindingInfo]> = descriptor_buffer_bindings(
            &descriptor,
            &public_output_bindings,
            &explicit_output_bindings,
            &pipeline_live_out_bindings,
        )?
        .into();

        for (group, binding) in bindings_reflection::declared_bindings(&wgsl) {
            if !buffer_bindings
                .iter()
                .any(|info| info.group == group && info.binding == binding)
            {
                return Err(BackendError::new(format!(
                    "lowered WGSL declares @group({group}) @binding({binding}) but pipeline metadata has no matching KernelDescriptor binding. Fix: keep Naga emission and pipeline binding derivation on the same KernelDescriptor."
                )));
            }
        }

        let max_group = buffer_bindings.iter().map(|b| b.group).max().unwrap_or(0);

        // Compile outside any lock so other threads can read the cache.
        let (device, _queue) = &*device_queue;

        let layout_fingerprint = bind_group_layout_fingerprint(&buffer_bindings)?;
        let bind_group_layouts = match bind_group_layout_cache.entry(layout_fingerprint) {
            dashmap::mapref::entry::Entry::Occupied(hit) => Arc::clone(hit.get()),
            dashmap::mapref::entry::Entry::Vacant(slot) => Arc::clone(&slot.insert(
                create_bind_group_layouts(device, &buffer_bindings, max_group)?,
            )),
        };
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vyre P-6 pipeline layout"),
            bind_group_layouts: &bind_group_layouts
                .iter()
                .map(|l| l.as_ref())
                .collect::<SmallVec<[_; 8]>>(),
            push_constant_ranges: &[],
        });

        // Only attempt the persistent pipeline cache when the device actually
        // enabled PIPELINE_CACHE. `enabled_features_for_adapter` (device.rs)
        // requests it only on backends that implement wgpu pipeline caches
        // (Vulkan/DX12); on Metal/GL this is `false` and we compile uncached. A
        // `create_pipeline_cache` call on a device without the feature is a
        // fatal validation abort, not a recoverable error.
        let pipeline_cache_handle = if device.features().contains(wgpu::Features::PIPELINE_CACHE) {
            Some(create_compiled_pipeline_cache(device, &artifact_key)?)
        } else {
            None
        };
        runtime::shader::dump_wgsl_if_requested("vyre P-6 cached shader module", &wgsl).map_err(
            |error| {
                BackendError::new(format!(
                    "failed to dump WGSL for compiled pipeline: {error}. Fix: set VYRE_DUMP_WGSL to a writable directory or unset it"
                ))
            },
        )?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vyre P-6 cached shader module"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("vyre P-6 cached pipeline"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: pipeline_cache_handle.as_ref().map(|h| &h.cache),
        });
        if let Some(error) =
            crate::runtime::device::pop_error_scope_now(device).map_err(|message| {
                BackendError::KernelCompileFailed {
                    backend: "wgpu".to_owned(),
                    compiler_message: format!(
                        "cached WGSL pipeline validation did not complete without a host wait: {message}"
                    ),
                }
            })?
        {
            return Err(BackendError::KernelCompileFailed {
                backend: "wgpu".to_owned(),
                compiler_message: format!(
                    "cached WGSL pipeline validation failed: {error}. Fix: validate the lowered WGSL, bind-group layout, and adapter limits before compiling."
                ),
            });
        }
        if let Some(handle) = &pipeline_cache_handle {
            persist_compiled_pipeline_cache(&artifact_key, &handle.cache)?;
        }

        let compiled_artifact = Arc::new(CachedPipelineArtifact {
            id: format!(
                "wgpu:{}",
                vyre_driver::pipeline::hex_short(&artifact_key.hash)
            ),
            pipeline: Arc::new(pipeline),
            bind_group_layouts,
            bind_group_cache: Arc::new(BindGroupCache::default()),
            execution_plan: execution_plan.clone(),
            output_bindings: output_bindings.clone(),
            buffer_bindings: buffer_bindings.clone(),
            output,
            output_word_count,
            workgroup_shape,
            workgroup_size,
            indirect: indirect.clone(),
            trap_tags: trap_tags.clone(),
            staging_pool: staging_pool.clone(),
        });

        pipeline_cache.insert(early_key, Arc::clone(&compiled_artifact));

        Ok(Arc::new(Self::from_cached_artifact(
            compiled_artifact.as_ref(),
            device_queue,
            persistent_pool,
        )))
    }

    /// Dispatch one chunk through this compiled pipeline.
    ///
    /// This is the synchronous primitive used by the host-ingress compatibility
    /// stream; callers that still receive chunks through CPU memory should use
    /// [`crate::engine::streaming::HostIngressStream`]. Canonical VYRE
    /// streaming is the device-resident megakernel queue in `vyre-runtime`.
    ///
    /// # Errors
    ///
    /// Returns a backend error if GPU dispatch or readback fails.
    pub fn push_chunk(
        &self,
        bytes: &[u8],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        // Route through dispatch_borrowed to avoid the owned-Vec copy
        // on the hot streaming path. Callers pass `&[u8]` per chunk;
        // dispatch() would allocate a `Vec<Vec<u8>>` just to wrap it.
        <Self as CompiledPipeline>::dispatch_borrowed(self, &[bytes], config)
    }

    pub(crate) fn output_binding(
        &self,
        binding: u32,
    ) -> Result<&OutputBindingLayout, BackendError> {
        self.output_bindings
            .iter()
            .find(|output| output.binding == binding)
            .ok_or_else(|| {
                BackendError::new(format!(
                    "missing output layout metadata for binding {binding}. Fix: keep output_bindings synchronized with writable BufferDecls during pipeline compilation."
                ))
            })
    }

    pub(crate) fn workgroups_for_dispatch(
        &self,
        config: &DispatchConfig,
    ) -> Result<[u32; 3], BackendError> {
        if let Some(grid) = config.grid_override {
            return Ok(grid);
        }
        // Non-1D workgroups have no unambiguous default grid: there's
        // no single right way to map an unknown element_count across
        // an N×M (or N×M×K) thread tile. Force the caller to set
        // grid_override explicitly rather than silently producing a
        // wrong dispatch.
        if self.workgroup_shape[1] != 1 || self.workgroup_shape[2] != 1 {
            return Err(BackendError::new(format!(
                "Fix: dispatch with non-1D workgroup_size {:?} requires DispatchConfig::grid_override. \
                 Set grid_override to the logical [x, y, z] dispatch shape you want.",
                self.workgroup_shape,
            )));
        }
        let output_word_count = u32::try_from(self.output_word_count).map_err(|error| {
            BackendError::new(format!(
                "compiled WGPU pipeline output word count {} does not fit u32: {error}. Fix: shard the dispatch before grid inference instead of saturating the launch size.",
                self.output_word_count
            ))
        })?;
        infer_dispatch_grid_for_count(output_word_count, self.workgroup_shape)
    }

    /// Substrate-neutral performance and accuracy plan computed for this
    /// compiled program.
    #[must_use]
    pub fn execution_plan(&self) -> &ExecutionPlan {
        &self.execution_plan
    }
}

impl WgpuPipeline {
    fn readback_persistent_outputs(
        &self,
        output_handles: &[crate::buffer::GpuBufferHandle],
        deadline: Option<Instant>,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let (device, queue) = &*self.device_queue;
        self::output_slots::resize_vec_with(
            outputs,
            output_handles.len(),
            Vec::new,
            "borrowed persistent output slots",
        )?;
        for ((handle, output), bytes) in output_handles
            .iter()
            .zip(self.output_bindings.iter())
            .zip(outputs.iter_mut())
        {
            crate::pipeline::output_readback::read_trimmed_output(
                handle,
                output,
                device,
                &self.staging_pool,
                queue,
                "borrowed persistent output",
                deadline,
                bytes,
            )?;
        }
        Ok(())
    }

    fn raise_if_trapped(
        &self,
        input_handles: &[crate::buffer::GpuBufferHandle],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        deadline: Option<Instant>,
    ) -> Result<(), BackendError> {
        let Some((input_index, _)) = self
            .buffer_bindings
            .iter()
            .filter(|info| info.kind != vyre_foundation::ir::MemoryKind::Shared && !info.is_output)
            .enumerate()
            .find(|(_, info)| info.internal_trap)
        else {
            return Ok(());
        };
        let Some(handle) = input_handles.get(input_index) else {
            return Err(BackendError::new(
                "internal wgpu trap buffer was not allocated. Fix: keep trap buffer binding metadata synchronized with legacy input handle allocation.",
            ));
        };
        let trap_sidecar_bytes = usize::try_from(TRAP_SIDECAR_WORDS)
            .map_err(|source| {
                BackendError::new(format!(
                    "trap sidecar word count cannot fit usize: {source}. Fix: keep TRAP_SIDECAR_WORDS within the host index ABI."
                ))
            })?
            .checked_mul(4)
            .ok_or_else(|| {
                BackendError::new(
                    "trap sidecar byte length overflowed usize. Fix: keep TRAP_SIDECAR_WORDS within the host index ABI.",
                )
            })?;
        let mut bytes = Vec::new();
        reserve_backend_vec(&mut bytes, trap_sidecar_bytes, "trap sidecar readback")?;
        handle.readback_prefix_until(
            device,
            Some(&self.staging_pool),
            queue,
            4,
            &mut bytes,
            deadline,
        )?;
        if bytes.len() < 4 {
            return Err(BackendError::new(format!(
                "internal wgpu trap flag readback returned {} bytes but 4 bytes are required. Fix: allocate the trap sidecar as {TRAP_SIDECAR_WORDS} u32 words.",
                bytes.len()
            )));
        }
        let flag = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if flag == 0 {
            return Ok(());
        }

        handle.readback_prefix_until(
            device,
            Some(&self.staging_pool),
            queue,
            u64::from(TRAP_SIDECAR_WORDS) * 4,
            &mut bytes,
            deadline,
        )?;
        trap_error_from_sidecar(&bytes, &self.trap_tags).map_or(Ok(()), Err)
    }

    fn enforce_static_output_budget(&self, config: &DispatchConfig) -> Result<(), BackendError> {
        let Some(limit) = config.max_output_bytes else {
            return Ok(());
        };
        let visible = self.execution_plan.strategy.readback.visible_bytes();
        let visible = usize::try_from(visible).map_err(|source| {
            BackendError::new(format!(
                "visible readback size cannot fit usize: {source}. Fix: split the Program output before dispatch."
            ))
        })?;
        if visible > limit {
            return Err(BackendError::new(format!(
                "visible readback size {visible} exceeds DispatchConfig.max_output_bytes {limit}. Fix: narrow BufferDecl::output_byte_range or raise max_output_bytes."
            )));
        }
        Ok(())
    }
}

pub(crate) fn trap_error_from_sidecar(bytes: &[u8], trap_tags: &[TrapTag]) -> Option<BackendError> {
    let required_len = usize::try_from(TRAP_SIDECAR_WORDS)
        .ok()
        .and_then(|words| words.checked_mul(4))
        .unwrap_or(usize::MAX);
    if bytes.len() < required_len {
        return Some(BackendError::new(format!(
            "internal wgpu trap readback returned {} bytes but {required_len} bytes are required. Fix: allocate the trap sidecar as {TRAP_SIDECAR_WORDS} u32 words.",
            bytes.len()
        )));
    }
    let flag = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if flag == 0 {
        return None;
    }
    let address = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let tag_code = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let lane = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let tag = trap_tags
        .iter()
        .find(|tag| tag.code == tag_code)
        .map(|tag| tag.tag.as_ref())
        .unwrap_or("unknown Node::Trap tag code");
    Some(BackendError::new(format!(
        "wgpu dispatch trapped: address={address}, tag_code={tag_code}, lane={lane}, tag=`{tag}`."
    )))
}

/// Buffer-binding validation, usage flags, and output-clear helpers
/// shared by every wgpu pipeline mode (single-shot, persistent,
/// compound). Hosts the `usage_for_binding`, `validate_handle`, and
/// `clear_outputs_for_bound` helpers all dispatch paths consume.
pub(crate) mod binding;
/// WGSL bind-group reflection scanner  -  extracts every
/// `(group, binding)` pair declared by lowered shader source so the
/// reusable pipeline wrapper can mirror the layout exactly when
/// creating bind groups. Misalignment is a validation error.
pub(crate) mod bindings_reflection;
/// `CompiledPipeline` trait dispatch entrypoints. Split out so the parent
/// pipeline module does not own both compilation and execution mechanics.
pub(crate) mod compiled_dispatch;
/// Compound-resource binding (multi-program shape with shared GPU
/// resources). Used by `engine::graph` to compose pipelines without
/// re-allocating bind groups between dispatches.
pub(crate) mod compound;
/// KernelDescriptor-to-WGPU binding metadata and bind-group layout derivation.
/// Keeping this out of the parent pipeline module preserves the rule that
/// pipeline files orchestrate compile/dispatch flow rather than owning every
/// metadata transformation.
pub(crate) mod descriptor_metadata;
/// On-disk WGSL + compiled-pipeline cache. Front-end calls
/// `load_or_compile_disk_wgsl` / `compiled_pipeline_cache_key` /
/// `persist_compiled_pipeline_cache` to skip Naga + Tint + driver
/// linkage for unchanged programs across `./cargo_full test` cycles.
pub(crate) mod disk_cache;
/// Sibling of `disk_cache`  -  cache invalidation triggered by source
/// edits, adapter changes, or feature-flag flips.
#[path = "pipeline/disk_cache_invalidation.rs"]
pub(crate) mod disk_cache_invalidation;
/// Trimmed output readback. Owns the contract that `output_byte_range`
/// transfers only meaningful bytes instead of whole output allocations.
pub(crate) mod output_readback;
/// Fallible output slot resizing shared by persistent and batched paths.
pub(crate) mod output_slots;
/// Persistent `Resource` to GPU-handle resolution and trap sidecar allocation.
pub(crate) mod persistent_resources;
// Tests for `disk_cache.rs` are declared *inside* `disk_cache.rs`
// itself (see `#[path = "disk_cache_tests.rs"] mod tests;` near the
// bottom of that file). The original layout used #[cfg(test)] mod
// tests inside the disk_cache.rs body so the test file's `super::*`
// resolved to disk_cache.rs's items; declaring it from the parent
// `pipeline` module instead breaks 55 unresolved-name references.
/// Persistent dispatch-item lifecycle (`DispatchItem`)  -  multi-call
/// reuse of bind groups, staging pools, and pipeline handles across
/// the same program-graph topology.
pub mod persistent;

#[cfg(test)]
mod tests;
