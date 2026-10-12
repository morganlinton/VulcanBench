use std::ffi::c_void;
use std::hash::BuildHasherDefault;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crossbeam_queue::ArrayQueue;
use cudarc::driver::sys::CUresult;
use dashmap::DashMap;
use rustc_hash::FxHasher;
use smallvec::SmallVec;
use vyre_driver::accounting::{
    checked_add_usize_lazy, checked_atomic_add_usize_guarded_with_order,
    checked_atomic_add_usize_with_order, checked_atomic_sub_usize,
    repair_atomic_sub_usize_with_order,
};
use vyre_driver::BackendError;

use super::host_memory;
use super::staging_reserve::{reserve_smallvec, reserve_vec, resize_vec_slots};

pub(crate) fn cuda_check(result: CUresult, operation: &str) -> Result<(), BackendError> {
    if result == CUresult::CUDA_SUCCESS {
        return Ok(());
    }
    Err(BackendError::DispatchFailed {
        code: Some(cuda_result_code(result)),
        message: format!("{operation} failed with {result:?}"),
    })
}

pub(crate) fn cuda_result_code(result: CUresult) -> i32 {
    result as i32
}

pub(crate) fn alloc_cuda_ptr(byte_len: usize, operation: &str) -> Result<u64, BackendError> {
    if byte_len == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {operation} cannot allocate zero device bytes through cuMemAlloc_v2. Keep zero-sized CUDA buffers as null sentinels or allocate at least one byte when a captured graph needs a stable address."
            ),
        });
    }
    let mut ptr = 0u64;
    // SAFETY: FFI to libcuda.so cuMemAlloc_v2. &mut ptr is a valid
    // *mut CUdeviceptr output parameter and byte_len is non-zero by the
    // guard above. cuda_check propagates non-success CUresult values.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemAlloc_v2(&mut ptr, byte_len),
            operation,
        )?;
    }
    if ptr == 0 {
        return Err(BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{operation} returned a null device pointer after reporting success for {byte_len} byte(s). Fix: update the CUDA driver or avoid this allocation shape."
            ),
        });
    }
    Ok(ptr)
}

#[derive(Debug)]
pub(crate) struct DispatchAllocations {
    pool: Arc<DeviceAllocationPool>,
    ptrs: SmallVec<[DeviceAllocation; 8]>,
    params: DeviceAllocation,
}

impl DispatchAllocations {
    pub(crate) fn new(
        buffer_count: usize,
        pool: Arc<DeviceAllocationPool>,
    ) -> Result<Self, BackendError> {
        let mut ptrs = SmallVec::new();
        reserve_smallvec(&mut ptrs, buffer_count, "dispatch allocation pointer")?;
        ptrs.extend((0..buffer_count).map(|_| DeviceAllocation::default()));
        Ok(Self {
            pool,
            ptrs,
            params: DeviceAllocation::default(),
        })
    }

    pub(crate) fn set_ptr(
        &mut self,
        index: usize,
        allocation: DeviceAllocation,
        context: &str,
    ) -> Result<(), BackendError> {
        let allocation_count = self.ptrs.len();
        let slot = self
            .ptrs
            .get_mut(index)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA dispatch allocation table {context} expected buffer index {index} but only {allocation_count} allocation slot(s) exist. Rebuild the binding plan before launch.",
                ),
            })?;
        *slot = allocation;
        Ok(())
    }

    pub(crate) fn ptr(&self, index: usize, context: &str) -> Result<u64, BackendError> {
        self.ptrs
            .get(index)
            .map(|allocation| allocation.ptr)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA dispatch allocation table {context} expected buffer index {index} but only {} allocation slot(s) exist. Rebuild the binding plan before launch.",
                    self.ptrs.len()
                ),
            })
    }

    pub(crate) fn byte_len(&self, index: usize, context: &str) -> Result<usize, BackendError> {
        self.ptrs
            .get(index)
            .map(|allocation| allocation.byte_len)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA dispatch allocation table {context} expected buffer index {index} but only {} allocation slot(s) exist. Rebuild the binding plan before launch.",
                    self.ptrs.len()
                ),
            })
    }

    pub(crate) fn set_params(&mut self, allocation: DeviceAllocation) {
        self.params = allocation;
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PinnedHostAllocation {
    ptr: *mut u8,
    pub(crate) byte_len: usize,
}

// SAFETY: PinnedHostAllocation owns a CUDA-pinned host pointer that
// is valid across threads. The pinned-host page is allocated by
// cuMemHostAlloc with CU_MEMHOSTALLOC_PORTABLE so it is addressable
// from every CUDA context on this process; the Rust-level
// PinnedHostAllocationPool synchronises bucket-cache access with
// DashMap + bounded ArrayQueue so concurrent take/release is safe. Send + Sync
// are sound because no thread can produce a torn read of the raw
// pointer (it is just an address) or the byte_len.
unsafe impl Send for PinnedHostAllocation {}
// SAFETY: see the Send impl above  -  same reasoning applies for
// shared (&) access; PinnedHostAllocation is Copy and never holds
// thread-local state.
unsafe impl Sync for PinnedHostAllocation {}

impl Default for PinnedHostAllocation {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            byte_len: 0,
        }
    }
}

