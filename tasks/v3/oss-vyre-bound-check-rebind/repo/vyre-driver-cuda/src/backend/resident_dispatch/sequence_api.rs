use smallvec::SmallVec;
use vyre_driver::BackendError;

use crate::backend::dispatch::CudaBackend;
use crate::backend::output_range::CudaOutputReadback;
use crate::backend::resident::{CudaResidentBuffer, ResidentViewCache};
use crate::backend::resident_dispatch::helpers::borrow_resident_sequence_output_slots;
use crate::backend::resident_dispatch_support::CudaResidentDispatchStep;
use crate::backend::staging_reserve::{reserve_smallvec, reserved_vec};

impl CudaBackend {
    pub(crate) fn dispatch_resident_sequence(
        &self,
        steps: &[CudaResidentDispatchStep<'_>],
    ) -> Result<(), BackendError> {
        self.dispatch_resident_sequence_read_many(steps, &[])
            .map(|_| ())
    }

    pub(crate) fn dispatch_resident_sequence_read_many(
        &self,
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        self.upload_resident_many_sequence_read_many(&[], steps, read_handles)
    }

    pub(crate) fn upload_resident_many_sequence_read_many(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = reserved_vec(read_handles.len(), "resident sequence read outputs")?;
        self.upload_resident_many_sequence_read_many_into(
            uploads,
            steps,
            read_handles,
            &mut outputs,
        )?;
        Ok(outputs)
    }

    pub(crate) fn upload_resident_many_sequence_read_many_into(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        let mut readbacks = SmallVec::<[CudaOutputReadback; 8]>::new();
        self.prepare_full_resident_readbacks(read_handles, &mut readbacks)?;
        self.upload_resident_many_sequence_read_ranges_into(
            uploads,
            steps,
            read_handles,
            &readbacks,
            outputs,
        )
    }

    pub(crate) fn clear_upload_resident_many_sequence_read_many_into(
        &self,
        clears: &[CudaResidentBuffer],
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        let mut fills = SmallVec::<[(CudaResidentBuffer, u8); 8]>::new();
        reserve_smallvec(&mut fills, clears.len(), "resident sequence clear fills")?;
        fills.extend(clears.iter().copied().map(|handle| (handle, 0)));
        self.fill_upload_resident_many_sequence_read_many_into(
            &fills,
            uploads,
            steps,
            read_handles,
            outputs,
        )
    }

    pub(crate) fn fill_upload_resident_many_sequence_read_many_into(
        &self,
        fills: &[(CudaResidentBuffer, u8)],
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        let mut readbacks = SmallVec::<[CudaOutputReadback; 8]>::new();
        self.prepare_full_resident_readbacks(read_handles, &mut readbacks)?;
        let mut borrowed_outputs =
            borrow_resident_sequence_output_slots(outputs, read_handles.len())?;
        self.fill_upload_resident_many_sequence_read_ranges_borrowed_into(
            fills,
            uploads,
            steps,
            read_handles,
            &readbacks,
            borrowed_outputs.as_mut_slice(),
        )
    }

    fn prepare_full_resident_readbacks(
        &self,
        read_handles: &[CudaResidentBuffer],
        readbacks: &mut SmallVec<[CudaOutputReadback; 8]>,
    ) -> Result<(), BackendError> {
        reserve_smallvec(
            readbacks,
            read_handles.len(),
            "resident sequence full readbacks",
        )?;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            read_handles.len(),
            "resident sequence full-readback view cache",
        )?;
        readbacks.clear();
        for &handle in read_handles {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident sequence full-readback view cache",
            )?;
            readbacks.push(CudaOutputReadback {
                device_offset: 0,
                byte_len: buffer.byte_len,
            });
        }
        Ok(())
    }

    pub(crate) fn upload_resident_many_sequence_read_ranges_into(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        let mut borrowed_outputs =
            borrow_resident_sequence_output_slots(outputs, read_handles.len())?;
        self.upload_resident_many_sequence_read_ranges_borrowed_into(
            uploads,
            steps,
            read_handles,
            readbacks,
            borrowed_outputs.as_mut_slice(),
        )
    }

    pub(crate) fn upload_resident_many_sequence_read_ranges_borrowed_into(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        self.fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into(
            &[],
            uploads,
            steps,
            &[],
            0,
            read_handles,
            readbacks,
            outputs,
        )
    }

    pub(crate) fn fill_upload_resident_many_sequence_read_ranges_borrowed_into(
        &self,
        fills: &[(CudaResidentBuffer, u8)],
        uploads: &[(CudaResidentBuffer, &[u8])],
        steps: &[CudaResidentDispatchStep<'_>],
        read_handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        self.fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into(
            fills,
            uploads,
            steps,
            &[],
            0,
            read_handles,
            readbacks,
            outputs,
        )
    }

    pub(crate) fn upload_resident_many_repeated_sequence_read_ranges_borrowed_into(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
        prefix_steps: &[CudaResidentDispatchStep<'_>],
        repeated_steps: &[CudaResidentDispatchStep<'_>],
        repeat_count: usize,
        read_handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        self.fill_upload_resident_many_repeated_sequence_read_ranges_borrowed_into(
            &[],
            uploads,
            prefix_steps,
            repeated_steps,
            repeat_count,
            read_handles,
            readbacks,
            outputs,
        )
    }
}
