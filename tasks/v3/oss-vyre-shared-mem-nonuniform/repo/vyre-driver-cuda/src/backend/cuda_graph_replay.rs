//! Replay helpers for captured CUDA graphs.

use std::ptr::NonNull;
use std::sync::Arc;

use smallvec::SmallVec;
use vyre_driver::BackendError;

use super::allocations::cuda_check;
use super::cuda_graph::{CachedCudaGraph, GraphExecGuard, StreamGuard};
use super::dispatch::CudaBackend;
use super::ordering::{classify_dense_permutation, DensePermutationDefect};
use super::staging_reserve::{reserve_smallvec, reserve_vec, reserved_vec, resize_vec_slots};
use crate::input_identity::{exact_input_key, ExactInputKey};

impl CachedCudaGraph {
    pub(crate) fn input_shape_matches(&self, inputs: &[&[u8]]) -> bool {
        inputs.len() == self.expected_input_lens.len()
            && self.input_indices.len() == self.expected_input_lens.len()
            && self
                .input_indices
                .iter()
                .zip(self.expected_input_lens.iter())
                .all(|(input_index, expected)| {
                    inputs
                        .get(*input_index)
                        .is_some_and(|input| input.len() == *expected)
                })
    }

    pub(crate) fn materialized_output_cache_matches(
        &self,
        inputs: &[&[u8]],
    ) -> Result<bool, BackendError> {
        let input_state = prepare_cuda_graph_replay_input_state(self, inputs)?;
        self.materialized_output_cache_matches_with_input_state(inputs, &input_state)
    }

    pub(crate) fn materialized_output_cache_matches_with_input_state(
        &self,
        inputs: &[&[u8]],
        input_state: &CudaGraphReplayInputState,
    ) -> Result<bool, BackendError> {
        if !(self.resident_input_replay_safe && self.host_outputs_initialized) {
            return Ok(false);
        }
        cached_input_bytes_match_with_key(self, inputs, &input_state.input_key)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CudaGraphReplayStats {
    input_bytes: u64,
    output_bytes: u64,
    host_upload_operations: u64,
    device_readback_operations: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CudaGraphReplayInputState {
    input_key: ExactInputKey,
}

#[derive(Clone, Copy, Debug)]
struct PreparedCudaGraphReplayLaunch {
    stats: CudaGraphReplayStats,
    resident_input_replay: bool,
}

fn launch_cuda_graph_exec(
    graph_exec: &GraphExecGuard,
    stream: &StreamGuard,
    label: &'static str,
) -> Result<(), BackendError> {
    let graph_exec = graph_exec.ptr();
    if graph_exec == NonNull::dangling() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA graph replay received a dangling CUgraphExec sentinel before {label}. Re-record the graph before replay."
            ),
        });
    }
    let stream = stream.ptr();
    if stream == NonNull::dangling() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA graph replay received a dangling CUstream sentinel before {label}. Re-record the graph before replay."
            ),
        });
    }
    // SAFETY: FFI to libcuda.so. `GraphExecGuard` and `StreamGuard` own
    // non-null CUDA handles and the dangling sentinels are rejected above.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuGraphLaunch(graph_exec.as_ptr(), stream.as_ptr()),
            label,
        )
    }
}

fn synchronize_cuda_graph_replay_stream(cached: &CachedCudaGraph) -> Result<(), BackendError> {
    // Single speculative poll: avoids the overhead of `cuStreamSynchronize`
    // on paths where the kernel has already completed by the time the host
    // reaches this point (e.g., very short kernels, warm caches).  If not
    // immediately ready, fall through directly to the blocking synchronize
    // rather than spinning: an unconditional multi-thousand-iteration spin
    // burns CPU on every replay regardless of kernel duration, adding host
    // overhead that outweighs any latency saved for long kernels.
    if crate::stream::query_raw_stream_ready(
        cached.stream.ptr().as_ptr(),
        "cuStreamQuery (cuda_graph)",
    )? {
        return Ok(());
    }
    crate::stream::synchronize_raw_stream(
        cached.stream.ptr().as_ptr(),
        "cuStreamSynchronize (cuda_graph)",
    )
}

fn cached_input_bytes_match(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
) -> Result<bool, BackendError> {
    let input_key = exact_input_key(inputs)?;
    cached_input_bytes_match_with_key(cached, inputs, &input_key)
}

fn cached_input_bytes_match_with_key(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
    input_key: &ExactInputKey,
) -> Result<bool, BackendError> {
    if cached.cached_input_key != *input_key {
        return Ok(false);
    }
    cached_input_bytes_match_after_key_match(cached, inputs)
}

fn cached_input_bytes_match_after_key_match(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
) -> Result<bool, BackendError> {
    if cached.input_host_bufs.len() != inputs.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph has {} pinned input buffer(s) for {} caller input(s). Re-record the graph; zip-based replay would skip input uploads.",
                cached.input_host_bufs.len(),
                inputs.len()
            ),
        });
    }
    for (slot_index, (slot, input_index)) in cached
        .input_host_bufs
        .iter()
        .zip(cached.input_indices.iter())
        .enumerate()
    {
        let src = cached_graph_input(inputs, *input_index, slot_index, "cached input compare")?;
        if src.len() > slot.byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA graph cached input comparison saw {} byte(s) for a {} byte pinned allocation. Re-record the graph for this input shape.",
                    src.len(),
                    slot.byte_len
                ),
            });
        }
        if src.is_empty() {
            continue;
        }
        let cached_bytes = {
            // SAFETY: `slot` owns a pinned allocation of at least `slot.byte_len`
            // bytes, and the length check above proves `src.len() <= slot.byte_len`.
            unsafe { std::slice::from_raw_parts(slot.as_ptr().cast::<u8>(), src.len()) }
        };
        if cached_bytes != src {
            return Ok(false);
        }
    }
    Ok(true)
}

impl CudaBackend {
    pub(crate) fn try_cuda_graph_materialized_cache_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<bool, BackendError> {
        let input_state = self.prepare_cuda_graph_replay_input_state(cached, inputs)?;
        self.try_cuda_graph_materialized_cache_with_input_state_into(
            cached,
            inputs,
            &input_state,
            outputs,
        )
    }

