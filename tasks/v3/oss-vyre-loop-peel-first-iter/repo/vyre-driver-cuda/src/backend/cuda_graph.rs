#![allow(unsafe_code)]
//! cudaGraph capture-and-replay path for repeat-shape Programs.
//!
//! Op id: `vyre-driver-cuda::cuda_graph`. Soundness: `Exact` over the
//! captured launch sequence. Cost-direction: read-only at the wire layer
//! (does not mutate Program); host-side dispatch overhead is amortized by
//! replacing repeated launch construction with a cached `CUgraphExec`.
//!
//! ## Why
//!
//! Latency-bound kernels can spend more time in host launch setup than in
//! device execution. cudaGraph captures the full launch sequence (memcpy +
//! kernel launch + readback) into a graph object once; subsequent dispatches
//! replay the cached executable graph with `cuGraphLaunch`.
//!
//! ## Constraints
//!
//! - **No allocation during capture.** `cuMemAlloc_v2` returns
//!   `CUDA_ERROR_STREAM_CAPTURE_UNSUPPORTED` while a stream is in capture
//!   mode. `record_cuda_graph` allocates ALL device buffers BEFORE
//!   `cuStreamBeginCapture_v2` and stores them in `CachedCudaGraph`.
//! - **Host pointers must persist.** The captured `cuMemcpyHtoDAsync_v2`
//!   records the host source pointer; the cached graph reuses the SAME
//!   pointer on every replay. `CachedCudaGraph` owns the input host buffers
//!   so callers can write new bytes into them without changing the address.
//! - **Shape-bound.** A cached graph captures one specific input/output
//!   byte layout. Calling `dispatch_via_cuda_graph` with mismatched input
//!   sizes returns `BackendError::InvalidProgram`  -  the caller must record
//!   a fresh graph for each shape.
//!
//! ## Lifecycle
//!
//! ```text
//! CachedCudaGraph::record  ──► CUgraph ──► CUgraphExec ──► live
//!                               │
//!                               ▼
//!                        owns input/output device pointers
//!                        owns input/output host buffers
//!                        owns dedicated CUstream
//!                        owns CUfunction (via module_cache)
//!                               │
//! CachedCudaGraph::drop ──► cuGraphExecDestroy ──► cuGraphDestroy
//!                       ──► cuStreamDestroy_v2
//!                       ──► cuMemFree_v2 for each device buffer

use std::ptr::NonNull;
use std::sync::Arc;

use cudarc::driver::sys::{CUgraphExec_st, CUgraph_st, CUstream_st};
use smallvec::SmallVec;
use vyre_driver::binding::BindingRole;
use vyre_driver::graph_capture::plan_graph_capture_bindings;
use vyre_driver::transfer_accounting::TransferAccountingPolicy;
use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

use super::allocations::{
    alloc_cuda_ptr, cuda_check, free_cuda_ptr_with_label, HostTransferAllocations,
    PinnedHostAllocation, PinnedHostAllocationPool,
};
use super::dispatch::CudaBackend;
use super::output_range::cuda_output_readback_for_binding;
use super::staging_reserve::reserve_smallvec;
use crate::backend::copy::aligned_async_copy_len;
use crate::input_identity::{exact_input_key, ExactInputKey};
use crate::numeric::CUDA_NUMERIC;

const CUDA_GRAPH_REPLAY_ACCOUNTING: TransferAccountingPolicy =
    TransferAccountingPolicy::new("CUDA graph", "record a smaller graph shape");

fn log_cuda_drop_result(op: &str, result: cudarc::driver::sys::CUresult) {
    if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
        tracing::error!(
            "Fix: {op} failed while releasing CUDA graph resources with {result:?}; ensure graph work has completed before resource drop."
        );
    }
}

fn cuda_graph_usize_to_u64(value: usize, label: &'static str) -> Result<u64, BackendError> {
    CUDA_NUMERIC.usize_to_u64(value, label)
}

fn cuda_graph_sample_input<'a>(
    sample_inputs: &'a [&[u8]],
    input_index: usize,
    binding_name: &str,
    context: &'static str,
) -> Result<&'a [u8], BackendError> {
    sample_inputs
        .get(input_index)
        .copied()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA graph capture {context} expected sample input index {input_index} for `{binding_name}` but only {} sample input(s) were supplied. Rebuild the binding plan or validate graph sample inputs before recording.",
                sample_inputs.len()
            ),
        })
}

/// CUDA driver constant: stream-capture-mode thread-local. Only the calling
/// thread's cuda calls are forbidden during capture (alloc-class operations
/// fail with `CUDA_ERROR_STREAM_CAPTURE_UNSUPPORTED`); other threads remain
/// free to allocate / launch. The alternative `GLOBAL` (value 0) blocks
/// alloc on every thread, which makes parallel test execution impossible
/// and would also stall any concurrent caller of `CudaBackend`.
/// Mirrors `CU_STREAM_CAPTURE_MODE_THREAD_LOCAL` from `cuda.h`.
const CU_STREAM_CAPTURE_MODE_THREAD_LOCAL: u32 = 1;

#[derive(Debug)]
pub(crate) struct DevicePtrGuard {
    ptr: u64,
}

impl DevicePtrGuard {
    fn new(ptr: u64) -> Self {
        Self { ptr }
    }

    fn ptr(&self) -> u64 {
        self.ptr
    }
}

impl Drop for DevicePtrGuard {
    fn drop(&mut self) {
        free_cuda_ptr_with_label(self.ptr, "CUDA graph device buffer");
    }
}

#[derive(Debug)]
pub(crate) struct StreamGuard {
    stream: NonNull<CUstream_st>,
}

impl StreamGuard {
    fn new(stream: NonNull<CUstream_st>) -> Self {
        Self { stream }
    }

    pub(crate) fn ptr(&self) -> NonNull<CUstream_st> {
        self.stream
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        if self.stream != NonNull::dangling() {
            crate::stream::destroy_raw_stream(
                self.stream.as_ptr(),
                "cuStreamDestroy_v2 (cuda_graph dedicated stream)",
            );
        }
    }
}

fn create_cuda_graph_stream() -> Result<StreamGuard, BackendError> {
    let _nonblocking_flag_contract = cudarc::driver::sys::CUstream_flags::CU_STREAM_NON_BLOCKING;
    crate::stream::create_non_blocking_raw_stream("cuStreamCreate (cuda_graph dedicated stream)")
        .map(StreamGuard::new)
}

fn synchronize_cuda_graph_stream(
    stream: &StreamGuard,
    label: &'static str,
) -> Result<(), BackendError> {
    // The shared stream boundary validates non-null ownership before issuing
    // cuStreamSynchronize(stream.ptr().as_ptr()), keeping graph capture off
    // CUDA's legacy default stream while preserving one raw FFI implementation.
    crate::stream::synchronize_raw_stream(stream.ptr().as_ptr(), label)
}

