use crate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};
use std::cell::{Cell, RefCell};
use vyre_foundation::ir::Program;

#[derive(Default)]
pub(super) struct RecordingResidentDispatcher {
    pub(super) next_handle: Cell<u64>,
    pub(super) alloc_count: Cell<usize>,
    pub(super) alloc_lengths: RefCell<Vec<usize>>,
    pub(super) resident_uploads: RefCell<Vec<(u64, usize)>>,
    pub(super) upload_handles: RefCell<Vec<Vec<u64>>>,
    pub(super) step_handles: RefCell<Vec<Vec<Vec<u64>>>>,
    pub(super) step_grids: RefCell<Vec<Vec<Option<[u32; 3]>>>>,
    pub(super) freed: RefCell<Vec<u64>>,
}

impl RecordingResidentDispatcher {
    pub(super) fn last_upload_handles(&self) -> Vec<u64> {
        self.upload_handles
            .borrow()
            .last()
            .cloned()
            .expect("Fix: test dispatcher expected at least one resident upload sequence")
    }

    pub(super) fn last_step_handles(&self) -> Vec<Vec<u64>> {
        self.step_handles
            .borrow()
            .last()
            .cloned()
            .expect("Fix: test dispatcher expected at least one resident dispatch sequence")
    }

    pub(super) fn last_step_grids(&self) -> Vec<Option<[u32; 3]>> {
        self.step_grids
            .borrow()
            .last()
            .cloned()
            .expect("Fix: test dispatcher expected at least one resident dispatch sequence")
    }

    pub(super) fn resident_alloc_lengths(&self) -> Vec<usize> {
        self.alloc_lengths.borrow().clone()
    }

    pub(super) fn resident_upload_lengths(&self) -> Vec<usize> {
        self.resident_uploads
            .borrow()
            .iter()
            .map(|(_, bytes)| *bytes)
            .collect()
    }

    pub(super) fn assert_no_resident_work(&self) {
        assert_eq!(
            self.alloc_count.get(),
            0,
            "zero-frontier fast paths must not allocate resident scratch"
        );
        assert!(
            self.upload_handles.borrow().is_empty(),
            "zero-frontier fast paths must not upload resident inputs"
        );
        assert!(
            self.step_handles.borrow().is_empty(),
            "zero-frontier fast paths must not dispatch resident kernels"
        );
    }
}

impl OptimizerDispatcher for RecordingResidentDispatcher {
    fn dispatch(
        &self,
        _program: &Program,
        _inputs: &[Vec<u8>],
        _grid_override: Option<[u32; 3]>,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        Err(DispatchError::Rejected(
            "Fix: recording dispatcher only supports resident sequence tests.".to_string(),
        ))
    }

    fn supports_persistent(&self) -> bool {
        true
    }

    fn alloc_resident(&self, byte_len: usize) -> Result<u64, DispatchError> {
        self.alloc_count.set(self.alloc_count.get() + 1);
        self.alloc_lengths.borrow_mut().push(byte_len);
        let handle = self.next_handle.get() + 1;
        self.next_handle.set(handle);
        Ok(handle)
    }

    fn free_resident(&self, handle: u64) -> Result<(), DispatchError> {
        self.freed.borrow_mut().push(handle);
        Ok(())
    }

    fn upload_resident(&self, handle: u64, bytes: &[u8]) -> Result<(), DispatchError> {
        self.resident_uploads
            .borrow_mut()
            .push((handle, bytes.len()));
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
}
