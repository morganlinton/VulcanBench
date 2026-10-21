//! Persistent-kernel queue execution for wgpu compute pipelines.
//!
//! This module provides host-side queue management and GPU dispatch for
//! persistent kernels. A persistent pipeline is compiled once and reused
//! across multiple work items, with only buffer contents changing between
//! calls. If the wgpu device is lost, the backend recovers and the pipeline
//! is recompiled automatically.

use std::sync::Arc;

use smallvec::SmallVec;
use vyre_driver::BackendError;
use vyre_driver::DispatchConfig;
use vyre_driver::VyreBackend;
use vyre_foundation::ir::Program;

/// One unit of persistent-kernel work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentPayloadWorkItem {
    /// Stable work identifier.
    pub id: u32,
    /// Input payload consumed by the resident kernel.
    pub payload: Vec<u8>,
}

/// Output produced for one persistent-kernel work item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkResult {
    /// Stable work identifier copied from the input item.
    pub id: u32,
    /// Output payload produced by the kernel body.
    pub payload: Vec<u8>,
}

/// GPU-side queue contract for persistent kernels.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PersistentQueue {
    items: SmallVec<[PersistentPayloadWorkItem; 16]>,
}

impl PersistentQueue {
    /// Create an empty persistent work queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: SmallVec::new(),
        }
    }

    /// Enqueue one work item.
    pub fn push(&mut self, item: PersistentPayloadWorkItem) {
        self.items.push(item);
    }

    /// Number of queued work items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true when the queue contains no work.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Resident-kernel execution summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentKernelReport {
    /// Number of kernel launches used to drain the queue.
    pub kernel_launches: u32,
    /// Results in queue order.
    pub results: Vec<WorkResult>,
}

/// Compile a persistent pipeline and drain `queue` through the GPU.
///
/// The pipeline and bind-group layout are created once and reused for every
/// work item. Only the input/output buffer contents change between launches.
/// If the backend reports device loss before or during drainage, recovery is
/// attempted and the pipeline is recompiled automatically.
///
/// # Errors
///
/// Returns [`BackendError`] when queue validation, pipeline compilation,
/// device recovery, or GPU dispatch fails.
pub fn run_persistent_kernel(
    backend: &crate::WgpuBackend,
    program: &Program,
    config: &DispatchConfig,
    queue: PersistentQueue,
) -> Result<PersistentKernelReport, BackendError> {
    if queue.is_empty() {
        return Ok(PersistentKernelReport {
            kernel_launches: 0,
            results: Vec::new(),
        });
    }

    let _work_items = u32::try_from(queue.items.len()).map_err(|_| {
        BackendError::new(
            "persistent queue length exceeds u32 GPU counters. Fix: shard work into multiple queues.",
        )
    })?;

    let pipeline = ensure_persistent_pipeline(backend, program, config)?;
    let mut payloads = SmallVec::<[&[u8]; 16]>::new();
    payloads.try_reserve(queue.items.len()).map_err(|source| {
        BackendError::new(format!(
            "persistent kernel could not reserve {} payload slice reference(s): {source}. Fix: split the persistent queue before dispatch.",
            queue.items.len()
        ))
    })?;
    payloads.extend(queue.items.iter().map(|item| item.payload.as_slice()));
    let output_batches = pipeline
        .dispatch_coalesced_borrowed(&payloads, config)
        .map_err(|error| {
            BackendError::new(format!(
                "persistent kernel coalesced dispatch failed for {} work items: {error}. Fix: verify the program and queued input payloads are compatible.",
                queue.items.len()
            ))
        })?;
    drop(payloads);
    if output_batches.len() != queue.items.len() {
        return Err(BackendError::new(format!(
            "persistent kernel returned {} output batch(es) for {} queued work item(s). Fix: keep coalesced dispatch output cardinality identical to queue length.",
            output_batches.len(),
            queue.items.len()
        )));
    }

    let mut results = Vec::new();
    results.try_reserve(queue.items.len()).map_err(|source| {
        BackendError::new(format!(
            "persistent kernel could not reserve {} work result slot(s): {source}. Fix: split the persistent queue before collecting outputs.",
            queue.items.len()
        ))
    })?;
    for (item_index, (item, outputs)) in queue.items.into_iter().zip(output_batches).enumerate() {
        if outputs.len() != 1 {
            return Err(BackendError::new(format!(
                "persistent kernel work item index {item_index} id={} returned {} output buffer(s); WorkResult requires exactly one payload. Fix: use a persistent program with one public output or extend PersistentKernelReport to carry multi-output results explicitly.",
                item.id,
                outputs.len()
            )));
        }
        let mut outputs = outputs.into_iter();
        let Some(payload) = outputs.next() else {
            return Err(BackendError::new(format!(
                "persistent kernel work item index {item_index} id={} returned no payload after output cardinality validation. Fix: keep persistent output extraction synchronized with the one-output WorkResult contract.",
                item.id
            )));
        };
        results.push(WorkResult {
            id: item.id,
            payload,
        });
    }

    Ok(PersistentKernelReport {
        kernel_launches: 1,
        results,
    })
}