    pub(crate) fn try_cuda_graph_materialized_cache_with_input_state_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        input_state: &CudaGraphReplayInputState,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<bool, BackendError> {
        if cached.materialized_output_cache_matches_with_input_state(inputs, input_state)? {
            collect_cuda_graph_outputs(cached, outputs)?;
            self.telemetry.record_cuda_graph_materialized_cache_hit();
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn enqueue_cuda_graph_replay(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
    ) -> Result<CudaGraphReplayStats, BackendError> {
        let input_state = self.prepare_cuda_graph_replay_input_state(cached, inputs)?;
        self.enqueue_cuda_graph_replay_with_input_state(cached, inputs, &input_state)
    }

    pub(crate) fn enqueue_cuda_graph_replay_with_input_state(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        input_state: &CudaGraphReplayInputState,
    ) -> Result<CudaGraphReplayStats, BackendError> {
        let prepared = prepare_cuda_graph_replay_launch(cached, inputs, input_state)?;
        launch_prepared_cuda_graph_replay(cached, &prepared, "cuGraphLaunch")?;
        self.telemetry.record_cuda_graph_launch();
        Ok(prepared.stats)
    }

    pub(crate) fn finish_cuda_graph_replay_into(
        &self,
        cached: &mut CachedCudaGraph,
        stats: CudaGraphReplayStats,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        synchronize_cuda_graph_replay_stream(cached)?;
        cached.device_inputs_initialized = true;
        self.telemetry.record_sync_point();
        self.record_cuda_graph_replay_stats(stats);
        collect_cuda_graph_outputs(cached, outputs)?;
        cached.host_outputs_initialized = true;
        Ok(())
    }

    pub(crate) fn record_cuda_graph_batched_replay_chunk(&self, lanes: u64) {
        self.telemetry.record_cuda_graph_batched_replay(lanes);
    }

    pub(crate) fn prepare_cuda_graph_replay_input_state(
        &self,
        cached: &CachedCudaGraph,
        inputs: &[&[u8]],
    ) -> Result<CudaGraphReplayInputState, BackendError> {
        prepare_cuda_graph_replay_input_state(cached, inputs)
    }

    pub(crate) fn prepare_cuda_graph_replay_input_state_with_key(
        &self,
        cached: &CachedCudaGraph,
        inputs: &[&[u8]],
        input_key: ExactInputKey,
    ) -> Result<CudaGraphReplayInputState, BackendError> {
        prepare_cuda_graph_replay_input_state_with_key(cached, inputs, input_key)
    }

    /// Replay a cached CUDA graph with new input bytes.
    pub fn dispatch_via_cuda_graph_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        let input_state = self.prepare_cuda_graph_replay_input_state(cached, inputs)?;
        self.dispatch_via_cuda_graph_with_input_state_into(cached, inputs, &input_state, outputs)
    }

    pub(crate) fn dispatch_via_cuda_graph_with_input_state_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        input_state: &CudaGraphReplayInputState,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        if self.try_cuda_graph_materialized_cache_with_input_state_into(
            cached,
            inputs,
            &input_state,
            outputs,
        )? {
            return Ok(());
        }
        let stats =
            self.enqueue_cuda_graph_replay_with_input_state(cached, inputs, &input_state)?;
        self.finish_cuda_graph_replay_into(cached, stats, outputs)
    }

    /// Replay a cached CUDA graph with CUDA event timing.
    ///
    /// Returns `Some(device_ns)` when a kernel was actually dispatched and CUDA
    /// event timing measured its device execution time.  Returns `None` when the
    /// materialized output cache was served directly (no kernel launched, no
    /// device timing available).  Callers must route `None` to
    /// `timed_dispatches_missing_device_time` rather than treating it as a
    /// 0-nanosecond measurement.
    pub(crate) fn dispatch_via_cuda_graph_timed_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<Option<u64>, BackendError> {
        let input_state = self.prepare_cuda_graph_replay_input_state(cached, inputs)?;
        self.dispatch_via_cuda_graph_timed_with_input_state_into(
            cached,
            inputs,
            &input_state,
            outputs,
        )
    }

