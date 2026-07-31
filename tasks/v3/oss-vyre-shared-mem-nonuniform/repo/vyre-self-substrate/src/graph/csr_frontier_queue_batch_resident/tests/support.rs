use crate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};
use std::cell::{Cell, RefCell};
use vyre_foundation::ir::Program;

#[derive(Default)]
pub(super) struct RecordingBatchDispatcher {
    pub(super) next_handle: Cell<u64>,
    pub(super) upload_handles: RefCell<Vec<Vec<u64>>>,
    pub(super) step_handles: RefCell<Vec<Vec<Vec<u64>>>>,
    pub(super) step_grids: RefCell<Vec<Vec<Option<[u32; 3]>>>>,
    pub(super) freed: RefCell<Vec<u64>>,
}

impl OptimizerDispatcher for RecordingBatchDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: batch resident queue tests should not use non-resident dispatch.".to_string(),
        ))
    }

    fn alloc_resident(&self, _byte_len: usize) -> Result<u64, DispatchError> {
        let handle = self.next_handle.get() + 1;
        self.next_handle.set(handle);
        Ok(handle)
    }

    fn upload_resident_many(&self, _uploads: &[(u64, &[u8])]) -> Result<(), DispatchError> {
        Ok(())
    }

    fn upload_resident_many_sequence_read_ranges_into(
        &self,
        uploads: &[(u64, &[u8])],
        steps: &[ResidentDispatchStep<'_>],
        read_ranges: &[ResidentReadRange],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), DispatchError> {
        self.upload_handles
            .borrow_mut()
            .push(uploads.iter().map(|(handle, _)| *handle).collect());
        self.step_handles
            .borrow_mut()
            .push(steps.iter().map(|step| step.handle_ids.to_vec()).collect());
        self.step_grids
            .borrow_mut()
            .push(steps.iter().map(|step| step.grid_override).collect());
        outputs.clear();
        outputs.extend(read_ranges.iter().map(|range| vec![0u8; range.byte_len]));
        Ok(())
    }

    fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
        self.freed.borrow_mut().push(handle);
        Ok(())
    }
}