#[derive(Clone, Copy)]
struct OutputClear {
    dst: u64,
    byte_len: usize,
}

fn record_cuda_graph_output_clears(
    clears: &[OutputClear],
    stream: &StreamGuard,
    label: &'static str,
) -> Result<(), BackendError> {
    for clear in clears {
        if clear.byte_len == 0 {
            continue;
        }
        // SAFETY: The output device allocation was created before capture
        // and retained by CachedCudaGraph for the captured graph lifetime.
        // Capturing this memset preserves the host-dispatch contract that
        // output-only buffers start as zero before sparse stores run.
        unsafe {
            super::copy::memset_d8_async_checked(
                clear.dst,
                0,
                clear.byte_len,
                stream.ptr().as_ptr(),
            )
            .map_err(|error| BackendError::DispatchFailed {
                code: None,
                message: format!("{label} failed: {error}"),
            })?;
        }
    }
    Ok(())
}

fn record_cuda_graph_output_readbacks(
    host_buffers: &mut [PinnedHostAllocation],
    output_lens: &[usize],
    readback_device_ptrs: &[u64],
    stream: &StreamGuard,
    label: &'static str,
) -> Result<(), BackendError> {
    if host_buffers.len() != output_lens.len() || output_lens.len() != readback_device_ptrs.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA graph output readback capture has {} host buffer(s), {} output length(s), and {} device pointer(s). Rebuild graph capture staging from one BindingPlan.",
                host_buffers.len(),
                output_lens.len(),
                readback_device_ptrs.len()
            ),
        });
    }
    for ((host_buf, output_len), device_ptr) in host_buffers
        .iter_mut()
        .zip(output_lens.iter())
        .zip(readback_device_ptrs.iter())
    {
        if *output_len == 0 {
            continue;
        }
        // SAFETY: The host buffer is pinned and retained by CachedCudaGraph
        // for the captured graph lifetime; device_ptr was validated from the
        // output allocation plus checked readback offset before capture.
        unsafe {
            super::copy::d2h_async_checked_with_label(
                host_buf.as_mut_ptr() as *mut std::ffi::c_void,
                *device_ptr,
                *output_len,
                stream.ptr().as_ptr(),
                label,
            )?;
        }
    }
    Ok(())
}

struct CaptureGuard {
    stream: NonNull<CUstream_st>,
    active: bool,
}

impl CaptureGuard {
    fn armed(stream: NonNull<CUstream_st>) -> Self {
        Self {
            stream,
            active: true,
        }
    }

    fn finish(
        &mut self,
        label: &'static str,
        null_message: &'static str,
    ) -> Result<GraphGuard, BackendError> {
        let graph = end_cuda_graph_capture(self.stream, label, null_message);
        self.disarm();
        graph
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        if self.active {
            match end_cuda_graph_capture(
                self.stream,
                "cuStreamEndCapture (capture guard drop)",
                "cuStreamEndCapture returned a null graph while dropping an active capture guard. Fix: ensure graph capture is finished explicitly before resource cleanup.",
            ) {
                Ok(graph) => drop(graph),
                Err(error) => tracing::error!(
                    "Fix: failed to end CUDA graph capture during guard drop: {error}"
                ),
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct GraphGuard {
    graph: NonNull<CUgraph_st>,
}

impl GraphGuard {
    fn new(graph: NonNull<CUgraph_st>) -> Self {
        Self { graph }
    }

    fn ptr(&self) -> NonNull<CUgraph_st> {
        self.graph
    }
}

impl Drop for GraphGuard {
    fn drop(&mut self) {
        if self.graph != NonNull::dangling() {
            destroy_cuda_graph_or_log(self.graph.as_ptr(), "CUDA graph guard drop");
        }
    }
}

#[derive(Debug)]
pub(crate) struct GraphExecGuard {
    graph_exec: NonNull<CUgraphExec_st>,
}

impl GraphExecGuard {
    fn new(graph_exec: NonNull<CUgraphExec_st>) -> Self {
        Self { graph_exec }
    }

    pub(crate) fn ptr(&self) -> NonNull<CUgraphExec_st> {
        self.graph_exec
    }
}

impl Drop for GraphExecGuard {
    fn drop(&mut self) {
        if self.graph_exec != NonNull::dangling() {
            destroy_cuda_graph_exec_or_log(self.graph_exec.as_ptr(), "CUDA graph exec guard drop");
        }
    }
}

fn begin_cuda_graph_capture(
    stream: &StreamGuard,
    label: &'static str,
) -> Result<CaptureGuard, BackendError> {
    // SAFETY: stream is a backend-owned non-blocking stream; CUDA validates
    // the opaque handle and returns a CUresult.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuStreamBeginCapture_v2(
                stream.ptr().as_ptr(),
                cudarc::driver::sys::CUstreamCaptureMode_enum::CU_STREAM_CAPTURE_MODE_THREAD_LOCAL,
            ),
            label,
        )?;
    }
    Ok(CaptureGuard::armed(stream.ptr()))
}

fn end_cuda_graph_capture(
    stream: NonNull<CUstream_st>,
    label: &'static str,
    null_message: &'static str,
) -> Result<GraphGuard, BackendError> {
    let mut graph_ptr: cudarc::driver::sys::CUgraph = std::ptr::null_mut();
    let status = {
        // SAFETY: stream is in capture mode for normal callers; guard-drop callers
        // are best-effort cleanup paths and CUDA returns a status if capture ended.
        unsafe { cudarc::driver::sys::cuStreamEndCapture(stream.as_ptr(), &mut graph_ptr) }
    };
    if status != cudarc::driver::sys::CUresult::CUDA_SUCCESS && !graph_ptr.is_null() {
        destroy_cuda_graph_or_log(graph_ptr, label);
    }
    cuda_check(status, label)?;
    let graph = NonNull::new(graph_ptr).ok_or_else(|| BackendError::DispatchFailed {
        code: None,
        message: null_message.to_string(),
    })?;
    Ok(GraphGuard::new(graph))
}

fn instantiate_cuda_graph(
    graph: &GraphGuard,
    label: &'static str,
    null_message: &'static str,
) -> Result<GraphExecGuard, BackendError> {
    let mut graph_exec_ptr: cudarc::driver::sys::CUgraphExec = std::ptr::null_mut();
    // SAFETY: graph is a valid captured graph handle; flags = 0 selects CUDA's
    // default executable graph instantiation policy.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuGraphInstantiateWithFlags(
                &mut graph_exec_ptr,
                graph.ptr().as_ptr(),
                0,
            ),
            label,
        )?;
    }
    let graph_exec = NonNull::new(graph_exec_ptr).ok_or_else(|| BackendError::DispatchFailed {
        code: None,
        message: null_message.to_string(),
    })?;
    Ok(GraphExecGuard::new(graph_exec))
}

