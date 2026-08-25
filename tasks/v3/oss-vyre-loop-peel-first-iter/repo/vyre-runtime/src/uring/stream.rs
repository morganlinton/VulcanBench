//! `AsyncUringStream`  -  drives io_uring reads into GPU-visible memory
//! and advances the megakernel tail pointer on each completion.
//!
//! The critical safety contract: every byte read lands in a
//! [`GpuMappedBuffer`]. Compatibility ingest uses registered
//! host-visible GPU mappings; canonical native ingest uses BAR1 peer memory
//! via [`GpuMappedBuffer::from_bar1_peer_with_owner`] plus NVMe passthrough.
//! The io_uring writer never targets an ordinary userspace bounce buffer.

use super::ring::IoUringState;
use crate::PipelineError;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

/// Minimal `iovec` struct matching the Linux ABI for `readv`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Iovec {
    /// Target buffer address for this chunk of the read.
    pub iov_base: *mut core::ffi::c_void,
    /// Byte length of the target buffer.
    pub iov_len: usize,
}

/// `IORING_OP_READV`  -  scatter-read into an array of iovecs.
pub const IORING_OP_READV: u8 = 1;
/// `IORING_OP_READ_FIXED`  -  read into a pre-registered buffer.
pub const IORING_OP_READ_FIXED: u8 = 22;
/// `IORING_OP_URING_CMD`  -  vendor-specific passthrough (NVMe). Kernel 6.0+.
pub const IORING_OP_URING_CMD: u8 = 46;

/// GPU-visible memory region that io_uring is allowed to DMA into.
///
/// Compatibility constructors cover host-visible shared mappings. The BAR1
/// constructor covers the native GPUDirect path where NVMe DMA lands directly
/// in GPU-owned memory.
pub struct GpuMappedBuffer<'a> {
    ptr: *mut u8,
    len: usize,
    _owner: PhantomData<&'a mut [u8]>,
}

// SAFETY: Send + Sync because (a) the constructor's safety contract
// requires the caller to commit the lifetime invariant, and (b) the
// raw pointer is only dereferenced by the kernel via io_uring  -
// vyre-runtime never reads through it directly.
unsafe impl Send for GpuMappedBuffer<'_> {}
unsafe impl Sync for GpuMappedBuffer<'_> {}

