//! Host-side telemetry decoders for the megakernel ring and control buffers.
//!
//! The runtime already exposes low-level helpers such as
//! `read_done_count`, `read_epoch`, and `read_metrics`. This module adds a
//! single structured snapshot surface useful for wrappers like VyreOffload.

use super::protocol::{control, slot, ARG0_WORD, OPCODE_WORD, STATUS_WORD, TENANT_WORD};
use super::scaling::{
    MegakernelLaunchPolicy, MegakernelLaunchRecommendation, MegakernelLaunchRequest,
    PriorityRequeueAccounting,
};
use super::staging_reserve::{
    reserve_hash_map_capacity, reserve_vec_capacity as reserve_target_capacity,
};
use crate::PipelineError;

mod sketch;
mod types;
mod errors;
pub use sketch::{CountMinSketch, SketchTelemetry, SketchTelemetryScratch};
use types::WindowAccumulator;
pub use types::{
    ControlSnapshot, MegakernelRuntimeCounters, MegakernelRuntimeEvidence,
    MegakernelWatchdogSnapshot, RingOccupancy, RingSlotSnapshot, RingStatus, RingTelemetry,
    RuntimeEvidenceMetricCoverage, RuntimeEvidenceMetricFamily, TelemetryDecodeCapacityEvidence,
    TelemetryDecodeScratch, WindowTelemetry, RUNTIME_IO_EVIDENCE_SCHEMA_VERSION,
    TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION,
};

const SLOT_WORDS_USIZE: usize = 16;

