use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};

use vyre::ir::Program;
use vyre::{CompiledPipeline, DispatchConfig, VyreBackend};
use vyre_driver::{BindingPlan, BindingRole, Resource};

use crate::gpu_backend::require_gpu_dispatch_backend;
use crate::hash::{blake3_128_from_hasher, blake3_128_update_len_prefixed, StableHash128};

const COMPILED_PIPELINE_CACHE_MAX_ENTRIES: usize = 4096;
const COMPILED_PIPELINE_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;
type ProgramPipelineCacheKey = [u8; 32];
type StagePipelineCacheKey = StableHash128;
type BackendPipelineCacheKey = StableHash128;
type BackendProgramPipelineCacheKey = (BackendPipelineCacheKey, ProgramPipelineCacheKey);
type BackendStagePipelineCacheKey = (BackendPipelineCacheKey, StagePipelineCacheKey);

fn backend_pipeline_cache_key(backend_id: &str) -> BackendPipelineCacheKey {
    let mut hash = blake3::Hasher::new();
    blake3_128_update_len_prefixed(&mut hash, backend_id.as_bytes());
    blake3_128_from_hasher(&hash)
}

fn compiled_pipeline_cache_estimated_bytes(program: &Program) -> usize {
    let stats = program.stats();
    let entry_bytes = program.entry_op_id.as_ref().map_or(0usize, String::len);
    stats
        .node_count
        .checked_mul(128)
        .and_then(|bytes| bytes.checked_add(program.buffers.len().saturating_mul(128)))
        .and_then(|bytes| bytes.checked_add(entry_bytes))
        .unwrap_or(usize::MAX)
}

mod backend_acquire;
mod borrowed_cache;
mod cache_utils;
mod resident_dispatch;

use borrowed_cache::stage_pipeline_cache_key_hex;
use cache_utils::BoundedPipelineCache;

pub(crate) use backend_acquire::shared_dispatch_backend;
pub(crate) use borrowed_cache::{
    dispatch_borrowed_cached_into, dispatch_borrowed_stage_cached_into, stage_pipeline_cache_key,
};
pub(crate) use resident_dispatch::{
    dispatch_resident_stage_cached, dispatch_resident_stage_readback_cached_into,
    free_resident_blobs, ResidentBlob, ResidentStageInput,
};