    pub(crate) fn dispatch_via_cuda_graph_timed_with_input_state_into(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
        input_state: &CudaGraphReplayInputState,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<Option<u64>, BackendError> {
        if self.try_cuda_graph_materialized_cache_with_input_state_into(
            cached,
            inputs,
            &input_state,
            outputs,
        )? {
            // Materialized output cache hit: outputs were copied from host-side
            // cache without launching any kernel.  Zero kernels launched means
            // the device performed exactly zero work, so report Some(0) -- the
            // exact, non-fabricated device time for a hit.  None would mean
            // "device time unknown" and route this to
            // timed_dispatches_missing_device_time, which is wrong: we know the
            // device did nothing.  Some(0) also lets the release perf gate see
            // the cache eliminate device work (0 ns) rather than an ambiguous
            // missing measurement.
            return Ok(Some(0));
        }
        self.warmup()?;
        let prepared = prepare_cuda_graph_replay_launch(cached, inputs, &input_state)?;

        let mut timing_events =
            crate::stream::CudaTimingEventPairLease::acquire(Arc::clone(&self.launch_resources))?;
        {
            let (start, end) = timing_events.events()?;
            start.record(cached.stream.ptr().as_ptr())?;
            launch_prepared_cuda_graph_replay(cached, &prepared, "cuGraphLaunch")?;
            self.telemetry.record_cuda_graph_launch();
            end.record(cached.stream.ptr().as_ptr())?;
            end.synchronize()?;
        }
        timing_events.mark_synchronized();
        cached.device_inputs_initialized = true;
        self.telemetry.record_sync_point();
        let device_ns = {
            let (start, end) = timing_events.events()?;
            start.elapsed_time_ns(end)?
        };
        self.record_cuda_graph_replay_stats(prepared.stats);
        collect_cuda_graph_outputs(cached, outputs)?;
        cached.host_outputs_initialized = true;
        Ok(Some(device_ns))
    }

    /// Replay a cached CUDA graph with CUDA event timing and allocated outputs.
    pub fn dispatch_via_cuda_graph_timed(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
    ) -> Result<vyre_driver::TimedDispatchResult, BackendError> {
        let started = std::time::Instant::now();
        let mut outputs = reserved_vec(
            cached.output_host_bufs.len(),
            "timed cuda graph replay output vector",
        )?;
        let device_ns = self.dispatch_via_cuda_graph_timed_into(cached, inputs, &mut outputs)?;
        let wall_ns = crate::numeric::CUDA_NUMERIC
            .elapsed_nanos_u64(started, "timed cuda graph replay wall latency")?;
        self.telemetry
            .record_timed_dispatch(wall_ns, device_ns, None, None);
        Ok(vyre_driver::TimedDispatchResult {
            outputs,
            wall_ns,
            device_ns,
            enqueue_ns: None,
            wait_ns: None,
        })
    }

    /// Convenience wrapper that allocates the output `Vec` internally.
    pub fn dispatch_via_cuda_graph(
        &self,
        cached: &mut CachedCudaGraph,
        inputs: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = reserved_vec(
            cached.output_host_bufs.len(),
            "cuda graph replay output vector",
        )?;
        self.dispatch_via_cuda_graph_into(cached, inputs, &mut outputs)?;
        Ok(outputs)
    }
}

impl CudaGraphReplayStats {
    fn from_cached(cached: &CachedCudaGraph) -> Self {
        Self {
            input_bytes: cached.replay_input_bytes,
            output_bytes: cached.replay_output_bytes,
            host_upload_operations: cached.replay_host_upload_operations,
            device_readback_operations: cached.replay_device_readback_operations,
        }
    }
}

fn prepare_cuda_graph_replay(
    cached: &mut CachedCudaGraph,
    inputs: &[&[u8]],
    input_state: &CudaGraphReplayInputState,
) -> Result<(CudaGraphReplayStats, bool), BackendError> {
    let resident_input_replay = cached.resident_input_replay_safe
        && cached.device_inputs_initialized
        && cached_input_bytes_match_with_key(cached, inputs, &input_state.input_key)?;

    if !resident_input_replay {
        for (slot_index, ((slot, input_index), transfer_len)) in cached
            .input_host_bufs
            .iter_mut()
            .zip(cached.input_indices.iter())
            .zip(cached.input_transfer_lens.iter())
            .enumerate()
        {
            let src = cached_graph_input(inputs, *input_index, slot_index, "input replay staging")?;
            slot.copy_from_slice(src)?;
            if *transfer_len > src.len() {
                slot.zero_range(src.len(), transfer_len - src.len())?;
            }
        }
        cached.cached_input_key = input_state.input_key;
        cached.device_inputs_initialized = false;
        cached.host_outputs_initialized = false;
    }
    let mut stats = CudaGraphReplayStats::from_cached(cached);
    if resident_input_replay {
        stats.input_bytes = 0;
        stats.host_upload_operations = 0;
    }
    Ok((stats, resident_input_replay))
}

fn prepare_cuda_graph_replay_launch(
    cached: &mut CachedCudaGraph,
    inputs: &[&[u8]],
    input_state: &CudaGraphReplayInputState,
) -> Result<PreparedCudaGraphReplayLaunch, BackendError> {
    let (stats, resident_input_replay) = prepare_cuda_graph_replay(cached, inputs, input_state)?;
    Ok(PreparedCudaGraphReplayLaunch {
        stats,
        resident_input_replay,
    })
}

fn launch_prepared_cuda_graph_replay(
    cached: &mut CachedCudaGraph,
    prepared: &PreparedCudaGraphReplayLaunch,
    label: &'static str,
) -> Result<(), BackendError> {
    let graph_exec = if prepared.resident_input_replay {
        &cached.resident_input_graph_exec
    } else {
        &cached.graph_exec
    };
    launch_cuda_graph_exec(graph_exec, &cached.stream, label)
}

fn prepare_cuda_graph_replay_input_state(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
) -> Result<CudaGraphReplayInputState, BackendError> {
    prepare_cuda_graph_replay_input_state_with_key(cached, inputs, exact_input_key(inputs)?)
}

fn prepare_cuda_graph_replay_input_state_with_key(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
    input_key: ExactInputKey,
) -> Result<CudaGraphReplayInputState, BackendError> {
    validate_cached_graph_inputs(cached, inputs)?;
    Ok(CudaGraphReplayInputState { input_key })
}

fn cached_graph_input<'a>(
    inputs: &[&'a [u8]],
    input_index: usize,
    slot_index: usize,
    context: &'static str,
) -> Result<&'a [u8], BackendError> {
    inputs
        .get(input_index)
        .copied()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph {context} slot {slot_index} maps to logical input {input_index}, but replay received only {} input(s). Re-record the graph from a valid BindingPlan.",
                inputs.len()
            ),
        })
}

fn validate_cached_graph_slot_index_map(
    indices: &[usize],
    expected_len: usize,
    slot_kind: &'static str,
    action: &'static str,
) -> Result<(), BackendError> {
    let mut sorted_indices = SmallVec::<[usize; 8]>::new();
    reserve_smallvec(
        &mut sorted_indices,
        indices.len(),
        "cuda graph slot index validation",
    )?;
    sorted_indices.extend(indices.iter().copied());
    crate::backend::ordering::sort_unstable_if_needed(sorted_indices.as_mut_slice());
    // Delegate the dense-permutation invariant to the single backend-neutral
    // owner (shared with the resident-dispatch index validators); format the
    // graph-replay-specific remediation from the classified defect.
    match classify_dense_permutation(&sorted_indices, expected_len) {
        Ok(()) => Ok(()),
        Err(DensePermutationDefect::Duplicate { index, slot }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph has a duplicate logical {slot_kind} index {index} at sorted slot {slot}; duplicate {slot_kind} indexes alias one logical slot onto two descriptors. Re-record the graph from Program::buffers logical {slot_kind} order before {action}.",
                ),
            })
        }
        Err(DensePermutationDefect::Sparse { index, slot }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph logical {slot_kind} index {index} at sorted slot {slot} is not dense over 0..{expected_len}. Re-record the graph from Program::buffers logical {slot_kind} order before {action}.",
                ),
            })
        }
        Err(DensePermutationDefect::LengthMismatch { resolved, expected }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph resolved {resolved} logical {slot_kind} index(es); expected {expected} {slot_kind} slot(s). Re-record the graph; descriptor-ordered graph {slot_kind}s must map back to Program::buffers {slot_kind} slots.",
                ),
            })
        }
    }
}

fn validate_cached_graph_input_index_map(
    input_indices: &[usize],
    expected_len: usize,
) -> Result<(), BackendError> {
    validate_cached_graph_slot_index_map(input_indices, expected_len, "input", "replay")
}

fn validate_cached_graph_output_index_map(
    output_indices: &[usize],
    expected_len: usize,
) -> Result<(), BackendError> {
    validate_cached_graph_slot_index_map(output_indices, expected_len, "output", "collection")
}

