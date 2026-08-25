//! Host mirrors for megakernel GPU-resident runtime buffers.

use super::execution::{Megakernel, MegakernelDispatchStats, MegakernelResidentHandles};
use super::io;
use super::planner::MegakernelWorkItem;
use super::protocol;
use super::protocol_api::{validate_control_bytes, validate_debug_log_bytes};
use super::readback::MegakernelReadback;
use super::scheduler::write_default_priority_offsets;
use crate::PipelineError;
use vyre_driver::backend::OutputBuffers;

/// Reusable host storage for resident megakernel dispatch/update loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegakernelResidentDispatchScratch {
    readback: MegakernelReadback,
    outputs: OutputBuffers,
}

impl Default for MegakernelResidentDispatchScratch {
    fn default() -> Self {
        Self {
            readback: MegakernelReadback::default(),
            outputs: (0..MegakernelResidentHandles::ABI_RESOURCE_COUNT)
                .map(|_| Vec::new())
                .collect(),
        }
    }
}

impl MegakernelResidentDispatchScratch {
    /// Allocate empty scratch. The first dispatch sizes the internal buffers;
    /// later dispatches reuse those allocations through the backend `_into`
    /// path and readback swap-drain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of backend output slots retained for reuse.
    #[must_use]
    pub fn retained_output_slots(&self) -> usize {
        self.outputs.len()
    }

    /// Total bytes retained in reusable backend output slots.
    #[must_use]
    pub fn retained_output_bytes(&self) -> usize {
        self.outputs.iter().map(Vec::capacity).sum()
    }
}

/// Host-side mirror of the four buffers kept resident by the persistent
/// megakernel runtime: control, ring, debug log, and IO queue.
#[derive(Debug)]
pub struct MegakernelResidentBuffers {
    control_bytes: Vec<u8>,
    ring_bytes: Vec<u8>,
    debug_log_bytes: Vec<u8>,
    io_queue_bytes: Vec<u8>,
    slot_count: u32,
    scratch: MegakernelResidentDispatchScratch,
}

impl Clone for MegakernelResidentBuffers {
    fn clone(&self) -> Self {
        Self {
            control_bytes: self.control_bytes.clone(),
            ring_bytes: self.ring_bytes.clone(),
            debug_log_bytes: self.debug_log_bytes.clone(),
            io_queue_bytes: self.io_queue_bytes.clone(),
            slot_count: self.slot_count,
            scratch: MegakernelResidentDispatchScratch::new(),
        }
    }
}

impl PartialEq for MegakernelResidentBuffers {
    fn eq(&self, other: &Self) -> bool {
        self.control_bytes == other.control_bytes
            && self.ring_bytes == other.ring_bytes
            && self.debug_log_bytes == other.debug_log_bytes
            && self.io_queue_bytes == other.io_queue_bytes
            && self.slot_count == other.slot_count
    }
}

impl Eq for MegakernelResidentBuffers {}

impl MegakernelResidentBuffers {
    /// Allocate a fresh host mirror for a megakernel's resident buffers.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any runtime buffer size overflows.
    pub fn new(
        slot_count: u32,
        tenant_count: u32,
        observable_slots: u32,
    ) -> Result<Self, PipelineError> {
        let control_capacity = protocol::control_byte_len(observable_slots).ok_or_else(|| {
            PipelineError::Backend(
                "megakernel resident control byte length overflowed usize. Fix: shard observable resident buffers before allocation."
                    .to_string(),
            )
        })?;
        let ring_capacity = protocol::ring_byte_len(slot_count).ok_or_else(|| {
            PipelineError::Backend(
                "megakernel resident ring byte length overflowed usize. Fix: shard resident rings before allocation."
                    .to_string(),
            )
        })?;
        let debug_log_capacity =
            protocol::debug_log_byte_len(protocol::debug::RECORD_CAPACITY).ok_or_else(|| {
                PipelineError::Backend(
                    "megakernel resident debug-log byte length overflowed usize. Fix: reduce debug record capacity before allocation."
                        .to_string(),
                )
        })?;
        let io_queue_capacity = io::empty_io_queue_byte_len(io::IO_SLOT_COUNT)?;
        let mut control_bytes = Vec::new();
        reserve_resident_bytes(
            &mut control_bytes,
            control_capacity,
            "control",
            "shard observable resident buffers before allocation",
        )?;
        let mut ring_bytes = Vec::new();
        reserve_resident_bytes(
            &mut ring_bytes,
            ring_capacity,
            "ring",
            "shard resident rings before allocation",
        )?;
        let mut debug_log_bytes = Vec::new();
        reserve_resident_bytes(
            &mut debug_log_bytes,
            debug_log_capacity,
            "debug-log",
            "reduce debug record capacity before allocation",
        )?;
        let mut io_queue_bytes = Vec::new();
        reserve_resident_bytes(
            &mut io_queue_bytes,
            io_queue_capacity,
            "io-queue",
            "reduce resident IO queue capacity before allocation",
        )?;
        let mut buffers = Self {
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
            slot_count,
            scratch: MegakernelResidentDispatchScratch::new(),
        };
        buffers.reset(tenant_count, observable_slots)?;
        Ok(buffers)
    }

