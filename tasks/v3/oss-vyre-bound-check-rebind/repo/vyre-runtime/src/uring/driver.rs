//! End-to-end ingest driver: file/NVMe -> io_uring -> mapped slot -> io_queue.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::megakernel::MegakernelIoQueue;
use crate::PipelineError;

#[cfg(feature = "uring-cmd-nvme")]
use super::gpudirect::encode_nvme_read_sqe;
use super::gpudirect::GpuDirectCapability;
use super::stream::{AsyncUringStream, GpuMappedBuffer, Iovec};

#[derive(Debug)]
struct PendingIngest {
    _file: Option<File>,
    tag: u32,
    completion: PendingCompletion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "uring-cmd-nvme"), allow(dead_code))]
enum PendingCompletion {
    ByteCountFromCqe,
    NativeNvmeStatus { expected_byte_count: u32 },
}

/// Host-visible completion surfaced after the DMA completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedIngest {
    /// Queue slot that completed.
    pub slot: u32,
    /// Number of bytes the kernel reported as transferred.
    pub byte_count: u32,
    /// Caller-defined tag mirrored into the `io_queue`.
    pub tag: u32,
}

/// Runtime telemetry for NVMe/file ingest into GPU-visible memory.
///
/// `cpu_bounce_bytes` is intentionally part of the public snapshot. The
/// `NvmeGpuIngestDriver` never targets an ordinary userspace bounce buffer, so
/// this counter must remain zero across both registered mapped reads and native
/// GPUDirect NVMe passthrough.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NvmeGpuIngestTelemetry {
    /// Bytes submitted to io_uring reads.
    pub submitted_bytes: u64,
    /// Bytes completed and published into the megakernel IO queue.
    pub completed_bytes: u64,
    /// Total read submissions accepted by io_uring.
    pub submitted_reads: u64,
    /// Completed reads published into the megakernel IO queue.
    pub completed_reads: u64,
    /// Submissions using `IORING_OP_READ_FIXED` into registered GPU-visible memory.
    pub registered_mapped_read_submissions: u64,
    /// Submissions using native `IORING_OP_URING_CMD` NVMe passthrough into BAR1 memory.
    pub gpudirect_nvme_submissions: u64,
    /// Bytes copied through ordinary userspace bounce buffers.
    pub cpu_bounce_bytes: u64,
    /// CQEs that completed with an error or without matching pending metadata.
    pub failed_completions: u64,
}

impl NvmeGpuIngestTelemetry {
    /// Inflight read count derived from accepted submissions and terminal CQEs.
    #[must_use]
    pub fn inflight_reads(self) -> u64 {
        self.submitted_reads
            .saturating_sub(self.completed_reads)
            .saturating_sub(self.failed_completions)
    }

    /// Read submissions recorded for a specific native ingest path.
    #[must_use]
    pub fn path_submissions(self, path: NativeReadPath) -> u64 {
        match path {
            NativeReadPath::RegisteredMappedRead => self.registered_mapped_read_submissions,
            NativeReadPath::GpuDirectNvmePassthrough => self.gpudirect_nvme_submissions,
        }
    }