fn ensure_persistent_pipeline(
    backend: &crate::WgpuBackend,
    program: &Program,
    config: &DispatchConfig,
) -> Result<Arc<crate::pipeline::WgpuPipeline>, BackendError> {
    if backend.device_lost() {
        return recover_and_recompile(backend, program, config);
    }
    backend.compile_persistent(program, config)
}

fn recover_and_recompile(
    backend: &crate::WgpuBackend,
    program: &Program,
    config: &DispatchConfig,
) -> Result<Arc<crate::pipeline::WgpuPipeline>, BackendError> {
    backend.try_recover().map_err(|error| {
        BackendError::new(format!(
            "persistent kernel dispatch encountered device loss and recovery failed: {error}. Fix: ensure the GPU adapter is available before dispatching persistent work."
        ))
    })?;
    backend.compile_persistent(program, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WgpuBackend;
    use vyre_driver::DispatchConfig;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

    fn add_one_program(words: u32) -> Program {
        let idx = Expr::gid_x();
        let in_bounds = Expr::lt(idx.clone(), Expr::u32(words));
        Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(words),
                BufferDecl::output("out", 1, DataType::U32)
                    .with_count(words)
                    .with_output_byte_range(0..(words as usize * 4)),
            ],
            [64, 1, 1],
            vec![
                Node::if_then(
                    in_bounds,
                    vec![Node::store(
                        "out",
                        idx.clone(),
                        Expr::add(Expr::load("input", idx), Expr::u32(1)),
                    )],
                ),
                Node::return_(),
            ],
        )
    }

    #[test]
    fn persistent_kernel_queue_dispatches_on_gpu() {
        let backend =
            WgpuBackend::acquire().expect("Fix: GPU must be available for persistent kernel test");
        let program = add_one_program(1);
        let mut queue = PersistentQueue::new();
        for id in 0..16 {
            queue.push(PersistentPayloadWorkItem {
                id,
                payload: (id + 100).to_le_bytes().to_vec(),
            });
        }

        let report = run_persistent_kernel(&backend, &program, &DispatchConfig::default(), queue)
            .expect("Fix: persistent kernel dispatch must succeed");

        assert_eq!(report.kernel_launches, 1);
        assert_eq!(report.results.len(), 16);
        for result in &report.results {
            let input_val = result.id + 100;
            let expected = input_val + 1;
            let actual = u32::from_le_bytes([
                result.payload[0],
                result.payload[1],
                result.payload[2],
                result.payload[3],
            ]);
            assert_eq!(
                actual, expected,
                "work item {}: expected {}, got {}",
                result.id, expected, actual
            );
        }
    }

    #[test]
    fn persistent_kernel_survives_device_loss_recovery() {
        let backend = WgpuBackend::acquire()
            .expect("Fix: GPU must be available for persistent kernel recovery test");
        let program = add_one_program(1);
        let mut queue = PersistentQueue::new();
        queue.push(PersistentPayloadWorkItem {
            id: 0,
            payload: 42u32.to_le_bytes().to_vec(),
        });

        // First dispatch.
        let report1 = run_persistent_kernel(
            &backend,
            &program,
            &DispatchConfig::default(),
            queue.clone(),
        )
        .expect("Fix: first persistent dispatch must succeed");
        assert_eq!(report1.results.len(), 1);
        let actual1 = u32::from_le_bytes([
            report1.results[0].payload[0],
            report1.results[0].payload[1],
            report1.results[0].payload[2],
            report1.results[0].payload[3],
        ]);
        assert_eq!(actual1, 43);

        // Simulate device loss.
        backend
            .force_device_lost()
            .expect("Fix: test hook must invalidate the cached device");
        assert!(backend.device_lost());

        // Recovery should happen automatically inside run_persistent_kernel.
        let report2 = run_persistent_kernel(&backend, &program, &DispatchConfig::default(), queue)
            .expect("Fix: persistent dispatch after recovery must succeed");
        assert_eq!(report2.results.len(), 1);
        let actual2 = u32::from_le_bytes([
            report2.results[0].payload[0],
            report2.results[0].payload[1],
            report2.results[0].payload[2],
            report2.results[0].payload[3],
        ]);
        assert_eq!(actual2, 43);
    }
}
