//! `CompiledPipeline` implementation for precompiled CUDA pipelines.
//!
//! The parent `pipeline` module owns construction and static launch state. This
//! module owns dispatch entrypoints, CUDA graph replay selection, dynamic GPU
//! dispatch when runtime policy changes, and persistent-resource output routing.

use std::sync::MutexGuard;

use smallvec::SmallVec;
use vyre_driver::{
    borrowed_input_batch_shapes_match, dispatch_configs_share_launch_shape, BackendError,
    BindingRole, CompiledPipeline, DispatchConfig, OutputBuffers, Resource,
};

use crate::backend::cuda_graph_replay::{CudaGraphReplayInputState, CudaGraphReplayStats};
use crate::backend::resident_dispatch::{next_resident_handle, CudaResidentDispatch};
use crate::backend::staging_reserve::{reserve_smallvec, reserved_vec, resize_vec_slots};
use crate::backend::CachedCudaGraph;
use crate::numeric::CUDA_NUMERIC;
use crate::pipeline::materialized_cache::{materialized_input_key, MaterializedInputKey};
use crate::pipeline::{
    cuda_graph_lane_count_for_batch, cuda_graph_replay_enabled, CudaCompiledPipeline,
    MaterializedPipelineOutputCache, MaterializedPipelineOutputCacheEntry,
    MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE,
};

fn compiled_graph_batch_inputs<'a>(
    batches: &'a [&[&[u8]]],
    batch_index: usize,
    context: &'static str,
) -> Result<&'a [&'a [u8]], BackendError> {
    batches
        .get(batch_index)
        .copied()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: compiled CUDA graph replay {context} expected batch index {batch_index} but only {} batch input set(s) were supplied. Rebuild the materialized batch partition before replay.",
                batches.len()
            ),
        })
}

fn compiled_graph_output<'a>(
    outputs: &'a [OutputBuffers],
    batch_index: usize,
    context: &'static str,
) -> Result<&'a OutputBuffers, BackendError> {
    outputs
        .get(batch_index)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: compiled CUDA graph replay {context} expected output batch index {batch_index} but only {} output slot(s) were available. Resize output slots before replay.",
                outputs.len()
            ),
        })
}

fn compiled_graph_output_mut<'a>(
    outputs: &'a mut [OutputBuffers],
    batch_index: usize,
    context: &'static str,
) -> Result<&'a mut OutputBuffers, BackendError> {
    let output_count = outputs.len();
    outputs
        .get_mut(batch_index)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: compiled CUDA graph replay {context} expected mutable output batch index {batch_index} but only {output_count} output slot(s) were available. Resize output slots before replay.",
            ),
        })
}

fn compiled_graph_lane<'a>(
    lanes: &'a [CachedCudaGraph],
    lane: usize,
    context: &'static str,
) -> Result<&'a CachedCudaGraph, BackendError> {
    let lane_count = lanes.len();
    lanes.get(lane).ok_or_else(|| BackendError::InvalidProgram {
        fix: format!(
            "Fix: compiled CUDA graph replay {context} expected lane {lane} but only {lane_count} cached graph lane(s) were available. Rebuild the graph replay lane plan.",
        ),
    })
}

fn compiled_graph_lane_mut<'a>(
    lanes: &'a mut [CachedCudaGraph],
    lane: usize,
    context: &'static str,
) -> Result<&'a mut CachedCudaGraph, BackendError> {
    let lane_count = lanes.len();
    lanes
        .get_mut(lane)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: compiled CUDA graph replay {context} expected lane {lane} but only {lane_count} cached graph lane(s) were available. Rebuild the graph replay lane plan.",
            ),
        })
}

#[derive(Clone, Copy, Debug)]
struct MaterializedBatchMiss {
    batch_index: usize,
    input_key: MaterializedInputKey,
}

#[derive(Clone, Copy, Debug)]
struct LaunchedMaterializedBatch {
    lane: usize,
    batch_index: usize,
    input_key: MaterializedInputKey,
    replay_stats: CudaGraphReplayStats,
}

#[derive(Debug)]
struct CachedGraphReplaySelection {
    graph: CachedCudaGraph,
    input_state: CudaGraphReplayInputState,
}

