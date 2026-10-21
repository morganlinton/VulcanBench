use crate::PipelineError;
use vyre_driver::backend::{OutputBuffers, Resource};

/// Per-dispatch host-side runtime instrumentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelDispatchStats {
    /// Bytes supplied to the backend across control, ring, debug, and IO buffers.
    pub input_bytes: u64,
    /// Bytes returned by the backend across all output buffers.
    pub output_bytes: u64,
    /// Host-visible readback bytes returned by this dispatch.
    pub readback_bytes: u64,
    /// Total host-visible bytes moved for this dispatch.
    pub bytes_moved: u64,
    /// Conservative host-visible device allocation volume for this dispatch.
    pub device_allocation_bytes: u64,
    /// Conservative count of fresh host-visible device buffer allocations.
    pub device_allocation_events: u32,
    /// Host-observed dispatch latency in nanoseconds.
    pub latency_ns: u64,
    /// Number of output buffers returned by the backend.
    pub output_buffers: u32,
    /// Number of resident megakernel resource rows submitted to the backend.
    pub resident_resource_rows: u32,
    /// Number of resident resource handles submitted across all rows.
    pub resident_resource_handles: u32,
    /// Number of kernel launches issued for this logical megakernel dispatch.
    pub kernel_launches: u32,
    /// Number of host-visible synchronization points needed to collect outputs.
    pub sync_points: u32,
    /// True when the first dispatch failed with device-loss symptoms and the
    /// runtime rebuilt the compiled pipeline before retrying.
    pub recovered_after_device_loss: bool,
}

impl MegakernelDispatchStats {
    /// Throughput over returned output bytes in bytes per second.
    #[must_use]
    pub fn output_bytes_per_second(&self) -> u64 {
        bytes_per_second_or_panic(self.output_bytes, self.latency_ns, "output bytes")
    }

    /// Throughput over host-visible readback bytes in bytes per second.
    #[must_use]
    pub fn readback_bytes_per_second(&self) -> u64 {
        bytes_per_second_or_panic(self.readback_bytes, self.latency_ns, "readback bytes")
    }

    /// Total host-visible byte movement rate in bytes per second.
    #[must_use]
    pub fn bytes_moved_per_second(&self) -> u64 {
        bytes_per_second_or_panic(self.bytes_moved, self.latency_ns, "moved bytes")
    }

    /// Conservative allocation volume rate in bytes per second.
    #[must_use]
    pub fn device_allocation_bytes_per_second(&self) -> u64 {
        bytes_per_second_or_panic(
            self.device_allocation_bytes,
            self.latency_ns,
            "device allocation bytes",
        )
    }
}

fn bytes_per_second_or_panic(bytes: u64, latency_ns: u64, _label: &'static str) -> u64 {
    if latency_ns == 0 {
        return 0;
    }
    let scaled = (bytes as u128) * 1_000_000_000u128;
    let rate = scaled / u128::from(latency_ns);
    rate.min(u128::from(u64::MAX)) as u64
}

/// Backend outputs paired with host-side dispatch instrumentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegakernelDispatchOutput {
    /// Backend output buffers.
    pub buffers: Vec<Vec<u8>>,
    /// Host-side dispatch instrumentation.
    pub stats: MegakernelDispatchStats,
}

/// Backend outputs for a resident-handle batch plus aggregate host-side
/// instrumentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegakernelBatchDispatchOutput {
    /// One output-buffer set per submitted resident handle tuple.
    pub batches: Vec<Vec<Vec<u8>>>,
    /// Aggregate host-side dispatch instrumentation for the whole batch.
    pub stats: MegakernelDispatchStats,
}

/// Reusable host scratch for batched resident-handle megakernel dispatch.
///
/// This scratch owns the transient resource rows submitted to the backend and
/// the nested host readback buffers returned by batched dispatch. Reusing one
/// value across repeated batches avoids rebuilding `Vec<[Resource; 4]>`,
/// `Vec<Vec<Vec<u8>>>`, and per-output byte slots in many-small-launch loops.
#[derive(Debug, Default)]
pub struct MegakernelResidentBatchScratch {
    pub(super) resources: Vec<[Resource; 4]>,
    pub(super) batches: Vec<OutputBuffers>,
    pub(super) active_batches: usize,
}