impl PinnedHostAllocation {
    pub(crate) fn as_ptr(&self) -> *const c_void {
        self.ptr.cast()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut c_void {
        self.ptr.cast()
    }

    pub(crate) fn copy_from_slice(&mut self, bytes: &[u8]) -> Result<(), BackendError> {
        if bytes.len() > self.byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host upload attempted to copy {} byte(s) into a {} byte allocation. Recompute transfer sizing before enqueueing DMA.",
                    bytes.len(),
                    self.byte_len
                ),
            });
        }
        if bytes.is_empty() {
            return Ok(());
        }
        // SAFETY: bytes.as_ptr() is a valid &[u8] source for bytes.len()
        // bytes; self.ptr is a CUDA-pinned host allocation of self.byte_len
        // bytes (checked above proves bytes.len() ≤ self.byte_len);
        // pinned-host memory and stack/heap memory cannot overlap so
        // copy_nonoverlapping's non-aliasing precondition holds.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr, bytes.len());
        }
        Ok(())
    }

    pub(crate) fn zero_range(
        &mut self,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<(), BackendError> {
        let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
            byte_offset,
            byte_len,
            self.byte_len,
            || {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host zero-fill overflowed usize at offset {byte_offset} len {byte_len}."
                ),
            }
            },
            |end| {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host zero-fill requested byte range [{byte_offset}..{end}) from a {} byte allocation.",
                    self.byte_len
                ),
            }
            },
        )?;
        if end == byte_offset {
            return Ok(());
        }
        // SAFETY: checked range validation proves byte_offset..end lies
        // inside this pinned-host allocation; &mut self prevents
        // concurrent readers while the DMA padding tail is initialised.
        unsafe {
            std::ptr::write_bytes(self.ptr.add(byte_offset), 0, byte_len);
        }
        Ok(())
    }

    pub(crate) fn copy_u32_le_words(&mut self, words: &[u32]) -> Result<(), BackendError> {
        let byte_len = std::mem::size_of_val(words);
        if byte_len > self.byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host u32 upload attempted to copy {byte_len} byte(s) into a {} byte allocation. Recompute parameter staging size before launch.",
                    self.byte_len
                ),
            });
        }
        if byte_len == 0 {
            return Ok(());
        }
        #[cfg(target_endian = "little")]
        // SAFETY: same as copy_from_slice  -  words.as_ptr() is a valid
        // &[u32] source for byte_len bytes (size_of_val); self.ptr owns
        // self.byte_len ≥ byte_len bytes of pinned-host memory by the
        // checked guard above; cast to
        // u8 is safe because u32 → u8 narrowing of a pointer reads the
        // same address space.
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr().cast::<u8>(), self.ptr, byte_len);
        }
        #[cfg(not(target_endian = "little"))]
        {
            // SAFETY: self.ptr is a valid pinned-host allocation of
            // self.byte_len ≥ byte_len bytes (debug_assert above) and is
            // not aliased while we hold &mut self.
            let dst = unsafe { std::slice::from_raw_parts_mut(self.ptr, byte_len) };
            for (chunk, word) in dst.chunks_exact_mut(4).zip(words) {
                chunk.copy_from_slice(&word.to_le_bytes());
            }
        }
        Ok(())
    }

    pub(crate) fn copy_prefix_into(
        &self,
        byte_len: usize,
        dst: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        if byte_len > self.byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host readback attempted to copy {byte_len} byte(s) from a {} byte allocation. Recompute output transfer sizing before collecting results.",
                    self.byte_len
                ),
            });
        }
        copy_raw_bytes_into_vec(self.ptr, byte_len, dst)
    }

    pub(crate) fn copy_range_into(
        &self,
        byte_offset: usize,
        byte_len: usize,
        dst: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
            byte_offset,
            byte_len,
            self.byte_len,
            || {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host readback range overflowed usize at offset {byte_offset} len {byte_len}. Recompute output transfer slicing before collecting results."
                ),
            }
            },
            |end| {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA pinned-host readback attempted to copy byte range [{byte_offset}..{end}) from a {} byte allocation. Recompute fused output transfer slicing before collecting results.",
                    self.byte_len
                ),
            }
            },
        )?;
        if byte_len == 0 {
            dst.clear();
            return Ok(());
        }
        // SAFETY: checked_usize_byte_range_end_lazy above proved
        // byte_offset..byte_offset + byte_len is inside this allocation, and
        // zero-byte ranges returned before pointer arithmetic.
        let src = unsafe { self.ptr.add(byte_offset) };
        copy_raw_bytes_into_vec(src, byte_len, dst)
    }
}