impl CompiledPipeline for CudaCompiledPipeline {
    fn id(&self) -> &str {
        &self.id
    }

    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let borrowed = vyre_driver::borrowed_input_slices(inputs, "cuda compiled borrowed input")?;
        self.dispatch_borrowed(&borrowed, config)
    }

    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        if !dispatch_configs_share_launch_shape(&self.compiled_config, config) {
            return self
                .backend
                .dispatch_borrowed_async_with_ptx_key(
                    &self.program,
                    inputs,
                    config,
                    &self.ptx_src,
                    self.module_key,
                )?
                .await_result();
        }
        let mut outputs = reserved_vec(self.prepared.output_binding_indices.len(), "output")?;
        self.dispatch_borrowed_into(inputs, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_borrowed_timed(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_DISPATCH_RANGE);
        if !dispatch_configs_share_launch_shape(&self.compiled_config, config) {
            return self.backend.dispatch_borrowed_timed_with_ptx_key(
                &self.program,
                inputs,
                config,
                &self.ptx_src,
                self.module_key,
            );
        }
        if !cuda_graph_replay_enabled() || self.prepared.cooperative {
            return self.backend.dispatch_borrowed_timed_with_ptx_key(
                &self.program,
                inputs,
                config,
                &self.ptx_src,
                self.module_key,
            );
        }
        let started = std::time::Instant::now();
        let mut outputs = reserved_vec(self.prepared.output_binding_indices.len(), "timed output")?;
        let input_key = materialized_input_key(inputs)?;
        if self.materialized_output_cache_hit_with_key_into(inputs, &input_key, &mut outputs)? {
            let wall_ns = CUDA_NUMERIC
                .elapsed_nanos_u64(started, "cuda graph materialized timed hit wall latency")?;
            // Materialized output cache hit: no kernel was launched, so the
            // device performed exactly zero work.  Report Some(0) -- the exact,
            // non-fabricated device time for a hit.  None would mean "device
            // time unknown" and route this to timed_dispatches_missing_device_time,
            // which is wrong: we know the device did nothing.  Some(0) also lets
            // the release perf gate see the cache eliminate device work (0 ns).
            self.backend
                .telemetry
                .record_timed_dispatch(wall_ns, Some(0), None, None);
            return Ok(vyre_driver::TimedDispatchResult {
                outputs,
                wall_ns,
                device_ns: Some(0),
                enqueue_ns: None,
                wait_ns: None,
            });
        }
        let (mut cached, input_state) = match self
            .take_cached_graph_with_replay_state(inputs, &input_key)?
        {
            Some(selection) => (selection.graph, selection.input_state),
            None => {
                let cached =
                    self.backend
                        .record_cuda_graph_borrowed(&self.program, inputs, config)?;
                let input_state = self
                    .backend
                    .prepare_cuda_graph_replay_input_state_with_key(&cached, inputs, input_key)?;
                (cached, input_state)
            }
        };
        let replay_result = self
            .backend
            .dispatch_via_cuda_graph_timed_with_input_state_into(
                &mut cached,
                inputs,
                &input_state,
                &mut outputs,
            );
        if replay_result.is_ok() {
            self.return_cached_graph(cached)?;
            self.remember_materialized_output_cache_with_key(inputs, input_key, &outputs)?;
        } else {
            std::mem::forget(cached);
        }
        let device_ns = replay_result?;
        let wall_ns = CUDA_NUMERIC.elapsed_nanos_u64(started, "cuda graph replay wall latency")?;
        self.backend
            .telemetry
            .record_timed_dispatch(wall_ns, device_ns, None, None);
        Ok(vyre_driver::TimedDispatchResult {
            outputs,
            wall_ns,
            device_ns,
            enqueue_ns: None,
            wait_ns: None,
        })
    }

    fn dispatch_borrowed_into(
        &self,
        inputs: &[&[u8]],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_DISPATCH_RANGE);
        if !dispatch_configs_share_launch_shape(&self.compiled_config, config) {
            self.backend
                .dispatch_borrowed_async_with_ptx_key(
                    &self.program,
                    inputs,
                    config,
                    &self.ptx_src,
                    self.module_key,
                )?
                .await_result_into(outputs)?;
            return Ok(());
        }
        if !cuda_graph_replay_enabled() || self.prepared.cooperative {
            self.backend
                .dispatch_borrowed_async_with_ptx_key(
                    &self.program,
                    inputs,
                    config,
                    &self.ptx_src,
                    self.module_key,
                )?
                .await_result_into(outputs)?;
            return Ok(());
        }
        let input_key = materialized_input_key(inputs)?;
        if self.materialized_output_cache_hit_with_key_into(inputs, &input_key, outputs)? {
            return Ok(());
        }
        let (mut cached, input_state) = match self
            .take_cached_graph_with_replay_state(inputs, &input_key)?
        {
            Some(selection) => (selection.graph, selection.input_state),
            None => {
                let cached =
                    self.backend
                        .record_cuda_graph_borrowed(&self.program, inputs, config)?;
                let input_state = self
                    .backend
                    .prepare_cuda_graph_replay_input_state_with_key(&cached, inputs, input_key)?;
                (cached, input_state)
            }
        };
        let replay_result = self.backend.dispatch_via_cuda_graph_with_input_state_into(
            &mut cached,
            inputs,
            &input_state,
            outputs,
        );
        if replay_result.is_ok() {
            self.return_cached_graph(cached)?;
            self.remember_materialized_output_cache_with_key(inputs, input_key, outputs)?;
        } else {
            std::mem::forget(cached);
        }
        replay_result
    }

    fn dispatch_borrowed_batched(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = reserved_vec(batches.len(), "batched output")?;
        self.dispatch_borrowed_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_borrowed_batched_into(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let _profiler_range = crate::profiler::cuda_profiler_range(
            crate::profiler::CUDA_PIPELINE_BATCH_DISPATCH_RANGE,
        );
        if batches.is_empty() {
            outputs.clear();
            return Ok(());
        }
        if cuda_graph_replay_enabled()
            && !self.prepared.cooperative
            && dispatch_configs_share_launch_shape(&self.compiled_config, config)
            && borrowed_input_batch_shapes_match(batches)
        {
            return self.dispatch_borrowed_batched_via_cuda_graph_lanes(batches, config, outputs);
        }
        let mut pending = SmallVec::<[_; 8]>::new();
        reserve_smallvec(&mut pending, batches.len(), "pending dispatch")?;
        if dispatch_configs_share_launch_shape(&self.compiled_config, config) {
            for inputs in batches {
                pending.push(self.backend.dispatch_prepared_borrowed_async_with_ptx_key(
                    &self.program,
                    inputs,
                    &self.ptx_src,
                    self.module_key,
                    &self.prepared,
                )?);
            }
        } else {
            for inputs in batches {
                pending.push(self.backend.dispatch_borrowed_async_with_ptx_key(
                    &self.program,
                    inputs,
                    config,
                    &self.ptx_src,
                    self.module_key,
                )?);
            }
        }

        resize_vec_slots(outputs, pending.len(), "batched output")?;
        for (dispatch, item_outputs) in pending.into_iter().zip(outputs.iter_mut()) {
            dispatch.await_result_into(item_outputs)?;
        }
        Ok(())
    }

    fn dispatch_persistent_handles(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<OutputBuffers, BackendError> {
        let mut outputs = reserved_vec(
            self.prepared.output_binding_indices.len(),
            "persistent output",
        )?;
        self.dispatch_persistent_handles_into(inputs, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_persistent_handles_timed(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_DISPATCH_RANGE);
        if !dispatch_configs_share_launch_shape(&self.compiled_config, config)
            || crate::instrumentation::cuda_resident_borrowed_fallback_enabled()
        {
            let started = std::time::Instant::now();
            let outputs = self.dispatch_persistent_handles(inputs, config)?;
            let wall_ns = crate::numeric::CUDA_NUMERIC
                .elapsed_nanos_u64(started, "compiled persistent fallback wall latency")?;
            self.backend
                .telemetry
                .record_timed_dispatch(wall_ns, None, None, None);
            return Ok(vyre_driver::TimedDispatchResult {
                outputs,
                wall_ns,
                device_ns: None,
                enqueue_ns: None,
                wait_ns: None,
            });
        }

        let started = std::time::Instant::now();
        let enqueue_started = std::time::Instant::now();
        let handles = self.backend.resident_handles_from_resources(inputs)?;
        let dispatch = self.backend.dispatch_resident_async_concrete_with_ptx_key(
            &self.program,
            &handles,
            config,
            &self.ptx_src,
            self.module_key,
            true,
            (self.static_params.ptr != 0).then_some(self.static_params.ptr),
            true,
            &self.prepared,
        )?;
        let enqueue_ns = crate::numeric::CUDA_NUMERIC
            .elapsed_nanos_u64(enqueue_started, "compiled persistent enqueue latency")?;
        let wait_started = std::time::Instant::now();
        let (outputs, device_ns) = dispatch.pending.await_timed_result()?;
        let wait_ns = crate::numeric::CUDA_NUMERIC
            .elapsed_nanos_u64(wait_started, "compiled persistent wait latency")?;
        let wall_ns = crate::numeric::CUDA_NUMERIC
            .elapsed_nanos_u64(started, "compiled persistent wall latency")?;
        self.backend.telemetry.record_timed_dispatch(
            wall_ns,
            device_ns,
            Some(enqueue_ns),
            Some(wait_ns),
        );
        Ok(vyre_driver::TimedDispatchResult {
            outputs,
            wall_ns,
            device_ns,
            enqueue_ns: Some(enqueue_ns),
            wait_ns: Some(wait_ns),
        })
    }

    fn dispatch_persistent_handles_into(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_DISPATCH_RANGE);
        let handles = self.backend.resident_handles_from_resources(inputs)?;
        if dispatch_configs_share_launch_shape(&self.compiled_config, config)
            && !crate::instrumentation::cuda_resident_borrowed_fallback_enabled()
        {
            let dispatch = self.backend.dispatch_resident_async_concrete_with_ptx_key(
                &self.program,
                &handles,
                config,
                &self.ptx_src,
                self.module_key,
                false,
                (self.static_params.ptr != 0).then_some(self.static_params.ptr),
                true,
                &self.prepared,
            )?;
            let (dispatch_outputs, _) = dispatch.pending.await_timed_result()?;
            vyre_driver::replace_output_buffers_preserving_slots(dispatch_outputs, outputs);
            return Ok(());
        }
        self.backend.dispatch_resident_outputs_with_ptx_key_into(
            &self.program,
            &handles,
            config,
            &self.ptx_src,
            self.module_key,
            outputs,
        )
    }

    fn dispatch_persistent_handles_batched(
        &self,
        batches: &[&[Resource]],
        config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, BackendError> {
        let mut outputs = reserved_vec(batches.len(), "persistent batched output")?;
        self.dispatch_persistent_handles_batched_into(batches, config, &mut outputs)?;
        Ok(outputs)
    }

    fn dispatch_persistent_handles_batched_into(
        &self,
        batches: &[&[Resource]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let _profiler_range = crate::profiler::cuda_profiler_range(
            crate::profiler::CUDA_PIPELINE_BATCH_DISPATCH_RANGE,
        );
        if batches.is_empty() {
            outputs.clear();
            return Ok(());
        }
        let mut resident_batches =
            SmallVec::<[SmallVec<[crate::backend::CudaResidentBuffer; 8]>; 8]>::new();
        reserve_smallvec(&mut resident_batches, batches.len(), "resident batch")?;
        for batch in batches {
            resident_batches.push(self.backend.resident_handles_from_resources(batch)?);
        }

        self.dispatch_resident_batches_into(&resident_batches, config, outputs)
    }

    fn dispatch_persistent_handle_rows_into(
        &self,
        rows: &[[Resource; 4]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let _profiler_range = crate::profiler::cuda_profiler_range(
            crate::profiler::CUDA_PIPELINE_BATCH_DISPATCH_RANGE,
        );
        if rows.is_empty() {
            outputs.clear();
            return Ok(());
        }
        let mut resident_batches =
            SmallVec::<[SmallVec<[crate::backend::CudaResidentBuffer; 8]>; 8]>::new();
        reserve_smallvec(&mut resident_batches, rows.len(), "resident row batch")?;
        for row in rows {
            resident_batches.push(
                self.backend
                    .resident_handles_from_resources(row.as_slice())?,
            );
        }

        self.dispatch_resident_batches_into(&resident_batches, config, outputs)
    }

    fn dispatch_persistent_resource_outputs(
        &self,
        inputs: &[Resource],
        config: &DispatchConfig,
    ) -> Result<Vec<Resource>, BackendError> {
        let _profiler_range =
            crate::profiler::cuda_profiler_range(crate::profiler::CUDA_PIPELINE_DISPATCH_RANGE);
        let handles = self.backend.resident_handles_from_resources(inputs)?;
        let borrowed_fallback = crate::instrumentation::cuda_resident_borrowed_fallback_enabled();
        let same_shape = dispatch_configs_share_launch_shape(&self.compiled_config, config);
        let prepared_storage;
        let (prepared, static_params_ptr) = if same_shape {
            (
                &self.prepared,
                (self.static_params.ptr != 0).then_some(self.static_params.ptr),
            )
        } else {
            prepared_storage =
                self.backend
                    .prepare_resident_dispatch(&self.program, &handles, config)?;
            (&prepared_storage, None)
        };
        let mut output_handles = SmallVec::<[crate::backend::CudaResidentBuffer; 8]>::new();
        reserve_smallvec(
            &mut output_handles,
            prepared.output_binding_indices.len(),
            "compiled resident resource output handle",
        )?;
        let mut next_handle = 0usize;
        for binding in &prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let handle = next_resident_handle(
                &handles,
                &mut next_handle,
                "compiled resident resource output routing",
            )?;
            if binding.output_index.is_some() {
                output_handles.push(handle);
            }
        }
        if borrowed_fallback {
            self.backend
                .dispatch_resident_via_borrowed(&self.program, &handles, config)?;
        } else {
            self.backend
                .dispatch_resident_async_concrete_with_ptx_key(
                    &self.program,
                    &handles,
                    config,
                    &self.ptx_src,
                    self.module_key,
                    false,
                    static_params_ptr,
                    false,
                    prepared,
                )?
                .pending
                .await_timed_result()?;
        }
        let mut resources = reserved_vec(output_handles.len(), "resource output")?;
        for handle in output_handles {
            resources.push(Resource::Resident(handle.id));
        }
        Ok(resources)
    }
}

impl CudaCompiledPipeline {
    fn dispatch_resident_batches_into(
        &self,
        resident_batches: &[SmallVec<[crate::backend::CudaResidentBuffer; 8]>],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        if resident_batches.is_empty() {
            outputs.clear();
            return Ok(());
        }
        if !dispatch_configs_share_launch_shape(&self.compiled_config, config) {
            return self.dispatch_dynamic_persistent_batches_concurrently(
                resident_batches,
                config,
                outputs,
            );
        }

        let resident_dispatch = self
            .backend
            .dispatch_resident_batch_async_concrete_with_ptx_key(
                &self.program,
                resident_batches,
                config,
                &self.ptx_src,
                self.module_key,
                (self.static_params.ptr != 0).then_some(self.static_params.ptr),
                &self.prepared,
            );
        let resident_dispatch = resident_dispatch?;
        let output_handles = resident_dispatch.output_handles;
        let output_readbacks = resident_dispatch.output_readbacks;
        resident_dispatch.pending.await_timed_result()?;
        self.backend.download_resident_readback_batches_many_into(
            &output_handles,
            &output_readbacks,
            outputs,
        )
    }

    fn dispatch_dynamic_persistent_batches_concurrently(
        &self,
        resident_batches: &[SmallVec<[crate::backend::CudaResidentBuffer; 8]>],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let mut dispatches = SmallVec::<[CudaResidentDispatch; 8]>::new();
        reserve_smallvec(
            &mut dispatches,
            resident_batches.len(),
            "dynamic resident dispatch",
        )?;
        for handles in resident_batches {
            let prepared =
                self.backend
                    .prepare_resident_dispatch(&self.program, handles, config)?;
            dispatches.push(self.backend.dispatch_resident_async_concrete_with_ptx_key(
                &self.program,
                handles,
                config,
                &self.ptx_src,
                self.module_key,
                false,
                None,
                true,
                &prepared,
            )?);
        }

        resize_vec_slots(outputs, dispatches.len(), "dynamic resident output")?;
        for (dispatch, item_outputs) in dispatches.into_iter().zip(outputs.iter_mut()) {
            let output_handles = dispatch.output_handles;
            let output_readbacks = dispatch.output_readbacks;
            dispatch.pending.await_timed_result()?;
            self.backend.download_resident_readbacks_many_into(
                &output_handles,
                &output_readbacks,
                item_outputs,
            )?;
        }
        Ok(())
    }

    fn dispatch_borrowed_batched_via_cuda_graph_lanes(
        &self,
        batches: &[&[&[u8]]],
        config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let miss_entries = self.materialized_output_batch_cache_partition_into(batches, outputs)?;
        if miss_entries.is_empty() {
            return Ok(());
        }

        let mut miss_batches = SmallVec::<[&[&[u8]]; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>::new();
        reserve_smallvec(
            &mut miss_batches,
            miss_entries.len(),
            "cuda graph miss batch",
        )?;
        for miss in miss_entries.iter().copied() {
            miss_batches.push(compiled_graph_batch_inputs(
                batches,
                miss.batch_index,
                "miss partition",
            )?);
        }
        let lane_count =
            cuda_graph_lane_count_for_batch(&self.backend.caps, &self.prepared, &miss_batches)?;
        let first_miss =
            miss_entries
                .first()
                .copied()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: compiled CUDA graph replay produced zero miss entry after a non-empty miss partition. Rebuild the materialized batch partition before lane replay."
                        .to_string(),
                })?;
        let first_miss_batch =
            miss_batches
                .first()
                .copied()
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: compiled CUDA graph replay produced zero miss batch input sets after a non-empty miss partition. Rebuild the materialized batch partition before lane replay."
                        .to_string(),
                })?;
        let mut lanes = SmallVec::<[CachedCudaGraph; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>::new();
        reserve_smallvec(&mut lanes, lane_count, "cuda graph lane")?;
        for _ in 0..lane_count {
            lanes.push(
                match self.take_cached_graph_with_key(first_miss_batch, &first_miss.input_key)? {
                    Some(cached) => cached,
                    None => self.backend.record_cuda_graph_borrowed(
                        &self.program,
                        first_miss_batch,
                        config,
                    )?,
                },
            );
        }

        for (chunk_index, chunk) in miss_entries.chunks(lane_count).enumerate() {
            let mut launched = SmallVec::<
                [LaunchedMaterializedBatch; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE],
            >::new();
            let chunk_start = match chunk_index.checked_mul(lane_count).ok_or_else(|| {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA graph replay chunk {chunk_index} with {lane_count} lane(s) overflowed miss-entry indexing; split the replay batch."
                    ),
                }
            }) {
                Ok(chunk_start) => chunk_start,
                Err(error) => return self.return_cached_graph_lanes_after_error(lanes, error),
            };
            for lane in 0..chunk.len() {
                let miss_entry_index =
                    match chunk_start.checked_add(lane).ok_or_else(|| {
                        BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA graph replay chunk {chunk_index} lane {lane} overflowed miss-entry indexing; split the replay batch."
                            ),
                        }
                    }) {
                        Ok(miss_entry_index) => miss_entry_index,
                        Err(error) => {
                            return self.finish_and_return_cuda_graph_lanes_after_error(
                                lanes, &launched, outputs, error,
                            );
                        }
                    };
                let miss = match miss_entries.get(miss_entry_index).copied().ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA graph replay chunk {chunk_index} lane {lane} resolved outside the miss-entry table; rebuild the compiled pipeline."
                        ),
                    }
                }) {
                    Ok(miss) => miss,
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                };
                let batch_index = miss.batch_index;
                let inputs = match compiled_graph_batch_inputs(batches, batch_index, "lane replay")
                {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                };
                let lane_graph = match compiled_graph_lane(&lanes, lane, "prepare replay input") {
                    Ok(lane_graph) => lane_graph,
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                };
                let input_state = match self.backend.prepare_cuda_graph_replay_input_state_with_key(
                    lane_graph,
                    inputs,
                    miss.input_key,
                ) {
                    Ok(input_state) => input_state,
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                };
                let lane_graph =
                    match compiled_graph_lane_mut(&mut lanes, lane, "materialized cache probe") {
                        Ok(lane_graph) => lane_graph,
                        Err(error) => {
                            return self.finish_and_return_cuda_graph_lanes_after_error(
                                lanes, &launched, outputs, error,
                            );
                        }
                    };
                let output_slot = match compiled_graph_output_mut(
                    outputs,
                    batch_index,
                    "materialized cache probe",
                ) {
                    Ok(output_slot) => output_slot,
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                };
                match self
                    .backend
                    .try_cuda_graph_materialized_cache_with_input_state_into(
                        lane_graph,
                        inputs,
                        &input_state,
                        output_slot,
                    ) {
                    Ok(true) => {
                        let output_slot = match compiled_graph_output(
                            outputs,
                            batch_index,
                            "materialized cache remember",
                        ) {
                            Ok(output_slot) => output_slot,
                            Err(error) => {
                                return self.finish_and_return_cuda_graph_lanes_after_error(
                                    lanes, &launched, outputs, error,
                                );
                            }
                        };
                        if let Err(error) = self.remember_materialized_output_cache_with_key(
                            inputs,
                            miss.input_key,
                            output_slot,
                        ) {
                            return self.finish_and_return_cuda_graph_lanes_after_error(
                                lanes, &launched, outputs, error,
                            );
                        }
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                }
                let lane_graph =
                    match compiled_graph_lane_mut(&mut lanes, lane, "enqueue lane replay") {
                        Ok(lane_graph) => lane_graph,
                        Err(error) => {
                            return self.finish_and_return_cuda_graph_lanes_after_error(
                                lanes, &launched, outputs, error,
                            );
                        }
                    };
                match self.backend.enqueue_cuda_graph_replay_with_input_state(
                    lane_graph,
                    inputs,
                    &input_state,
                ) {
                    Ok(replay_stats) => launched.push(LaunchedMaterializedBatch {
                        lane,
                        batch_index,
                        input_key: miss.input_key,
                        replay_stats,
                    }),
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                }
            }
            if !launched.is_empty() {
                match CUDA_NUMERIC.usize_to_u64(launched.len(), "cuda graph replay lane count") {
                    Ok(lanes) => self.backend.record_cuda_graph_batched_replay_chunk(lanes),
                    Err(error) => {
                        return self.finish_and_return_cuda_graph_lanes_after_error(
                            lanes, &launched, outputs, error,
                        );
                    }
                }
            }
            if let Err(error) =
                self.finish_cuda_graph_indexed_lane_replays(&mut lanes, &launched, outputs)
            {
                std::mem::forget(lanes);
                return Err(error);
            }
            for launched_batch in launched.iter().copied() {
                let inputs = match compiled_graph_batch_inputs(
                    batches,
                    launched_batch.batch_index,
                    "materialized replay remember",
                ) {
                    Ok(inputs) => inputs,
                    Err(error) => {
                        return self.return_cached_graph_lanes_after_error(lanes, error);
                    }
                };
                let output_slot = match compiled_graph_output(
                    outputs,
                    launched_batch.batch_index,
                    "materialized replay remember",
                ) {
                    Ok(output_slot) => output_slot,
                    Err(error) => {
                        return self.return_cached_graph_lanes_after_error(lanes, error);
                    }
                };
                if let Err(error) = self.remember_materialized_output_cache_with_key(
                    inputs,
                    launched_batch.input_key,
                    output_slot,
                ) {
                    return self.return_cached_graph_lanes_after_error(lanes, error);
                }
            }
        }

        self.return_cached_graph_lanes(lanes)
    }

    fn materialized_output_batch_cache_partition_into(
        &self,
        batches: &[&[&[u8]]],
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<SmallVec<[MaterializedBatchMiss; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>, BackendError>
    {
        let mut input_keys = SmallVec::<
            [(usize, MaterializedInputKey); MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE],
        >::new();
        reserve_smallvec(
            &mut input_keys,
            batches.len(),
            "cuda graph materialized batch input key",
        )?;
        for (batch_index, inputs) in batches.iter().enumerate() {
            input_keys.push((batch_index, materialized_input_key(inputs)?));
        }
        let mut miss_entries =
            SmallVec::<[MaterializedBatchMiss; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>::new();
        reserve_smallvec(
            &mut miss_entries,
            batches.len(),
            "cuda graph materialized batch miss index",
        )?;
        let mut hit_snapshots = SmallVec::<[_; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>::new();
        reserve_smallvec(
            &mut hit_snapshots,
            batches.len(),
            "cuda graph materialized batch hit snapshot",
        )?;
        {
            let cache = self.lock_materialized_output_cache("during batch cache replay")?;
            for (batch_index, input_key) in input_keys.iter().copied() {
                let inputs =
                    compiled_graph_batch_inputs(batches, batch_index, "batch cache lookup")?;
                if let Some(snapshot) = cache.snapshot_with_key(inputs, &input_key) {
                    hit_snapshots.push((batch_index, snapshot));
                } else {
                    miss_entries.push(MaterializedBatchMiss {
                        batch_index,
                        input_key,
                    });
                }
            }
        }
        resize_vec_slots(
            outputs,
            batches.len(),
            "cuda graph materialized batch output",
        )?;
        for (batch_index, snapshot) in hit_snapshots {
            snapshot.copy_into(compiled_graph_output_mut(
                outputs,
                batch_index,
                "batch cache hit copy",
            )?)?;
            self.backend
                .telemetry
                .record_cuda_graph_materialized_cache_hit();
        }
        Ok(miss_entries)
    }

    fn materialized_output_cache_hit_with_key_into(
        &self,
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
        outputs: &mut OutputBuffers,
    ) -> Result<bool, BackendError> {
        let snapshot = {
            let cache = self.lock_materialized_output_cache("during single-dispatch replay")?;
            cache.snapshot_with_key(inputs, input_key)
        };
        let Some(snapshot) = snapshot else {
            return Ok(false);
        };
        snapshot.copy_into(outputs)?;
        self.backend
            .telemetry
            .record_cuda_graph_materialized_cache_hit();
        Ok(true)
    }

    fn remember_materialized_output_cache_with_key(
        &self,
        inputs: &[&[u8]],
        input_key: MaterializedInputKey,
        outputs: &OutputBuffers,
    ) -> Result<(), BackendError> {
        let Some(entry) = MaterializedPipelineOutputCacheEntry::new_with_key_if_cacheable(
            inputs, &input_key, outputs,
        )?
        else {
            return Ok(());
        };
        let mut cache = self.lock_materialized_output_cache("while storing keyed replay output")?;
        cache.remember_entry(entry)
    }

    fn lock_materialized_output_cache(
        &self,
        action: &'static str,
    ) -> Result<MutexGuard<'_, MaterializedPipelineOutputCache>, BackendError> {
        self.materialized_output_cache.lock().map_err(|_| {
            BackendError::DispatchFailed {
                code: None,
                message: format!(
                    "CUDA compiled-pipeline materialized output cache lock poisoned {action}. Fix: rebuild the compiled pipeline after a panic during materialized cache access."
                ),
            }
        })
    }

    fn finish_cuda_graph_indexed_lane_replays(
        &self,
        lanes: &mut [CachedCudaGraph],
        launched: &[LaunchedMaterializedBatch],
        outputs: &mut [OutputBuffers],
    ) -> Result<(), BackendError> {
        let mut finish_error = None;
        for launched_batch in launched.iter().copied() {
            let lane = match compiled_graph_lane_mut(
                lanes,
                launched_batch.lane,
                "finish indexed lane replay",
            ) {
                Ok(lane) => lane,
                Err(error) => {
                    if finish_error.is_none() {
                        finish_error = Some(error);
                    }
                    continue;
                }
            };
            let output = match compiled_graph_output_mut(
                outputs,
                launched_batch.batch_index,
                "finish indexed lane replay",
            ) {
                Ok(output) => output,
                Err(error) => {
                    if let Err(cleanup_error) = self
                        .finish_cuda_graph_lane_replay_discarding_outputs(
                            lane,
                            launched_batch.replay_stats,
                        )
                    {
                        if finish_error.is_none() {
                            finish_error = Some(cleanup_error);
                        }
                    }
                    if finish_error.is_none() {
                        finish_error = Some(error);
                    }
                    continue;
                }
            };
            if let Err(error) = self.backend.finish_cuda_graph_replay_into(
                lane,
                launched_batch.replay_stats,
                output,
            ) {
                if finish_error.is_none() {
                    finish_error = Some(error);
                }
            }
        }
        if let Some(error) = finish_error {
            return Err(error);
        }
        Ok(())
    }

    fn finish_cuda_graph_lane_replay_discarding_outputs(
        &self,
        lane: &mut CachedCudaGraph,
        replay_stats: CudaGraphReplayStats,
    ) -> Result<(), BackendError> {
        let mut discard_outputs = reserved_vec(
            lane.output_host_bufs.len(),
            "discarded cuda graph lane output",
        )?;
        self.backend
            .finish_cuda_graph_replay_into(lane, replay_stats, &mut discard_outputs)
    }

    fn finish_and_return_cuda_graph_lanes_after_error(
        &self,
        mut lanes: SmallVec<[CachedCudaGraph; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>,
        launched: &[LaunchedMaterializedBatch],
        outputs: &mut [OutputBuffers],
        error: BackendError,
    ) -> Result<(), BackendError> {
        if let Err(finish_error) =
            self.finish_cuda_graph_indexed_lane_replays(&mut lanes, launched, outputs)
        {
            std::mem::forget(lanes);
            return Err(finish_error);
        }
        self.return_cached_graph_lanes_after_error(lanes, error)
    }

    fn return_cached_graph_lanes_after_error(
        &self,
        lanes: SmallVec<[CachedCudaGraph; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>,
        error: BackendError,
    ) -> Result<(), BackendError> {
        self.return_cached_graph_lanes(lanes)?;
        Err(error)
    }

    fn take_cached_graph_with_key(
        &self,
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
    ) -> Result<Option<CachedCudaGraph>, BackendError> {
        Ok(self
            .take_cached_graph_with_replay_state(inputs, input_key)?
            .map(|selection| selection.graph))
    }

    fn take_cached_graph_with_replay_state(
        &self,
        inputs: &[&[u8]],
        input_key: &MaterializedInputKey,
    ) -> Result<Option<CachedGraphReplaySelection>, BackendError> {
        let mut graphs = self.graph_cache.lock().map_err(|_| {
            BackendError::DispatchFailed {
                code: None,
                message: "CUDA compiled-pipeline graph cache lock poisoned. Fix: rebuild the compiled pipeline after a panic during graph replay.".to_string(),
            }
        })?;
        let mut first_shape_match = None;
        for (index, cached) in graphs.iter().enumerate() {
            if !cached.input_shape_matches(inputs) {
                continue;
            }
            let input_state = self
                .backend
                .prepare_cuda_graph_replay_input_state_with_key(cached, inputs, *input_key)?;
            if cached.materialized_output_cache_matches_with_input_state(inputs, &input_state)? {
                return Ok(Some(CachedGraphReplaySelection {
                    graph: graphs.swap_remove(index),
                    input_state,
                }));
            }
            if first_shape_match.is_none() {
                first_shape_match = Some((index, input_state));
            }
        }
        Ok(
            first_shape_match.map(|(index, input_state)| CachedGraphReplaySelection {
                graph: graphs.swap_remove(index),
                input_state,
            }),
        )
    }

    fn return_cached_graph(&self, cached: CachedCudaGraph) -> Result<(), BackendError> {
        let mut graphs = self.graph_cache.lock().map_err(|_| {
            BackendError::DispatchFailed {
                code: None,
                message: "CUDA compiled-pipeline graph cache lock poisoned while returning a graph. Fix: rebuild the compiled pipeline after a panic during graph replay.".to_string(),
            }
        })?;
        if graphs.len() < MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE {
            graphs.push(cached);
        }
        Ok(())
    }

    fn return_cached_graph_lanes(
        &self,
        lanes: SmallVec<[CachedCudaGraph; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]>,
    ) -> Result<(), BackendError> {
        let mut graphs = self.graph_cache.lock().map_err(|_| {
            BackendError::DispatchFailed {
                code: None,
                message: "CUDA compiled-pipeline graph cache lock poisoned while returning graph lanes. Fix: rebuild the compiled pipeline after a panic during batched graph replay.".to_string(),
            }
        })?;
        for lane in lanes {
            if graphs.len() >= MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE {
                break;
            }
            graphs.push(lane);
        }
        Ok(())
    }
}
