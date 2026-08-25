//! Batched resident CSR frontier-queue execution.
//!
//! This module owns multi-query sparse traversal over one resident CSR graph:
//! each query gets resident scratch slots, all frontiers are uploaded together,
//! all queue/traverse kernels are submitted as one resident sequence, and all
//! frontier outputs are compactly read back at the end.

mod dispatch;

#[cfg(test)]
mod tests;

pub use dispatch::{run_resident_csr_queue_batch_budgeted_into, run_resident_csr_queue_batch_into};

use vyre_foundation::ir::Program;

use crate::graph::csr_frontier_queue_scratch::ResidentCsrQueueMaterializer;
use crate::graph::resident_handles::free_unique_resident_handles;
use crate::hardware::scratch::reserve_vec as reserve_graph_vec;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher, ResidentReadRange};

/// Reusable resident scratch for batched CSR queue traversal queries.
#[derive(Debug, Default)]
pub struct ResidentCsrQueueBatchScratch {
    handles: Vec<ResidentCsrQueueBatchQueryHandles>,
    shape: Option<ResidentCsrQueueBatchShape>,
    program_shape: Option<ResidentCsrQueueBatchProgramShape>,
    clear_frontier_out_program: Option<Program>,
    queue_len_init_program: Option<Program>,
    word_counts_program: Option<Program>,
    word_block_offsets_program: Option<Program>,
    queue_program: Option<Program>,
    high_len_init_program: Option<Program>,
    split_low_program: Option<Program>,
    traverse_program: Option<Program>,
    frontier_payloads: Vec<Vec<u8>>,
    readbacks: Vec<Vec<u8>>,
    clear_handle_sets: Vec<[u64; 1]>,
    queue_len_handle_sets: Vec<[u64; 1]>,
    word_count_handle_sets: Vec<[u64; 3]>,
    word_block_offsets_handle_sets: Vec<[u64; 1]>,
    queue_handle_sets: Vec<[u64; 3]>,
    atomic_word_queue_handle_sets: Vec<[u64; 4]>,
    word_prefix_queue_handle_sets: Vec<[u64; 5]>,
    traverse_handle_sets: Vec<[u64; 6]>,
    high_len_handle_sets: Vec<[u64; 1]>,
    split_low_handle_sets: Vec<[u64; 8]>,
    high_traverse_handle_sets: Vec<[u64; 6]>,
    read_ranges: Vec<ResidentReadRange>,
}

impl ResidentCsrQueueBatchScratch {
    /// Number of resident per-query scratch slots currently retained.
    #[must_use]
    pub fn resident_query_slots(&self) -> usize {
        self.handles.len()
    }

    /// Total host staging capacity retained for frontier uploads.
    #[must_use]
    pub fn frontier_payload_capacity(&self) -> usize {
        self.frontier_payloads.iter().map(Vec::capacity).sum()
    }

    /// Free all batch scratch resident buffers.
    pub fn free(&mut self, dispatcher: &dyn OptimizerDispatcher) -> Result<(), DispatchError> {
        let handle_slots = self.handles.len().checked_mul(8).ok_or_else(|| {
            DispatchError::BackendError(
                "Fix: resident CSR queue batch scratch free handle count overflowed.".to_string(),
            )
        })?;
        let mut handles_to_free = Vec::new();
        reserve_graph_vec(
            &mut handles_to_free,
            handle_slots,
            "resident CSR queue batch scratch free handles",
        )?;
        for handles in self.handles.drain(..) {
            handles_to_free.extend([
                handles.frontier,
                handles.active_queue,
                handles.queue_len,
                handles.frontier_out,
            ]);
            if let Some(word_partials) = handles.word_partials {
                handles_to_free.push(word_partials);
            }
            if let Some(block_totals) = handles.block_totals {
                handles_to_free.push(block_totals);
            }
            if let Some(high_queue) = handles.high_queue {
                handles_to_free.push(high_queue);
            }
            if let Some(high_len) = handles.high_len {
                handles_to_free.push(high_len);
            }
        }
        let free_result = free_unique_resident_handles(
            dispatcher,
            &handles_to_free,
            "resident CSR queue batch scratch",
        );
        self.shape = None;
        self.program_shape = None;
        self.clear_frontier_out_program = None;
        self.queue_len_init_program = None;
        self.word_counts_program = None;
        self.word_block_offsets_program = None;
        self.queue_program = None;
        self.high_len_init_program = None;
        self.split_low_program = None;
        self.traverse_program = None;
        self.frontier_payloads.clear();
        self.readbacks.clear();
        self.clear_handle_sets.clear();
        self.queue_len_handle_sets.clear();
        self.word_count_handle_sets.clear();
        self.word_block_offsets_handle_sets.clear();
        self.queue_handle_sets.clear();
        self.atomic_word_queue_handle_sets.clear();
        self.word_prefix_queue_handle_sets.clear();
        self.traverse_handle_sets.clear();
        self.high_len_handle_sets.clear();
        self.split_low_handle_sets.clear();
        self.high_traverse_handle_sets.clear();
        self.read_ranges.clear();
        free_result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentCsrQueueBatchQueryHandles {
    frontier: u64,
    active_queue: u64,
    queue_len: u64,
    frontier_out: u64,
    word_partials: Option<u64>,
    block_totals: Option<u64>,
    high_queue: Option<u64>,
    high_len: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentCsrQueueBatchShape {
    batch_len: usize,
    frontier_bytes: usize,
    queue_capacity: u32,
    high_queue_capacity: u32,
    node_count: u32,
    materializer: ResidentCsrQueueMaterializer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResidentCsrQueueBatchProgramShape {
    queue_capacity: u32,
    allow_mask: u32,
    node_count: u32,
    edge_count: u32,
    materializer: ResidentCsrQueueMaterializer,
    traverse_kind: crate::graph::csr_frontier_queue_scratch::ResidentCsrQueueTraverseKind,
}