fn copy_raw_bytes_into_vec(
    src: *const u8,
    byte_len: usize,
    dst: &mut Vec<u8>,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        dst.clear();
        return Ok(());
    }
    if dst.capacity() < byte_len {
        reserve_vec(dst, byte_len, "CUDA readback output bytes")?;
    }
    dst.clear();
    // SAFETY: src is a non-null pointer to byte_len readable bytes
    // (caller's contract  -  every internal call site passes a CUDA-host
    // allocation pointer). dst.as_mut_ptr() points to dst's owned
    // capacity which is ≥ byte_len after the fallible reservation above.
    // dst is freshly cleared so set_len(byte_len) leaves the new
    // contents initialised by the copy.
    unsafe {
        std::ptr::copy_nonoverlapping(src, dst.as_mut_ptr(), byte_len);
        dst.set_len(byte_len);
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PinnedHostAllocationPool {
    free: DashMap<usize, ArrayQueue<usize>, BuildHasherDefault<FxHasher>>,
    cached_bytes: AtomicUsize,
    max_cached_bytes: usize,
}

impl PinnedHostAllocationPool {
    pub(crate) fn new(max_cached_bytes: usize) -> Self {
        Self {
            free: DashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            cached_bytes: AtomicUsize::new(0),
            max_cached_bytes,
        }
    }

    pub(crate) fn acquire(&self, byte_len: usize) -> Result<PinnedHostAllocation, BackendError> {
        let bucket = allocation_bucket(byte_len, "CUDA pinned host allocation")?;
        if let Some(ptr) = self.take_cached(bucket)? {
            return Ok(PinnedHostAllocation {
                ptr: ptr as *mut u8,
                byte_len: bucket,
            });
        }
        self.free.entry(bucket).or_insert_with(|| {
            ArrayQueue::new(allocation_bucket_cache_slots(bucket, self.max_cached_bytes))
        });
        let ptr = host_memory::alloc_pinned_host_buffer(bucket, "cuMemHostAlloc")?;
        Ok(PinnedHostAllocation {
            ptr: ptr.cast(),
            byte_len: bucket,
        })
    }

    pub(crate) fn clear(&self) -> Result<(), BackendError> {
        for entry in &self.free {
            while let Some(ptr) = entry.value().pop() {
                host_memory::free_pinned_host_buffer(
                    ptr as *mut c_void,
                    "cuMemFreeHost (pinned host pool clear)",
                );
            }
        }
        self.free.clear();
        self.cached_bytes.store(0, Ordering::Release);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn cached_bytes(&self) -> usize {
        self.cached_bytes.load(Ordering::Acquire)
    }

    fn take_cached(&self, bucket: usize) -> Result<Option<usize>, BackendError> {
        let Some(queue) = self.free.get(&bucket) else {
            return Ok(None);
        };
        let Some(ptr) = queue.pop() else {
            return Ok(None);
        };
        subtract_cached_bytes_or_repair(
            &self.cached_bytes,
            bucket,
            "CUDA pinned-host allocation-pool cached bytes",
        );
        Ok(Some(ptr))
    }

    pub(crate) fn release(&self, allocation: PinnedHostAllocation) {
        if allocation.ptr.is_null() || allocation.byte_len == 0 {
            return;
        }
        let Some(queue) = self.free.get(&allocation.byte_len) else {
            host_memory::free_pinned_host_buffer(
                allocation.ptr.cast(),
                "cuMemFreeHost (pinned host pool release without bucket)",
            );
            return;
        };
        if !reserve_cached_bytes(
            &self.cached_bytes,
            self.max_cached_bytes,
            allocation.byte_len,
        ) {
            host_memory::free_pinned_host_buffer(
                allocation.ptr.cast(),
                "cuMemFreeHost (pinned host pool cache over budget)",
            );
            return;
        }

        if let Err(ptr) = queue.push(allocation.ptr.addr()) {
            subtract_cached_bytes_or_repair(
                &self.cached_bytes,
                allocation.byte_len,
                "CUDA pinned-host allocation-pool cached bytes",
            );
            host_memory::free_pinned_host_buffer(
                ptr as *mut c_void,
                "cuMemFreeHost (pinned host pool queue full)",
            );
        }
    }
}

impl Drop for PinnedHostAllocationPool {
    fn drop(&mut self) {
        for entry in &self.free {
            while let Some(ptr) = entry.value().pop() {
                host_memory::free_pinned_host_buffer(
                    ptr as *mut c_void,
                    "cuMemFreeHost (pinned host pool drop)",
                );
            }
        }
        self.cached_bytes.store(0, Ordering::Release);
    }
}

#[derive(Debug)]
pub(crate) struct HostTransferAllocations {
    pool: Arc<PinnedHostAllocationPool>,
    allocations: SmallVec<[PinnedHostAllocation; 8]>,
    outputs: SmallVec<[HostOutputTransfer; 8]>,
}

#[derive(Clone, Copy, Debug)]
struct HostOutputTransfer {
    allocation_index: Option<usize>,
    byte_len: usize,
}

impl HostTransferAllocations {
    pub(crate) fn with_capacity(
        pool: Arc<PinnedHostAllocationPool>,
        transfer_capacity: usize,
        output_capacity: usize,
    ) -> Result<Self, BackendError> {
        let mut allocations = SmallVec::new();
        reserve_smallvec(&mut allocations, transfer_capacity, "pinned-host transfer")?;
        let mut outputs = SmallVec::new();
        reserve_smallvec(&mut outputs, output_capacity, "pinned-host output")?;
        Ok(Self {
            pool,
            allocations,
            outputs,
        })
    }

    pub(crate) fn push_upload(&mut self, bytes: &[u8]) -> Result<*const c_void, BackendError> {
        if bytes.is_empty() {
            return Ok(std::ptr::null());
        }
        self.reserve_next_allocation_slot("byte upload")?;
        let mut allocation = self.pool.acquire(bytes.len())?;
        allocation.copy_from_slice(bytes)?;
        let ptr = allocation.as_ptr();
        self.allocations.push(allocation);
        Ok(ptr)
    }

    pub(crate) fn push_upload_padded(
        &mut self,
        bytes: &[u8],
        transfer_byte_len: usize,
    ) -> Result<*const c_void, BackendError> {
        if bytes.is_empty() {
            return Ok(std::ptr::null());
        }
        if transfer_byte_len < bytes.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA padded upload length {transfer_byte_len} is smaller than logical upload length {}.",
                    bytes.len()
                ),
            });
        }
        self.reserve_next_allocation_slot("padded byte upload")?;
        let mut allocation = self.pool.acquire(transfer_byte_len)?;
        allocation.copy_from_slice(bytes)?;
        allocation.zero_range(bytes.len(), transfer_byte_len - bytes.len())?;
        let ptr = allocation.as_ptr();
        self.allocations.push(allocation);
        Ok(ptr)
    }

    pub(crate) fn push_u32_words(&mut self, words: &[u32]) -> Result<*const c_void, BackendError> {
        let byte_len = std::mem::size_of_val(words);
        if byte_len == 0 {
            return Ok(std::ptr::null());
        }
        self.reserve_next_allocation_slot("u32 word upload")?;
        let mut allocation = self.pool.acquire(byte_len)?;
        allocation.copy_u32_le_words(words)?;
        let ptr = allocation.as_ptr();
        self.allocations.push(allocation);
        Ok(ptr)
    }

    pub(crate) fn push_u32_words_padded(
        &mut self,
        words: &[u32],
        transfer_byte_len: usize,
    ) -> Result<*const c_void, BackendError> {
        let byte_len = std::mem::size_of_val(words);
        if byte_len == 0 {
            return Ok(std::ptr::null());
        }
        if transfer_byte_len < byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA padded parameter upload length {transfer_byte_len} is smaller than logical parameter length {byte_len}."
                ),
            });
        }
        self.reserve_next_allocation_slot("padded u32 word upload")?;
        let mut allocation = self.pool.acquire(transfer_byte_len)?;
        allocation.copy_u32_le_words(words)?;
        allocation.zero_range(byte_len, transfer_byte_len - byte_len)?;
        let ptr = allocation.as_ptr();
        self.allocations.push(allocation);
        Ok(ptr)
    }

    pub(crate) fn push_output(&mut self, byte_len: usize) -> Result<*mut c_void, BackendError> {
        self.reserve_next_output_slot("output readback")?;
        if byte_len == 0 {
            self.outputs.push(HostOutputTransfer {
                allocation_index: None,
                byte_len,
            });
            return Ok(std::ptr::null_mut());
        }
        self.reserve_next_allocation_slot("output readback")?;
        let mut allocation = self.pool.acquire(byte_len)?;
        let ptr = allocation.as_mut_ptr();
        let index = self.allocations.len();
        self.allocations.push(allocation);
        self.outputs.push(HostOutputTransfer {
            allocation_index: Some(index),
            byte_len,
        });
        Ok(ptr)
    }

    pub(crate) fn push_output_padded(
        &mut self,
        byte_len: usize,
        transfer_byte_len: usize,
    ) -> Result<*mut c_void, BackendError> {
        self.reserve_next_output_slot("padded output readback")?;
        if byte_len == 0 {
            self.outputs.push(HostOutputTransfer {
                allocation_index: None,
                byte_len,
            });
            return Ok(std::ptr::null_mut());
        }
        if transfer_byte_len < byte_len {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA padded output readback length {transfer_byte_len} is smaller than logical output length {byte_len}."
                ),
            });
        }
        self.reserve_next_allocation_slot("padded output readback")?;
        let mut allocation = self.pool.acquire(transfer_byte_len)?;
        let ptr = allocation.as_mut_ptr();
        let index = self.allocations.len();
        self.allocations.push(allocation);
        self.outputs.push(HostOutputTransfer {
            allocation_index: Some(index),
            byte_len,
        });
        Ok(ptr)
    }

    fn reserve_next_allocation_slot(&mut self, context: &'static str) -> Result<(), BackendError> {
        let next_len = vyre_driver::accounting::checked_add_usize_lazy(
            self.allocations.len(),
            1,
            || {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA pinned-host allocation table length overflowed while staging {context}. Split the dispatch before host transfer staging."
                    ),
                }
            },
        )?;
        reserve_smallvec(&mut self.allocations, next_len, "pinned-host transfer")
    }

    fn reserve_next_output_slot(&mut self, context: &'static str) -> Result<(), BackendError> {
        let next_len = vyre_driver::accounting::checked_add_usize_lazy(
            self.outputs.len(),
            1,
            || {
                BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA pinned-host output table length overflowed while staging {context}. Split the dispatch before output readback staging."
                    ),
                }
            },
        )?;
        reserve_smallvec(&mut self.outputs, next_len, "pinned-host output")
    }

    pub(crate) fn collect_outputs_into(
        &self,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        resize_vec_slots(
            outputs,
            self.outputs.len(),
            "CUDA host transfer output vector",
        )?;
        self.reserve_output_slots_into(outputs.iter_mut().enumerate())?;
        self.collect_output_slots_into(outputs.iter_mut().enumerate())
    }

    pub(crate) fn collect_borrowed_outputs_into(
        &self,
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if outputs.len() != self.outputs.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA borrowed output collection received {} output slot(s) for {} pending readback(s). Pass one output buffer per declared CUDA output.",
                    outputs.len(),
                    self.outputs.len()
                ),
            });
        }
        self.reserve_output_slots_into(
            outputs
                .iter_mut()
                .enumerate()
                .map(|(output_index, output)| (output_index, &mut **output)),
        )?;
        self.collect_output_slots_into(
            outputs
                .iter_mut()
                .enumerate()
                .map(|(output_index, output)| (output_index, &mut **output)),
        )
    }

    fn reserve_output_slots_into<'a>(
        &self,
        outputs: impl IntoIterator<Item = (usize, &'a mut Vec<u8>)>,
    ) -> Result<(), BackendError> {
        for (output_index, output) in outputs {
            self.reserve_output_slot_into(output_index, output)?;
        }
        Ok(())
    }

    fn reserve_output_slot_into(
        &self,
        output_index: usize,
        output: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let Some(&transfer) = self.outputs.get(output_index) else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA output preflight requested output index {output_index}, but only {} output transfer(s) exist.",
                    self.outputs.len()
                ),
            });
        };
        if let Some(allocation_index) = transfer.allocation_index {
            let Some(allocation) = self.allocations.get(allocation_index) else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA output preflight for transfer {output_index} references allocation index {allocation_index}, but only {} allocation(s) exist.",
                        self.allocations.len()
                    ),
                });
            };
            if transfer.byte_len > allocation.byte_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA output preflight for transfer {output_index} needs {} byte(s) from a {} byte pinned-host allocation. Recompute output transfer sizing before collecting results.",
                        transfer.byte_len, allocation.byte_len
                    ),
                });
            }
        }
        reserve_vec(output, transfer.byte_len, "CUDA host transfer output bytes")
    }

    fn collect_output_slots_into<'a>(
        &self,
        outputs: impl IntoIterator<Item = (usize, &'a mut Vec<u8>)>,
    ) -> Result<(), BackendError> {
        for (output_index, output) in outputs {
            self.collect_output_into(output_index, output)?;
        }
        Ok(())
    }

    pub(crate) fn collect_output_into(
        &self,
        output_index: usize,
        output: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let Some(&transfer) = self.outputs.get(output_index) else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA output collection requested output index {output_index}, but only {} output transfer(s) exist.",
                    self.outputs.len()
                ),
            });
        };
        if let Some(allocation_index) = transfer.allocation_index {
            let Some(allocation) = self.allocations.get(allocation_index) else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA output transfer {output_index} references allocation index {allocation_index}, but only {} allocation(s) exist.",
                        self.allocations.len()
                    ),
                });
            };
            allocation.copy_prefix_into(transfer.byte_len, output)?;
        } else {
            output.clear();
        }
        Ok(())
    }

    pub(crate) fn collect_output_range_into(
        &self,
        output_index: usize,
        byte_offset: usize,
        byte_len: usize,
        output: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let Some(&transfer) = self.outputs.get(output_index) else {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA ranged output collection requested output index {output_index}, but only {} output transfer(s) exist.",
                    self.outputs.len()
                ),
            });
        };
        let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
            byte_offset,
            byte_len,
            transfer.byte_len,
            || {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA ranged output collection overflowed usize at offset {byte_offset} len {byte_len}."
                ),
            }
            },
            |end| {
                BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA ranged output collection requested byte range [{byte_offset}..{end}) from output transfer {output_index}, but that transfer has {} byte(s).",
                    transfer.byte_len
                ),
            }
            },
        )?;
        if let Some(allocation_index) = transfer.allocation_index {
            let Some(allocation) = self.allocations.get(allocation_index) else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA ranged output transfer {output_index} references allocation index {allocation_index}, but only {} allocation(s) exist.",
                        self.allocations.len()
                    ),
                });
            };
            allocation.copy_range_into(byte_offset, byte_len, output)?;
        } else {
            output.clear();
        }
        Ok(())
    }
}