    /// Validate that the snapshot describes a completed zero-copy ingest run for `path`.
    ///
    /// This method intentionally does not validate a benchmark's expected byte
    /// count; it validates the runtime invariant that every submitted read on
    /// the selected path completed without CPU bounce buffers or path mixing.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] with an actionable fix when the
    /// snapshot reports a CPU bounce copy, failed/inflight reads, incomplete
    /// byte accounting, incomplete read accounting, or submissions on the
    /// wrong native ingest path.
    pub fn validate_completed_zero_copy(self, path: NativeReadPath) -> Result<(), PipelineError> {
        if self.cpu_bounce_bytes != 0 {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest copied {} bytes through a CPU bounce buffer. Fix: route reads through registered GPU-visible slots or native GPUDirect NVMe passthrough.",
                self.cpu_bounce_bytes
            )));
        }
        if self.failed_completions != 0 {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest reported {} failed completions. Fix: inspect CQE status before publishing slots to the megakernel IO queue.",
                self.failed_completions
            )));
        }
        let inflight = self.inflight_reads();
        if inflight != 0 {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest left {inflight} reads inflight. Fix: drain completions before taking release telemetry snapshots."
            )));
        }
        if self.submitted_bytes != self.completed_bytes {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest byte accounting mismatch: submitted={}, completed={}. Fix: account CQE byte counts exactly once.",
                self.submitted_bytes, self.completed_bytes
            )));
        }
        if self.submitted_reads != self.completed_reads {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest read accounting mismatch: submitted={}, completed={}. Fix: account every terminal CQE exactly once.",
                self.submitted_reads, self.completed_reads
            )));
        }
        let selected_path_submissions = self.path_submissions(path);
        if selected_path_submissions != self.submitted_reads {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest path submission mismatch for {path:?}: path_submissions={}, submitted_reads={}. Fix: construct the driver with the same native read path used by the benchmark.",
                selected_path_submissions, self.submitted_reads
            )));
        }
        let mixed_path_submissions = match path {
            NativeReadPath::RegisteredMappedRead => self.gpudirect_nvme_submissions,
            NativeReadPath::GpuDirectNvmePassthrough => self.registered_mapped_read_submissions,
        };
        if mixed_path_submissions != 0 {
            return Err(PipelineError::Backend(format!(
                "NVMe GPU ingest mixed {mixed_path_submissions} submissions from the non-selected path into {path:?}. Fix: keep registered mapped reads and native GPUDirect passthrough telemetry separate."
            )));
        }
        Ok(())
    }

    fn record_submit(
        &mut self,
        path: NativeReadPath,
        byte_count: u32,
    ) -> Result<(), PipelineError> {
        self.submitted_bytes = checked_telemetry_add(
            self.submitted_bytes,
            u64::from(byte_count),
            "submitted bytes",
        )?;
        self.submitted_reads = checked_telemetry_add(self.submitted_reads, 1, "submitted reads")?;
        match path {
            NativeReadPath::RegisteredMappedRead => {
                self.registered_mapped_read_submissions = checked_telemetry_add(
                    self.registered_mapped_read_submissions,
                    1,
                    "registered mapped read submissions",
                )?;
            }
            NativeReadPath::GpuDirectNvmePassthrough => {
                self.gpudirect_nvme_submissions = checked_telemetry_add(
                    self.gpudirect_nvme_submissions,
                    1,
                    "GPUDirect NVMe submissions",
                )?;
            }
        }
        Ok(())
    }

    fn record_complete(&mut self, byte_count: u32) -> Result<(), PipelineError> {
        self.completed_bytes = checked_telemetry_add(
            self.completed_bytes,
            u64::from(byte_count),
            "completed bytes",
        )?;
        self.completed_reads = checked_telemetry_add(self.completed_reads, 1, "completed reads")?;
        Ok(())
    }

    fn record_failed_completion(&mut self) -> Result<(), PipelineError> {
        self.failed_completions =
            checked_telemetry_add(self.failed_completions, 1, "failed completions")?;
        Ok(())
    }
}

/// Native-read strategy used by [`NvmeGpuIngestDriver`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeReadPath {
    /// `IORING_OP_READ_FIXED` into a registered GPU-visible mapping.
    ///
    /// This removes the userspace bounce buffer but still uses normal file
    /// reads submitted by the CPU. It is the compatibility path for filesystems
    /// and GPU memory APIs that do not expose BAR1 peer DMA.
    RegisteredMappedRead,
    /// `IORING_OP_URING_CMD` NVMe read into BAR1 peer memory.
    ///
    /// This is the canonical native ingest path: CPU submits one NVMe command,
    /// the device DMAs bytes directly into GPU-owned memory, and the megakernel
    /// consumes the published slot.
    GpuDirectNvmePassthrough,
}

/// Wire the Linux ingest loop end-to-end without userspace bounce buffers.
pub struct NvmeGpuIngestDriver<'a> {
    stream: AsyncUringStream<'a>,
    mapped_slots: Vec<GpuMappedBuffer<'a>>,
    registered_iovecs: Vec<Iovec>,
    megakernel_io_queue: MegakernelIoQueue,
    pending: Vec<Option<PendingIngest>>,
    slot_bytes: usize,
    read_path: NativeReadPath,
    telemetry: NvmeGpuIngestTelemetry,
}

impl<'a> NvmeGpuIngestDriver<'a> {
    /// Split one mapped staging buffer into `slot_count` fixed-size slots and
    /// register those slots for `IORING_OP_READ_FIXED`.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the buffer cannot be evenly
    /// partitioned into non-empty slots, or an io_uring syscall error if
    /// buffer registration fails.
    pub fn new(
        stream: AsyncUringStream<'a>,
        slot_count: u32,
        megakernel_io_queue: MegakernelIoQueue,
    ) -> Result<Self, PipelineError> {
        Self::new_with_path(
            stream,
            slot_count,
            megakernel_io_queue,
            NativeReadPath::RegisteredMappedRead,
        )
    }