fn validate_cached_graph_inputs(
    cached: &CachedCudaGraph,
    inputs: &[&[u8]],
) -> Result<(), BackendError> {
    if cached.input_host_bufs.len() != cached.expected_input_lens.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph has {} pinned input buffer(s) but {} expected input length(s). Re-record the graph before replay.",
                cached.input_host_bufs.len(),
                cached.expected_input_lens.len()
            ),
        });
    }
    if cached.input_transfer_lens.len() != cached.expected_input_lens.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph has {} input transfer length(s) but {} expected input length(s). Re-record the graph; zip-based replay would skip or truncate input uploads.",
                cached.input_transfer_lens.len(),
                cached.expected_input_lens.len()
            ),
        });
    }
    validate_cached_graph_input_index_map(&cached.input_indices, cached.expected_input_lens.len())?;
    if inputs.len() != cached.expected_input_lens.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph expects {} inputs but received {}.",
                cached.expected_input_lens.len(),
                inputs.len()
            ),
        });
    }
    for (idx, ((input_index, expected_len), transfer_len)) in cached
        .input_indices
        .iter()
        .zip(cached.expected_input_lens.iter())
        .zip(cached.input_transfer_lens.iter())
        .enumerate()
    {
        let input = cached_graph_input(inputs, *input_index, idx, "shape validation")?;
        let received_len = input.len();
        if received_len != *expected_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph input {idx} expects {expected_len} bytes but \
                     received {}  -  re-record the graph for this input shape.",
                    received_len
                ),
            });
        }
        if *transfer_len < *expected_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph input {idx} expects {expected_len} bytes but its captured transfer length is {transfer_len}. Re-record the graph before replay; truncated graph memcpy would leave stale device input bytes.",
                ),
            });
        }
    }
    Ok(())
}

fn collect_cuda_graph_outputs(
    cached: &CachedCudaGraph,
    outputs: &mut Vec<Vec<u8>>,
) -> Result<(), BackendError> {
    if cached.output_host_bufs.len() != cached.output_lens.len()
        || cached.output_indices.len() != cached.output_lens.len()
    {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph has {} pinned output buffer(s), {} logical output index(es), and {} output length(s). Re-record the graph before collecting outputs.",
                cached.output_host_bufs.len(),
                cached.output_indices.len(),
                cached.output_lens.len()
            ),
        });
    }
    validate_cached_graph_output_index_map(&cached.output_indices, cached.output_lens.len())?;
    resize_vec_slots(
        outputs,
        cached.output_lens.len(),
        "cuda graph replay output vector",
    )?;
    reserve_cuda_graph_output_slots(&cached.output_indices, &cached.output_lens, outputs)?;
    let output_count = outputs.len();
    for (slot_index, (buf, (output_index, byte_len))) in cached
        .output_host_bufs
        .iter()
        .zip(cached.output_indices.iter().zip(cached.output_lens.iter()))
        .enumerate()
    {
        let output = outputs
            .get_mut(*output_index)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph output slot {slot_index} maps to logical output {output_index}, but collection has only {} output slot(s). Re-record the graph from a valid BindingPlan.",
                    output_count
                ),
            })?;
        buf.copy_prefix_into(*byte_len, output)?;
    }
    Ok(())
}

fn reserve_cuda_graph_output_slots(
    output_indices: &[usize],
    output_lens: &[usize],
    outputs: &mut [Vec<u8>],
) -> Result<(), BackendError> {
    if output_indices.len() != output_lens.len() || output_lens.len() != outputs.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: cached cuda graph output preflight expected {} logical output index(es), {} output length(s), and {} caller output slot(s). Re-record the graph before collecting outputs.",
                output_indices.len(),
                output_lens.len(),
                outputs.len()
            ),
        });
    }
    let output_count = outputs.len();
    for (slot_index, (output_index, byte_len)) in
        output_indices.iter().zip(output_lens.iter()).enumerate()
    {
        let output = outputs
            .get_mut(*output_index)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: cached cuda graph output preflight slot {slot_index} maps to logical output {output_index}, but collection has only {} output slot(s). Re-record the graph from a valid BindingPlan.",
                    output_count
                ),
            })?;
        reserve_vec(output, *byte_len, "cuda graph replay output bytes")?;
    }
    Ok(())
}

impl CudaBackend {
    fn record_cuda_graph_replay_stats(&self, stats: CudaGraphReplayStats) {
        self.telemetry
            .record_host_to_device_bytes(stats.input_bytes);
        self.telemetry
            .record_device_to_host_readback(stats.output_bytes);
        self.telemetry
            .record_host_upload_operations(stats.host_upload_operations);
        self.telemetry
            .record_device_readback_operations(stats.device_readback_operations);
    }
}

#[cfg(test)]
mod source_contract_tests {
    use super::{
        cached_graph_input, validate_cached_graph_input_index_map,
        validate_cached_graph_output_index_map,
    };

    #[test]
    fn cached_graph_replay_input_index_map_accepts_reordered_descriptor_inputs() {
        validate_cached_graph_input_index_map(&[2, 0, 1], 3).expect(
            "Fix: descriptor-ordered CUDA graph inputs may map to reordered logical slots.",
        );

        let first = [0xA1, 0xA2];
        let second = [0xB1];
        let third = [0xC1, 0xC2, 0xC3];
        let inputs: &[&[u8]] = &[first.as_slice(), second.as_slice(), third.as_slice()];

        assert_eq!(
            cached_graph_input(inputs, 2, 0, "test replay")
                .expect("Fix: graph replay should resolve logical input 2 for descriptor slot 0."),
            third.as_slice()
        );
        assert_eq!(
            cached_graph_input(inputs, 0, 1, "test replay")
                .expect("Fix: graph replay should resolve logical input 0 for descriptor slot 1."),
            first.as_slice()
        );
    }

    #[test]
    fn cached_graph_replay_input_index_map_rejects_stale_or_non_dense_maps() {
        let duplicate = validate_cached_graph_input_index_map(&[0, 0, 2], 3).unwrap_err();
        assert_eq!(
            duplicate.to_string().contains("duplicate"),
            true,
            "Fix: duplicate CUDA graph logical input indexes must fail before replay can alias an input slot: {duplicate}"
        );
        let sparse = validate_cached_graph_input_index_map(&[0, 2, 3], 3).unwrap_err();
        assert_eq!(
            sparse.to_string().contains("dense"),
            true,
            "Fix: sparse CUDA graph logical input indexes must fail before replay can skip an input slot: {sparse}"
        );
        let truncated = validate_cached_graph_input_index_map(&[0, 1], 3).unwrap_err();
        assert_eq!(
            truncated.to_string().contains("expected 3"),
            true,
            "Fix: truncated CUDA graph logical input maps must fail before zip-based replay staging: {truncated}"
        );

        let only = [0xAA];
        let inputs: &[&[u8]] = &[only.as_slice()];
        let stale = cached_graph_input(inputs, 1, 0, "test replay").unwrap_err();
        assert_eq!(
            stale.to_string().contains("logical input 1"),
            true,
            "Fix: stale CUDA graph logical input indexes must become BackendError, not a panic or wrong-slot replay: {stale}"
        );
    }

