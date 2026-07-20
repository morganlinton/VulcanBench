use super::*;
use crate::megakernel::readback::MegakernelReadback;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use vyre_driver::backend::OutputBuffers;
use vyre_foundation::ir::{Ident, Node};
use vyre_foundation::memory_model::MemoryOrdering;

#[derive(Default)]
struct GridSyncBackend {
    segment_lengths: Mutex<Vec<usize>>,
}

impl vyre_driver::backend::private::Sealed for GridSyncBackend {}

impl VyreBackend for GridSyncBackend {
    fn id(&self) -> &'static str {
        "grid-sync-recording"
    }

    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(inputs.to_vec())
    }

    fn dispatch_borrowed(
        &self,
        program: &Program,
        inputs: &[&[u8]],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        let entry = program.entry();
        let segment_len = match entry {
            [Node::Region { body, .. }] => body.len(),
            other => other.len(),
        };
        self.segment_lengths
            .lock()
            .expect("Fix: grid-sync recording mutex must not be poisoned")
            .push(segment_len);
        Ok(inputs.iter().map(|input| input.to_vec()).collect())
    }
}

#[derive(Default)]
struct PersistentHandleBackend {
    calls: Arc<Mutex<Vec<[u64; 4]>>>,
    row_batch_calls: Arc<AtomicUsize>,
}

impl vyre_driver::backend::private::Sealed for PersistentHandleBackend {}

impl VyreBackend for PersistentHandleBackend {
    fn id(&self) -> &'static str {
        "persistent-handle-recording"
    }

    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Err(vyre_driver::BackendError::new(
                "host-byte dispatch should not run. Fix: route resident handles through dispatch_persistent_handles.",
            ))
    }

    fn compile_native(
        &self,
        _program: &Program,
        _config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, vyre_driver::BackendError> {
        Ok(Some(Arc::new(PersistentHandlePipeline {
            calls: Arc::clone(&self.calls),
            row_batch_calls: Arc::clone(&self.row_batch_calls),
        })))
    }
}

struct PersistentHandlePipeline {
    calls: Arc<Mutex<Vec<[u64; 4]>>>,
    row_batch_calls: Arc<AtomicUsize>,
}

impl vyre_driver::backend::private::Sealed for PersistentHandlePipeline {}

impl CompiledPipeline for PersistentHandlePipeline {
    fn id(&self) -> &str {
        "persistent-handle-recording:pipeline"
    }

    fn dispatch(
        &self,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Err(vyre_driver::BackendError::new(
            "host-byte compiled dispatch should not run. Fix: use persistent handles.",
        ))
    }

    fn dispatch_persistent_handles(
        &self,
        inputs: &[Resource],
        _config: &DispatchConfig,
    ) -> Result<OutputBuffers, vyre_driver::BackendError> {
        let handles: Vec<u64> = inputs
            .iter()
            .map(|resource| match resource {
                Resource::Resident(handle) => *handle,
                Resource::Borrowed(_) => 0,
            })
            .collect();
        let handles: [u64; 4] = handles.try_into().map_err(|_| {
                vyre_driver::BackendError::new(
                    "persistent handle ABI requires exactly four resources. Fix: pass control, ring, debug_log, and io_queue handles.",
                )
            })?;
        self.calls
            .lock()
            .expect("Fix: persistent-handle recording mutex must not be poisoned")
            .push(handles);
        Ok(vec![vec![1, 2, 3, 4]])
    }

    fn dispatch_persistent_handles_into(
        &self,
        inputs: &[Resource],
        _config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        let handles: Vec<u64> = inputs
            .iter()
            .map(|resource| match resource {
                Resource::Resident(handle) => *handle,
                Resource::Borrowed(_) => 0,
            })
            .collect();
        let handles: [u64; 4] = handles.try_into().map_err(|_| {
            vyre_driver::BackendError::new(
                "persistent handle ABI requires exactly four resources. Fix: pass control, ring, debug_log, and io_queue handles.",
            )
        })?;
        self.calls
            .lock()
            .expect("Fix: persistent-handle recording mutex must not be poisoned")
            .push(handles);
        if outputs.is_empty() {
            outputs.push(Vec::new());
        } else {
            outputs.truncate(1);
        }
        outputs[0].clear();
        outputs[0].extend_from_slice(&[1, 2, 3, 4]);
        Ok(())
    }