    /// Construct a driver that requires native GPUDirect NVMe passthrough.
    ///
    /// This constructor fails loudly when `uring-cmd-nvme` is not compiled in
    /// or `nvidia-fs` is not active. Callers that need the VYRE canonical path
    /// should use this instead of [`NvmeGpuIngestDriver::new`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::NvmePassthroughDisabled`] when the feature is
    /// absent, [`PipelineError::Backend`] when the host probe rejects
    /// GPUDirect, or the same slot-partitioning errors as
    /// [`NvmeGpuIngestDriver::new`].
    pub fn new_gpudirect(
        stream: AsyncUringStream<'a>,
        slot_count: u32,
        megakernel_io_queue: MegakernelIoQueue,
    ) -> Result<Self, PipelineError> {
        match GpuDirectCapability::probe() {
            GpuDirectCapability::Available { .. } => Self::new_with_path(
                stream,
                slot_count,
                megakernel_io_queue,
                NativeReadPath::GpuDirectNvmePassthrough,
            ),
            GpuDirectCapability::FeatureDisabled => Err(PipelineError::NvmePassthroughDisabled),
            GpuDirectCapability::Unavailable { reason } => Err(PipelineError::Backend(format!(
                "GPUDirect native read unavailable: {reason}. Fix: install/enable nvidia-fs, use a BAR1-backed GpuMappedBuffer, or use NvmeGpuIngestDriver::new for registered mapped reads."
            ))),
        }
    }