    #[test]
    fn cached_graph_replay_output_index_map_accepts_reordered_descriptor_outputs() {
        validate_cached_graph_output_index_map(&[1, 0, 2], 3).expect(
            "Fix: descriptor-ordered CUDA graph outputs may map to reordered logical slots.",
        );
        let duplicate = validate_cached_graph_output_index_map(&[0, 0, 2], 3).unwrap_err();
        assert_eq!(
            duplicate.to_string().contains("duplicate"),
            true,
            "Fix: duplicate CUDA graph logical output indexes must fail before collection can alias an output slot: {duplicate}"
        );
        let sparse = validate_cached_graph_output_index_map(&[0, 2, 3], 3).unwrap_err();
        assert_eq!(
            sparse.to_string().contains("dense"),
            true,
            "Fix: sparse CUDA graph logical output indexes must fail before collection can skip an output slot: {sparse}"
        );
        let truncated = validate_cached_graph_output_index_map(&[0, 1], 3).unwrap_err();
        assert_eq!(
            truncated.to_string().contains("expected 3"),
            true,
            "Fix: truncated CUDA graph logical output maps must fail before positional collection can drop a slot: {truncated}"
        );
    }

    #[test]
    fn cuda_graph_replay_uses_fallible_output_staging_reservation() {
        let source = include_str!("cuda_graph_replay.rs");
        assert!(source.contains(
            "use super::staging_reserve::{reserve_smallvec, reserve_vec, reserved_vec, resize_vec_slots};"
        ));
        assert!(source.contains("fn collect_cuda_graph_outputs("));
        assert!(source.contains(") -> Result<(), BackendError>"));
        assert!(!source.contains(concat!(
            "Vec::with_capacity",
            "(cached.output_host_bufs.len())"
        )));
        assert!(
            source.contains("resize_vec_slots(")
                && !source.contains(concat!("outputs", ".extend("))
                && !source.contains(concat!("outputs", ".resize_with(")),
            "Fix: CUDA graph replay output staging must use the shared fallible resize helper instead of bespoke growth."
        );
        let collector = source
            .split("fn collect_cuda_graph_outputs(")
            .nth(1)
            .and_then(|tail| tail.split("fn reserve_cuda_graph_output_slots(").next())
            .expect("Fix: CUDA graph replay must expose output collection before output-slot preflight.");
        let preflight = collector
            .find(
                "reserve_cuda_graph_output_slots(&cached.output_indices, &cached.output_lens, outputs)?",
            )
            .expect(
                "Fix: CUDA graph replay output collection must preflight every caller output slot.",
            );
        let output_lookup = collector
            .find(".get_mut(*output_index)")
            .expect("Fix: CUDA graph replay output collection must route by logical output index.");
        let copy = collector
            .find("buf.copy_prefix_into(*byte_len, output)?")
            .expect("Fix: CUDA graph replay output collection must copy pinned graph outputs.");
        assert!(
            preflight < output_lookup && output_lookup < copy,
            "Fix: CUDA graph replay must reserve every caller output before copying any pinned graph output bytes."
        );
        let preflight_helper = source
            .split("fn reserve_cuda_graph_output_slots(")
            .nth(1)
            .and_then(|tail| tail.split("impl CudaBackend").next())
            .expect("Fix: CUDA graph replay must expose output-slot preflight before backend telemetry.");
        assert!(
            preflight_helper.contains("output_indices.len() != output_lens.len()")
                && preflight_helper.contains(".get_mut(*output_index)")
                && preflight_helper.contains("reserve_vec(output, *byte_len, \"cuda graph replay output bytes\")?"),
            "Fix: CUDA graph replay output preflight must validate cardinality, route by logical output index, and reserve every destination byte capacity."
        );
        assert!(
            source.contains("cached.input_host_bufs.len() != cached.expected_input_lens.len()")
                && source.contains("cached.input_transfer_lens.len() != cached.expected_input_lens.len()")
                && source.contains("validate_cached_graph_input_index_map(&cached.input_indices")
                && source.contains("cached_graph_input(inputs, *input_index")
                && source.contains("descriptor-ordered graph inputs must map back to Program::buffers input slots")
                && source.contains("zip-based replay would skip or truncate input uploads")
                && source.contains("*transfer_len < *expected_len")
                && source.contains("truncated graph memcpy would leave stale device input bytes")
                && source.contains("zip-based replay would skip input uploads")
                && source.contains(".zip(cached.input_indices.iter())")
                && source.contains(".zip(cached.input_transfer_lens.iter())")
                && !source.contains(concat!("inputs", "[idx]"))
                && source.contains("cached.output_host_bufs.len() != cached.output_lens.len()")
                && source.contains("cached.output_indices.len() != cached.output_lens.len()")
                && source.contains("validate_cached_graph_output_index_map(&cached.output_indices")
                && source.contains(".zip(cached.output_indices.iter().zip(cached.output_lens.iter()))"),
            "Fix: CUDA graph replay must validate cached graph input/output metadata before zip-based staging."
        );
        assert_eq!(
            source
                .matches(concat!("cudarc::driver::sys::", "cuGraphLaunch("))
                .count(),
            1,
            "Fix: CUDA graph replay must keep raw cuGraphLaunch behind one checked helper."
        );
        assert!(
            source.contains("fn launch_cuda_graph_exec(")
                && source.contains("dangling CUgraphExec sentinel")
                && source.contains("dangling CUstream sentinel"),
            "Fix: CUDA graph replay launch helper must validate graph and stream handles before FFI."
        );
    }