    fn dispatch_persistent_handles_batched(
        &self,
        batches: &[&[Resource]],
        _config: &DispatchConfig,
    ) -> Result<Vec<OutputBuffers>, vyre_driver::BackendError> {
        let mut outputs = Vec::with_capacity(batches.len());
        for (index, inputs) in batches.iter().enumerate() {
            let handles: Vec<u64> = inputs
                .iter()
                .map(|resource| match resource {
                    Resource::Resident(handle) => *handle,
                    Resource::Borrowed(_) => 0,
                })
                .collect();
            let handles: [u64; 4] = handles.try_into().map_err(|_| {
                    vyre_driver::BackendError::new(
                        "batched persistent handle ABI requires exactly four resources. Fix: pass control, ring, debug_log, and io_queue handles.",
                    )
                })?;
            self.calls
                .lock()
                .expect("Fix: persistent-handle recording mutex must not be poisoned")
                .push(handles);
            outputs.push(vec![vec![u8::try_from(index).unwrap_or(u8::MAX)]]);
        }
        Ok(outputs)
    }

    fn dispatch_persistent_handles_batched_into(
        &self,
        batches: &[&[Resource]],
        _config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), vyre_driver::BackendError> {
        if outputs.len() < batches.len() {
            outputs.resize_with(batches.len(), Vec::new);
        } else {
            outputs.truncate(batches.len());
        }
        for (index, (inputs, row)) in batches.iter().zip(outputs.iter_mut()).enumerate() {
            let handles: Vec<u64> = inputs
                .iter()
                .map(|resource| match resource {
                    Resource::Resident(handle) => *handle,
                    Resource::Borrowed(_) => 0,
                })
                .collect();
            let handles: [u64; 4] = handles.try_into().map_err(|_| {
                vyre_driver::BackendError::new(
                    "batched persistent handle ABI requires exactly four resources. Fix: pass control, ring, debug_log, and io_queue handles.",
                )
            })?;
            self.calls
                .lock()
                .expect("Fix: persistent-handle recording mutex must not be poisoned")
                .push(handles);
            if row.is_empty() {
                row.push(Vec::new());
            } else {
                row.truncate(1);
            }
            row[0].clear();
            row[0].push(u8::try_from(index).unwrap_or(u8::MAX));
        }
        Ok(())
    }

    fn dispatch_persistent_handle_rows_into(
        &self,
        rows: &[[Resource; 4]],
        _config: &DispatchConfig,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), vyre_driver::BackendError> {
        self.row_batch_calls.fetch_add(1, Ordering::SeqCst);
        if outputs.len() < rows.len() {
            outputs.resize_with(rows.len(), Vec::new);
        } else {
            outputs.truncate(rows.len());
        }
        for (index, (inputs, row)) in rows.iter().zip(outputs.iter_mut()).enumerate() {
            let handles = std::array::from_fn(|index| match &inputs[index] {
                Resource::Resident(handle) => *handle,
                Resource::Borrowed(_) => 0,
            });
            self.calls
                .lock()
                .expect("Fix: persistent-handle recording mutex must not be poisoned")
                .push(handles);
            if row.is_empty() {
                row.push(Vec::new());
            } else {
                row.truncate(1);
            }
            row[0].clear();
            row[0].push(u8::try_from(index).unwrap_or(u8::MAX));
        }
        Ok(())
    }
}

struct EchoPipeline;

impl vyre_driver::backend::private::Sealed for EchoPipeline {}

impl CompiledPipeline for EchoPipeline {
    fn id(&self) -> &str {
        "echo:pipeline"
    }

