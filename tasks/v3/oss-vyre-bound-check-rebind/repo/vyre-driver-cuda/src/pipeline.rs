//! Precompiled CUDA pipeline implementation.

use std::sync::{Arc, Mutex};

use smallvec::SmallVec;
use vyre_driver::accounting::checked_add_usize_lazy;
use vyre_driver::binding::BindingRole;
use vyre_driver::input_identity::{domain_separated_exact_input_key, ExactInputKey};
use vyre_driver::{backend::private, BackendError, DispatchConfig, LaunchPlan};
use vyre_foundation::ir::Program;

use crate::backend::allocations::DeviceAllocation;
use crate::backend::module_cache::PtxSourceCacheKey;
use crate::backend::{CachedCudaGraph, CudaBackend, CudaDispatchPlan, ModuleCacheKey};
use crate::device::CudaDeviceCaps;

mod compiled_dispatch;
mod materialized_cache;
mod static_params;

#[cfg(test)]
pub(crate) use materialized_cache::{
    materialized_input_key, MAX_MATERIALIZED_OUTPUT_CACHE_BYTES_PER_PIPELINE,
};
pub(crate) use materialized_cache::{
    MaterializedPipelineOutputCache, MaterializedPipelineOutputCacheEntry,
};
use static_params::upload_static_launch_params;

/// CUDA pipeline with PTX already lowered and loaded into the backend cache.
#[derive(Debug)]
pub(crate) struct CudaCompiledPipeline {
    backend: CudaBackend,
    program: Arc<Program>,
    ptx_src: Arc<str>,
    module_key: ModuleCacheKey,
    prepared: CudaDispatchPlan,
    compiled_config: DispatchConfig,
    graph_cache: Mutex<SmallVec<[CachedCudaGraph; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>>,
    materialized_output_cache: Mutex<MaterializedPipelineOutputCache>,
    static_params: DeviceAllocation,
    id: String,
}

pub(crate) const MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE: usize = 32;
const CUDA_GRAPH_REPLAY_SMS_PER_LANE: usize = 8;
const CUDA_GRAPH_REPLAY_MIN_CONCURRENT_LANES: usize = 2;
const CUDA_GRAPH_REPLAY_VRAM_FRACTION_DENOMINATOR: u64 = 64;
const CUDA_COMPILED_PIPELINE_ID_DOMAIN: &[u8] = b"vyre.cuda.pipeline.compiled.v1";

fn cuda_compiled_pipeline_identity_key(
    ptx_source_key: &[u8; 32],
    module_key: &[u8; 32],
    launch: &LaunchPlan,
) -> Result<ExactInputKey, BackendError> {
    let element_count = launch.element_count.to_le_bytes();
    let workgroup_x = launch.workgroup[0].to_le_bytes();
    let workgroup_y = launch.workgroup[1].to_le_bytes();
    let workgroup_z = launch.workgroup[2].to_le_bytes();
    let grid_x = launch.grid[0].to_le_bytes();
    let grid_y = launch.grid[1].to_le_bytes();
    let grid_z = launch.grid[2].to_le_bytes();
    domain_separated_exact_input_key(
        CUDA_COMPILED_PIPELINE_ID_DOMAIN,
        0,
        0,
        &[
            ptx_source_key.as_slice(),
            module_key.as_slice(),
            element_count.as_slice(),
            workgroup_x.as_slice(),
            workgroup_y.as_slice(),
            workgroup_z.as_slice(),
            grid_x.as_slice(),
            grid_y.as_slice(),
            grid_z.as_slice(),
        ],
    )
}

impl CudaCompiledPipeline {
    /// Construct a compiled CUDA pipeline.
    pub(crate) fn new(
        backend: CudaBackend,
        program: Arc<Program>,
        ptx_src: Arc<str>,
        ptx_source_key: PtxSourceCacheKey,
        module_key: ModuleCacheKey,
        config: &DispatchConfig,
        prepared: CudaDispatchPlan,
    ) -> Result<Self, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_COMPILE_RANGE);
        let trace = crate::instrumentation::cuda_stage_trace_enabled();
        let started = std::time::Instant::now();
        if trace {
            tracing::debug!(
                "[cuda-pipeline] start entry={}",
                program.entry_op_id.as_deref().unwrap_or("<anonymous>")
            );
        }
        let digest = cuda_compiled_pipeline_identity_key(
            ptx_source_key.as_bytes(),
            &module_key.0,
            &prepared.launch,
        )?;
        if trace {
            tracing::debug!(
                "[cuda-pipeline] +{}ms digest ready",
                started.elapsed().as_millis()
            );
        }
        let static_params = upload_static_launch_params(&backend, &prepared.launch.param_words)?;
        if trace {
            tracing::debug!(
                "[cuda-pipeline] +{}ms static params ready bytes={}",
                started.elapsed().as_millis(),
                static_params.byte_len
            );
        }
        Ok(Self {
            backend,
            program,
            ptx_src,
            module_key,
            prepared,
            compiled_config: config.clone(),
            graph_cache: Mutex::new(SmallVec::new()),
            materialized_output_cache: Mutex::new(MaterializedPipelineOutputCache::default()),
            static_params,
            id: format!("cuda:{}", blake3::Hash::from(digest).to_hex()),
        })
    }
}