    #[test]
    fn timed_and_untimed_graph_replay_share_resident_input_skip_copy_path() {
        let source = include_str!("cuda_graph_replay.rs");
        assert!(
            source.contains("fn prepare_cuda_graph_replay(")
                && source.matches("prepare_cuda_graph_replay(cached, inputs,").count() >= 2,
            "Fix: timed and untimed CUDA graph replay must share one resident-input preparation path."
        );
        assert!(
            source.contains("fn prepare_cuda_graph_replay_launch(")
                && source.contains("fn launch_prepared_cuda_graph_replay(")
                && source
                    .matches(
                        "launch_prepared_cuda_graph_replay(cached, &prepared, \"cuGraphLaunch\")"
                    )
                    .count()
                    == 2,
            "Fix: timed and untimed CUDA graph replay must share prepared launch graph selection."
        );
        let launch_helper = source
            .split("fn launch_prepared_cuda_graph_replay(")
            .nth(1)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - prepared CUDA graph launch helper must exist")
            .split("fn prepare_cuda_graph_replay_input_state(")
            .next()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - prepared launch helper must precede input-state preparation");
        assert!(
            !launch_helper.contains("cached.device_inputs_initialized = true;"),
            "Fix: CUDA graph replay must not mark device inputs initialized immediately after cuGraphLaunch; the stream/timing fence must complete first."
        );
        assert!(
            source.contains("synchronize_cuda_graph_replay_stream(cached)?;\n        cached.device_inputs_initialized = true;")
                && source.contains("end.synchronize()?;")
                && source.contains("timing_events.mark_synchronized();\n        cached.device_inputs_initialized = true;"),
            "Fix: CUDA graph replay must mark resident device inputs initialized only after successful untimed and timed completion fences."
        );
        let timed_section = source
            .split("pub(crate) fn dispatch_via_cuda_graph_timed_into")
            .nth(1)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - timed CUDA graph replay entrypoint must exist")
            .split("/// Convenience wrapper")
            .next()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - timed replay section must precede convenience wrapper");
        assert!(
            timed_section.contains("prepare_cuda_graph_replay_launch(cached, inputs, &input_state)?")
                && timed_section.contains("launch_prepared_cuda_graph_replay(cached, &prepared, \"cuGraphLaunch\")")
                && !timed_section.contains("for (slot, src) in cached.input_host_bufs"),
            "Fix: timed CUDA graph replay must use resident-input graph replay when safe instead of always copying host inputs."
        );
        let timed_sync_pos = timed_section
            .find("end.synchronize()?;")
            .expect("Fix: timed CUDA graph replay must fence the end timing event before trusting resident inputs.");
        let timing_release_pos = timed_section
            .find("timing_events.mark_synchronized();")
            .expect("Fix: timed CUDA graph replay must mark timing events reusable only after end-event synchronization.");
        let initialized_pos = timed_section
            .find("cached.device_inputs_initialized = true;")
            .expect("Fix: timed CUDA graph replay must promote resident inputs after completion is proven.");
        assert!(
            timed_sync_pos < timing_release_pos && timing_release_pos < initialized_pos,
            "Fix: timed CUDA graph replay must prove timing-event completion before timing lease reuse or resident-input promotion."
        );
    }

    #[test]
    fn materialized_graph_cache_is_shared_by_single_and_batched_replay_paths() {
        let replay_source = include_str!("cuda_graph_replay.rs");
        let compiled_dispatch = include_str!("../pipeline/compiled_dispatch.rs");
        assert!(
            replay_source
                .contains("pub(crate) fn try_cuda_graph_materialized_cache_with_input_state_into(")
                && replay_source.contains(
                    "pub(crate) fn dispatch_via_cuda_graph_with_input_state_into("
                )
                && replay_source.contains(
                    "pub(crate) fn dispatch_via_cuda_graph_timed_with_input_state_into("
                ),
            "Fix: single CUDA graph replay must route materialized output cache hits through the shared helper."
        );
        assert!(
            compiled_dispatch.contains("materialized_output_batch_cache_partition_into")
                && compiled_dispatch.contains("let miss_entries =")
                && compiled_dispatch.contains("for (chunk_index, chunk) in miss_entries.chunks(lane_count).enumerate()")
                && compiled_dispatch.contains("chunk_index")
                && compiled_dispatch.contains(".checked_mul(lane_count)")
                && compiled_dispatch.contains("prepare_cuda_graph_replay_input_state")
                && compiled_dispatch.contains("try_cuda_graph_materialized_cache_with_input_state_into")
                && compiled_dispatch.contains("enqueue_cuda_graph_replay_with_input_state")
                && compiled_dispatch.contains("continue;")
                && compiled_dispatch.contains("[LaunchedMaterializedBatch; MAX_GRAPH_CACHE_ENTRIES_PER_PIPELINE]")
                && compiled_dispatch.contains("input_key: miss.input_key"),
            "Fix: batched CUDA graph replay must partition materialized exact-input cache hits before lane planning, reuse precomputed input keys, and only finish lanes that actually launched."
        );
    }

    #[test]
    fn cached_graph_input_key_gates_byte_compare_and_rewrites_invalidate_host_outputs() {
        let replay_source = include_str!("cuda_graph_replay.rs");
        let graph_source = include_str!("cuda_graph.rs");
        assert!(
            replay_source.contains("use crate::input_identity::{exact_input_key, ExactInputKey};")
                && replay_source.contains("fn cached_input_bytes_match_with_key(")
                && replay_source.contains("if cached.cached_input_key != *input_key")
                && replay_source.contains("cached_input_bytes_match_after_key_match"),
            "Fix: raw CUDA graph exact-input checks must use the shared tuple key as a fast reject before expensive pinned-host byte comparison."
        );
        assert!(
            replay_source.contains("let input_key = exact_input_key(inputs)?;")
                && replay_source.contains("cached.cached_input_key = input_state.input_key;")
                && replay_source.contains("cached.device_inputs_initialized = false;")
                && replay_source.contains("cached.host_outputs_initialized = false;"),
            "Fix: rewriting cached graph host inputs must update the exact-input key and immediately invalidate resident device inputs plus materialized host outputs before graph launch/finish can fail."
        );
        let prepare_replay = replay_source
            .split("fn prepare_cuda_graph_replay(")
            .nth(1)
            .expect("Fix: CUDA graph replay preparation must stay centralized.")
            .split("fn prepare_cuda_graph_replay_launch(")
            .next()
            .expect("Fix: replay preparation must precede prepared-launch construction.");
        let key_update = prepare_replay
            .find("cached.cached_input_key = input_state.input_key;")
            .expect("Fix: replay preparation must update the cached input key after rewriting host inputs.");
        let device_invalidate = prepare_replay
            .find("cached.device_inputs_initialized = false;")
            .expect("Fix: replay preparation must invalidate resident device inputs before launch can fail.");
        let host_invalidate = prepare_replay
            .find("cached.host_outputs_initialized = false;")
            .expect("Fix: replay preparation must invalidate materialized host outputs before launch can fail.");
        assert!(
            key_update < device_invalidate && device_invalidate < host_invalidate,
            "Fix: rewritten CUDA graph inputs must invalidate resident device state before the graph can be re-used after an enqueue failure."
        );
        assert!(
            graph_source.contains("pub(crate) cached_input_key: ExactInputKey")
                && graph_source.contains("let cached_input_key = exact_input_key(sample_inputs)?;"),
            "Fix: recorded CUDA graphs must initialize cached_input_key from the captured sample inputs."
        );
    }