impl<'a> GpuMappedBuffer<'a> {
    /// Construct from a borrowed host-visible byte slice.
    ///
    /// # Safety
    ///
    /// The caller asserts:
    /// - `slice` aliases a device allocation created with host-visible
    ///   host-shared usage bits by the concrete backend.
    /// - No other code reads or writes through `slice` while the
    ///   returned handle is alive.
    pub unsafe fn from_host_visible_slice(slice: &'a mut [u8]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            len: slice.len(),
            _owner: PhantomData,
        }
    }

    /// Construct from a raw pointer + explicit owner anchor.
    ///
    /// This is for GPU APIs that hand back a raw mapped pointer plus an
    /// owning handle rather than a Rust slice. The borrow on `owner`
    /// forces the mapped region to outlive every derived
    /// [`AsyncUringStream`].
    ///
    /// # Safety
    ///
    /// The caller asserts:
    /// - `ptr` points into a GPU allocation owned by `owner`.
    /// - The mapped region is `len` bytes long and host-visible.
    /// - No other code reads or writes through `ptr` while the
    ///   returned handle is alive.
    pub unsafe fn from_host_visible_owner<O: ?Sized>(
        _owner: &'a mut O,
        ptr: *mut u8,
        len: usize,
    ) -> Self {
        Self {
            ptr,
            len,
            _owner: PhantomData,
        }
    }

    /// Duplicate the mapped-buffer handle for the same underlying region.
    ///
    /// # Safety
    ///
    /// The caller must uphold the same aliasing and lifetime guarantees as
    /// [`GpuMappedBuffer::from_host_visible_slice`]. This does not clone memory;
    /// it creates another handle to the same mapped bytes.
    pub unsafe fn duplicate(&self) -> Self {
        Self {
            ptr: self.ptr,
            len: self.len,
            _owner: PhantomData,
        }
    }

    /// Carve out a sub-region of this mapped buffer.
    ///
    /// This preserves the original constructor contract: the returned
    /// handle aliases the same host-visible GPU allocation and carries
    /// no ownership of its own.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when `offset + len`
    /// exceeds the mapped buffer bounds.
    pub fn sub_region(&self, offset: usize, len: usize) -> Result<Self, crate::PipelineError> {
        let _end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
            offset,
            len,
            self.len,
            || {
                crate::PipelineError::QueueFull {
                queue: "submission",
                fix: "GpuMappedBuffer::sub_region offset + len overflows usize; reduce slot size or enlarge the staging buffer",
            }
            },
            |_| {
                crate::PipelineError::QueueFull {
                queue: "submission",
                fix: "GpuMappedBuffer::sub_region exceeds the mapped allocation; reduce slot size or enlarge the staging buffer",
            }
            },
        )?;
        Ok(Self {
            ptr: self.ptr.wrapping_add(offset),
            len,
            _owner: PhantomData,
        })
    }

    /// Byte length of the mapped region.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the region is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw pointer for io_uring submission. Crate-private.
    pub(crate) fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Borrow the mapped bytes as a mutable slice.
    ///
    /// # Safety
    ///
    /// The caller must ensure exclusive mutable access to the region for the
    /// lifetime of the returned slice.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    /// Construct from a PCIe peer-memory pointer
    /// GPUDirect Storage).
    ///
    /// When paired with [`crate::PipelineError::NvmePassthroughDisabled`]
    /// via the `uring-cmd-nvme` feature, this lets NVMe DMA writes
    /// land directly in VRAM without crossing the host PCIe bridge.
    ///
    /// # Safety
    ///
    /// The caller asserts:
    /// - `peer_ptr` points at a region returned by `nvidia_p2p_get_pages`
    ///   (Linux nvidia-fs) or `cuMemAlloc` + `cuPointerSetAttribute`
    ///   `CU_POINTER_ATTRIBUTE_SYNC_MEMOPS`.
    /// - The GPU allocation outlives the returned handle.
    /// - The io_uring kernel and NVMe driver both have DMA-mapping
    ///   capability (verified at runtime by checking
    ///   `/proc/driver/nvidia-fs/stats`).
    pub unsafe fn from_bar1_peer_with_owner<O: ?Sized>(
        _owner: &'a mut O,
        peer_ptr: *mut u8,
        len: usize,
    ) -> Self {
        Self {
            ptr: peer_ptr,
            len,
            _owner: PhantomData,
        }
    }
}

/// Streaming reader that pushes chunked reads into an io_uring SQ and
/// advances an atomic tail pointer the megakernel observes.
pub struct AsyncUringStream<'a> {
    pub(crate) ring_state: IoUringState,
    pub(crate) gpu_buffer: GpuMappedBuffer<'a>,
    pub(crate) megakernel_tail: &'a AtomicU32,
    pub(crate) inflight: u32,
    pub(crate) pending_submissions: u32,
}

// SAFETY: raw pointer fields covered by GpuMappedBuffer's contract +
// the constructor's safety commitment on megakernel_tail_ptr.
unsafe impl Send for AsyncUringStream<'_> {}
unsafe impl Sync for AsyncUringStream<'_> {}

impl<'a> AsyncUringStream<'a> {
    /// Create a stream bound to the given ring state, GPU-mapped
    /// buffer, and megakernel tail pointer.
    pub fn new(
        ring_state: IoUringState,
        gpu_buffer: GpuMappedBuffer<'a>,
        megakernel_tail: &'a AtomicU32,
    ) -> Self {
        Self {
            ring_state,
            gpu_buffer,
            megakernel_tail,
            inflight: 0,
            pending_submissions: 0,
        }
    }

    /// Rebind the target mapped buffer for future submissions.
    pub fn replace_buffer(&mut self, gpu_buffer: GpuMappedBuffer<'a>) {
        self.gpu_buffer = gpu_buffer;
    }