impl MegakernelResidentBatchScratch {
    /// Create empty resident-batch scratch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Preallocate scratch for a known hot batch shape.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Backend`] when OOM prevents reserving the
    /// requested capacity. Callers must handle the error; the old infallible
    /// wrapper that silently returned empty scratch on failure has been removed.
    pub fn with_capacity(
        batch_count: usize,
        output_slots_per_batch: usize,
    ) -> Result<Self, PipelineError> {
        let mut resources = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut resources, batch_count)
            .map_err(|error| {
                PipelineError::Backend(format!(
                    "megakernel resident batch scratch could not reserve {batch_count} resource row(s): {error}. Fix: split persistent-handle batches before dispatch."
                ))
            })?;
        let mut batches = Vec::new();
        vyre_foundation::allocation::try_reserve_vec_to_capacity(&mut batches, batch_count)
            .map_err(|error| {
                PipelineError::Backend(format!(
                    "megakernel resident batch scratch could not reserve {batch_count} batch row(s): {error}. Fix: split persistent-handle batches before dispatch."
                ))
            })?;
        for _ in 0..batch_count {
            let mut outputs = Vec::new();
            vyre_foundation::allocation::try_reserve_vec_to_capacity(
                &mut outputs,
                output_slots_per_batch,
            )
            .map_err(|error| {
                PipelineError::Backend(format!(
                    "megakernel resident batch scratch could not reserve {output_slots_per_batch} output slot(s): {error}. Fix: reduce resident output fanout or split persistent-handle batches."
                ))
            })?;
            outputs.resize_with(output_slots_per_batch, Vec::new);
            batches.push(outputs);
        }
        Ok(Self {
            resources,
            batches,
            active_batches: 0,
        })
    }

    /// Retained decoded output batches from the most recent dispatch.
    #[must_use]
    pub fn batches(&self) -> &[OutputBuffers] {
        &self.batches[..self.active_batches.min(self.batches.len())]
    }

    /// Mutable retained output batches for callers that want to drain or
    /// decode in place after dispatch.
    pub fn batches_mut(&mut self) -> &mut Vec<OutputBuffers> {
        &mut self.batches
    }

    /// Clear logical scratch contents while retaining allocations.
    pub fn clear(&mut self) {
        self.resources.clear();
        self.active_batches = 0;
        for batch in &mut self.batches {
            for output in batch {
                output.clear();
            }
        }
    }

    /// Current retained resource-row capacity.
    #[must_use]
    pub fn resource_capacity(&self) -> usize {
        self.resources.capacity()
    }

    /// Current retained batch-row capacity.
    #[must_use]
    pub fn batch_capacity(&self) -> usize {
        self.batches.capacity()
    }
}

/// GPU-resident buffer handles for the four-buffer megakernel ABI.
///
/// Backends that implement persistent handles can keep control, ring, debug,
/// and IO queue buffers resident across launches. Runtime callers use this
/// type when a host byte mirror would force avoidable copies on the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelResidentHandles {
    /// Resident control-buffer handle.
    pub control: u64,
    /// Resident ring-buffer handle.
    pub ring: u64,
    /// Resident debug-log buffer handle.
    pub debug_log: u64,
    /// Resident IO-queue buffer handle.
    pub io_queue: u64,
}

impl MegakernelResidentHandles {
    /// Number of resident ABI resources passed to one persistent megakernel dispatch.
    pub const ABI_RESOURCE_COUNT: usize = 4;

    /// Construct resident handles in megakernel ABI binding order.
    #[must_use]
    pub const fn new(control: u64, ring: u64, debug_log: u64, io_queue: u64) -> Self {
        Self {
            control,
            ring,
            debug_log,
            io_queue,
        }
    }

    pub(super) fn resources(self) -> [Resource; Self::ABI_RESOURCE_COUNT] {
        [
            Resource::Resident(self.control),
            Resource::Resident(self.ring),
            Resource::Resident(self.debug_log),
            Resource::Resident(self.io_queue),
        ]
    }
}