    /// Reinitialize this host mirror in place for the same resident geometry.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any runtime buffer size overflows.
    pub fn reset(&mut self, tenant_count: u32, observable_slots: u32) -> Result<(), PipelineError> {
        Megakernel::try_encode_control_into(
            false,
            tenant_count,
            observable_slots,
            &mut self.control_bytes,
        )?;
        write_default_priority_offsets(&mut self.control_bytes, self.slot_count)?;
        Megakernel::try_encode_empty_ring_into(self.slot_count, &mut self.ring_bytes)?;
        Megakernel::try_encode_empty_debug_log_into(
            protocol::debug::RECORD_CAPACITY,
            &mut self.debug_log_bytes,
        )?;
        io::try_encode_empty_io_queue_into(io::IO_SLOT_COUNT, &mut self.io_queue_bytes)?;
        Ok(())
    }

    /// Build a resident-buffer mirror from caller-owned byte buffers.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any buffer violates the megakernel ABI.
    pub fn from_parts(
        slot_count: u32,
        control_bytes: Vec<u8>,
        ring_bytes: Vec<u8>,
        debug_log_bytes: Vec<u8>,
        io_queue_bytes: Vec<u8>,
    ) -> Result<Self, PipelineError> {
        validate_control_bytes(&control_bytes)?;
        validate_debug_log_bytes(&debug_log_bytes)?;
        io::validate_io_queue_bytes(&io_queue_bytes)?;
        let expected_ring_bytes = protocol::ring_byte_len(slot_count).ok_or_else(|| {
            PipelineError::Backend(
                "megakernel resident ring byte length overflowed usize. Fix: shard resident rings before allocation."
                    .to_string(),
            )
        })?;
        if ring_bytes.len() != expected_ring_bytes {
            return Err(PipelineError::Backend(format!(
                "megakernel resident ring has {} bytes, expected {expected_ring_bytes}. Fix: build resident rings with the same slot_count as the Megakernel handle.",
                ring_bytes.len()
            )));
        }
        Ok(Self {
            control_bytes,
            ring_bytes,
            debug_log_bytes,
            io_queue_bytes,
            slot_count,
            scratch: MegakernelResidentDispatchScratch::new(),
        })
    }

    /// Publish one work slot into the resident ring mirror.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the slot is out of bounds or
    /// still in flight.
    pub fn publish_slot(
        &mut self,
        slot_idx: u32,
        tenant_id: u32,
        opcode: u32,
        args: &[u32],
    ) -> Result<(), PipelineError> {
        Megakernel::publish_slot(&mut self.ring_bytes, slot_idx, tenant_id, opcode, args)
    }

    /// Publish a contiguous fixed-ABI work-item window into the resident ring
    /// mirror without resetting unrelated slots.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::QueueFull`] when the target slots are outside
    /// the resident ring, still in flight, or contain an unpublished opcode.
    pub fn publish_work_items(
        &mut self,
        start_slot: u32,
        tenant_id: u32,
        items: &[MegakernelWorkItem],
    ) -> Result<u32, PipelineError> {
        Megakernel::publish_work_items(&mut self.ring_bytes, start_slot, tenant_id, items)
    }