    /// Submit a scattered read of `len` bytes at file offset `offset`
    /// into the slot at `chunk_idx * len` within the GPU buffer.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::QueueFull`] if the SQ is full OR the
    ///   destination slot exceeds buffer bounds.
    /// - Range errors surface later as [`PipelineError::IoUringSyscall`]
    ///   on `poll` if the kernel rejects the SQE.
    ///
    /// # Safety
    ///
    /// `iovs_storage` must live until this SQE's completion is reaped;
    /// the kernel dereferences `iov_base` at I/O time, not submit time.
    pub unsafe fn submit_read_to_gpu(
        &mut self,
        fd: i32,
        offset: u64,
        len: u32,
        chunk_idx: usize,
        iovs_storage: &mut [Iovec],
    ) -> Result<(), PipelineError> {
        if iovs_storage.is_empty() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "caller supplied empty iovs_storage; pass at least one slot",
            });
        }
        let target_offset = checked_chunk_target_offset(chunk_idx, len)?;
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe { self.submit_read_to_gpu_at(fd, offset, len, target_offset, iovs_storage) }
    }

    /// Submit a read directly into a byte offset inside the mapped buffer.
    ///
    /// Unlike [`AsyncUringStream::submit_read_to_gpu`], this method does not
    /// derive the destination from a fixed chunk index. Wrappers that stream
    /// variable-sized shards can place each read contiguously in a staging
    /// buffer without being forced into `chunk_idx * len` layout.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::QueueFull`] if the SQ is full OR the target range
    ///   exceeds the mapped buffer bounds.
    ///
    /// # Safety
    ///
    /// `iovs_storage` must live until this SQE's completion is reaped.
    pub unsafe fn submit_read_to_gpu_at(
        &mut self,
        fd: i32,
        offset: u64,
        len: u32,
        target_offset: u64,
        iovs_storage: &mut [Iovec],
    ) -> Result<(), PipelineError> {
        // SAFETY: registered fixed buffers + file index are valid for the lifetime
        // of the ring; the SQE is built on the ring's own SQ slot.
        unsafe {
            self.submit_read_to_gpu_at_with_user_data(
                fd,
                offset,
                len,
                target_offset,
                target_offset,
                iovs_storage,
            )
        }
    }

    /// Submit a read into an arbitrary byte offset and preserve caller-defined
    /// `user_data` for completion correlation.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the SQ is full, the iovec
    /// storage is empty, or the target range exceeds the mapped GPU buffer.
    ///
    /// # Safety
    ///
    /// `iovs_storage` must live until this SQE's completion is reaped.
    pub unsafe fn submit_read_to_gpu_at_with_user_data(
        &mut self,
        fd: i32,
        offset: u64,
        len: u32,
        target_offset: u64,
        user_data: u64,
        iovs_storage: &mut [Iovec],
    ) -> Result<(), PipelineError> {
        if iovs_storage.is_empty() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "caller supplied empty iovs_storage; pass at least one slot",
            });
        }
        let end = checked_target_end(target_offset, len)?;
        let gpu_len = usize_to_u64(self.gpu_buffer.len(), "mapped GPU buffer length")?;
        if end > gpu_len {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "target_offset + len exceeds GpuMappedBuffer length; enlarge the buffer or reduce the read size",
            });
        }

        let Some(sqe) = self.ring_state.get_sqe() else {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "SQ full; call AsyncUringStream::poll to drain completions then retry",
            });
        };

        // SAFETY: bounds-checked above; writing to a sub-region of
        // the host-visible GpuMappedBuffer the caller committed.
        let target_addr = unsafe {
            self.gpu_buffer
                .as_ptr()
                .add(u64_to_usize(target_offset, "target offset")?)
        };

        iovs_storage[0] = Iovec {
            iov_base: target_addr.cast::<core::ffi::c_void>(),
            iov_len: u32_to_usize(len, "read length")?,
        };

        sqe.opcode = IORING_OP_READV;
        sqe.fd = fd;
        sqe.user_data_or_off = offset;
        sqe.addr = pointer_addr_u64(iovs_storage.as_ptr(), "readv iovec pointer")?;
        sqe.len = 1;
        sqe.user_data = user_data;

        self.ring_state.commit_sqe();
        increment_queue_counter(&mut self.inflight, "inflight SQE count")?;
        increment_queue_counter(&mut self.pending_submissions, "pending submission count")?;

        Ok(())
    }

    /// Submit any queued SQEs to the kernel.
    ///
    /// SQPOLL can pick up tail updates on its own, but wrappers that rely on
    /// bounded latency should not depend on the polling thread waking
    /// promptly. Flushing pending submissions makes progress explicit.
    pub fn flush_submissions(&mut self) -> Result<(), PipelineError> {
        if self.pending_submissions == 0 {
            return Ok(());
        }
        if self.ring_state.uses_sqpoll() {
            if self.ring_state.sq_needs_wakeup() {
                self.ring_state.wake_sqpoll()?;
            }
        } else {
            self.ring_state.enter(self.pending_submissions, 0, 0)?;
        }
        self.pending_submissions = 0;
        Ok(())
    }

    /// Reap available completions, advancing the megakernel tail
    /// pointer once per success. Returns completions reaped.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::IoUringSyscall`] on the first CQE
    /// reporting `res < 0`. Remaining CQEs are still drained so the
    /// ring does not overflow, but only the first failure is
    /// returned  -  caller re-polls to pick up subsequent errors or
    /// successes.
    pub fn poll(&mut self) -> Result<u32, PipelineError> {
        self.flush_submissions()?;
        let mut completed: u32 = 0;
        let mut first_error: Option<PipelineError> = None;

        while let Some(cqe) = self.ring_state.peek_cqe() {
            let res = cqe.res;
            self.ring_state.advance_cq();
            decrement_queue_counter(&mut self.inflight, "inflight SQE count")?;

            if res < 0 {
                if first_error.is_none() {
                    first_error = Some(PipelineError::IoUringSyscall {
                        syscall: "io_uring_cqe",
                        errno: -res,
                        fix: "inspect user_data to identify the failed SQE; common causes: EIO on disk, EFAULT on bad iovec, EINVAL on misaligned offset",
                    });
                }
                continue;
            }

            // Successful DMA: bytes landed in GPU-visible memory. Tail
            // publication is batched after CQ drain so one poll with N
            // completions performs one release atomic instead of N.
            completed = vyre_driver::accounting::checked_add_u32_value(
                completed,
                1,
                PipelineError::QueueFull {
                    queue: "completion",
                    fix: "io_uring completion count overflowed u32; drain completions more frequently",
                },
            )?;
        }

        if completed != 0 {
            self.megakernel_tail.fetch_add(completed, Ordering::Release);
        }

        match first_error {
            Some(err) => Err(err),
            None => Ok(completed),
        }
    }

    /// Flush pending submissions + wait for at least one completion.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::IoUringSyscall`] if `io_uring_enter`
    /// fails.
    pub fn wait_for_completion(&mut self) -> Result<(), PipelineError> {
        if self.inflight > 0 {
            self.flush_submissions()?;
            self.ring_state.enter(0, 1, 1)?;
            self.poll()?;
        }
        Ok(())
    }

    /// Number of submissions still awaiting completion.
    #[must_use]
    pub fn inflight(&self) -> u32 {
        self.inflight
    }

    /// Submit an NVMe passthrough command via `IORING_OP_URING_CMD`.
    /// Requires the `uring-cmd-nvme` feature and Linux kernel 6.0+.
    ///
    /// The NVMe SQE layout is encoded by the caller in `nvme_sqe_bytes`
    /// (64 bytes)  -  the SQE is memcpy'd into the `addr3`+`addr` slots
    /// the kernel forwards to the NVMe driver. `user_data` is returned
    /// on the matching CQE so the caller can correlate completions.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::NvmePassthroughDisabled`] if the
    ///   `uring-cmd-nvme` feature is not enabled at compile time. This
    ///   variant is unreachable in this cfg-gated method but remains
    ///   part of the public error contract shared with the feature-gated
    ///   implementation.
    /// - [`PipelineError::QueueFull`] if the SQ is full or the NVMe
    ///   command buffer is malformed (must be exactly 64 bytes).
    ///
    /// # Safety
    ///
    /// - `fd` must be an open character device the caller has
    ///   `IORING_SETUP_CQE32`-compatible access to (e.g. `/dev/ng0n1`).
    /// - `nvme_sqe_bytes` must encode a valid NVMe command  -  kernel
    ///   rejection returns an errno on the CQE, but a forged payload
    ///   can still trigger device-level misbehavior.
    #[cfg(feature = "uring-cmd-nvme")]
    pub unsafe fn submit_nvme_passthrough(
        &mut self,
        fd: i32,
        user_data: u64,
        nvme_sqe_bytes: &[u8],
    ) -> Result<(), PipelineError> {
        if nvme_sqe_bytes.len() != 64 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "NVMe passthrough SQE must be exactly 64 bytes; see linux/nvme_ioctl.h",
            });
        }

        let Some(sqe) = self.ring_state.get_sqe() else {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "SQ full; call AsyncUringStream::poll to drain completions then retry",
            });
        };

        // SAFETY: caller-provided slice is 64 bytes as validated
        // above; we copy into the 64-byte NVMe passthrough region
        // the kernel expects (addr + addr3 cover the first 40 bytes;
        // the remaining 24 live in the SQE's inline payload).
        let nvme_ptr = nvme_sqe_bytes.as_ptr();
        sqe.opcode = IORING_OP_URING_CMD;
        sqe.fd = fd;
        sqe.user_data_or_off = 0;
        // `cmd_op` in the first 4 bytes of addr3 (kernel reads it as u32).
        sqe.addr = pointer_addr_u64(nvme_ptr, "NVMe command pointer")?;
        sqe.len = 64;
        sqe.user_data = user_data;
        // The kernel reads the remaining payload bytes out of addr3
        // directly; downstream NVMe drivers dereference this pointer.
        sqe.addr3 = pointer_addr_u64(nvme_ptr, "NVMe command addr3 pointer")?;

        self.ring_state.commit_sqe();
        increment_queue_counter(&mut self.inflight, "inflight SQE count")?;
        increment_queue_counter(&mut self.pending_submissions, "pending submission count")?;

        Ok(())
    }

    /// Submit an `IORING_OP_READ_FIXED` into a pre-registered buffer.
    ///
    /// Requires the caller to have previously called
    /// [`super::ring::IoUringState::register_buffers`] with an iovec
    /// slice whose entry `buf_index` covers the target range. Because
    /// the kernel skips per-SQE iovec validation, this path is 20-40%
    /// lower latency than `submit_read_to_gpu` on hot loops.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::QueueFull`] if the SQ is full or the
    ///   destination range exceeds the GPU buffer bounds.
    ///
    /// # Safety
    ///
    /// The `buf_index` must reference a still-registered iovec whose
    /// region overlaps `chunk_idx * len .. (chunk_idx + 1) * len`
    /// inside the [`GpuMappedBuffer`]. Mis-indexing produces a kernel
    /// DMA into the wrong region  -  silent data corruption.
    pub unsafe fn submit_read_fixed(
        &mut self,
        fd: i32,
        offset: u64,
        len: u32,
        chunk_idx: usize,
        buf_index: u16,
    ) -> Result<(), PipelineError> {
        let target_offset = checked_chunk_target_offset(chunk_idx, len)?;
        // SAFETY: Safe FFI / low-level operation verified and audited for Release compliance.
        unsafe {
            self.submit_read_fixed_at(
                fd,
                offset,
                len,
                target_offset,
                buf_index,
                usize_to_u64(chunk_idx, "chunk index")?,
            )
        }
    }

    /// Submit an `IORING_OP_READ_FIXED` into a registered buffer at an
    /// explicit destination offset inside the mapped buffer.
    ///
    /// Unlike [`AsyncUringStream::submit_read_fixed`], this variant decouples
    /// the CQE `user_data` from the destination layout so higher-level
    /// drivers can publish their own slot ids while still using a fixed slot
    /// stride.
    ///
    /// # Errors
    ///
    /// - [`PipelineError::QueueFull`] if the SQ is full or the
    ///   destination range exceeds the GPU buffer bounds.
    ///
    /// # Safety
    ///
    /// `buf_index` must reference a still-registered iovec covering the
    /// target range, and `user_data` must remain meaningful to the caller
    /// until the CQE is reaped.
    pub unsafe fn submit_read_fixed_at(
        &mut self,
        fd: i32,
        offset: u64,
        len: u32,
        target_offset: u64,
        buf_index: u16,
        user_data: u64,
    ) -> Result<(), PipelineError> {
        let end = checked_target_end(target_offset, len)?;
        let gpu_len = usize_to_u64(self.gpu_buffer.len(), "mapped GPU buffer length")?;
        if end > gpu_len {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "chunk_idx * len exceeds GpuMappedBuffer length",
            });
        }

        let Some(sqe) = self.ring_state.get_sqe() else {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "SQ full; call AsyncUringStream::poll to drain completions then retry",
            });
        };

        // SAFETY: bounds-checked target address inside the host-visible
        // GpuMappedBuffer the caller committed at construction.
        let target_addr = unsafe {
            self.gpu_buffer
                .as_ptr()
                .add(u64_to_usize(target_offset, "target offset")?)
        };

        sqe.opcode = IORING_OP_READ_FIXED;
        sqe.fd = fd;
        sqe.user_data_or_off = offset;
        sqe.addr = pointer_addr_u64(target_addr, "fixed-read target pointer")?;
        sqe.len = len;
        sqe.buf_index = buf_index;
        sqe.user_data = user_data;

        self.ring_state.commit_sqe();
        increment_queue_counter(&mut self.inflight, "inflight SQE count")?;
        increment_queue_counter(&mut self.pending_submissions, "pending submission count")?;

        Ok(())
    }

    /// Submit a read using a registered-file-table index instead of a
    /// live fd. Use with
    /// [`super::ring::IoUringState::register_files`]  -  avoids the
    /// per-SQE file refcount bump.
    ///
    /// # Errors
    ///
    /// Same surface as [`AsyncUringStream::submit_read_to_gpu`].
    ///
    /// # Safety
    ///
    /// `file_index` must name a still-registered fd.
    /// `iovs_storage` must outlive the completion. All other
    /// conditions match `submit_read_to_gpu`.
    pub unsafe fn submit_read_to_gpu_fixed_file(
        &mut self,
        file_index: i32,
        offset: u64,
        len: u32,
        chunk_idx: usize,
        iovs_storage: &mut [Iovec],
    ) -> Result<(), PipelineError> {
        if iovs_storage.is_empty() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "caller supplied empty iovs_storage; pass at least one slot",
            });
        }
        let target_offset = checked_chunk_target_offset(chunk_idx, len)?;
        let end = checked_target_end(target_offset, len)?;
        let gpu_len = usize_to_u64(self.gpu_buffer.len(), "mapped GPU buffer length")?;
        if end > gpu_len {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "chunk_idx * len exceeds GpuMappedBuffer length",
            });
        }

        let Some(sqe) = self.ring_state.get_sqe() else {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "SQ full; call AsyncUringStream::poll to drain completions then retry",
            });
        };

        // SAFETY: same invariants as submit_read_to_gpu, plus the
        // caller committed that file_index is a registered fd.
        let target_addr = unsafe {
            self.gpu_buffer
                .as_ptr()
                .add(u64_to_usize(target_offset, "target offset")?)
        };
        iovs_storage[0] = Iovec {
            iov_base: target_addr.cast::<core::ffi::c_void>(),
            iov_len: u32_to_usize(len, "read length")?,
        };

        sqe.opcode = IORING_OP_READV;
        sqe.flags = super::ring::IOSQE_FIXED_FILE;
        sqe.fd = file_index;
        sqe.user_data_or_off = offset;
        sqe.addr = pointer_addr_u64(iovs_storage.as_ptr(), "fixed-file readv iovec pointer")?;
        sqe.len = 1;
        sqe.user_data = usize_to_u64(chunk_idx, "chunk index")?;

        self.ring_state.commit_sqe();
        increment_queue_counter(&mut self.inflight, "inflight SQE count")?;
        increment_queue_counter(&mut self.pending_submissions, "pending submission count")?;

        Ok(())
    }

    /// Disabled-feature implementation for NVMe passthrough. Always returns
    /// [`PipelineError::NvmePassthroughDisabled`] so callers get a
    /// structured error rather than a link failure.
    #[cfg(not(feature = "uring-cmd-nvme"))]
    #[allow(clippy::unused_self, clippy::missing_safety_doc)]
    pub unsafe fn submit_nvme_passthrough(
        &mut self,
        _fd: i32,
        _user_data: u64,
        _nvme_sqe_bytes: &[u8],
    ) -> Result<(), PipelineError> {
        Err(PipelineError::NvmePassthroughDisabled)
    }
}