fn upload_cuda_graph_exec(
    graph_exec: &GraphExecGuard,
    stream: &StreamGuard,
    label: &'static str,
) -> Result<(), BackendError> {
    // SAFETY: both handles are owned by CachedCudaGraph and remain live for
    // the upload call.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuGraphUpload(graph_exec.ptr().as_ptr(), stream.ptr().as_ptr()),
            label,
        )
    }
}

fn destroy_cuda_graph_or_log(graph: cudarc::driver::sys::CUgraph, label: &str) {
    if graph.is_null() {
        return;
    }
    // SAFETY: graph is an owned CUDA graph handle; destroy is used from Drop
    // and cleanup paths so failures are logged.
    unsafe {
        log_cuda_drop_result(label, cudarc::driver::sys::cuGraphDestroy(graph));
    }
}

fn destroy_cuda_graph_exec_or_log(graph_exec: cudarc::driver::sys::CUgraphExec, label: &str) {
    if graph_exec.is_null() {
        return;
    }
    // SAFETY: graph_exec is an owned CUDA executable graph handle; destroy is
    // used from Drop paths so failures are logged.
    unsafe {
        log_cuda_drop_result(label, cudarc::driver::sys::cuGraphExecDestroy(graph_exec));
    }
}

fn add_cuda_graph_replay_bytes(
    total: &mut u64,
    bytes: usize,
    label: &str,
) -> Result<(), BackendError> {
    CUDA_GRAPH_REPLAY_ACCOUNTING.add_bytes(total, bytes, label)
}

fn add_cuda_graph_replay_operation(total: &mut u64, label: &str) -> Result<(), BackendError> {
    CUDA_GRAPH_REPLAY_ACCOUNTING.add_u64_counter(total, 1, label, "operation accounting")
}

struct GraphHostBuffers {
    pool: Arc<PinnedHostAllocationPool>,
    input: SmallVec<[PinnedHostAllocation; 8]>,
    output: SmallVec<[PinnedHostAllocation; 8]>,
}

impl GraphHostBuffers {
    fn try_with_capacity(
        pool: Arc<PinnedHostAllocationPool>,
        input_capacity: usize,
        output_capacity: usize,
    ) -> Result<Self, BackendError> {
        let mut buffers = Self {
            pool,
            input: SmallVec::new(),
            output: SmallVec::new(),
        };
        reserve_smallvec(
            &mut buffers.input,
            input_capacity,
            "cuda graph input host buffers",
        )?;
        reserve_smallvec(
            &mut buffers.output,
            output_capacity,
            "cuda graph output host buffers",
        )?;
        Ok(buffers)
    }

    fn push_input(&mut self, bytes: &[u8]) -> Result<(), BackendError> {
        if bytes.is_empty() {
            self.input.push(PinnedHostAllocation::default());
            return Ok(());
        }
        let mut allocation = self.pool.acquire(bytes.len())?;
        allocation.copy_from_slice(bytes)?;
        self.input.push(allocation);
        Ok(())
    }

    fn push_input_padded(
        &mut self,
        bytes: &[u8],
        transfer_byte_len: usize,
    ) -> Result<(), BackendError> {
        if bytes.is_empty() {
            self.input.push(PinnedHostAllocation::default());
            return Ok(());
        }
        if transfer_byte_len < bytes.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA graph transfer length {} is smaller than logical input length {}.",
                    transfer_byte_len,
                    bytes.len()
                ),
            });
        }
        let mut allocation = self.pool.acquire(transfer_byte_len)?;
        allocation.copy_from_slice(bytes)?;
        if transfer_byte_len > bytes.len() {
            allocation.zero_range(bytes.len(), transfer_byte_len - bytes.len())?;
        }
        self.input.push(allocation);
        Ok(())
    }

    fn push_output(&mut self, byte_len: usize) -> Result<(), BackendError> {
        if byte_len == 0 {
            self.output.push(PinnedHostAllocation::default());
            return Ok(());
        }
        self.output.push(self.pool.acquire(byte_len)?);
        Ok(())
    }

    fn into_raw(
        mut self,
    ) -> (
        SmallVec<[PinnedHostAllocation; 8]>,
        SmallVec<[PinnedHostAllocation; 8]>,
    ) {
        let input = std::mem::take(&mut self.input);
        let output = std::mem::take(&mut self.output);
        (input, output)
    }
}