impl Drop for HostTransferAllocations {
    fn drop(&mut self) {
        for allocation in self.allocations.drain(..) {
            self.pool.release(allocation);
        }
    }
}

impl Drop for DispatchAllocations {
    fn drop(&mut self) {
        for allocation in self.ptrs.drain(..) {
            self.pool.release(allocation);
        }
        self.pool.release(std::mem::take(&mut self.params));
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeviceAllocation {
    pub(crate) ptr: u64,
    pub(crate) byte_len: usize,
}

#[derive(Debug)]
pub(crate) struct DeviceAllocationPool {
    free: DashMap<usize, ArrayQueue<u64>, BuildHasherDefault<FxHasher>>,
    cached_bytes: AtomicUsize,
    allocated_bytes: AtomicUsize,
    max_cached_bytes: usize,
}

impl DeviceAllocationPool {
    pub(crate) fn new(max_cached_bytes: usize) -> Self {
        Self {
            free: DashMap::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            cached_bytes: AtomicUsize::new(0),
            allocated_bytes: AtomicUsize::new(0),
            max_cached_bytes,
        }
    }

    pub(crate) fn acquire(&self, byte_len: usize) -> Result<DeviceAllocation, BackendError> {
        let bucket = allocation_bucket(byte_len, "CUDA device allocation")?;
        if let Some(ptr) = self.take_cached(bucket)? {
            return Ok(DeviceAllocation {
                ptr,
                byte_len: bucket,
            });
        }
        self.free.entry(bucket).or_insert_with(|| {
            ArrayQueue::new(allocation_bucket_cache_slots(bucket, self.max_cached_bytes))
        });
        let ptr = alloc_cuda_ptr(bucket, "cuMemAlloc_v2")?;
        if let Err(error) = add_cached_bytes(
            &self.allocated_bytes,
            bucket,
            "CUDA allocation-pool live device bytes",
        ) {
            free_cuda_ptr(ptr);
            return Err(error);
        }
        Ok(DeviceAllocation {
            ptr,
            byte_len: bucket,
        })
    }

    pub(crate) fn cached_bytes(&self) -> Result<usize, BackendError> {
        Ok(self.cached_bytes.load(Ordering::Acquire))
    }

    pub(crate) fn allocated_bytes(&self) -> Result<usize, BackendError> {
        Ok(self.allocated_bytes.load(Ordering::Acquire))
    }

    pub(crate) fn clear(&self) -> Result<(), BackendError> {
        let mut freed_bytes = 0usize;
        for entry in &self.free {
            while let Some(ptr) = entry.value().pop() {
                free_cuda_ptr(ptr);
                freed_bytes = checked_add_usize_lazy(freed_bytes, *entry.key(), || {
                    BackendError::InvalidProgram {
                        fix: "Fix: CUDA allocation-pool clear byte accounting overflowed usize; allocator state is corrupt."
                            .to_string(),
                    }
                })?;
            }
        }
        self.free.clear();
        self.cached_bytes.store(0, Ordering::Release);
        subtract_cached_bytes_or_repair(
            &self.allocated_bytes,
            freed_bytes,
            "CUDA allocation-pool live device bytes",
        );
        Ok(())
    }

    fn take_cached(&self, bucket: usize) -> Result<Option<u64>, BackendError> {
        let Some(queue) = self.free.get(&bucket) else {
            return Ok(None);
        };
        let Some(ptr) = queue.pop() else {
            return Ok(None);
        };
        subtract_cached_bytes_or_repair(
            &self.cached_bytes,
            bucket,
            "CUDA allocation-pool cached device bytes",
        );
        Ok(Some(ptr))
    }

    pub(crate) fn release(&self, allocation: DeviceAllocation) {
        if allocation.ptr == 0 || allocation.byte_len == 0 {
            return;
        }
        let Some(queue) = self.free.get(&allocation.byte_len) else {
            free_cuda_ptr(allocation.ptr);
            if let Err(error) = subtract_cached_bytes(&self.allocated_bytes, allocation.byte_len) {
                tracing::error!("{error}");
            }
            return;
        };
        if !reserve_cached_bytes(
            &self.cached_bytes,
            self.max_cached_bytes,
            allocation.byte_len,
        ) {
            free_cuda_ptr(allocation.ptr);
            if let Err(error) = subtract_cached_bytes(&self.allocated_bytes, allocation.byte_len) {
                tracing::error!("{error}");
            }
            return;
        }

        if let Err(ptr) = queue.push(allocation.ptr) {
            subtract_cached_bytes_or_repair(
                &self.cached_bytes,
                allocation.byte_len,
                "CUDA allocation-pool cached device bytes",
            );
            free_cuda_ptr(ptr);
            if let Err(error) = subtract_cached_bytes(&self.allocated_bytes, allocation.byte_len) {
                tracing::error!("{error}");
            }
        }
    }
}

impl Drop for DeviceAllocationPool {
    fn drop(&mut self) {
        for entry in &self.free {
            while let Some(ptr) = entry.value().pop() {
                free_cuda_ptr(ptr);
            }
        }
        self.cached_bytes.store(0, Ordering::Release);
        self.allocated_bytes.store(0, Ordering::Release);
    }
}

fn allocation_bucket(byte_len: usize, label: &'static str) -> Result<usize, BackendError> {
    byte_len
        .max(1)
        .checked_next_power_of_two()
        .ok_or_else(|| BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{label} request of {byte_len} bytes cannot be rounded to a power-of-two bucket. Fix: cap dispatch buffer sizes before allocation."
            ),
        })
}