fn read_word(buf: &[u8], word_idx: usize) -> Option<u32> {
    let off = word_idx.checked_mul(4)?;
    let end = off.checked_add(4)?;
    let bytes = buf.get(off..end)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn try_read_slot_chunk_word(slot_bytes: &[u8], word_idx: u32) -> Result<u32, PipelineError> {
    let word_idx = telemetry_u32_to_usize(word_idx, "slot word index")?;
    let off = word_idx.checked_mul(4).ok_or_else(|| {
        errors::slot_word_offset_overflow()
    })?;
    let end = off.checked_add(4).ok_or_else(|| {
        errors::slot_word_end_overflow()
    })?;
    let bytes = slot_bytes.get(off..end).ok_or_else(|| {
        errors::missing_slot_word(word_idx, slot_bytes.len())
    })?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn is_sorted_unique_u32(values: &[u32]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ControlSnapshot {
    /// Decode a structured control-buffer view.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn decode(control_bytes: &[u8]) -> Self {
        match Self::try_decode(control_bytes) {
            Ok(snapshot) => snapshot,
            Err(_) => Self::default(),
        }
    }

    /// Decode a structured control-buffer view into caller-owned storage.
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn decode_into(control_bytes: &[u8], out: &mut Self) {
        // Resetting to default on decode failure silently reports zeroed
        // telemetry as if it were a real reading (Law 10). Fail loud; callers
        // use try_decode_into.
        if let Err(error) = Self::try_decode_into(control_bytes, out) {
            panic!("vyre-runtime telemetry control-buffer decode failed: {error}");
        }
    }

    /// Strictly decode a structured control-buffer view into owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any fixed control word is missing from
    /// the control snapshot.
    pub fn try_decode(control_bytes: &[u8]) -> Result<Self, PipelineError> {
        let mut out = Self::default();
        Self::try_decode_into(control_bytes, &mut out)?;
        Ok(out)
    }

    /// Strictly decode a structured control-buffer view.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when any fixed control word is missing from
    /// the control snapshot.
    pub fn try_decode_into(control_bytes: &[u8], out: &mut Self) -> Result<(), PipelineError> {
        validate_control_snapshot(control_bytes)?;
        out.shutdown =
            read_required_control_word(control_bytes, control_word_index(control::SHUTDOWN)?)? != 0;
        out.done_count =
            read_required_control_word(control_bytes, control_word_index(control::DONE_COUNT)?)?;
        out.epoch = read_required_control_word(control_bytes, control_word_index(control::EPOCH)?)?;
        out.metrics.clear();
        reserve_target_capacity(
            &mut out.metrics,
            telemetry_u32_to_usize(control::METRICS_SLOTS, "metrics slot count")?,
            "metrics",
        )?;
        for i in 0..control::METRICS_SLOTS {
            let count = read_required_control_word(
                control_bytes,
                control_offset_index(control::METRICS_BASE, i)?,
            )?;
            if count > 0 {
                out.metrics.push((i, count));
            }
        }
        out.tenant_fairness.clear();
        reserve_target_capacity(
            &mut out.tenant_fairness,
            telemetry_u32_to_usize(control::TENANT_FAIRNESS_SLOTS, "tenant fairness slot count")?,
            "tenant fairness",
        )?;
        for i in 0..control::TENANT_FAIRNESS_SLOTS {
            out.tenant_fairness.push(read_required_control_word(
                control_bytes,
                control_offset_index(control::TENANT_FAIRNESS_BASE, i)?,
            )?);
        }
        out.priority_fairness.clear();
        reserve_target_capacity(
            &mut out.priority_fairness,
            telemetry_u32_to_usize(
                control::PRIORITY_FAIRNESS_SLOTS,
                "priority fairness slot count",
            )?,
            "priority fairness",
        )?;
        for i in 0..control::PRIORITY_FAIRNESS_SLOTS {
            out.priority_fairness.push(read_required_control_word(
                control_bytes,
                control_offset_index(control::PRIORITY_FAIRNESS_BASE, i)?,
            )?);
        }
        Ok(())
    }
}

impl RingTelemetry {
    /// Decode the ring and control buffers into one structured snapshot.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn decode(control_bytes: &[u8], ring_bytes: &[u8]) -> Self {
        Self::decode_with_window_opcodes(control_bytes, ring_bytes, &[])
    }

    /// Strictly decode ring and control bytes after validating ABI alignment.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when buffers are truncated or not aligned to
    /// the megakernel wire protocol.
    pub fn try_decode(control_bytes: &[u8], ring_bytes: &[u8]) -> Result<Self, PipelineError> {
        Self::try_decode_with_window_opcodes(control_bytes, ring_bytes, &[])
    }

    /// Decode the ring and control buffers, additionally grouping any slots
    /// whose opcode is present in `window_opcodes` into ticketed route-window
    /// telemetry records.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn decode_with_window_opcodes(
        control_bytes: &[u8],
        ring_bytes: &[u8],
        window_opcodes: &[u32],
    ) -> Self {
        match Self::try_decode_with_window_opcodes(control_bytes, ring_bytes, window_opcodes) {
            Ok(telemetry) => telemetry,
            Err(_) => Self::default(),
        }
    }

    /// Decode the ring and control buffers into caller-owned telemetry and
    /// scratch storage.
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn decode_with_window_opcodes_into(
        control_bytes: &[u8],
        ring_bytes: &[u8],
        window_opcodes: &[u32],
        out: &mut Self,
        scratch: &mut TelemetryDecodeScratch,
    ) {
        Self::try_decode_with_window_opcodes_into(
            control_bytes,
            ring_bytes,
            window_opcodes,
            out,
            scratch,
        )
        .unwrap_or_else(|_| {
            *out = Self::default();
            scratch.clear();
        });
    }

    fn try_decode_with_window_opcodes_into_unchecked(
        control_bytes: &[u8],
        ring_bytes: &[u8],
        window_opcodes: &[u32],
        out: &mut Self,
        scratch: &mut TelemetryDecodeScratch,
    ) -> Result<(), PipelineError> {
        enum WindowOpcodeMatcher<'a> {
            None,
            Single(u32),
            DenseBitmap(u128),
            SmallSlice(&'a [u32]),
            LargeSlice(&'a [u32]),
        }

        ControlSnapshot::try_decode_into(control_bytes, &mut out.control)?;
        let slot_count = ring_bytes.len() / slot_byte_len()?;
        out.occupancy = RingOccupancy::default();
        out.slots.clear();
        reserve_target_capacity(&mut out.slots, slot_count, "ring slots")?;
        out.windows.clear();
        scratch.window_opcodes.clear();
        scratch.windows.clear();
        let window_opcode_lookup = if window_opcodes.is_empty() {
            &[][..]
        } else if is_sorted_unique_u32(window_opcodes) {
            window_opcodes
        } else {
            reserve_target_capacity(
                &mut scratch.window_opcodes,
                window_opcodes.len(),
                "window opcode scratch",
            )?;
            scratch.window_opcodes.extend_from_slice(window_opcodes);
            scratch.window_opcodes.sort_unstable();
            scratch.window_opcodes.dedup();
            scratch.window_opcodes.as_slice()
        };
        let window_opcode_matcher = match window_opcode_lookup {
            [] => WindowOpcodeMatcher::None,
            [opcode] => WindowOpcodeMatcher::Single(*opcode),
            opcodes if opcodes.len() > 1 && opcodes.iter().all(|opcode| *opcode < 128) => {
                let bitmap = opcodes
                    .iter()
                    .fold(0_u128, |acc, &opcode| acc | (1_u128 << opcode));
                WindowOpcodeMatcher::DenseBitmap(bitmap)
            }
            opcodes if opcodes.len() <= 8 => WindowOpcodeMatcher::SmallSlice(opcodes),
            opcodes => WindowOpcodeMatcher::LargeSlice(opcodes),
        };
        if !matches!(window_opcode_matcher, WindowOpcodeMatcher::None) {
            reserve_hash_map_capacity(
                &mut scratch.windows,
                slot_count,
                "window accumulator scratch",
            )?;
        }
        let decode_windows = !matches!(window_opcode_matcher, WindowOpcodeMatcher::None);

        let slot_byte_len = slot_byte_len()?;
        for (slot_idx, slot_bytes) in ring_bytes.chunks_exact(slot_byte_len).enumerate() {
            let slot_idx = u32::try_from(slot_idx).map_err(|source| {
                PipelineError::Backend(format!(
                    "megakernel telemetry slot index cannot fit u32: {source}. Fix: shard ring snapshots before host decode."
                ))
            })?;
            let status_raw = try_read_slot_chunk_word(slot_bytes, STATUS_WORD)?;
            let status = RingStatus::from_raw(status_raw);
            match status {
                RingStatus::Empty => out.occupancy.empty += 1,
                RingStatus::Published => out.occupancy.published += 1,
                RingStatus::Claimed => out.occupancy.claimed += 1,
                RingStatus::Done => out.occupancy.done += 1,
                RingStatus::WaitIo => out.occupancy.wait_io += 1,
                RingStatus::Yield => out.occupancy.yield_count += 1,
                RingStatus::Requeue => out.occupancy.requeue += 1,
                RingStatus::Fault => out.occupancy.fault += 1,
                RingStatus::Unknown(_) => out.occupancy.unknown += 1,
            }
            let tenant_id = try_read_slot_chunk_word(slot_bytes, TENANT_WORD)?;
            let opcode = try_read_slot_chunk_word(slot_bytes, OPCODE_WORD)?;
            let args_prefix = [
                try_read_slot_chunk_word(slot_bytes, ARG0_WORD)?,
                try_read_slot_chunk_word(slot_bytes, ARG0_WORD + 1)?,
                try_read_slot_chunk_word(slot_bytes, ARG0_WORD + 2)?,
            ];
            let is_window_opcode = match window_opcode_matcher {
                WindowOpcodeMatcher::None => false,
                WindowOpcodeMatcher::Single(expected) => opcode == expected,
                WindowOpcodeMatcher::DenseBitmap(bitmap) => {
                    opcode < 128 && ((bitmap >> opcode) & 1) == 1
                }
                WindowOpcodeMatcher::SmallSlice(window_opcodes) => window_opcodes.contains(&opcode),
                WindowOpcodeMatcher::LargeSlice(window_opcodes) => {
                    window_opcodes.binary_search(&opcode).is_ok()
                }
            };
            if decode_windows && is_window_opcode {
                let ticket = args_prefix[0];
                let class_tag = args_prefix[1];
                let entry =
                    scratch
                        .windows
                        .entry((ticket, opcode))
                        .or_insert_with(|| WindowAccumulator {
                            tenant_id,
                            opcode,
                            ..WindowAccumulator::default()
                        });
                match class_tag {
                    0 => entry.required_slots += 1,
                    1 => entry.lookahead_slots += 1,
                    _ => {}
                }
                match status {
                    RingStatus::Published => entry.published += 1,
                    RingStatus::Claimed => entry.claimed += 1,
                    RingStatus::Done => entry.done += 1,
                    RingStatus::WaitIo => entry.wait_io += 1,
                    RingStatus::Yield => entry.yield_count += 1,
                    RingStatus::Requeue => entry.requeue += 1,
                    RingStatus::Fault => entry.fault += 1,
                    RingStatus::Empty | RingStatus::Unknown(_) => {}
                }
            }
            out.slots.push(RingSlotSnapshot {
                slot_idx,
                status,
                tenant_id,
                opcode,
                args_prefix,
            });
        }

        reserve_target_capacity(&mut out.windows, scratch.windows.len(), "window output")?;
        for (&(ticket, _), acc) in &scratch.windows {
            out.windows.push(WindowTelemetry {
                ticket,
                tenant_id: acc.tenant_id,
                opcode: acc.opcode,
                required_slots: acc.required_slots,
                lookahead_slots: acc.lookahead_slots,
                published: acc.published,
                claimed: acc.claimed,
                done: acc.done,
                wait_io: acc.wait_io,
                yield_count: acc.yield_count,
                requeue: acc.requeue,
                fault: acc.fault,
            });
        }
        out.windows
            .sort_unstable_by_key(|window| (window.ticket, window.opcode));
        Ok(())
    }

    /// Strictly decode ring/control bytes and group selected window opcodes.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when buffers are truncated or not aligned to
    /// the megakernel wire protocol.
    pub fn try_decode_with_window_opcodes(
        control_bytes: &[u8],
        ring_bytes: &[u8],
        window_opcodes: &[u32],
    ) -> Result<Self, PipelineError> {
        validate_telemetry_buffers(control_bytes, ring_bytes)?;
        let mut out = Self::default();
        let mut scratch = TelemetryDecodeScratch::new();
        Self::try_decode_with_window_opcodes_into_unchecked(
            control_bytes,
            ring_bytes,
            window_opcodes,
            &mut out,
            &mut scratch,
        )?;
        Ok(out)
    }

    /// Strictly decode ring/control bytes into caller-owned telemetry and
    /// scratch storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when buffers are truncated or not aligned to
    /// the megakernel wire protocol.
    pub fn try_decode_with_window_opcodes_into(
        control_bytes: &[u8],
        ring_bytes: &[u8],
        window_opcodes: &[u32],
        out: &mut Self,
        scratch: &mut TelemetryDecodeScratch,
    ) -> Result<(), PipelineError> {
        validate_telemetry_buffers(control_bytes, ring_bytes)?;
        Self::try_decode_with_window_opcodes_into_unchecked(
            control_bytes,
            ring_bytes,
            window_opcodes,
            out,
            scratch,
        )?;
        Ok(())
    }

    /// Active slots matching a given opcode.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn active_slots_for_opcode(&self, opcode: u32) -> Vec<&RingSlotSnapshot> {
        match self.try_active_slots_for_opcode(opcode) {
            Ok(slots) => slots,
            Err(_) => Vec::default(),
        }
    }

    /// Active slots matching a given opcode with fallible output staging.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when output storage cannot be reserved.
    pub fn try_active_slots_for_opcode(
        &self,
        opcode: u32,
    ) -> Result<Vec<&RingSlotSnapshot>, PipelineError> {
        let mut out = Vec::default();
        self.try_active_slots_for_opcode_into(opcode, &mut out)?;
        Ok(out)
    }

    /// Active slots matching a given opcode as an iterator.
    pub fn active_slots_for_opcode_iter(
        &self,
        opcode: u32,
    ) -> impl Iterator<Item = &RingSlotSnapshot> {
        self.slots
            .iter()
            .filter(move |slot| slot.opcode == opcode && slot.status.is_active())
    }

    /// Active slots matching a given opcode into caller-owned storage.
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn active_slots_for_opcode_into<'a>(
        &'a self,
        opcode: u32,
        out: &mut Vec<&'a RingSlotSnapshot>,
    ) {
        // Clearing to empty on failure silently reports "no active slots" when
        // the readback actually failed (Law 10). Fail loud; callers use
        // try_active_slots_for_opcode_into.
        if let Err(error) = self.try_active_slots_for_opcode_into(opcode, out) {
            panic!("vyre-runtime telemetry active-slots readback failed: {error}");
        }
    }

    /// Active slots matching a given opcode into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when output storage cannot be reserved.
    pub fn try_active_slots_for_opcode_into<'a>(
        &'a self,
        opcode: u32,
        out: &mut Vec<&'a RingSlotSnapshot>,
    ) -> Result<(), PipelineError> {
        out.clear();
        reserve_target_capacity(out, self.slots.len(), "active slot output")?;
        self.slots
            .iter()
            .filter(|slot| slot.opcode == opcode && slot.status.is_active())
            .for_each(|slot| out.push(slot));
        Ok(())
    }

    /// Unfinished ticketed windows.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn active_windows(&self) -> Vec<&WindowTelemetry> {
        match self.try_active_windows() {
            Ok(windows) => windows,
            Err(_) => Vec::default(),
        }
    }

    /// Unfinished ticketed windows with fallible output staging.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when output storage cannot be reserved.
    pub fn try_active_windows(&self) -> Result<Vec<&WindowTelemetry>, PipelineError> {
        let mut out = Vec::default();
        self.try_active_windows_into(&mut out)?;
        Ok(out)
    }

    /// Unfinished ticketed windows into caller-owned storage.
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn active_windows_into<'a>(&'a self, out: &mut Vec<&'a WindowTelemetry>) {
        // Clearing to empty on failure silently reports "no active windows"
        // when the readback actually failed (Law 10). Fail loud; callers use
        // try_active_windows_into.
        if let Err(error) = self.try_active_windows_into(out) {
            panic!("vyre-runtime telemetry active-windows readback failed: {error}");
        }
    }

    /// Unfinished ticketed windows into caller-owned storage.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when output storage cannot be reserved.
    pub fn try_active_windows_into<'a>(
        &'a self,
        out: &mut Vec<&'a WindowTelemetry>,
    ) -> Result<(), PipelineError> {
        out.clear();
        reserve_target_capacity(out, self.windows.len(), "active window output")?;
        self.windows
            .iter()
            .filter(|window| window.is_active())
            .for_each(|window| out.push(window));
        Ok(())
    }

    /// Summarize priority requeue/aging pressure visible in the ring snapshot.
    #[must_use]
    pub fn priority_accounting(&self) -> PriorityRequeueAccounting {
        PriorityRequeueAccounting {
            requeue_count: u64::from(self.occupancy.requeue),
            aged_promotions: 0,
            max_priority_age: 0,
        }
    }

    /// Aggregate queue, idle, fairness, and drain counters into one cheap
    /// runtime snapshot for SRE dashboards and launch-policy feedback.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn runtime_counters(&self) -> MegakernelRuntimeCounters {
        match self.try_runtime_counters() {
            Ok(counters) => counters,
            Err(_) => zero_runtime_counters(),
        }
    }

    /// Fallibly aggregate queue, idle, fairness, and drain counters.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when counter aggregation overflows or decoded
    /// telemetry contains an impossible relationship.
    pub fn try_runtime_counters(&self) -> Result<MegakernelRuntimeCounters, PipelineError> {
        let total_slots = self.occupancy.total_slots();
        let queue_depth = self.occupancy.queue_depth();
        let gpu_idle_slots = self.occupancy.empty;
        let gpu_idle_ppm = if total_slots == 0 {
            0
        } else {
            let raw_idle_ppm = (u64::from(gpu_idle_slots) * 1_000_000) / u64::from(total_slots);
            raw_idle_ppm.min(1_000_000) as u32
        };
        let frontier_density_bps = try_density_bps(queue_depth, total_slots)?;
        let active_slots = total_slots.saturating_sub(gpu_idle_slots);
        let occupancy_proxy_bps = try_density_bps(active_slots, total_slots)?;
        let tenant_fairness_total = try_sum_u32_as_u64(
            &self.control.tenant_fairness,
            "tenant fairness total",
            "shard tenant counters before telemetry aggregation",
        )?;
        let priority_fairness_total = try_sum_u32_as_u64(
            &self.control.priority_fairness,
            "priority fairness total",
            "shard priority counters before telemetry aggregation",
        )?;
        let tenant_fairness_skew = try_fairness_skew(&self.control.tenant_fairness)?;
        Ok(MegakernelRuntimeCounters {
            total_slots,
            queue_depth,
            gpu_idle_slots,
            gpu_idle_ppm,
            frontier_density_bps,
            occupancy_proxy_bps,
            drained_slots: self.control.done_count,
            unreclaimed_done_slots: self.occupancy.done,
            tenant_fairness_total,
            tenant_fairness_skew,
            priority_fairness_total,
            requeue_slots: self.occupancy.requeue,
            fault_slots: self.occupancy.fault,
        })
    }

    /// Derive persistent-kernel health from two snapshots without polling the
    /// device or synchronizing with the GPU.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn health_since(&self, previous: &RingTelemetry) -> MegakernelWatchdogSnapshot {
        match self.try_health_since(previous) {
            Ok(snapshot) => snapshot,
            Err(_) => zero_watchdog_snapshot(),
        }
    }

    /// Fallibly derive persistent-kernel health from two snapshots.
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError`] when counters wrap, move backwards, or cannot
    /// be aggregated without overflow.
    pub fn try_health_since(
        &self,
        previous: &RingTelemetry,
    ) -> Result<MegakernelWatchdogSnapshot, PipelineError> {
        let counters = self.try_runtime_counters()?;
        let done_delta = self
            .control
            .done_count
            .checked_sub(previous.control.done_count)
            .ok_or_else(|| {
                errors::done_counter_backwards(previous.control.done_count, self.control.done_count)
            })?;
        let suspected_stall =
            counters.queue_depth > 0 && done_delta == 0 && counters.fault_slots == 0;
        Ok(MegakernelWatchdogSnapshot {
            done_delta,
            queue_depth: counters.queue_depth,
            fault_slots: counters.fault_slots,
            requeue_slots: counters.requeue_slots,
            gpu_idle_ppm: counters.gpu_idle_ppm,
            suspected_stall,
        })
    }

    /// Feed telemetry into the shared launch policy.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the supplied adapter limits are malformed.
    pub fn recommend_launch(
        &self,
        mut request: MegakernelLaunchRequest,
    ) -> Result<MegakernelLaunchRecommendation, vyre_driver::BackendError> {
        let counters = self
            .try_runtime_counters()
            .map_err(errors::launch_telemetry_failed)?;
        if request.graph_node_count == 0 {
            request.graph_node_count = counters.total_slots;
        }
        if request.graph_edge_count == 0 {
            request.graph_edge_count = counters.queue_depth;
        }
        if request.frontier_density_bps == 0 {
            request.frontier_density_bps = counters.frontier_density_bps;
        }
        request.hot_opcode_count = self
            .control
            .metrics
            .iter()
            .filter(|(_, count)| *count > 0)
            .count()
            .try_into()
            .map_err(errors::hot_opcode_count_overflow)?;
        let mut hot_window_count = 0usize;
        for window in &self.windows {
            let demand = window
                .required_slots
                .checked_add(window.lookahead_slots)
                .ok_or_else(|| {
                    errors::route_window_demand_overflow()
                })?;
            if demand >= 4 {
                hot_window_count = hot_window_count.checked_add(1).ok_or_else(|| {
                    errors::hot_window_count_overflow()
                })?;
            }
        }
        request.hot_window_count = hot_window_count
            .try_into()
            .map_err(errors::hot_window_count_too_wide)?;
        request.requeue_count = request
            .requeue_count
            .checked_add(u64::from(self.occupancy.requeue))
            .ok_or_else(errors::requeue_count_overflow)?;
        MegakernelLaunchPolicy::standard().recommend(request)
    }
}

/// All-zero runtime counters, returned by the infallible `runtime_counters`
/// accessor when the fallible decode path reports an error.
#[cfg(any(test, feature = "legacy-infallible"))]
fn zero_runtime_counters() -> MegakernelRuntimeCounters {
    MegakernelRuntimeCounters {
        total_slots: 0,
        queue_depth: 0,
        gpu_idle_slots: 0,
        gpu_idle_ppm: 0,
        frontier_density_bps: 0,
        occupancy_proxy_bps: 0,
        drained_slots: 0,
        unreclaimed_done_slots: 0,
        tenant_fairness_total: 0,
        tenant_fairness_skew: 0,
        priority_fairness_total: 0,
        requeue_slots: 0,
        fault_slots: 0,
    }
}

/// All-zero watchdog snapshot, returned by the infallible `health_since`
/// accessor when the fallible derivation path reports an error.
#[cfg(any(test, feature = "legacy-infallible"))]
fn zero_watchdog_snapshot() -> MegakernelWatchdogSnapshot {
    MegakernelWatchdogSnapshot {
        done_delta: 0,
        queue_depth: 0,
        fault_slots: 0,
        requeue_slots: 0,
        gpu_idle_ppm: 0,
        suspected_stall: false,
    }
}

fn read_required_control_word(control_bytes: &[u8], word_idx: usize) -> Result<u32, PipelineError> {
    read_word(control_bytes, word_idx).ok_or_else(|| errors::missing_control_word(word_idx))
}

fn try_density_bps(numerator: u32, denominator: u32) -> Result<u16, PipelineError> {
    if denominator == 0 {
        return Ok(0);
    }
    let bps = (u64::from(numerator) * 10_000) / u64::from(denominator);
    u16::try_from(bps.min(u64::from(u16::MAX))).map_err(errors::density_bps_overflow)
}

fn validate_telemetry_buffers(
    control_bytes: &[u8],
    ring_bytes: &[u8],
) -> Result<(), PipelineError> {
    validate_control_snapshot(control_bytes)?;
    let slot_bytes = slot_byte_len()?;
    if ring_bytes.len() % slot_bytes != 0 {
        return Err(errors::ring_slot_alignment(ring_bytes.len(), slot_bytes));
    }
    let slot_count = ring_bytes.len() / slot_bytes;
    if u32::try_from(slot_count).is_err() {
        return Err(errors::ring_slot_count_too_wide(slot_count));
    }
    Ok(())
}

fn validate_control_snapshot(control_bytes: &[u8]) -> Result<(), PipelineError> {
    let min_control = super::protocol::control_byte_len(0).ok_or_else(|| {
        errors::control_length_overflow()
    })?;
    if control_bytes.len() < min_control || control_bytes.len() % 4 != 0 {
        return Err(errors::bad_control_snapshot(
            control_bytes.len(),
            min_control,
        ));
    }
    Ok(())
}

fn slot_byte_len() -> Result<usize, PipelineError> {
    SLOT_WORDS_USIZE.checked_mul(4).ok_or_else(|| {
        errors::slot_byte_width_overflow()
    })
}

fn telemetry_u32_to_usize(value: u32, label: &'static str) -> Result<usize, PipelineError> {
    usize::try_from(value).map_err(|source| errors::telemetry_u32_to_usize(value, label, source))
}

fn control_word_index(word: u32) -> Result<usize, PipelineError> {
    usize::try_from(word).map_err(|source| errors::control_word_index(word, source))
}

fn control_offset_index(base: u32, offset: u32) -> Result<usize, PipelineError> {
    let word = base.checked_add(offset).ok_or_else(|| {
        errors::control_word_offset_overflow()
    })?;
    control_word_index(word)
}

fn try_sum_u32_as_u64(
    counters: &[u32],
    label: &'static str,
    fix: &'static str,
) -> Result<u64, PipelineError> {
    counters.iter().try_fold(0u64, |acc, &count| {
        acc.checked_add(u64::from(count)).ok_or_else(|| {
            errors::counter_sum_overflow(label, fix)
        })
    })
}

fn try_fairness_skew(counters: &[u32]) -> Result<u32, PipelineError> {
    let mut min_nonzero = u32::MAX;
    let mut max = 0u32;
    for &count in counters {
        if count != 0 {
            min_nonzero = min_nonzero.min(count);
            max = max.max(count);
        }
    }
    if min_nonzero == u32::MAX {
        Ok(0)
    } else {
        max.checked_sub(min_nonzero).ok_or_else(|| {
            errors::fairness_skew_invalid(max, min_nonzero)
        })
    }
}

#[cfg(test)]
mod tests {
    include!("telemetry_tests.rs");
}