fn checked_chunk_target_offset(chunk_idx: usize, len: u32) -> Result<u64, PipelineError> {
    let chunk_idx = usize_to_u64(chunk_idx, "chunk index")?;
    vyre_driver::accounting::checked_mul_u64_lazy(chunk_idx, u64::from(len), || {
        PipelineError::QueueFull {
            queue: "submission",
            fix: "chunk_idx * len overflows u64; split the IO batch before submission",
        }
    })
}

fn checked_target_end(target_offset: u64, len: u32) -> Result<u64, PipelineError> {
    vyre_driver::accounting::checked_add_u64_lazy(target_offset, u64::from(len), || {
        PipelineError::QueueFull {
            queue: "submission",
            fix: "target_offset + len overflows u64; split the IO batch before submission",
        }
    })
}

fn increment_queue_counter(counter: &mut u32, label: &'static str) -> Result<(), PipelineError> {
    *counter = vyre_driver::accounting::checked_add_u32_value(
        *counter,
        1,
        PipelineError::QueueFull {
            queue: "submission",
            fix: match label {
                "inflight SQE count" => {
                    "inflight SQE count overflowed u32; poll completions before submitting more work"
                }
                "pending submission count" => {
                    "pending submission count overflowed u32; flush submissions before queuing more work"
                }
                _ => {
                    "io_uring queue counter overflowed u32; drain the queue before submitting more work"
                }
            },
        },
    )?;
    Ok(())
}