fn allocation_bucket_cache_slots(bucket: usize, max_cached_bytes: usize) -> usize {
    const ALLOCATION_BUCKET_MAX_SLOTS: usize = 1024;
    let slots_by_budget = max_cached_bytes
        .checked_div(bucket.max(1))
        .unwrap_or(0)
        .max(1);
    slots_by_budget.min(ALLOCATION_BUCKET_MAX_SLOTS)
}

fn reserve_cached_bytes(counter: &AtomicUsize, max_cached_bytes: usize, bytes: usize) -> bool {
    checked_atomic_add_usize_guarded_with_order(
        counter,
        bytes,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |_, _| (),
        |next| {
            if next > max_cached_bytes {
                Err(())
            } else {
                Ok(())
            }
        },
    )
    .is_ok()
}

fn add_cached_bytes(
    counter: &AtomicUsize,
    bytes: usize,
    label: &'static str,
) -> Result<(), BackendError> {
    checked_atomic_add_usize_with_order(
        counter,
        bytes,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |observed, attempted| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {label} accounting overflowed while adding {attempted} to observed {observed}; shard the allocation workload before enqueueing more CUDA work."
                ),
            }
        },
    )
}

fn subtract_cached_bytes(counter: &AtomicUsize, bytes: usize) -> Result<(), BackendError> {
    checked_atomic_sub_usize(counter, bytes, |observed, attempted| {
        BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA allocation-pool byte accounting underflowed while subtracting {attempted} from observed {observed}; allocator state is corrupt."
                ),
            }
    })
}

fn subtract_cached_bytes_or_repair(counter: &AtomicUsize, bytes: usize, label: &'static str) {
    repair_atomic_sub_usize_with_order(
        counter,
        bytes,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |observed, attempted| {
            tracing::error!(
                "{label} underflowed while subtracting {attempted} from observed {observed}; repaired accounting to zero."
            );
        },
    );
}

pub(crate) fn free_cuda_ptr_with_label(ptr: u64, label: &str) {
    if ptr == 0 {
        return;
    }
    // SAFETY: FFI to libcuda.so cuMemFree_v2. ptr was returned by a
    // matching cuMemAlloc_v2 call (the pool owns the lifetime); the
    // null guard above ensures we never pass 0. CUDA_SUCCESS check records
    // unexpected failures without propagating (free runs on Drop / pool clear
    // paths where ?-propagation is not available).
    unsafe {
        let result = cudarc::driver::sys::cuMemFree_v2(ptr);
        if result != CUresult::CUDA_SUCCESS {
            tracing::error!(
                "Fix: cuMemFree_v2 failed while releasing {label} with {result:?}; ensure all launches using the allocation have completed."
            );
        }
    }
}