    /// Apply a strict dispatch readback to the resident host mirror.
    pub fn apply_readback(&mut self, readback: MegakernelReadback) {
        self.control_bytes = readback.control_bytes;
        self.ring_bytes = readback.ring_bytes;
        self.debug_log_bytes = readback.debug_log_bytes;
        self.io_queue_bytes = readback.io_queue_bytes;
    }

    /// Dispatch these buffers through `megakernel`, then update the mirror.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch(
        &mut self,
        megakernel: &Megakernel,
    ) -> Result<MegakernelReadback, PipelineError> {
        self.dispatch_update(megakernel)?;
        Ok(self.snapshot_readback())
    }

    /// Dispatch these buffers through `megakernel` and update this mirror in
    /// place without cloning the readback into a second owned copy.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch_update(&mut self, megakernel: &Megakernel) -> Result<(), PipelineError> {
        self.dispatch_update_observed(megakernel)?;
        Ok(())
    }

    /// Dispatch these buffers through `megakernel`, update this mirror in
    /// place, and return dispatch instrumentation without cloning a snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch_update_observed(
        &mut self,
        megakernel: &Megakernel,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        if megakernel.slot_count() != self.slot_count {
            return Err(PipelineError::Backend(format!(
                "resident buffer slot_count {} does not match megakernel slot_count {}. Fix: allocate resident buffers from the same Megakernel geometry.",
                self.slot_count,
                megakernel.slot_count()
            )));
        }
        let stats = megakernel.dispatch_with_io_queue_readback_borrowed_into(
            &self.control_bytes,
            &self.ring_bytes,
            &self.debug_log_bytes,
            &self.io_queue_bytes,
            &mut self.scratch.readback,
            &mut self.scratch.outputs,
        )?;
        std::mem::swap(
            &mut self.control_bytes,
            &mut self.scratch.readback.control_bytes,
        );
        std::mem::swap(&mut self.ring_bytes, &mut self.scratch.readback.ring_bytes);
        std::mem::swap(
            &mut self.debug_log_bytes,
            &mut self.scratch.readback.debug_log_bytes,
        );
        std::mem::swap(
            &mut self.io_queue_bytes,
            &mut self.scratch.readback.io_queue_bytes,
        );
        Ok(stats)
    }

    /// Dispatch these buffers through `megakernel` using caller-owned scratch,
    /// update this mirror in place, and return dispatch instrumentation.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when dispatch or readback validation fails.
    pub fn dispatch_update_observed_with_scratch(
        &mut self,
        megakernel: &Megakernel,
        scratch: &mut MegakernelResidentDispatchScratch,
    ) -> Result<MegakernelDispatchStats, PipelineError> {
        if megakernel.slot_count() != self.slot_count {
            return Err(PipelineError::Backend(format!(
                "resident buffer slot_count {} does not match megakernel slot_count {}. Fix: allocate resident buffers from the same Megakernel geometry.",
                self.slot_count,
                megakernel.slot_count()
            )));
        }
        let stats = megakernel.dispatch_with_io_queue_readback_borrowed_into(
            &self.control_bytes,
            &self.ring_bytes,
            &self.debug_log_bytes,
            &self.io_queue_bytes,
            &mut scratch.readback,
            &mut scratch.outputs,
        )?;
        self.swap_readback_from(&mut scratch.readback);
        Ok(stats)
    }

    fn swap_readback_from(&mut self, readback: &mut MegakernelReadback) {
        std::mem::swap(&mut self.control_bytes, &mut readback.control_bytes);
        std::mem::swap(&mut self.ring_bytes, &mut readback.ring_bytes);
        std::mem::swap(&mut self.debug_log_bytes, &mut readback.debug_log_bytes);
        std::mem::swap(&mut self.io_queue_bytes, &mut readback.io_queue_bytes);
    }

    /// Clone the current host mirror into a strict readback record.
    #[must_use]
    pub fn snapshot_readback(&self) -> MegakernelReadback {
        MegakernelReadback {
            control_bytes: self.control_bytes.clone(),
            ring_bytes: self.ring_bytes.clone(),
            debug_log_bytes: self.debug_log_bytes.clone(),
            io_queue_bytes: self.io_queue_bytes.clone(),
        }
    }

    /// Clone the current host mirror into caller-owned readback storage.
    pub fn snapshot_readback_into(&self, out: &mut MegakernelReadback) {
        out.control_bytes.clone_from(&self.control_bytes);
        out.ring_bytes.clone_from(&self.ring_bytes);
        out.debug_log_bytes.clone_from(&self.debug_log_bytes);
        out.io_queue_bytes.clone_from(&self.io_queue_bytes);
    }

    /// Control-buffer mirror bytes.
    #[must_use]
    pub fn control_bytes(&self) -> &[u8] {
        &self.control_bytes
    }

    /// Ring-buffer mirror bytes.
    #[must_use]
    pub fn ring_bytes(&self) -> &[u8] {
        &self.ring_bytes
    }

    /// Mutable ring-buffer mirror bytes.
    #[must_use]
    pub fn ring_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.ring_bytes
    }

    /// Debug-log mirror bytes.
    #[must_use]
    pub fn debug_log_bytes(&self) -> &[u8] {
        &self.debug_log_bytes
    }

    /// IO-queue mirror bytes.
    #[must_use]
    pub fn io_queue_bytes(&self) -> &[u8] {
        &self.io_queue_bytes
    }

    /// Resident ring slot count.
    #[must_use]
    pub const fn slot_count(&self) -> u32 {
        self.slot_count
    }

    /// Number of backend output slots retained by the default resident
    /// dispatch scratch.
    #[must_use]
    pub fn retained_default_output_slots(&self) -> usize {
        self.scratch.retained_output_slots()
    }

    /// Total capacity retained by the default resident dispatch scratch.
    #[must_use]
    pub fn retained_default_output_bytes(&self) -> usize {
        self.scratch.retained_output_bytes()
    }
}