    fn new_with_path(
        stream: AsyncUringStream<'a>,
        slot_count: u32,
        megakernel_io_queue: MegakernelIoQueue,
        read_path: NativeReadPath,
    ) -> Result<Self, PipelineError> {
        let total_len = stream.gpu_buffer.len();
        let slot_count_usize =
            usize::try_from(slot_count).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "slot_count does not fit host usize; reduce the ingest slot count",
            })?;
        let slot_bytes = partition_slot_bytes(total_len, slot_count_usize)?;

        let mut mapped_slots = Vec::new();
        let mut registered_iovecs = Vec::new();
        let mut pending = Vec::new();
        reserve_ingest_vec_capacity(
            &mut mapped_slots,
            slot_count_usize,
            "mapped GPU ingest slots",
        )?;
        reserve_ingest_vec_capacity(
            &mut registered_iovecs,
            slot_count_usize,
            "registered io_uring iovecs",
        )?;
        reserve_ingest_vec_capacity(&mut pending, slot_count_usize, "pending ingest slots")?;
        for slot in 0..slot_count_usize {
            let offset = slot * slot_bytes;
            let slot_buffer = stream.gpu_buffer.sub_region(offset, slot_bytes)?;
            registered_iovecs.push(Iovec {
                iov_base: slot_buffer.as_ptr().cast(),
                iov_len: slot_buffer.len(),
            });
            mapped_slots.push(slot_buffer);
        }
        pending.resize_with(slot_count_usize, || None);
        Ok(Self {
            stream,
            mapped_slots,
            registered_iovecs,
            megakernel_io_queue,
            pending,
            slot_bytes,
            read_path,
            telemetry: NvmeGpuIngestTelemetry::default(),
        })
    }

    /// Read an entire file into a fixed ingest slot.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the slot is already in
    /// flight or the file is larger than the slot capacity.
    pub fn submit_file(&mut self, path: &Path, slot: u32) -> Result<(), PipelineError> {
        let slot_usize = self.validate_slot_for_submit(slot)?;

        let file = File::open(path).map_err(|error| {
            PipelineError::Backend(format!("open {} failed: {error}", path.display()))
        })?;
        let file_len = file
            .metadata()
            .map_err(|error| {
                PipelineError::Backend(format!("stat {} failed: {error}", path.display()))
            })?
            .len();
        let slot_bytes_u64 = usize_to_u64(self.slot_bytes, "ingest slot byte length")?;
        if file_len > slot_bytes_u64 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "file exceeds the configured ingest slot size; enlarge the mapped staging buffer or segment the file",
            });
        }

        let byte_count = u32::try_from(file_len).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "file length exceeds u32 read size even though it fit the slot; split the ingest file",
        })?;
        let target_offset = slot_byte_offset(slot_usize, self.slot_bytes)?;
        let slot_iovec = &mut self.registered_iovecs[slot_usize..slot_usize + 1];
        // SAFETY: `slot_iovec` and file descriptor stay live until the CQE is reaped.
        unsafe {
            self.stream.submit_read_to_gpu_at(
                file.as_raw_fd(),
                0,
                byte_count,
                target_offset,
                slot_iovec,
            )?;
        }
        self.telemetry
            .record_submit(NativeReadPath::RegisteredMappedRead, byte_count)?;
        self.pending[slot_usize] = Some(PendingIngest {
            _file: Some(file),
            tag: slot,
            completion: PendingCompletion::ByteCountFromCqe,
        });
        Ok(())
    }

    /// Submit one native NVMe read directly into the mapped slot.
    ///
    /// `nvme_fd` must name an NVMe character device such as `/dev/ng0n1`.
    /// `mapped_slots[slot]` must be a BAR1 peer-memory region created with
    /// [`GpuMappedBuffer::from_bar1_peer_with_owner`]. On completion the slot is
    /// published to the megakernel `io_queue`; the CPU does not copy bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::NvmePassthroughDisabled`] when the build lacks
    /// native NVMe passthrough support, [`PipelineError::QueueFull`] when the
    /// slot or byte range is invalid, or [`PipelineError::Backend`] when this
    /// driver was constructed for the compatibility path.
    ///
    /// # Safety
    ///
    /// The caller must ensure `nvme_fd`, `namespace_id`, `starting_lba`, and
    /// `blocks` describe a valid device range, and that the mapped slot remains
    /// a valid peer-DMA destination until its CQE is reaped.
    #[cfg(feature = "uring-cmd-nvme")]
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn submit_native_nvme_read(
        &mut self,
        nvme_fd: i32,
        namespace_id: u32,
        starting_lba: u64,
        blocks: u32,
        bytes_per_block: u32,
        slot: u32,
    ) -> Result<(), PipelineError> {
        if self.read_path != NativeReadPath::GpuDirectNvmePassthrough {
            return Err(PipelineError::Backend(
                "native NVMe read submitted on a registered-mapped-read driver. Fix: construct with NvmeGpuIngestDriver::new_gpudirect and a BAR1-backed GpuMappedBuffer.".to_string(),
            ));
        }
        let slot_usize = self.validate_slot_for_submit(slot)?;
        if blocks == 0 || bytes_per_block == 0 {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "native NVMe reads require non-zero block count and bytes_per_block",
            });
        }
        let byte_count = vyre_driver::accounting::checked_mul_u32_value(
            blocks,
            bytes_per_block,
            PipelineError::QueueFull {
                queue: "submission",
                fix: "native NVMe read byte count overflowed u32; submit a smaller range",
            },
        )?;
        let byte_count_usize =
            usize::try_from(byte_count).map_err(|_| PipelineError::QueueFull {
                queue: "submission",
                fix: "native NVMe read byte count cannot fit host usize; submit a smaller range",
            })?;
        if byte_count_usize > self.slot_bytes {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "native NVMe read exceeds the configured ingest slot size; enlarge the BAR1 mapped slot or submit fewer blocks",
            });
        }

        let dest = usize_to_u64(
            self.mapped_slots[slot_usize].as_ptr().addr(),
            "mapped BAR1 destination pointer",
        )?;
        let sqe = encode_nvme_read_sqe(namespace_id, starting_lba, blocks, dest);
        let user_data = slot_byte_offset(slot_usize, self.slot_bytes)?;
        // SAFETY: forwarded from this method's contract; the SQE is built
        // from validated scalar fields and a slot-local BAR1 destination.
        unsafe {
            self.stream
                .submit_nvme_passthrough(nvme_fd, user_data, &sqe)?;
        }
        self.telemetry
            .record_submit(NativeReadPath::GpuDirectNvmePassthrough, byte_count)?;
        self.pending[slot_usize] = Some(PendingIngest {
            _file: None,
            tag: slot,
            completion: PendingCompletion::NativeNvmeStatus {
                expected_byte_count: byte_count,
            },
        });
        Ok(())
    }

    /// Disabled-feature variant of [`NvmeGpuIngestDriver::submit_native_nvme_read`].
    ///
    /// # Errors
    ///
    /// Always returns [`PipelineError::NvmePassthroughDisabled`].
    ///
    /// # Safety
    ///
    /// This method does not touch `nvme_fd`; it exists to keep the public API
    /// structured across feature sets.
    #[cfg(not(feature = "uring-cmd-nvme"))]
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn submit_native_nvme_read(
        &mut self,
        _nvme_fd: i32,
        _namespace_id: u32,
        _starting_lba: u64,
        _blocks: u32,
        _bytes_per_block: u32,
        _slot: u32,
    ) -> Result<(), PipelineError> {
        Err(PipelineError::NvmePassthroughDisabled)
    }

    /// Flush submissions, reap CQEs, and publish the completed slots into the
    /// megakernel `io_queue`.
    pub fn poll_completions(&mut self) -> Result<Vec<CompletedIngest>, PipelineError> {
        let mut completed = Vec::new();
        self.poll_completions_into(&mut completed)?;
        Ok(completed)
    }

    /// Flush submissions, reap CQEs, and append completed slots into
    /// caller-owned storage.
    ///
    /// Reusing `completed` across polls keeps the ingest hot path allocation
    /// free after driver construction.
    pub fn poll_completions_into(
        &mut self,
        completed: &mut Vec<CompletedIngest>,
    ) -> Result<(), PipelineError> {
        completed.clear();
        self.stream.flush_submissions()?;
        let inflight_capacity =
            usize::try_from(self.stream.inflight).map_err(|_| PipelineError::Backend(
                "io_uring inflight completion count cannot fit host usize. Fix: shard ingest submissions before polling completions."
                    .to_string(),
            ))?;
        reserve_ingest_vec_capacity(completed, inflight_capacity, "completed ingest records")?;
        let mut first_error: Option<PipelineError> = None;

        while let Some(cqe) = self.stream.ring_state.peek_cqe() {
            let res = cqe.res;
            if self.slot_bytes == 0 {
                return Err(PipelineError::Backend(
                    "io_uring ingest driver has zero slot_bytes. Fix: construct NvmeGpuIngestDriver with at least one non-empty mapped slot.".to_string(),
                ));
            }
            let user_data = usize::try_from(cqe.user_data).map_err(|_| {
                PipelineError::Backend(format!(
                    "io_uring CQE user_data {} does not fit host usize. Fix: keep slot byte offsets within host addressable range.",
                    cqe.user_data
                ))
            })?;
            let slot = user_data / self.slot_bytes;
            self.stream.ring_state.advance_cq();
            self.stream.inflight = self.stream.inflight.checked_sub(1).ok_or_else(|| {
                PipelineError::Backend(
                    "io_uring completion arrived with zero inflight submissions. Fix: audit submit/completion accounting before reusing this stream.".to_string(),
                )
            })?;

            let pending = self.pending.get_mut(slot).and_then(Option::take);
            if res < 0 {
                self.telemetry.record_failed_completion()?;
                if first_error.is_none() {
                    first_error = Some(PipelineError::IoUringSyscall {
                        syscall: "io_uring_cqe",
                        errno: -res,
                        fix: "inspect the offending file descriptor and slot metadata; common causes are EIO on disk or EFAULT on an invalid registered buffer",
                    });
                }
                continue;
            }

            let pending = match pending {
                Some(pending) => pending,
                None => {
                    self.telemetry.record_failed_completion()?;
                    if first_error.is_none() {
                        first_error = Some(PipelineError::Backend(format!(
                            "CQE for slot {slot} arrived without matching pending metadata"
                        )));
                    }
                    continue;
                }
            };
            let byte_count = match pending.completion {
                PendingCompletion::ByteCountFromCqe => {
                    u32::try_from(res).map_err(|_| PipelineError::Backend(format!(
                        "io_uring CQE byte count {res} cannot fit u32. Fix: split ingest reads so completions stay within the megakernel io_queue ABI."
                    )))?
                }
                PendingCompletion::NativeNvmeStatus {
                    expected_byte_count,
                } => {
                    if res != 0 {
                        self.telemetry.record_failed_completion()?;
                        if first_error.is_none() {
                            first_error = Some(PipelineError::Backend(format!(
                                "NVMe passthrough completion for slot {slot} returned non-zero status {res}. Fix: inspect namespace id, LBA range, permissions, and nvidia-fs state."
                            )));
                        }
                        continue;
                    }
                    expected_byte_count
                }
            };
            let slot_u32 = u32::try_from(slot).map_err(|_| PipelineError::Backend(format!(
                "io_uring completion slot {slot} cannot fit u32. Fix: shard ingest slots before publishing to the megakernel io_queue."
            )))?;
            self.megakernel_io_queue
                .publish_slot(slot_u32, slot_u32, byte_count, pending.tag)?;
            self.telemetry.record_complete(byte_count)?;
            completed.push(CompletedIngest {
                slot: slot_u32,
                byte_count,
                tag: pending.tag,
            });
        }

        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Borrow the raw io_queue bytes for backend upload/readback.
    #[must_use]
    pub fn megakernel_io_queue(&self) -> &MegakernelIoQueue {
        &self.megakernel_io_queue
    }

    /// Mutable access to the io_queue bytes.
    #[must_use]
    pub fn megakernel_io_queue_mut(&mut self) -> &mut MegakernelIoQueue {
        &mut self.megakernel_io_queue
    }

    /// Fixed slot size in bytes.
    #[must_use]
    pub fn slot_bytes(&self) -> usize {
        self.slot_bytes
    }

    /// Number of registered slots.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.registered_iovecs.len()
    }

    /// Read path this driver was constructed to use.
    #[must_use]
    pub fn read_path(&self) -> NativeReadPath {
        self.read_path
    }

    /// Snapshot ingest telemetry counters.
    #[must_use]
    pub fn telemetry_snapshot(&self) -> NvmeGpuIngestTelemetry {
        self.telemetry
    }

    /// Reset ingest telemetry counters without changing pending submissions.
    pub fn reset_telemetry(&mut self) {
        self.telemetry = NvmeGpuIngestTelemetry::default();
    }

    fn validate_slot_for_submit(&self, slot: u32) -> Result<usize, PipelineError> {
        let slot_usize = usize::try_from(slot).map_err(|_| PipelineError::QueueFull {
            queue: "submission",
            fix: "slot index cannot fit host usize; shard mapped ingest slots",
        })?;
        if slot_usize >= self.mapped_slots.len() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "slot exceeds the configured mapped-slot count",
            });
        }
        if self.pending[slot_usize].is_some() {
            return Err(PipelineError::QueueFull {
                queue: "submission",
                fix: "slot already has an in-flight ingest; drain completions before reusing it",
            });
        }
        Ok(slot_usize)
    }
}