pub(crate) fn free_cuda_ptr(ptr: u64) {
    free_cuda_ptr_with_label(ptr, "CUDA allocation");
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::{
        copy_raw_bytes_into_vec, subtract_cached_bytes, HostOutputTransfer,
        HostTransferAllocations, PinnedHostAllocation, PinnedHostAllocationPool,
    };

    #[test]
    fn copy_raw_bytes_into_vec_reuses_capacity_without_zero_fill_resize() {
        let src = [1u8, 2, 3, 4, 5, 6];
        let mut dst = Vec::with_capacity(16);
        dst.extend_from_slice(&[9, 9, 9, 9]);
        let capacity = dst.capacity();

        copy_raw_bytes_into_vec(src.as_ptr(), 4, &mut dst).unwrap();

        assert_eq!(dst, vec![1, 2, 3, 4]);
        assert_eq!(dst.capacity(), capacity);

        copy_raw_bytes_into_vec(src[2..].as_ptr(), 0, &mut dst).unwrap();
        assert!(dst.is_empty());
        assert_eq!(dst.capacity(), capacity);
    }

    #[test]
    fn copy_raw_bytes_into_vec_preserves_last_good_output_when_reservation_fails() {
        let src = std::ptr::NonNull::<u8>::dangling().as_ptr();
        let mut dst = vec![7, 8, 9];
        let capacity = dst.capacity();

        let error = copy_raw_bytes_into_vec(src, usize::MAX, &mut dst)
            .expect_err("Fix: impossible CUDA readback reservation must fail before clobbering");

        assert!(
            error.to_string().contains("CUDA readback output bytes"),
            "error should identify the failed readback reservation: {error}"
        );
        assert_eq!(dst, vec![7, 8, 9]);
        assert_eq!(dst.capacity(), capacity);
    }

    #[test]
    fn zero_byte_output_readback_does_not_acquire_pinned_host_memory() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(Arc::clone(&pool), 0, 1)
            .expect("Fix: host transfer table should reserve");

        let ptr = transfers
            .push_output(0)
            .expect("Fix: zero-byte output reservation must not touch CUDA allocation APIs");

        assert!(ptr.is_null());
        assert!(transfers.allocations.is_empty());

        let mut outputs = vec![vec![1, 2, 3]];
        let capacity = outputs[0].capacity();
        transfers.collect_outputs_into(&mut outputs).unwrap();

        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].is_empty());
        assert_eq!(outputs[0].capacity(), capacity);
        assert_eq!(pool.cached_bytes(), 0);
    }

    #[test]
    fn borrowed_zero_byte_output_readback_preserves_caller_capacity() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(Arc::clone(&pool), 0, 1)
            .expect("Fix: host transfer table should reserve");
        let ptr = transfers.push_output(0).expect(
            "Fix: zero-byte borrowed output reservation must not touch CUDA allocation APIs",
        );
        let mut output = Vec::with_capacity(32);
        output.extend_from_slice(&[7, 7, 7, 7]);
        let capacity = output.capacity();

        assert!(ptr.is_null());
        transfers
            .collect_borrowed_outputs_into(&mut [&mut output])
            .unwrap();

        assert!(output.is_empty());
        assert_eq!(output.capacity(), capacity);
        assert_eq!(pool.cached_bytes(), 0);
    }

    #[test]
    fn host_transfer_output_collection_preflights_all_slots_before_copying() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(pool, 1, 2)
            .expect("Fix: host transfer table should reserve");
        transfers.allocations.push(PinnedHostAllocation {
            ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(),
            byte_len: 1,
        });
        transfers.outputs.push(HostOutputTransfer {
            allocation_index: Some(0),
            byte_len: 1,
        });
        transfers.outputs.push(HostOutputTransfer {
            allocation_index: Some(9),
            byte_len: 1,
        });
        let mut outputs = vec![vec![0xaa], vec![0xbb]];

        let error = transfers.collect_outputs_into(&mut outputs).expect_err(
            "Fix: invalid second CUDA output transfer must fail before copying output zero.",
        );

        assert!(
            error.to_string().contains("CUDA output preflight"),
            "error must identify output preflight before materialization, got: {error}"
        );
        assert_eq!(
            outputs,
            vec![vec![0xaa], vec![0xbb]],
            "CUDA host-transfer output collection must not mutate an earlier output before a later transfer descriptor fails preflight."
        );
    }

    #[test]
    fn owned_and_borrowed_output_collection_share_one_slot_iterator() {
        let source = include_str!("allocations.rs");
        let host_transfer_impl = source
            .split("impl HostTransferAllocations {")
            .nth(1)
            .expect("Fix: HostTransferAllocations impl must exist")
            .split("impl Drop for HostTransferAllocations")
            .next()
            .expect("Fix: HostTransferAllocations impl must precede Drop impl");

        assert!(
            host_transfer_impl.contains("fn collect_output_slots_into"),
            "Fix: CUDA host-transfer output collection must expose one shared slot iterator."
        );
        assert!(
            host_transfer_impl.contains("fn reserve_output_slots_into")
                && host_transfer_impl.contains("fn reserve_output_slot_into"),
            "Fix: CUDA host-transfer output collection must preflight destination slots before shared collection."
        );
        for method in ["collect_outputs_into", "collect_borrowed_outputs_into"] {
            let method_body = host_transfer_impl
                .split(&format!("pub(crate) fn {method}"))
                .nth(1)
                .unwrap_or_else(|| panic!("Fix: HostTransferAllocations must expose {method}."));
            let reserve_pos = method_body
                .find("self.reserve_output_slots_into")
                .unwrap_or_else(|| panic!("Fix: {method} must preflight output slots."));
            let collect_pos = method_body
                .find("self.collect_output_slots_into")
                .unwrap_or_else(|| panic!("Fix: {method} must collect output slots."));
            assert!(
                reserve_pos < collect_pos,
                "Fix: {method} must reserve every CUDA host-transfer output before copying any slot."
            );
        }
        assert_eq!(
            host_transfer_impl
                .matches(concat!("self.collect_output_slots_", "into("))
                .count(),
            2,
            "Fix: owned and borrowed CUDA output collection must both use the shared slot iterator."
        );
        assert!(
            !host_transfer_impl.contains("for output_index in 0..self.outputs.len()"),
            "Fix: owned CUDA output collection must not carry a separate index loop from borrowed collection."
        );
    }

    #[test]
    fn zero_byte_uploads_do_not_acquire_pinned_host_memory() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(Arc::clone(&pool), 2, 0)
            .expect("Fix: host transfer table should reserve");

        let bytes_ptr = transfers
            .push_upload(&[])
            .expect("Fix: empty byte upload must not touch CUDA allocation APIs");
        let words_ptr = transfers
            .push_u32_words(&[])
            .expect("Fix: empty parameter upload must not touch CUDA allocation APIs");

        assert!(bytes_ptr.is_null());
        assert!(words_ptr.is_null());
        assert!(transfers.allocations.is_empty());
        assert_eq!(pool.cached_bytes(), 0);
    }

    #[test]
    fn host_transfer_pushes_reserve_metadata_before_pinned_memory_acquire() {
        let source = include_str!("allocations.rs");
        let host_transfers = source
            .split("impl HostTransferAllocations {")
            .nth(1)
            .expect("Fix: HostTransferAllocations impl must exist")
            .split("impl Drop for HostTransferAllocations")
            .next()
            .expect("Fix: HostTransferAllocations impl must precede Drop impl");

        for (method, reservation) in [
            (
                "push_upload",
                "reserve_next_allocation_slot(\"byte upload\")?",
            ),
            (
                "push_upload_padded",
                "reserve_next_allocation_slot(\"padded byte upload\")?",
            ),
            (
                "push_u32_words",
                "reserve_next_allocation_slot(\"u32 word upload\")?",
            ),
            (
                "push_u32_words_padded",
                "reserve_next_allocation_slot(\"padded u32 word upload\")?",
            ),
            (
                "push_output",
                "reserve_next_allocation_slot(\"output readback\")?",
            ),
            (
                "push_output_padded",
                "reserve_next_allocation_slot(\"padded output readback\")?",
            ),
        ] {
            let method_body = host_transfers
                .split(&format!("pub(crate) fn {method}"))
                .nth(1)
                .unwrap_or_else(|| panic!("Fix: HostTransferAllocations must expose {method}."));
            let reserve_pos = method_body.find(reservation).unwrap_or_else(|| {
                panic!("Fix: {method} must reserve metadata before acquisition.")
            });
            let acquire_pos = method_body
                .find("self.pool.acquire(")
                .unwrap_or_else(|| panic!("Fix: {method} must acquire pinned host memory."));

            assert!(
                reserve_pos < acquire_pos,
                "Fix: {method} must reserve host-transfer metadata before acquiring pinned host memory."
            );
        }

        for (method, reservation) in [
            (
                "push_output",
                "reserve_next_output_slot(\"output readback\")?",
            ),
            (
                "push_output_padded",
                "reserve_next_output_slot(\"padded output readback\")?",
            ),
        ] {
            let method_body = host_transfers
                .split(&format!("pub(crate) fn {method}"))
                .nth(1)
                .unwrap_or_else(|| panic!("Fix: HostTransferAllocations must expose {method}."));
            let reserve_pos = method_body
                .find(reservation)
                .unwrap_or_else(|| panic!("Fix: {method} must reserve output metadata."));
            let push_pos = method_body
                .find("self.outputs.push")
                .unwrap_or_else(|| panic!("Fix: {method} must record an output transfer."));

            assert!(
                reserve_pos < push_pos,
                "Fix: {method} must reserve output metadata before recording the readback slot."
            );
        }

        assert!(
            host_transfers.contains("fn reserve_next_allocation_slot")
                && host_transfers.contains("fn reserve_next_output_slot")
                && host_transfers.contains("checked_add_usize_lazy")
                && host_transfers.contains("reserve_smallvec(&mut self.allocations")
                && host_transfers.contains("reserve_smallvec(&mut self.outputs"),
            "Fix: host-transfer push metadata growth must use checked fallible SmallVec reservation helpers."
        );
    }

    #[test]
    fn pinned_host_copy_rejects_oversized_upload_in_release_path() {
        let mut allocation = PinnedHostAllocation {
            ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(),
            byte_len: 2,
        };
        let error = allocation
            .copy_from_slice(&[1, 2, 3])
            .expect_err("oversized pinned-host upload must return a typed error");

        assert!(
            error.to_string().contains("attempted to copy 3 byte"),
            "error should describe the oversized host upload: {error}"
        );
    }

    #[test]
    fn pinned_host_readback_rejects_oversized_prefix_in_release_path() {
        let allocation = PinnedHostAllocation {
            ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(),
            byte_len: 2,
        };
        let mut output = Vec::new();
        let error = allocation
            .copy_prefix_into(3, &mut output)
            .expect_err("oversized pinned-host readback must return a typed error");

        assert!(
            error.to_string().contains("attempted to copy 3 byte"),
            "error should describe the oversized host readback: {error}"
        );
    }

    #[test]
    fn pinned_host_readback_range_copies_exact_slice_without_reallocating_output() {
        let source = [10u8, 11, 12, 13, 14, 15];
        let allocation = PinnedHostAllocation {
            ptr: source.as_ptr().cast_mut(),
            byte_len: source.len(),
        };
        let mut output = Vec::with_capacity(16);
        output.extend_from_slice(&[99, 99, 99]);
        let capacity = output.capacity();

        allocation
            .copy_range_into(2, 3, &mut output)
            .expect("Fix: pinned-host ranged readback should copy the requested slice.");

        assert_eq!(output, vec![12, 13, 14]);
        assert_eq!(
            output.capacity(),
            capacity,
            "Fix: ranged readback collection must preserve caller-owned output capacity when it is sufficient."
        );
    }

    #[test]
    fn pinned_host_readback_range_rejects_out_of_bounds_slice_without_clobbering_output() {
        let allocation = PinnedHostAllocation {
            ptr: std::ptr::NonNull::<u8>::dangling().as_ptr(),
            byte_len: 4,
        };
        let mut output = vec![1, 2, 3];
        let capacity = output.capacity();

        let error = allocation
            .copy_range_into(3, 2, &mut output)
            .expect_err("Fix: out-of-bounds pinned-host ranged readback must fail.");

        assert!(
            error.to_string().contains("byte range [3..5)"),
            "error should describe the invalid ranged readback: {error}"
        );
        assert_eq!(output, vec![1, 2, 3]);
        assert_eq!(output.capacity(), capacity);
    }

    #[test]
    fn pinned_host_readback_range_uses_checked_bounds_before_pointer_arithmetic() {
        let source = include_str!("allocations.rs");
        let copy_range = source
            .split("pub(crate) fn copy_range_into")
            .nth(1)
            .and_then(|tail| tail.split("fn copy_raw_bytes_into_vec").next())
            .expect("Fix: pinned-host ranged readback helper must precede raw byte copy helper.");

        assert!(
            copy_range.contains("checked_usize_byte_range_end_lazy")
                && copy_range.contains("byte_len == 0")
                && copy_range.contains("self.ptr.add(byte_offset)")
                && !copy_range.contains("wrapping_add(byte_offset)"),
            "Fix: CUDA pinned-host ranged readback must validate the byte range before in-bounds pointer arithmetic instead of wrapping addresses."
        );
    }

    #[test]
    fn borrowed_output_collection_rejects_slot_count_mismatch() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(pool, 0, 1)
            .expect("Fix: host transfer table should reserve");
        transfers.outputs.push(HostOutputTransfer {
            allocation_index: None,
            byte_len: 0,
        });
        let error = transfers
            .collect_borrowed_outputs_into(&mut [])
            .expect_err("borrowed output collection must reject slot-count mismatch");

        assert!(
            error
                .to_string()
                .contains("one output buffer per declared CUDA output"),
            "error should describe the borrowed output slot mismatch: {error}"
        );
    }

    #[test]
    fn ranged_output_collection_rejects_out_of_bounds_transfer_slice() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let mut transfers = HostTransferAllocations::with_capacity(pool, 0, 1)
            .expect("Fix: host transfer table should reserve");
        transfers.outputs.push(HostOutputTransfer {
            allocation_index: None,
            byte_len: 0,
        });
        let mut output = vec![9, 9, 9];
        let capacity = output.capacity();

        let error = transfers
            .collect_output_range_into(0, 1, 1, &mut output)
            .expect_err("Fix: ranged output collection must reject out-of-bounds transfer slices.");

        assert!(
            error.to_string().contains("byte range [1..2)"),
            "error should describe the invalid output transfer slice: {error}"
        );
        assert_eq!(output, vec![9, 9, 9]);
        assert_eq!(output.capacity(), capacity);
    }

    #[test]
    fn output_collection_rejects_out_of_range_transfer_index() {
        let pool = Arc::new(PinnedHostAllocationPool::new(0));
        let transfers = HostTransferAllocations::with_capacity(pool, 0, 0)
            .expect("Fix: host transfer table should reserve");
        let mut output = Vec::new();
        let error = transfers
            .collect_output_into(0, &mut output)
            .expect_err("output collection must reject out-of-range transfer indexes");

        assert!(
            error.to_string().contains("requested output index 0"),
            "error should describe the invalid output transfer index: {error}"
        );
    }

    #[test]
    fn subtract_cached_bytes_fails_loudly_on_accounting_underflow() {
        let counter = AtomicUsize::new(4);
        let error = subtract_cached_bytes(&counter, 8)
            .expect_err("Fix: allocation-pool underflow must return a typed error.");
        assert!(error.to_string().contains("underflowed"));
        assert_eq!(counter.load(Ordering::Acquire), 4);
    }

    #[test]
    fn allocation_pool_accounting_uses_checked_arithmetic_not_saturation() {
        let source = include_str!("allocations.rs");
        assert!(
            !source.contains(concat!(".", "saturating_add"))
                && !source.contains(concat!(".", "saturating_sub")),
            "Fix: CUDA allocation-pool byte accounting must not saturate overflow or underflow."
        );
        let counter = AtomicUsize::new(8);
        subtract_cached_bytes(&counter, 3)
            .expect("Fix: valid allocation-pool subtraction should succeed.");
        assert_eq!(counter.load(Ordering::Acquire), 5);
    }

    #[test]
    fn cuda_device_allocation_is_freed_when_live_accounting_fails_after_alloc() {
        let source = include_str!("allocations.rs");
        let acquire = source
            .split("pub(crate) fn acquire(&self, byte_len: usize) -> Result<DeviceAllocation, BackendError>")
            .nth(1)
            .and_then(|tail| tail.split("pub(crate) fn cached_bytes").next())
            .expect("Fix: DeviceAllocationPool::acquire source must be discoverable");
        assert!(
            acquire.contains("if let Err(error) = add_cached_bytes(")
                && acquire.contains("free_cuda_ptr(ptr);")
                && acquire.contains("return Err(error);"),
            "Fix: CUDA device allocation must free cuMemAlloc_v2 output if live-byte accounting fails after allocation."
        );
        assert!(
            source.contains("subtract_cached_bytes_or_repair("),
            "Fix: cached CUDA allocation-pool byte accounting must repair non-critical cache counters instead of rejecting valid pool reuse."
        );
    }

    #[test]
    fn cuda_device_alloc_and_free_use_single_checked_ffi_boundary() {
        let allocations = include_str!("allocations.rs");
        let resident = include_str!("resident.rs");
        let cuda_graph = include_str!("cuda_graph.rs");
        let alloc_ffi = concat!("cudarc::driver::sys::", "cuMemAlloc_v2(");
        let free_ffi = concat!("cudarc::driver::sys::", "cuMemFree_v2(");

        assert_eq!(
            allocations.matches(alloc_ffi).count(),
            1,
            "Fix: CUDA device allocation must keep raw cuMemAlloc_v2 behind alloc_cuda_ptr."
        );
        assert_eq!(
            allocations.matches(free_ffi).count(),
            1,
            "Fix: CUDA device free must keep raw cuMemFree_v2 behind free_cuda_ptr_with_label."
        );
        assert_eq!(
            resident.matches(alloc_ffi).count() + cuda_graph.matches(alloc_ffi).count(),
            0,
            "Fix: resident and cudaGraph allocation paths must route through alloc_cuda_ptr."
        );
        assert_eq!(
            resident.matches(free_ffi).count() + cuda_graph.matches(free_ffi).count(),
            0,
            "Fix: resident and cudaGraph free paths must route through free_cuda_ptr."
        );
        assert!(
            allocations.contains("fn alloc_cuda_ptr(")
                && allocations.contains("cannot allocate zero device bytes")
                && allocations.contains("returned a null device pointer after reporting success"),
            "Fix: shared CUDA allocation helper must validate zero-byte requests and impossible null success returns."
        );
    }

    #[test]
    fn cuda_dispatch_and_host_transfer_tables_reserve_fallibly() {
        let source = include_str!("allocations.rs");
        assert!(
            source.contains("reserve_smallvec")
                && source.contains("buffer_count")
                && source.contains("allocations")
                && source.contains("transfer_capacity")
                && source.contains("output_capacity"),
            "Fix: CUDA dispatch and host-transfer staging tables must reserve fallibly before launch."
        );
        assert!(
            !source.contains(concat!("SmallVec::with_capacity", "(buffer_count)"))
                && !source.contains(concat!("ptrs", ".resize(buffer_count"))
                && !source.contains(concat!(
                    "SmallVec::with_capacity",
                    "(transfer_capacity)"
                ))
                && !source.contains(concat!(
                    "SmallVec::with_capacity",
                    "(output_capacity)"
                )),
            "Fix: CUDA allocation staging must not use infallible SmallVec capacity constructors in production."
        );
    }

    #[test]
    fn pinned_host_transfer_bounds_are_checked_without_debug_assert_contracts() {
        let source = include_str!("allocations.rs");
        let pinned_allocation = source
            .split("impl PinnedHostAllocation {")
            .nth(1)
            .expect("Fix: pinned-host allocation impl must be present")
            .split("#[derive(Debug)]\npub(crate) struct PinnedHostAllocationPool")
            .next()
            .expect("Fix: pinned-host allocation impl must precede pool type");
        let host_transfers = source
            .split("impl HostTransferAllocations {")
            .nth(1)
            .expect("Fix: host transfer impl must be present")
            .split("impl Drop for HostTransferAllocations")
            .next()
            .expect("Fix: host transfer impl must precede Drop impl");

        assert!(
            pinned_allocation.contains("pub(crate) fn copy_from_slice(&mut self, bytes: &[u8]) -> Result<(), BackendError>")
                && pinned_allocation.contains("pub(crate) fn copy_u32_le_words(&mut self, words: &[u32]) -> Result<(), BackendError>")
                && host_transfers.contains("pub(crate) fn collect_borrowed_outputs_into(")
                && !host_transfers.contains("debug_assert_eq!(outputs.len(), self.outputs.len())")
                && !pinned_allocation.contains("debug_assert!(byte_len <= self.byte_len)")
                && !pinned_allocation.contains("debug_assert!(bytes.len() <= self.byte_len)"),
            "Fix: CUDA pinned-host transfer bounds must be checked in release builds, not guarded only by debug_assert."
        );
    }
}