impl Drop for GraphHostBuffers {
    fn drop(&mut self) {
        for allocation in self.input.drain(..).chain(self.output.drain(..)) {
            self.pool.release(allocation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GraphHostBuffers;
    use crate::backend::PinnedHostAllocationPool;
    use std::sync::Arc;

    #[test]
    fn cuda_graph_zero_byte_host_buffers_do_not_acquire_pinned_memory() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut buffers = GraphHostBuffers::try_with_capacity(Arc::clone(&pool), 1, 1)
            .expect("Fix: graph host buffers should reserve tiny test capacities");

        buffers
            .push_input(&[])
            .expect("Fix: zero-byte graph input must not call CUDA host allocation APIs");
        buffers
            .push_output(0)
            .expect("Fix: zero-byte graph output must not call CUDA host allocation APIs");

        assert!(buffers.input[0].as_ptr().is_null());
        assert!(buffers.output[0].as_ptr().is_null());
        assert_eq!(pool.cached_bytes(), 0);
    }

    #[test]
    fn cuda_graph_padded_input_upload_zero_fills_tail() {
        // Pinned host memory requires an initialized, thread-bound CUDA
        // context; acquire one first (held for the whole test so it outlives
        // the pinned buffers).
        let _device = crate::device::CudaDeviceHandle::acquire_ordinal(0)
            .expect("Fix: acquire a CUDA device/context before pinned host allocation");
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut buffers = GraphHostBuffers::try_with_capacity(Arc::clone(&pool), 1, 1)
            .expect("Fix: padded input staging should use fallible pinned buffer acquisition");

        buffers.push_input_padded(&[1_u8, 2, 3], 16).expect(
            "Fix: padded input staging should allocate enough capacity for async DMA copies",
        );

        let mut out = Vec::new();
        buffers.input[0]
            .copy_prefix_into(16, &mut out)
            .expect("Fix: copy back staged input staging bytes to verify alignment padding");

        assert_eq!(out, &[1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn cached_cuda_graph_stores_owned_guards_not_raw_cuda_resources() {
        let source = include_str!("cuda_graph.rs");

        assert!(
            source.contains("pub(crate) graph_exec: GraphExecGuard"),
            "Fix: CachedCudaGraph must own CUgraphExec through GraphExecGuard, not a raw pointer field."
        );
        assert!(
            source.contains("pub(crate) graph: GraphGuard"),
            "Fix: CachedCudaGraph must own CUgraph through GraphGuard, not a raw pointer field."
        );
        assert!(
            source.contains("pub(crate) stream: StreamGuard"),
            "Fix: CachedCudaGraph must own CUstream through StreamGuard, not a raw pointer field."
        );
        assert!(
            source.contains("SmallVec<[DevicePtrGuard; 8]>"),
            "Fix: CachedCudaGraph must retain device allocations as DevicePtrGuard values so drop order owns cuMemFree."
        );
        assert!(
            !source.contains(concat!("pub(crate) graph", ": NonNull<CUgraph_st>"))
                && !source.contains(concat!(
                    "pub(crate) graph_exec",
                    ": NonNull<CUgraphExec_st>"
                ))
                && !source.contains(concat!("pub(crate) stream", ": NonNull<CUstream_st>"))
                && !source.contains(concat!("pub(crate) params_device_ptr", ": u64")),
            "Fix: CachedCudaGraph release ownership must not regress to raw CUDA resource fields."
        );
    }

    #[test]
    fn cuda_graph_lifecycle_ffi_is_single_sourced() {
        let source = include_str!("cuda_graph.rs");
        let begin = concat!("cudarc::driver::sys::", "cuStreamBeginCapture_v2(");
        let end = concat!("cudarc::driver::sys::", "cuStreamEndCapture(");
        let instantiate = concat!("cudarc::driver::sys::", "cuGraphInstantiateWithFlags(");
        let destroy_graph = concat!("cudarc::driver::sys::", "cuGraphDestroy(");
        let destroy_exec = concat!("cudarc::driver::sys::", "cuGraphExecDestroy(");
        let upload = concat!("cudarc::driver::sys::", "cuGraphUpload(");

        assert_eq!(source.matches(begin).count(), 1);
        assert_eq!(source.matches(end).count(), 1);
        assert_eq!(source.matches(instantiate).count(), 1);
        assert_eq!(source.matches(destroy_graph).count(), 1);
        assert_eq!(source.matches(destroy_exec).count(), 1);
        assert_eq!(source.matches(upload).count(), 1);
        assert!(
            source.contains("fn begin_cuda_graph_capture(")
                && source.contains("fn end_cuda_graph_capture(")
                && source.contains("fn instantiate_cuda_graph(")
                && source.contains("fn upload_cuda_graph_exec(")
                && source.contains("capture_guard.finish(")
                && source.contains("resident_capture_guard.finish("),
            "Fix: CUDA graph capture, instantiate, upload, and cleanup paths must route through shared lifecycle helpers."
        );
        let upload_cleanup = source
            .rsplit("let upload_result = (||")
            .next()
            .expect("Fix: CUDA graph upload must classify graph-upload cleanup errors.")
            .split("let (input_host_bufs, output_host_bufs) = host_buffers.into_raw();")
            .next()
            .expect("Fix: CUDA graph upload cleanup must precede cached graph ownership transfer.");
        assert!(
            upload_cleanup.contains("synchronize_cuda_graph_stream(")
                && upload_cleanup.contains("\"cuStreamSynchronize (cuda_graph upload cleanup)\"")
                && upload_cleanup.contains("In-flight CUDA graph resources will not be recycled.")
                && upload_cleanup.contains("std::mem::forget(stream);")
                && upload_cleanup.contains("std::mem::forget(graph_exec);")
                && upload_cleanup.contains("std::mem::forget(graph);")
                && upload_cleanup.contains("std::mem::forget(resident_input_graph_exec);")
                && upload_cleanup.contains("std::mem::forget(resident_input_graph);")
                && upload_cleanup.contains("std::mem::forget(params_device_ptr);")
                && upload_cleanup.contains("std::mem::forget(input_device_ptrs);")
                && upload_cleanup.contains("std::mem::forget(output_device_ptrs);")
                && upload_cleanup.contains("std::mem::forget(host_buffers);"),
            "Fix: CUDA graph upload errors must prove stream completion before dropping graph resources or leak all captured owners when completion is unproven."
        );
    }

    #[test]
    fn cuda_graph_output_readbacks_are_single_sourced_for_full_and_resident_captures() {
        let source = include_str!("cuda_graph.rs");

        assert_eq!(
            source
                .matches(concat!("record_cuda_graph_output_", "readbacks("))
                .count(),
            3,
            "Fix: full and resident-input CUDA graph captures must share one output D2H capture helper."
        );
        assert_eq!(
            source
                .matches(concat!("super::copy::", "d2h_async_checked_with_label"))
                .count(),
            1,
            "Fix: CUDA graph output D2H capture FFI must stay behind the shared readback helper."
        );
    }

    #[test]
    fn cuda_graph_capture_records_output_clears_for_sparse_outputs() {
        let source = include_str!("cuda_graph.rs");

        assert_eq!(
            source
                .matches(concat!("record_cuda_graph_output_", "clears("))
                .count(),
            3,
            "Fix: CUDA graph full and resident-input captures must share one output clear helper."
        );
        assert!(
            source.contains("cuMemsetD8Async (capture output clear)")
                && source.contains("cuMemsetD8Async (resident input capture output clear)")
                && source.contains("output_clears.push(OutputClear"),
            "Fix: CUDA graph replay must capture output-only zero fills before sparse stores so unwritten output lanes cannot retain stale device bytes."
        );
    }

    #[test]
    fn cuda_graph_capture_argument_tables_use_checked_fallible_reservation() {
        let source = include_str!("cuda_graph.rs");

        assert!(
            source.contains("launch_pointer_capacity")
                && source.contains("kernel_arg_capacity")
                && source.contains("reserve_smallvec("),
            "Fix: CUDA graph capture must use checked capacity math and fallible reservation for launch pointer and kernel argument tables."
        );
        assert!(
            !source.contains(concat!(
                "SmallVec",
                "::with_capacity",
                "(all_ptrs.len() + 1)"
            )),
            "Fix: CUDA graph capture must not use infallible kernel argument table growth on the release path."
        );
    }

    #[test]
    fn cuda_graph_binding_planning_is_shared_driver_logic() {
        let source = include_str!("cuda_graph.rs");
        let planner_import = concat!(
            "use vyre_driver::graph_capture::",
            "plan_graph_capture_bindings;"
        );
        let planner_call = concat!("plan_graph_capture_", "bindings(&prepared.bindings)");

        assert!(source.contains(planner_import));
        assert_eq!(source.matches(planner_call).count(), 1);
        assert!(!source.contains(concat!("fn cuda_graph_binding_", "capacities")));
        assert!(!source.contains(concat!("fn cuda_graph_capacity_", "add")));
        assert!(!source.contains(concat!("checked_add_", "usize_lazy")));
        assert!(
            source.contains("output_device_capacity")
                && source.contains("output_readback_capacity")
                && source.contains("kernel_pointer_capacity")
                && source.contains("kernel_argument_capacity"),
            "Fix: CUDA graph capture must use the shared driver capture plan instead of re-deriving backend-local capacities."
        );
    }

    #[test]
    fn cuda_graph_capture_uses_shared_fallible_smallvec_staging_reservation() {
        let source = include_str!("cuda_graph.rs");

        assert!(source.contains("use super::staging_reserve::reserve_smallvec;"));
        assert!(source.contains("fn try_with_capacity("));
        assert!(!source.contains(concat!("SmallVec", "::with_capacity")));
    }

    #[test]
    fn cuda_graph_telemetry_uses_shared_numeric_policy() {
        let source = include_str!("cuda_graph.rs");
        assert!(
            source.contains("use crate::numeric::CUDA_NUMERIC;")
                && source.contains("CUDA_NUMERIC.usize_to_u64(value, label)")
                && !source.contains(concat!("u64::try_from", "(value)")),
            "Fix: CUDA graph telemetry byte conversions must use the shared backend numeric policy."
        );
    }
}

/// A pre-recorded CUDA graph wrapping one full Program-dispatch sequence
/// (input HtoD memcpy + kernel launch + output DtoH memcpy). Hold on to this
/// across many `dispatch_via_cuda_graph` calls to amortize launch overhead.
///
/// `CachedCudaGraph` owns:
///   - The captured `CUgraph` and instantiated `CUgraphExec`.
///   - A dedicated `CUstream` used for capture + replay.
///   - Device pointers for every input + output buffer.
///   - Host buffers for every input (so callers write new bytes into the
///     same address the captured memcpy reads from) and every output (so
///     readback target stays stable across replays).
///
/// On drop, all CUDA resources are released in the right order.
#[derive(Debug)]
pub struct CachedCudaGraph {
    /// Backend reference  -  keeps the CUDA context alive for the cached
    /// graph's lifetime.
    pub(crate) backend: CudaBackend,
    /// Instantiated graph executable (owned). Destroyed in `drop` BEFORE
    /// `graph`.
    pub(crate) graph_exec: GraphExecGuard,
    /// Captured graph (owned). Destroyed in `drop`.
    pub(crate) graph: GraphGuard,
    /// Steady-state graph executable that reuses resident device inputs when
    /// the caller replays the same bytes and no input buffer is also an
    /// output buffer.
    pub(crate) resident_input_graph_exec: GraphExecGuard,
    /// Captured steady-state graph without host-to-device input copy nodes.
    pub(crate) resident_input_graph: GraphGuard,
    /// Dedicated stream used for capture + replay (owned). Destroyed in
    /// `drop` AFTER graph + graph_exec.
    pub(crate) stream: StreamGuard,
    /// Per-input host buffers. Callers write new input bytes here before
    /// each replay; the captured memcpy reads from these addresses.
    pub(crate) input_host_bufs: SmallVec<[PinnedHostAllocation; 8]>,
    /// Logical caller input index for each descriptor-ordered input host
    /// buffer. CUDA graph capture records memcpy nodes in descriptor order,
    /// while public replay inputs follow Program::buffers logical order.
    pub(crate) input_indices: SmallVec<[usize; 8]>,
    /// Per-input device pointers (allocated via `cuMemAlloc_v2`). Freed in
    /// `drop`.
    pub(crate) input_device_ptrs: SmallVec<[DevicePtrGuard; 8]>,
    /// Per-output device pointers (allocated via `cuMemAlloc_v2`). Freed
    /// in `drop`.
    pub(crate) output_device_ptrs: SmallVec<[DevicePtrGuard; 8]>,
    /// Per-output pinned host buffers. The captured DtoH memcpy writes into
    /// these stable addresses on every replay.
    pub(crate) output_host_bufs: SmallVec<[PinnedHostAllocation; 8]>,
    /// Logical output index for each descriptor-ordered output host buffer.
    /// CUDA graph readback nodes stay in descriptor order, while public
    /// result vectors follow Program::buffers logical output order.
    pub(crate) output_indices: SmallVec<[usize; 8]>,
    /// Exact byte lengths for each output. Pinned allocations are bucketed and
    /// can be larger than the logical output buffer.
    pub(crate) output_lens: SmallVec<[usize; 8]>,
    /// Total input bytes copied by every replay of this fixed-shape graph.
    pub(crate) replay_input_bytes: u64,
    /// Total output bytes read back by every replay of this fixed-shape graph.
    pub(crate) replay_output_bytes: u64,
    /// Non-empty host-to-device copy operations captured in each replay.
    pub(crate) replay_host_upload_operations: u64,
    /// Non-empty device-to-host copy operations captured in each replay.
    pub(crate) replay_device_readback_operations: u64,
    /// Expected input byte lengths. `dispatch_via_cuda_graph` validates
    /// the caller's input sizes match these  -  a mismatch means the graph
    /// is wrong-shape for the input and must be re-recorded.
    pub(crate) expected_input_lens: SmallVec<[usize; 8]>,
    /// Host-side transfer lengths used for async input uploads during capture
    /// and replay updates.
    pub(crate) input_transfer_lens: SmallVec<[usize; 8]>,
    /// Exact tuple-boundary-preserving key for bytes currently stored in
    /// `input_host_bufs`.
    pub(crate) cached_input_key: ExactInputKey,
    /// Whether the no-upload steady-state graph is semantically safe. It is
    /// disabled for input-output bindings because the kernel mutates the
    /// resident input buffer.
    pub(crate) resident_input_replay_safe: bool,
    /// Whether resident device inputs are known to match the cached host
    /// input bytes.
    pub(crate) device_inputs_initialized: bool,
    /// Whether pinned host output buffers contain a completed replay result
    /// for the cached host input bytes.
    pub(crate) host_outputs_initialized: bool,
    /// Param-buffer device pointer (single allocation; freed in `drop`).
    /// The kernel reads launch parameters (workgroup-related constants)
    /// from this buffer.
    pub(crate) params_device_ptr: DevicePtrGuard,
}

// SAFETY: `CachedCudaGraph` holds raw CUDA resource pointers (graph,
// graph_exec, stream, device pointers). All access goes through cudarc FFI
// calls that are documented thread-safe per the CUDA Driver API
// (`cuGraphLaunch`, `cuStreamSynchronize`, etc.). The pinned host buffers
// are mutated only through `&mut self`.
unsafe impl Send for CachedCudaGraph {}

impl Drop for CachedCudaGraph {
    fn drop(&mut self) {
        let _owned_cuda_resource_counts = (
            self.graph.ptr().as_ptr(),
            self.resident_input_graph.ptr().as_ptr(),
            self.input_device_ptrs.len(),
            self.output_device_ptrs.len(),
            self.params_device_ptr.ptr(),
        );
        if let Err(error) = self.backend.warmup() {
            tracing::error!(
                "Fix: CUDA backend warmup failed before graph resource drop: {error}. Cleanup will continue, but the CUDA context may be unhealthy."
            );
        }
        for allocation in self
            .input_host_bufs
            .drain(..)
            .chain(self.output_host_bufs.drain(..))
        {
            self.backend.host_pool.release(allocation);
        }
    }
}

impl CudaBackend {
    /// Record one full Program dispatch into a CUDA graph for fast replay.
    ///
    /// Allocates all device + host buffers, captures the dispatch sequence
    /// (HtoD memcpy → kernel launch → DtoH memcpy), and instantiates the
    /// captured graph. The returned `CachedCudaGraph` is a handle the
    /// caller drives via `dispatch_via_cuda_graph`.
    ///
    /// `sample_inputs` is used only to determine the input byte-layout
    /// shape captured into the graph; the caller passes the actual
    /// per-dispatch bytes via `dispatch_via_cuda_graph`. The bytes in
    /// `sample_inputs` are also copied into the cached host buffers as the
    /// initial state.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when device allocation fails, the kernel
    /// cannot be compiled or loaded, or the CUDA driver rejects any of the
    /// graph capture / instantiate operations.
    pub fn record_cuda_graph(
        &self,
        program: &Program,
        sample_inputs: &[Vec<u8>],
        config: &DispatchConfig,
    ) -> Result<CachedCudaGraph, BackendError> {
        let mut sample_refs = SmallVec::<[&[u8]; 8]>::new();
        reserve_smallvec(
            &mut sample_refs,
            sample_inputs.len(),
            "cuda graph borrowed sample input references",
        )?;
        for input in sample_inputs {
            sample_refs.push(input.as_slice());
        }
        self.record_cuda_graph_borrowed(program, &sample_refs, config)
    }

    /// Record one full Program dispatch into a CUDA graph using borrowed
    /// sample inputs.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when device allocation fails, the kernel
    /// cannot be compiled or loaded, or the CUDA driver rejects graph capture.
    pub fn record_cuda_graph_borrowed(
        &self,
        program: &Program,
        sample_inputs: &[&[u8]],
        config: &DispatchConfig,
    ) -> Result<CachedCudaGraph, BackendError> {
        if config.cooperative {
            return Err(BackendError::UnsupportedFeature {
                name: "cuda_graph_cooperative_capture (regular CUDA graph capture records cuLaunchKernel, not cuLaunchCooperativeKernel)"
                    .to_string(),
                backend: crate::CUDA_BACKEND_ID.to_string(),
            });
        }
        let _capture_serial = self.graph_capture_lock.lock().map_err(|_| {
            BackendError::DispatchFailed {
                code: None,
                message: "cuda graph capture lock poisoned. Fix: recreate CudaBackend after a panic during graph recording.".to_string(),
            }
        })?;
        self.warmup()?;

        // Compile + prepare. This lifts the program into PTX, computes the
        // binding plan, validates the program. All allocations / launches
        // below assume this succeeded.
        let prepared = self.prepare_host_dispatch(program, sample_inputs, config)?;
        let (ptx_src, ptx_source_key) = self.ptx_for_program_cached_with_key(program, config)?;
        let module_key = self.module_cache_key_for_ptx_source_key(ptx_source_key)?;
        let func = self.resolve_launch_function(&ptx_src, module_key, &prepared.launch, false)?;
        self.validate_transient_dispatch_memory_budget(
            &prepared,
            sample_inputs,
            "CUDA graph capture",
        )?;

        // Allocate all device buffers BEFORE capture. cuMemAlloc returns
        // CUDA_ERROR_STREAM_CAPTURE_UNSUPPORTED inside capture; allocating
        // up front is the only way to make capture work.
        let capture_binding_plan = plan_graph_capture_bindings(&prepared.bindings)?;
        let input_capacity = capture_binding_plan.input_device_capacity;
        let output_device_capacity = capture_binding_plan.output_device_capacity;
        let output_readback_capacity = capture_binding_plan.output_readback_capacity;
        let mut input_device_ptrs = SmallVec::<[DevicePtrGuard; 8]>::new();
        reserve_smallvec(
            &mut input_device_ptrs,
            input_capacity,
            "cuda graph input device pointer guards",
        )?;
        let mut input_indices = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut input_indices,
            input_capacity,
            "cuda graph logical input indices",
        )?;
        let mut output_device_ptrs = SmallVec::<[DevicePtrGuard; 8]>::new();
        reserve_smallvec(
            &mut output_device_ptrs,
            output_device_capacity,
            "cuda graph output device pointer guards",
        )?;
        let mut output_clears = SmallVec::<[OutputClear; 8]>::new();
        reserve_smallvec(
            &mut output_clears,
            output_device_capacity,
            "cuda graph output clear captures",
        )?;
        let mut output_indices = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut output_indices,
            output_readback_capacity,
            "cuda graph logical output indices",
        )?;
        let mut readback_device_ptrs = SmallVec::<[u64; 8]>::new();
        reserve_smallvec(
            &mut readback_device_ptrs,
            output_readback_capacity,
            "cuda graph readback device pointers",
        )?;
        let mut host_buffers = GraphHostBuffers::try_with_capacity(
            Arc::clone(&self.host_pool),
            input_capacity,
            output_readback_capacity,
        )?;
        let mut expected_input_lens = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut expected_input_lens,
            input_capacity,
            "cuda graph expected input byte lengths",
        )?;
        let mut input_transfer_lens = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut input_transfer_lens,
            input_capacity,
            "cuda graph input transfer byte lengths",
        )?;
        let mut output_lens = SmallVec::<[usize; 8]>::new();
        reserve_smallvec(
            &mut output_lens,
            output_readback_capacity,
            "cuda graph output byte lengths",
        )?;
        let mut replay_input_bytes = 0_u64;
        let mut replay_output_bytes = 0_u64;
        let mut replay_host_upload_operations = 0_u64;
        let mut replay_device_readback_operations = 0_u64;
        let resident_input_replay_safe = capture_binding_plan.resident_input_replay_safe;
        let cached_input_key = exact_input_key(sample_inputs)?;

        // Walk binding plan in order, allocating + classifying input vs output.
        for binding in &prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let byte_len = match binding.input_index {
                Some(input_index) => cuda_graph_sample_input(
                    sample_inputs,
                    input_index,
                    &binding.name,
                    "allocation sizing",
                )?
                .len(),
                None => binding
                    .static_byte_len
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA-graph output `{}` needs a static byte length to be \
                             cached. Set BufferDecl::with_count or output_byte_range before \
                             recording.",
                            binding.name
                        ),
                    })?,
            };
            let device_byte_len = if byte_len == 0 {
                1
            } else {
                aligned_async_copy_len(byte_len)?
            };
            let device_ptr = alloc_cuda_ptr(
                device_byte_len,
                "cuMemAlloc_v2 (cuda_graph input/output buffer)",
            )?;
            self.telemetry
                .record_transient_allocation_bytes(cuda_graph_usize_to_u64(
                    device_byte_len,
                    "cudaGraph input/output allocation bytes",
                )?);
            if let Some(input_index) = binding.input_index {
                let sample_input = cuda_graph_sample_input(
                    sample_inputs,
                    input_index,
                    &binding.name,
                    "input staging",
                )?;
                let input_len = sample_input.len();
                let input_transfer_len = if input_len == 0 {
                    0
                } else {
                    aligned_async_copy_len(input_len)?
                };
                expected_input_lens.push(input_len);
                input_transfer_lens.push(input_transfer_len);
                add_cuda_graph_replay_bytes(&mut replay_input_bytes, input_len, "input replay")?;
                if input_len != 0 {
                    add_cuda_graph_replay_operation(
                        &mut replay_host_upload_operations,
                        "host upload replay",
                    )?;
                }
                host_buffers.push_input_padded(sample_input, input_transfer_len)?;
                input_indices.push(input_index);
                input_device_ptrs.push(DevicePtrGuard::new(device_ptr));
            } else {
                output_clears.push(OutputClear {
                    dst: device_ptr,
                    byte_len: device_byte_len,
                });
                output_device_ptrs.push(DevicePtrGuard::new(device_ptr));
            }
            if let Some(output_index) = binding.output_index {
                let readback = cuda_output_readback_for_binding(
                    program.buffers(),
                    binding.buffer_index,
                    &binding.name,
                    byte_len,
                    "graph capture output readback",
                )?;
                host_buffers.push_output(readback.byte_len)?;
                output_indices.push(output_index);
                output_lens.push(readback.byte_len);
                add_cuda_graph_replay_bytes(
                    &mut replay_output_bytes,
                    readback.byte_len,
                    "output replay",
                )?;
                if readback.byte_len != 0 {
                    add_cuda_graph_replay_operation(
                        &mut replay_device_readback_operations,
                        "device readback replay",
                    )?;
                }
                let readback_ptr = vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                    device_ptr,
                    readback.device_offset,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA graph output readback device offset {} for `{}` does not fit CUdeviceptr arithmetic.",
                            readback.device_offset, binding.name
                        ),
                    }
                    },
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA graph readback pointer overflowed for output `{}` at device_ptr={device_ptr} offset={}. Re-record with a valid output range or split the output buffer.",
                            binding.name, readback.device_offset
                        ),
                    }
                    },
                )?;
                readback_device_ptrs.push(readback_ptr);
            }
        }

        // Allocate the param buffer separately (one per cached graph).
        let param_bytes = super::launch_params::launch_param_byte_len(
            &prepared.launch.param_words,
            "cudaGraph capture",
        )?;
        let param_copy_bytes = if param_bytes == 0 {
            0
        } else {
            aligned_async_copy_len(param_bytes)?
        };
        let params_device_ptr = if param_bytes != 0 {
            // SAFETY: param_bytes is u32-aligned and non-zero in this branch.
            let params_device_ptr =
                alloc_cuda_ptr(param_copy_bytes, "cuMemAlloc_v2 (cuda_graph param buffer)")?;
            self.telemetry
                .record_transient_allocation_bytes(cuda_graph_usize_to_u64(
                    param_copy_bytes,
                    "cudaGraph parameter allocation bytes",
                )?);
            params_device_ptr
        } else {
            0
        };
        let params_device_ptr = DevicePtrGuard::new(params_device_ptr);

        // Create dedicated stream for capture + replay.
        let stream = create_cuda_graph_stream()?;
        // SAFETY: FFI to libcuda.so. Pointer args were validated by the
        // matching alloc / store API; lifetimes are documented in the
        // surrounding function. cuda_check (or matching CUresult guard)
        // propagates non-success codes as BackendError.
        if param_bytes != 0 {
            let mut param_host_transfer =
                HostTransferAllocations::with_capacity(Arc::clone(&self.host_pool), 1, 0)?;
            let param_host_ptr = param_host_transfer
                .push_u32_words_padded(&prepared.launch.param_words, param_copy_bytes)?;
            // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
            unsafe {
                // Upload the param words once; the kernel reads them on every replay.
                // The async copy targets the dedicated stream so recording cannot
                // create an implicit dependency on CUDA's legacy default stream.
                super::copy::h2d_async_checked_with_label(
                    params_device_ptr.ptr(),
                    param_host_ptr,
                    param_copy_bytes,
                    stream.ptr().as_ptr(),
                    "cuMemcpyHtoDAsync_v2 (cuda_graph param init)",
                )?;
                synchronize_cuda_graph_stream(
                    &stream,
                    "cuStreamSynchronize (cuda_graph param init)",
                )?;
            }
            self.telemetry.record_sync_point();
        }

        let _ = CU_STREAM_CAPTURE_MODE_THREAD_LOCAL; // suppress unused-const warning
                                                     // Begin capture. Every cuda call on `stream` from here until end
                                                     // capture is recorded into the graph.
                                                     //
                                                     // SAFETY: stream is freshly created. The capture mode is constructed
                                                     // directly via the typed enum variant (THREAD_LOCAL) rather than
                                                     // `std::mem::transmute::<u32, _>(1)`  -  the old transmute would have
                                                     // been UB if the local u32 constant ever drifted away from a valid
                                                     // variant value (the enum has gaps at 3..). The typed variant is
                                                     // compile-time-checked and just as efficient.
        let mut capture_guard = begin_cuda_graph_capture(&stream, "cuStreamBeginCapture_v2")?;

        // Record HtoD memcpys for each input.
        for ((host_buf, input_len), (input_transfer_len, device_ptr)) in host_buffers
            .input
            .iter()
            .zip(expected_input_lens.iter())
            .zip(input_transfer_lens.iter().zip(input_device_ptrs.iter()))
        {
            if *input_len == 0 {
                continue;
            }
            let copy_len = if *input_transfer_len == 0 {
                *input_len
            } else {
                *input_transfer_len
            };
            // SAFETY: host_buf.as_ptr() is stable for the lifetime of CachedCudaGraph
            // (the Vec is owned by CachedCudaGraph and never reallocated  -  capacity is
            // set at construction). device_ptr was allocated above. Both pointers
            // outlive the captured graph.
            unsafe {
                super::copy::h2d_async_checked_with_label(
                    device_ptr.ptr(),
                    host_buf.as_ptr(),
                    copy_len,
                    stream.ptr().as_ptr(),
                    "cuMemcpyHtoDAsync_v2 (capture input)",
                )?;
            }
        }

        record_cuda_graph_output_clears(
            &output_clears,
            &stream,
            "cuMemsetD8Async (capture output clear)",
        )?;

        // Record kernel launch. Build kernel_args mirroring the production
        // launch_module path: per-buffer u64 ptr-of-ptr, then param ptr.
        let launch_pointer_capacity = capture_binding_plan.kernel_pointer_capacity;
        let mut all_ptrs = SmallVec::<[u64; 16]>::new();
        reserve_smallvec(
            &mut all_ptrs,
            launch_pointer_capacity,
            "graph capture launch pointer",
        )?;
        let mut input_iter = input_device_ptrs.iter();
        let mut output_iter = output_device_ptrs.iter();
        for binding in &prepared.bindings.bindings {
            if binding.role == BindingRole::Shared {
                continue;
            }
            let ptr = if binding.input_index.is_some() {
                input_iter
                    .next()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA graph capture binding plan expected an input pointer for `{}` but none was allocated.",
                            binding.name
                        ),
                    })?
                    .ptr()
            } else {
                output_iter
                    .next()
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA graph capture binding plan expected an output pointer for `{}` but none was allocated.",
                            binding.name
                        ),
                    })?
                    .ptr()
            };
            all_ptrs.push(ptr);
        }
        let kernel_arg_capacity = capture_binding_plan.kernel_argument_capacity;
        let mut kernel_args: SmallVec<[*mut std::ffi::c_void; 16]> = SmallVec::new();
        reserve_smallvec(
            &mut kernel_args,
            kernel_arg_capacity,
            "graph capture kernel argument",
        )?;
        for ptr in &mut all_ptrs {
            if *ptr == 0 {
                return Err(BackendError::InvalidProgram {
                    fix: "Fix: CUDA graph capture resolved a null kernel argument; graph launch arguments must preserve the lowered descriptor order."
                        .to_string(),
                });
            }
            kernel_args.push(ptr as *mut _ as *mut std::ffi::c_void);
        }
        let mut params_ref = params_device_ptr.ptr();
        kernel_args.push(&mut params_ref as *mut _ as *mut std::ffi::c_void);

        for _ in 0..prepared.fixpoint_iterations {
            super::launch::launch_cuda_function(
                func,
                kernel_args.as_mut_slice(),
                &prepared.launch,
                stream.ptr().as_ptr(),
                false,
                self.ptx_target_sm(),
                "cuLaunchKernel (capture)",
            )?;
        }

        record_cuda_graph_output_readbacks(
            &mut host_buffers.output,
            &output_lens,
            &readback_device_ptrs,
            &stream,
            "cuMemcpyDtoHAsync_v2 (capture output)",
        )?;

        // End capture and instantiate.
        let graph = capture_guard.finish(
            "cuStreamEndCapture",
            "cuStreamEndCapture returned a null graph after reporting success. Fix: update the CUDA driver or disable CUDA graph capture for this device.",
        )?;

        let graph_exec = instantiate_cuda_graph(
            &graph,
            "cuGraphInstantiateWithFlags",
            "cuGraphInstantiateWithFlags returned a null executable graph after reporting success. Fix: update the CUDA driver or disable CUDA graph capture for this device.",
        )?;

        // Capture a second steady-state graph for repeated identical inputs.
        // The full graph above remains the correctness path whenever input
        // bytes change; this graph removes only HtoD nodes after the device
        // input buffers are known-current.
        let mut resident_capture_guard = begin_cuda_graph_capture(
            &stream,
            "cuStreamBeginCapture_v2 (resident input cuda_graph)",
        )?;
        record_cuda_graph_output_clears(
            &output_clears,
            &stream,
            "cuMemsetD8Async (resident input capture output clear)",
        )?;
        for _ in 0..prepared.fixpoint_iterations {
            super::launch::launch_cuda_function(
                func,
                kernel_args.as_mut_slice(),
                &prepared.launch,
                stream.ptr().as_ptr(),
                false,
                self.ptx_target_sm(),
                "cuLaunchKernel (resident input capture)",
            )?;
        }
        record_cuda_graph_output_readbacks(
            &mut host_buffers.output,
            &output_lens,
            &readback_device_ptrs,
            &stream,
            "cuMemcpyDtoHAsync_v2 (resident input capture output)",
        )?;
        let resident_input_graph = resident_capture_guard.finish(
            "cuStreamEndCapture (resident input cuda_graph)",
            "cuStreamEndCapture returned a null resident-input graph after reporting success. Fix: update the CUDA driver or disable CUDA graph capture for this device.",
        )?;

        let resident_input_graph_exec = instantiate_cuda_graph(
            &resident_input_graph,
            "cuGraphInstantiateWithFlags (resident input cuda_graph)",
            "cuGraphInstantiateWithFlags returned a null resident-input executable graph after reporting success. Fix: update the CUDA driver or disable CUDA graph capture for this device.",
        )?;

        let upload_result = (|| {
            upload_cuda_graph_exec(&graph_exec, &stream, "cuGraphUpload")?;
            upload_cuda_graph_exec(
                &resident_input_graph_exec,
                &stream,
                "cuGraphUpload (resident input cuda_graph)",
            )
        })();
        if let Err(error) = upload_result {
            match synchronize_cuda_graph_stream(
                &stream,
                "cuStreamSynchronize (cuda_graph upload cleanup)",
            ) {
                Ok(()) => {
                    self.telemetry.record_sync_point();
                    return Err(error);
                }
                Err(sync_error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA graph upload stream after upload error: {sync_error}. In-flight CUDA graph resources will not be recycled."
                    );
                    std::mem::forget(stream);
                    std::mem::forget(graph_exec);
                    std::mem::forget(graph);
                    std::mem::forget(resident_input_graph_exec);
                    std::mem::forget(resident_input_graph);
                    std::mem::forget(params_device_ptr);
                    std::mem::forget(input_device_ptrs);
                    std::mem::forget(output_device_ptrs);
                    std::mem::forget(host_buffers);
                    return Err(error);
                }
            }
        }

        let (input_host_bufs, output_host_bufs) = host_buffers.into_raw();

        Ok(CachedCudaGraph {
            backend: self.clone(),
            graph_exec,
            graph,
            resident_input_graph_exec,
            resident_input_graph,
            stream,
            input_host_bufs,
            input_indices,
            input_device_ptrs,
            output_device_ptrs,
            output_host_bufs,
            output_indices,
            output_lens,
            input_transfer_lens,
            replay_input_bytes,
            replay_output_bytes,
            replay_host_upload_operations,
            replay_device_readback_operations,
            expected_input_lens,
            cached_input_key,
            resident_input_replay_safe,
            device_inputs_initialized: false,
            host_outputs_initialized: false,
            params_device_ptr,
        })
    }
}