    fn dispatch(
        &self,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(inputs.to_vec())
    }

    fn dispatch_borrowed(
        &self,
        inputs: &[&[u8]],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(inputs.iter().map(|input| input.to_vec()).collect())
    }
}

struct EchoBackend;

impl vyre_driver::backend::private::Sealed for EchoBackend {}

impl VyreBackend for EchoBackend {
    fn id(&self) -> &'static str {
        "echo"
    }

    fn dispatch(
        &self,
        _program: &Program,
        inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Ok(inputs.to_vec())
    }

    fn compile_native(
        &self,
        _program: &Program,
        _config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, vyre_driver::BackendError> {
        Ok(Some(Arc::new(EchoPipeline)))
    }
}

struct RecoveringBackend {
    dispatch_calls: Arc<AtomicUsize>,
    expected_outputs_addr: usize,
    expected_slot_addr: usize,
}

impl vyre_driver::backend::private::Sealed for RecoveringBackend {}

impl VyreBackend for RecoveringBackend {
    fn id(&self) -> &'static str {
        "recovering"
    }

    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Err(vyre_driver::BackendError::new(
            "host-byte dispatch should not run. Fix: compile_native must provide the recovering pipeline.",
        ))
    }

    fn compile_native(
        &self,
        _program: &Program,
        _config: &DispatchConfig,
    ) -> Result<Option<Arc<dyn CompiledPipeline>>, vyre_driver::BackendError> {
        Ok(Some(Arc::new(RecoveringPipeline {
            dispatch_calls: Arc::clone(&self.dispatch_calls),
            expected_outputs_addr: self.expected_outputs_addr,
            expected_slot_addr: self.expected_slot_addr,
        })))
    }
}

struct RecoveringPipeline {
    dispatch_calls: Arc<AtomicUsize>,
    expected_outputs_addr: usize,
    expected_slot_addr: usize,
}

impl vyre_driver::backend::private::Sealed for RecoveringPipeline {}

impl CompiledPipeline for RecoveringPipeline {
    fn id(&self) -> &str {
        "recovering:pipeline"
    }

    fn dispatch(
        &self,
        _inputs: &[Vec<u8>],
        _config: &DispatchConfig,
    ) -> Result<Vec<Vec<u8>>, vyre_driver::BackendError> {
        Err(vyre_driver::BackendError::new(
            "host-byte compiled dispatch should not run. Fix: dispatch_borrowed_into must be used.",
        ))
    }

    fn dispatch_borrowed_into(
        &self,
        _inputs: &[&[u8]],
        _config: &DispatchConfig,
        outputs: &mut OutputBuffers,
    ) -> Result<(), vyre_driver::BackendError> {
        let call = self.dispatch_calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Err(vyre_driver::BackendError::new(
                "device lost during test dispatch. Fix: recover and retry without discarding caller-owned output storage.",
            ));
        }
        assert_eq!(outputs.as_ptr() as usize, self.expected_outputs_addr);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].as_ptr() as usize, self.expected_slot_addr);
        outputs[0].clear();
        outputs[0].extend_from_slice(&[9, 8, 7, 6]);
        Ok(())
    }
}

fn grid_sync_program() -> Program {
    let base = super::super::builder::build_program_sharded_slots(1, 1, &[]);
    base.with_rewritten_entry(vec![Node::Region {
        generator: Ident::from("grid_sync_test"),
        source_region: None,
        body: Arc::new(vec![
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            },
            Node::Return,
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
            },
            Node::Return,
        ]),
    }])
}

#[path = "grid_sync_contracts.rs"]
mod grid_sync_contracts;
#[path = "persistent_single_contracts.rs"]
mod persistent_single_contracts;
#[path = "persistent_batch_contracts.rs"]
mod persistent_batch_contracts;
#[path = "readback_contracts.rs"]
mod readback_contracts;
#[path = "recovery_contracts.rs"]
mod recovery_contracts;
