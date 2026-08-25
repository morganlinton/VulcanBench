//! GPU-resident dispatch graph execution (Innovation I.14).
//!
//! A graph records many dependent pipeline dispatches into one command buffer
//! and submits it once. This gives callers one CPU-to-GPU launch while the GPU
//! executes the ordered kernel sequence already resident in the command queue.

use crate::buffer::GpuBufferHandle;
use crate::pipeline::compound::CompoundResource;
use crate::pipeline::WgpuPipeline;
use smallvec::SmallVec;
use vyre_driver::{BackendError, DispatchConfig};

/// A GPU-resident or host-side resource.
#[derive(Clone)]
pub enum GpuResource {
    /// Host-side byte slice.
    Borrowed(Vec<u8>),
    /// GPU-resident buffer handle.
    Resident(GpuBufferHandle),
}

impl From<Vec<u8>> for GpuResource {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Borrowed(bytes)
    }
}

impl From<GpuBufferHandle> for GpuResource {
    fn from(handle: GpuBufferHandle) -> Self {
        Self::Resident(handle)
    }
}

/// Ordered graph of compiled wgpu pipeline dispatches.
#[derive(Default)]
pub struct GpuDispatchGraph {
    ops: SmallVec<[GraphOp; 8]>,
}

#[derive(Clone)]
struct GraphOp {
    pipeline: WgpuPipeline,
    input: GpuResource,
}

impl GpuDispatchGraph {
    /// Create an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ops: SmallVec::new(),
        }
    }

    /// Append one pipeline dispatch to the graph.
    pub fn push(&mut self, pipeline: WgpuPipeline, input: impl Into<GpuResource>) {
        self.ops.push(GraphOp {
            pipeline,
            input: input.into(),
        });
    }

    /// Number of dispatch nodes in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Return true when the graph has no dispatch nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Execute all graph nodes with one queue submission.
    ///
    /// # Errors
    ///
    /// Returns a backend error if any pipeline binding, dispatch, or readback
    /// fails. Empty graphs return an empty output vector.
    pub fn dispatch(&self, config: &DispatchConfig) -> Result<Vec<Vec<Vec<u8>>>, BackendError> {
        if self.ops.is_empty() {
            return Ok(Vec::new());
        }

        // V7-PERF-021: Zero-copy graph execution (I.14).
        // Convert GpuResources to substrate-neutral Resources for the pipeline engine.
        // Resident handles are identified by their process-stable id.
        let mut internal_requests: SmallVec<[(&WgpuPipeline, CompoundResource<'_>); 8]> =
            SmallVec::with_capacity(self.ops.len());
        for op in &self.ops {
            let res = match &op.input {
                GpuResource::Borrowed(bytes) => CompoundResource::Borrowed(bytes),
                GpuResource::Resident(handle) => CompoundResource::Resident(handle.id()),
            };
            internal_requests.push((&op.pipeline, res));
        }

        WgpuPipeline::dispatch_compound_borrowed(&internal_requests, config)
    }
}

/// CPU-side launch accounting for graph dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchAccounting {
    /// Number of queue submissions required by sequential per-op dispatch.
    pub sequential_submissions: usize,
    /// Number of queue submissions required by graph dispatch.
    pub graph_submissions: usize,
}

impl LaunchAccounting {
    /// Return the integer launch-count reduction from graph recording.
    #[must_use]
    pub fn reduction_factor(self) -> usize {
        self.sequential_submissions / self.graph_submissions.max(1)
    }
}

/// Compute launch-count accounting for an ordered graph with `op_count` nodes.
#[must_use]
pub fn launch_accounting(op_count: usize) -> LaunchAccounting {
    LaunchAccounting {
        sequential_submissions: op_count,
        graph_submissions: usize::from(op_count > 0),
    }
}