    #[test]
    fn raw_graph_replay_prepares_input_state_once_per_dispatch_path() {
        let replay_source = include_str!("cuda_graph_replay.rs");
        assert!(
            replay_source.contains("pub(crate) struct CudaGraphReplayInputState")
                && replay_source.contains("fn prepare_cuda_graph_replay_input_state(")
                && replay_source.contains("validate_cached_graph_inputs(cached, inputs)?;")
                && replay_source.contains("input_key: exact_input_key(inputs)?"),
            "Fix: CUDA graph replay must centralize shape validation and exact-input key creation in a reusable input-state object."
        );
        let untimed_section = replay_source
            .split("pub fn dispatch_via_cuda_graph_into")
            .nth(1)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - untimed CUDA graph replay entrypoint must exist")
            .split("/// Replay a cached CUDA graph with CUDA event timing.")
            .next()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - untimed replay section must precede timed replay");
        assert_eq!(
            untimed_section
                .matches("prepare_cuda_graph_replay_input_state(cached, inputs)?")
                .count(),
            1,
            "Fix: untimed raw CUDA graph replay must validate/hash inputs once and reuse that state for materialized-cache check plus launch preparation."
        );
        assert!(
            untimed_section.contains("try_cuda_graph_materialized_cache_with_input_state_into")
                && untimed_section.contains("enqueue_cuda_graph_replay_with_input_state"),
            "Fix: untimed raw CUDA graph replay must pass the prepared input state through both cache and launch paths."
        );
        assert!(
            replay_source.contains("dispatch_via_cuda_graph_with_input_state_into(cached, inputs, &input_state, outputs)")
                && replay_source.contains("pub(crate) fn dispatch_via_cuda_graph_with_input_state_into("),
            "Fix: untimed raw CUDA graph replay must expose a with-input-state entrypoint so compiled pipelines can reuse precomputed exact-input keys."
        );
        let timed_section = replay_source
            .split("pub(crate) fn dispatch_via_cuda_graph_timed_into")
            .nth(1)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - timed CUDA graph replay entrypoint must exist")
            .split("/// Replay a cached CUDA graph with CUDA event timing and allocated outputs.")
            .next()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - timed replay section must precede timed wrapper");
        assert_eq!(
            timed_section
                .matches("prepare_cuda_graph_replay_input_state(cached, inputs)?")
                .count(),
            1,
            "Fix: timed raw CUDA graph replay must validate/hash inputs once and reuse that state before event timing."
        );
        assert!(
            timed_section.contains("try_cuda_graph_materialized_cache_with_input_state_into")
                && timed_section.contains("prepare_cuda_graph_replay_launch(cached, inputs, &input_state)?")
                && timed_section.contains("launch_prepared_cuda_graph_replay(cached, &prepared, \"cuGraphLaunch\")"),
            "Fix: timed raw CUDA graph replay must reuse the prepared input state for materialized and resident-input replay decisions."
        );
        assert!(
            replay_source.contains("dispatch_via_cuda_graph_timed_with_input_state_into(\n            cached,\n            inputs,\n            &input_state,\n            outputs,\n        )")
                && replay_source.contains("pub(crate) fn dispatch_via_cuda_graph_timed_with_input_state_into("),
            "Fix: timed raw CUDA graph replay must expose a with-input-state entrypoint so compiled pipelines can reuse precomputed exact-input keys."
        );
    }

    #[test]
    fn compiled_batch_graph_misses_reuse_materialized_cache_input_keys() {
        let replay_source = include_str!("cuda_graph_replay.rs");
        let compiled_dispatch = include_str!("../pipeline/compiled_dispatch.rs");
        assert!(
            replay_source.contains("prepare_cuda_graph_replay_input_state_with_key")
                && replay_source.contains("input_key: ExactInputKey")
                && replay_source.contains("validate_cached_graph_inputs(cached, inputs)?;")
                && replay_source.contains("Ok(CudaGraphReplayInputState { input_key })"),
            "Fix: raw CUDA graph replay must accept a precomputed exact-input key while still validating graph shape."
        );
        assert!(
            compiled_dispatch.contains("struct MaterializedBatchMiss")
                && compiled_dispatch.contains("input_key: MaterializedInputKey")
                && compiled_dispatch.contains("materialized_input_key(inputs)?")
                && compiled_dispatch.contains("cache.snapshot_with_key(inputs, &input_key)")
                && compiled_dispatch.contains("prepare_cuda_graph_replay_input_state_with_key")
                && compiled_dispatch.contains("take_cached_graph_with_key(")
                && compiled_dispatch.contains("&first_miss.input_key")
                && compiled_dispatch.contains("miss.input_key"),
            "Fix: compiled batched CUDA graph replay must reuse materialized-cache exact-input keys for graph-cache selection and graph miss replay instead of hashing each miss again."
        );
        let partition_section = compiled_dispatch
            .split("fn materialized_output_batch_cache_partition_into")
            .nth(1)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - compiled materialized batch partition function must exist")
            .split("fn materialized_output_cache_hit_into")
            .next()
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - batch partition section must precede single cache helper");
        let key_position = partition_section
            .find("materialized_input_key(inputs)?")
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - batch partition must compute exact-input keys");
        let lock_position = partition_section
            .find("let cache = self.lock_materialized_output_cache")
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - batch partition must acquire materialized cache lock");
        let resize_position = partition_section
            .find("resize_vec_slots(\n            outputs,\n            batches.len(),\n            \"cuda graph materialized batch output\",")
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - batch partition must resize output slots before hit copy");
        let hit_copy_position = partition_section
            .find("snapshot.copy_into(compiled_graph_output_mut(")
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - batch partition must copy materialized cache hits");
        assert!(
            partition_section.contains("for (batch_index, inputs) in batches.iter().enumerate()")
                && partition_section.contains("input_keys.push((batch_index, materialized_input_key(inputs)?));")
                && partition_section.contains("let cache = self.lock_materialized_output_cache")
                && key_position < lock_position,
            "Fix: compiled materialized batch replay must compute exact-input keys before acquiring the materialized-output cache lock."
        );
        assert!(
            key_position < resize_position
                && lock_position < resize_position
                && resize_position < hit_copy_position,
            "Fix: compiled materialized batch replay must finish exact-input key and cache-snapshot partitioning before resizing caller-owned output slots, then resize before copying cache hits."
        );
    }