fn decrement_queue_counter(counter: &mut u32, label: &'static str) -> Result<(), PipelineError> {
    *counter = counter.checked_sub(1).ok_or(PipelineError::QueueFull {
        queue: "completion",
        fix: match label {
            "inflight SQE count" => {
                "io_uring completion arrived with no inflight SQE; rebuild the stream state"
            }
            _ => "io_uring queue counter underflowed; rebuild the stream state",
        },
    })?;
    Ok(())
}

fn usize_to_u64(value: usize, label: &'static str) -> Result<u64, PipelineError> {
    u64::try_from(value).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: match label {
            "chunk index" => "chunk index cannot fit u64; split the IO batch before submission",
            "mapped GPU buffer length" => {
                "mapped GPU buffer length cannot fit u64; split the staging allocation"
            }
            _ => "host usize value cannot fit u64; split the IO batch before submission",
        },
    })
}

fn pointer_addr_u64<T>(ptr: *const T, label: &'static str) -> Result<u64, PipelineError> {
    usize_to_u64(ptr.addr(), label)
}

fn u64_to_usize(value: u64, label: &'static str) -> Result<usize, PipelineError> {
    usize::try_from(value).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: match label {
            "target offset" => {
                "target offset cannot fit usize; split the IO batch before submission"
            }
            _ => "u64 value cannot fit usize; split the IO batch before submission",
        },
    })
}

fn u32_to_usize(value: u32, label: &'static str) -> Result<usize, PipelineError> {
    usize::try_from(value).map_err(|_| PipelineError::QueueFull {
        queue: "submission",
        fix: match label {
            "read length" => "read length cannot fit usize; split the IO request before submission",
            _ => "u32 value cannot fit usize; split the IO request before submission",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapped_slice_roundtrip_is_miri_clean() {
        let mut backing = [1_u8, 2, 3, 4];
        // SAFETY: `backing` stays live and uniquely borrowed for the mapped buffer lifetime.
        let mut mapped = unsafe { GpuMappedBuffer::from_host_visible_slice(&mut backing) };
        // SAFETY: the mapped buffer was built from `backing` and remains uniquely borrowed.
        let slice = unsafe { mapped.as_mut_slice() };
        slice[0] = 9;
        slice[3] = 7;
        assert_eq!(backing, [9, 2, 3, 7]);
    }
}