fn checked_telemetry_add(
    current: u64,
    increment: u64,
    label: &'static str,
) -> Result<u64, PipelineError> {
    vyre_driver::accounting::checked_add_u64_lazy(current, increment, || {
        PipelineError::Backend(format!(
            "io_uring ingest telemetry {label} overflowed u64. Fix: snapshot and reset telemetry before counters saturate."
        ))
    })
}

fn usize_to_u64(value: usize, label: &'static str) -> Result<u64, PipelineError> {
    u64::try_from(value).map_err(|_| {
        PipelineError::Backend(format!(
            "{label} cannot fit u64. Fix: shard io_uring GPU ingest buffers before submission."
        ))
    })
}

fn slot_byte_offset(slot_idx: usize, slot_bytes: usize) -> Result<u64, PipelineError> {
    let offset = vyre_driver::accounting::checked_mul_usize_lazy(slot_idx, slot_bytes, || {
        PipelineError::Backend(
            "io_uring ingest slot byte offset overflowed usize. Fix: shard mapped ingest slots."
                .to_string(),
        )
    })?;
    usize_to_u64(offset, "io_uring ingest slot byte offset")
}

fn reserve_ingest_vec_capacity<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), PipelineError> {
    if vec.capacity() >= capacity {
        return Ok(());
    }
    vec.try_reserve_exact(capacity - vec.capacity())
        .map_err(|error| {
            PipelineError::Backend(format!(
                "io_uring GPU ingest failed to reserve {field} for {capacity} entries: {error}. Fix: reduce ingest slot fan-out or shard the ingest batch."
            ))
        })
}