fn reserve_resident_bytes(
    bytes: &mut Vec<u8>,
    capacity: usize,
    label: &'static str,
    fix: &'static str,
) -> Result<(), PipelineError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(bytes, capacity).map_err(|error| {
        PipelineError::Backend(format!(
            "megakernel resident {label} byte reservation failed for {capacity} bytes: {error}. Fix: {fix}."
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::megakernel::protocol::opcode;
    use std::sync::Arc;
    use vyre_driver::backend::{CompiledPipeline, DispatchConfig, VyreBackend};

    struct ResidentEchoPipeline;

    impl vyre_driver::backend::private::Sealed for ResidentEchoPipeline {}

    impl CompiledPipeline for ResidentEchoPipeline {
        fn id(&self) -> &str {
            "resident-echo:pipeline"
        }

        fn dispatch(
            &self,
            inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<OutputBuffers, vyre_driver::BackendError> {
            Ok(inputs.to_vec())
        }

        fn dispatch_borrowed_into(
            &self,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), vyre_driver::BackendError> {
            if outputs.len() != inputs.len() {
                outputs.resize_with(inputs.len(), Vec::new);
            }
            for (slot, input) in outputs.iter_mut().zip(inputs.iter().copied()) {
                slot.clear();
                slot.extend_from_slice(input);
            }
            Ok(())
        }
    }

    struct ResidentEchoBackend;

    impl vyre_driver::backend::private::Sealed for ResidentEchoBackend {}

    impl VyreBackend for ResidentEchoBackend {
        fn id(&self) -> &'static str {
            "resident-echo"
        }

        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<OutputBuffers, vyre_driver::BackendError> {
            Ok(inputs.to_vec())
        }

        fn compile_native(
            &self,
            _program: &vyre_foundation::ir::Program,
            _config: &DispatchConfig,
        ) -> Result<Option<Arc<dyn CompiledPipeline>>, vyre_driver::BackendError> {
            Ok(Some(Arc::new(ResidentEchoPipeline)))
        }
    }

    #[test]
    fn resident_buffers_keep_runtime_abi_separate_from_publish_logic() {
        let mut buffers = MegakernelResidentBuffers::new(4, 2, 8).unwrap();
        buffers
            .publish_slot(2, 1, opcode::STORE_U32, &[7, 9])
            .unwrap();
        assert_eq!(buffers.slot_count(), 4);
        assert_eq!(
            buffers.ring_bytes().len(),
            protocol::ring_byte_len(4).unwrap()
        );
    }

    #[test]
    fn resident_buffers_publish_work_items_without_ring_reset() {
        let mut buffers = MegakernelResidentBuffers::new(4, 2, 8).unwrap();
        let sentinel = 0xCAFE_BABEu32;
        let sentinel_offset =
            (3 * protocol::SLOT_WORDS as usize + protocol::ARG0_WORD as usize) * 4;
        buffers.ring_bytes_mut()[sentinel_offset..sentinel_offset + 4]
            .copy_from_slice(&sentinel.to_le_bytes());
        let items = [MegakernelWorkItem {
            op_handle: opcode::STORE_U32,
            input_handle: 10,
            output_handle: 20,
            param: 30,
        }];

        let published = buffers.publish_work_items(1, 2, &items).unwrap();

        assert_eq!(published, 1);
        let read = |slot: usize, word: u32| {
            let start = (slot * protocol::SLOT_WORDS as usize + word as usize) * 4;
            u32::from_le_bytes(buffers.ring_bytes()[start..start + 4].try_into().unwrap())
        };
        assert_eq!(read(1, protocol::STATUS_WORD), protocol::slot::PUBLISHED);
        assert_eq!(read(1, protocol::OPCODE_WORD), opcode::STORE_U32);
        assert_eq!(read(1, protocol::TENANT_WORD), 2);
        assert_eq!(read(1, protocol::ARG0_WORD), 10);
        assert_eq!(read(1, protocol::ARG0_WORD + 1), 20);
        assert_eq!(read(1, protocol::ARG0_WORD + 2), 30);
        assert_eq!(read(3, protocol::ARG0_WORD), sentinel);
    }

    #[test]
    fn resident_buffers_seed_priority_offsets_for_priority_scheduler() {
        let buffers = MegakernelResidentBuffers::new(10, 2, 0).unwrap();
        let read = |word: u32| {
            let start = word as usize * 4;
            u32::from_le_bytes(
                buffers.control_bytes()[start..start + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(read(protocol::control::PRIORITY_OFFSETS_BASE), 0);
        assert_eq!(
            read(
                protocol::control::PRIORITY_OFFSETS_BASE + super::super::scheduler::PRIORITY_LEVELS
            ),
            10
        );
    }

    #[test]
    fn resident_buffers_reset_reuses_encoded_storage() {
        let mut buffers = MegakernelResidentBuffers::new(8, 2, 4).unwrap();
        let control_ptr = buffers.control_bytes.as_ptr();
        let ring_ptr = buffers.ring_bytes.as_ptr();
        let debug_ptr = buffers.debug_log_bytes.as_ptr();
        let io_ptr = buffers.io_queue_bytes.as_ptr();

        buffers.reset(2, 4).unwrap();

        assert_eq!(buffers.control_bytes.as_ptr(), control_ptr);
        assert_eq!(buffers.ring_bytes.as_ptr(), ring_ptr);
        assert_eq!(buffers.debug_log_bytes.as_ptr(), debug_ptr);
        assert_eq!(buffers.io_queue_bytes.as_ptr(), io_ptr);
        assert!(buffers.ring_bytes.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn resident_buffers_preallocate_exact_runtime_buffer_capacities() {
        let buffers = MegakernelResidentBuffers::new(8, 2, 4).unwrap();
        assert_eq!(
            buffers.control_bytes.capacity(),
            buffers.control_bytes.len()
        );
        assert_eq!(buffers.ring_bytes.capacity(), buffers.ring_bytes.len());
        assert_eq!(
            buffers.debug_log_bytes.capacity(),
            buffers.debug_log_bytes.len()
        );
        assert_eq!(
            buffers.io_queue_bytes.capacity(),
            buffers.io_queue_bytes.len()
        );
    }

    #[test]
    fn resident_buffers_reject_mismatched_ring_shape() {
        let control = Megakernel::try_encode_control(false, 1, 0).unwrap();
        let ring = Megakernel::try_encode_empty_ring(2).unwrap();
        let debug =
            Megakernel::try_encode_empty_debug_log(protocol::debug::RECORD_CAPACITY).unwrap();
        let io = io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap();
        let error = MegakernelResidentBuffers::from_parts(4, control, ring, debug, io)
            .expect_err("resident ring shape must match declared slot count");
        assert!(error.to_string().contains("resident ring"));
    }

    #[test]
    fn snapshot_readback_into_reuses_buffers() {
        let buffers = MegakernelResidentBuffers::new(4, 2, 8).unwrap();
        let mut readback = buffers.snapshot_readback();
        let control_capacity = readback.control_bytes.capacity();
        let ring_capacity = readback.ring_bytes.capacity();
        let debug_capacity = readback.debug_log_bytes.capacity();
        let io_capacity = readback.io_queue_bytes.capacity();

        buffers.snapshot_readback_into(&mut readback);
        assert_eq!(readback.control_bytes.capacity(), control_capacity);
        assert_eq!(readback.ring_bytes.capacity(), ring_capacity);
        assert_eq!(readback.debug_log_bytes.capacity(), debug_capacity);
        assert_eq!(readback.io_queue_bytes.capacity(), io_capacity);
        assert_eq!(readback.ring_bytes, buffers.ring_bytes());
    }

    #[test]
    fn resident_readback_swap_preserves_previous_mirror_for_scratch_reuse() {
        let mut buffers = MegakernelResidentBuffers::new(4, 2, 8).unwrap();
        let previous_control = buffers.control_bytes.clone();
        let previous_ring = buffers.ring_bytes.clone();
        let previous_debug = buffers.debug_log_bytes.clone();
        let previous_io = buffers.io_queue_bytes.clone();
        let mut readback = MegakernelReadback {
            control_bytes: Megakernel::try_encode_control(false, 3, 8).unwrap(),
            ring_bytes: Megakernel::try_encode_empty_ring(4).unwrap(),
            debug_log_bytes: Megakernel::try_encode_empty_debug_log(
                protocol::debug::RECORD_CAPACITY,
            )
            .unwrap(),
            io_queue_bytes: io::try_encode_empty_io_queue(io::IO_SLOT_COUNT).unwrap(),
        };

        buffers.swap_readback_from(&mut readback);

        assert_eq!(readback.control_bytes, previous_control);
        assert_eq!(readback.ring_bytes, previous_ring);
        assert_eq!(readback.debug_log_bytes, previous_debug);
        assert_eq!(readback.io_queue_bytes, previous_io);
        assert_ne!(buffers.control_bytes(), readback.control_bytes.as_slice());
    }

    #[test]
    fn default_dispatch_update_reuses_internal_output_scratch() {
        let kernel = Megakernel::bootstrap_sharded(Arc::new(ResidentEchoBackend), 1, 1, Vec::new())
            .expect("Fix: resident echo backend must compile megakernel");
        let mut buffers = MegakernelResidentBuffers::new(1, 1, 0).unwrap();
        assert_eq!(
            buffers.retained_default_output_slots(),
            MegakernelResidentHandles::ABI_RESOURCE_COUNT,
            "Fix: resident dispatch scratch must pre-seed ABI output slots before the first dispatch."
        );
        let initial_output_slots_ptr = buffers.scratch.outputs.as_ptr();

        buffers
            .dispatch_update_observed(&kernel)
            .expect("Fix: default resident dispatch update must use reusable scratch");
        assert_eq!(buffers.retained_default_output_slots(), 4);
        let output_slots_ptr = buffers.scratch.outputs.as_ptr();
        assert_eq!(
            output_slots_ptr, initial_output_slots_ptr,
            "Fix: first resident dispatch update must not grow the output shell."
        );

        buffers
            .dispatch_update_observed(&kernel)
            .expect("Fix: repeated default resident dispatch update must reuse scratch");

        assert_eq!(buffers.retained_default_output_slots(), 4);
        assert_eq!(buffers.scratch.outputs.as_ptr(), output_slots_ptr);
        assert!(buffers.retained_default_output_bytes() > 0);
    }
}