    /// VDC-001: timed CUDA graph replay must return `None` on a materialized
    /// cache hit, not `Ok(0)`.  A `0`-nanosecond device time is physically
    /// impossible and silently corrupts `timed_device_measurements` by counting
    /// cache hits as real GPU measurements.
    #[test]
    fn timed_graph_replay_returns_none_device_ns_on_materialized_cache_hit() {
        let source = include_str!("cuda_graph_replay.rs");
        // The timed inner helper must have `-> Result<Option<u64>, BackendError>`.
        assert!(
            source.contains("-> Result<Option<u64>, BackendError>"),
            "Fix: dispatch_via_cuda_graph_timed_with_input_state_into must return \
             Result<Option<u64>, BackendError> so a materialized cache hit can be \
             distinguished from a real 0-ns device measurement."
        );
        // On a cache hit the function must return Ok(None), not Ok(0).
        assert!(
            source.contains("return Ok(None);"),
            "Fix: timed CUDA graph replay must return Ok(None) on materialized cache hit; \
             Ok(0) injects a fabricated 0-ns measurement into timed_device_measurements."
        );
        // Ok(0) must not appear anywhere in the timed inner path.
        let timed_inner = source
            .split("pub(crate) fn dispatch_via_cuda_graph_timed_with_input_state_into(")
            .nth(1)
            .expect("Fix: timed inner replay function must exist in cuda_graph_replay.rs");
        // The function ends at Ok(Some(device_ns)) — extract that slice.
        let timed_inner_body = timed_inner
            .split("Ok(Some(device_ns))")
            .next()
            .expect("Fix: timed CUDA graph replay must return Ok(Some(device_ns)) on an actual dispatch.");
        assert!(
            !timed_inner_body.contains("Ok(0)"),
            "Fix: timed CUDA graph replay must not return Ok(0); use Ok(None) for cache hits \
             so callers route to timed_dispatches_missing_device_time."
        );
        // The public timed wrapper must pass device_ns (Option<u64>) directly to
        // record_timed_dispatch, not wrap it in an extra Some().
        let wrapper = source
            .split("pub fn dispatch_via_cuda_graph_timed(")
            .nth(1)
            .expect("Fix: public timed wrapper must exist in cuda_graph_replay.rs")
            .split("/// Convenience wrapper")
            .next()
            .expect("Fix: public timed wrapper must precede the convenience wrapper");
        assert!(
            wrapper.contains("record_timed_dispatch(wall_ns, device_ns, None, None)"),
            "Fix: dispatch_via_cuda_graph_timed must pass device_ns: Option<u64> directly \
             to record_timed_dispatch; wrapping with Some() re-introduces the fabricated \
             0-ns measurement on cache hits."
        );
        assert!(
            wrapper.contains("device_ns,") && !wrapper.contains("device_ns: Some(device_ns)"),
            "Fix: dispatch_via_cuda_graph_timed must not wrap device_ns in Some(); \
             the Option<u64> from the inner function must propagate transparently."
        );
    }

    /// VDC-002: the untimed CUDA graph replay sync helper must not contain an
    /// unconditional multi-iteration spin loop.  A 4096-iteration spin burns CPU
    /// on every replay regardless of kernel duration; for kernels that outlast
    /// the spin budget all iterations are wasted before the blocking path that
    /// should have been taken immediately.
    #[test]
    fn graph_replay_stream_sync_does_not_unconditionally_spin_before_blocking_wait() {
        let source = include_str!("cuda_graph_replay.rs");
        let sync_fn = source
            .split("fn synchronize_cuda_graph_replay_stream(")
            .nth(1)
            .expect("Fix: synchronize_cuda_graph_replay_stream must exist in cuda_graph_replay.rs")
            .split("fn cached_input_bytes_match(")
            .next()
            .expect("Fix: stream sync helper must precede cached_input_bytes_match");
        // The fixed implementation retains exactly one speculative poll (no loop).
        assert!(
            !sync_fn.contains("for _ in 0.."),
            "Fix: CUDA graph replay stream sync must not use a fixed-count spin loop; \
             call cuStreamSynchronize directly after one speculative poll."
        );
        assert!(
            !sync_fn.contains("spin_loop()"),
            "Fix: CUDA graph replay stream sync must not call std::hint::spin_loop(); \
             unconditional spinning wastes CPU for every replay dispatch."
        );
        // The single speculative poll plus blocking synchronize must both be present.
        assert!(
            sync_fn.contains("query_raw_stream_ready(") && sync_fn.contains("synchronize_raw_stream("),
            "Fix: CUDA graph replay stream sync must retain one speculative poll \
             (query_raw_stream_ready) followed by a blocking synchronize_raw_stream \
             when not immediately ready."
        );
        // No spin-limit constant should exist anymore. Scope the search to the
        // production code (everything before the test module) so this assertion's
        // own identifier text in `source` cannot self-match.
        let production = source
            .split("mod source_contract_tests")
            .next()
            .expect("Fix: production code must precede the source_contract_tests module");
        assert!(
            !production.contains("CUDA_GRAPH_REPLAY_SPIN_QUERY_LIMIT"),
            "Fix: the spin-limit constant must be removed from production; \
             the spin loop it governed is no longer present."
        );
    }

    /// VDC-003: `configure_jit_cache` must be `pub(crate)` to prevent external
    /// callers from mutating the CUDA JIT cache env-vars after backend bring-up
    /// has frozen them via the `CONFIGURED` OnceLock.
    #[test]
    fn configure_jit_cache_is_not_pub_to_external_callers() {
        let source = include_str!("../jit_cache.rs");
        assert!(
            source.contains("pub(crate) fn configure_jit_cache("),
            "Fix: configure_jit_cache must be pub(crate), not pub; external callers must \
             use configure_jit_cache_default() which is guarded by the CONFIGURED OnceLock."
        );
        assert!(
            !source.contains("\npub fn configure_jit_cache("),
            "Fix: configure_jit_cache must not be pub; it bypasses the CONFIGURED OnceLock \
             and can mutate process-wide CUDA env-vars after backend bring-up has frozen them."
        );
    }
}