fn partition_slot_bytes(total_len: usize, slot_count: usize) -> Result<usize, PipelineError> {
    if slot_count == 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "NvmeGpuIngestDriver requires at least one slot",
        });
    }
    let slot_bytes = total_len / slot_count;
    if slot_bytes == 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "mapped staging buffer is too small to partition into the requested slot count",
        });
    }
    if total_len % slot_count != 0 {
        return Err(PipelineError::QueueFull {
            queue: "submission",
            fix: "mapped staging buffer length must divide evenly by slot_count so every byte belongs to exactly one DMA slot",
        });
    }
    Ok(slot_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_slot_bytes_accepts_exact_slot_geometry() {
        assert_eq!(partition_slot_bytes(4096 * 8, 8).unwrap(), 4096);
    }

    #[test]
    fn partition_slot_bytes_rejects_zero_slots() {
        let error = partition_slot_bytes(4096, 0).expect_err("zero slots must fail");
        assert!(matches!(error, PipelineError::QueueFull { .. }));
    }

    #[test]
    fn partition_slot_bytes_rejects_remainder_bytes() {
        let error = partition_slot_bytes(4097, 4)
            .expect_err("remainder bytes create unreachable DMA capacity");
        assert!(matches!(error, PipelineError::QueueFull { .. }));
    }

    #[test]
    fn partition_slot_bytes_rejects_zero_byte_slots() {
        let error = partition_slot_bytes(3, 4).expect_err("zero-byte DMA slots must fail");
        assert!(matches!(error, PipelineError::QueueFull { .. }));
    }

    #[test]
    fn ingest_telemetry_tracks_zero_cpu_bounce_registered_reads() {
        let mut telemetry = NvmeGpuIngestTelemetry::default();
        telemetry
            .record_submit(NativeReadPath::RegisteredMappedRead, 4096)
            .expect("Fix: telemetry submit accounting must fit.");
        telemetry
            .record_complete(4096)
            .expect("Fix: telemetry completion accounting must fit.");

        assert_eq!(telemetry.submitted_bytes, 4096);
        assert_eq!(telemetry.completed_bytes, 4096);
        assert_eq!(telemetry.submitted_reads, 1);
        assert_eq!(telemetry.completed_reads, 1);
        assert_eq!(telemetry.registered_mapped_read_submissions, 1);
        assert_eq!(telemetry.gpudirect_nvme_submissions, 0);
        assert_eq!(telemetry.cpu_bounce_bytes, 0);
        assert_eq!(telemetry.inflight_reads(), 0);
    }

    #[test]
    fn ingest_telemetry_tracks_zero_cpu_bounce_gpudirect_reads() {
        let mut telemetry = NvmeGpuIngestTelemetry::default();
        telemetry
            .record_submit(NativeReadPath::GpuDirectNvmePassthrough, 8192)
            .expect("Fix: telemetry submit accounting must fit.");
        telemetry
            .record_failed_completion()
            .expect("Fix: telemetry failure accounting must fit.");

        assert_eq!(telemetry.submitted_bytes, 8192);
        assert_eq!(telemetry.completed_bytes, 0);
        assert_eq!(telemetry.gpudirect_nvme_submissions, 1);
        assert_eq!(telemetry.registered_mapped_read_submissions, 0);
        assert_eq!(telemetry.failed_completions, 1);
        assert_eq!(telemetry.cpu_bounce_bytes, 0);
        assert_eq!(telemetry.inflight_reads(), 0);
    }

    #[test]
    fn ingest_telemetry_reports_overflow_instead_of_wrapping() {
        let error = checked_telemetry_add(u64::MAX, 1, "test counter")
            .expect_err("Fix: telemetry counters must fail before wrapping.");
        assert!(
            error.to_string().contains("overflowed u64"),
            "Fix: telemetry overflow errors must be actionable: {error}"
        );
    }

    #[test]
    fn ingest_staging_reservation_reports_capacity_overflow() {
        let mut bytes = Vec::<u8>::new();
        let error = reserve_ingest_vec_capacity(&mut bytes, usize::MAX, "test ingest bytes")
            .expect_err("Fix: impossible ingest staging capacity must be a typed error.");

        assert!(
            error
                .to_string()
                .contains("failed to reserve test ingest bytes"),
            "Fix: ingest staging reserve failure must name the failed field: {error}"
        );
    }

    #[test]
    fn ingest_staging_reservation_reuses_existing_capacity() {
        let mut bytes = Vec::<u8>::with_capacity(8);
        let original_capacity = bytes.capacity();

        reserve_ingest_vec_capacity(&mut bytes, 4, "test ingest bytes")
            .expect("Fix: lower target capacity should reuse existing staging.");

        assert_eq!(bytes.capacity(), original_capacity);
    }
}