impl Drop for CudaCompiledPipeline {
    fn drop(&mut self) {
        self.backend
            .transient_pool
            .release(std::mem::take(&mut self.static_params));
    }
}

impl private::Sealed for CudaCompiledPipeline {}

fn cuda_graph_replay_enabled() -> bool {
    crate::instrumentation::cuda_graph_replay_enabled()
}

pub(crate) fn cuda_graph_lane_count_for_batch(
    caps: &CudaDeviceCaps,
    prepared: &CudaDispatchPlan,
    batches: &[&[&[u8]]],
) -> Result<usize, BackendError> {
    if batches.is_empty() {
        return Ok(0);
    }
    let hardware_lanes = cuda_graph_hardware_lane_capacity(caps)?;
    let shape_bytes = cuda_graph_shape_cached_bytes(prepared, batches[0])?;
    let shape_bytes_u64 = u64::try_from(shape_bytes).map_err(|_| BackendError::InvalidProgram {
        fix: "Fix: CUDA graph replay shape byte count exceeds u64; split the replay batch before lane planning.".to_string(),
    })?;
    let host_memory_budget_cap = u64::try_from(usize::MAX).map_err(|source| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: host usize::MAX cannot fit u64 while planning CUDA graph lanes: {source}; use a supported host pointer width."
            ),
        }
    })?;
    let memory_budget_u64 = (caps.total_memory / CUDA_GRAPH_REPLAY_VRAM_FRACTION_DENOMINATOR)
        .max(shape_bytes_u64)
        .min(host_memory_budget_cap);
    let memory_budget = usize::try_from(memory_budget_u64).map_err(|source| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA graph replay memory budget {memory_budget_u64} cannot fit usize: {source}; split the replay batch before lane planning."
            ),
        }
    })?;
    let memory_lanes = if shape_bytes == 0 {
        MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE
    } else {
        (memory_budget / shape_bytes)
            .max(1)
            .min(MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE)
    };
    Ok(batches.len().min(hardware_lanes).min(memory_lanes).max(1))
}

fn cuda_graph_hardware_lane_capacity(caps: &CudaDeviceCaps) -> Result<usize, BackendError> {
    if !caps.concurrent_kernels {
        return Ok(1);
    }
    let sms = usize::try_from(caps.multi_processor_count_u32()).map_err(|source| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA multiprocessor count cannot fit usize during graph lane planning: {source}; reject corrupt device capabilities before compiling graph replay."
            ),
        }
    });
    let sms = sms?;
    let lanes = sms.div_ceil(CUDA_GRAPH_REPLAY_SMS_PER_LANE);
    Ok(lanes
        .max(CUDA_GRAPH_REPLAY_MIN_CONCURRENT_LANES)
        .min(MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE))
}

fn cuda_graph_shape_cached_bytes(
    prepared: &CudaDispatchPlan,
    inputs: &[&[u8]],
) -> Result<usize, BackendError> {
    let mut bytes = bucketed_len(std::mem::size_of_val(
        prepared.launch.param_words.as_slice(),
    ))?;
    for binding in &prepared.bindings.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        let byte_len = binding
            .input_index
            .and_then(|input_index| inputs.get(input_index).map(|input| input.len()))
            .or(binding.static_byte_len)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA graph replay shape cache found binding `{}` without a runtime input or static byte length. Preserve concrete binding byte lengths during dispatch planning instead of treating missing sizes as zero.",
                    binding.name
                ),
            })?;
        bytes = add_shape_bytes(bytes, bucketed_len(byte_len)?)?;
        if binding.input_index.is_some() {
            bytes = add_shape_bytes(bytes, bucketed_len(byte_len)?)?;
        }
        if binding.output_index.is_some() {
            bytes = add_shape_bytes(bytes, bucketed_len(byte_len)?)?;
        }
    }
    Ok(bytes)
}

fn add_shape_bytes(total: usize, component: usize) -> Result<usize, BackendError> {
    checked_add_usize_lazy(total, component, || {
        BackendError::InvalidProgram {
        fix: "Fix: CUDA graph replay cached shape byte count overflowed; split the replay batch before graph-cache lane planning.".to_string(),
    }
    })
}

fn bucketed_len(byte_len: usize) -> Result<usize, BackendError> {
    byte_len
        .max(1)
        .checked_next_power_of_two()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA graph replay bucketed shape byte count overflowed; split the oversized input or disable graph replay for this shape.".to_string(),
        })
}

#[cfg(test)]
mod tests;
