use super::slot;
use rustc_hash::FxHashMap;

/// Decoded top-level ring slot state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingStatus {
    /// Slot is free.
    Empty,
    /// Slot is published and waiting for a worker.
    Published,
    /// Slot has been claimed by a worker.
    Claimed,
    /// Slot completed and can be recycled.
    Done,
    /// Slot is waiting for an asynchronous IO continuation.
    WaitIo,
    /// Slot yielded execution back to the scheduler.
    Yield,
    /// Slot is heavily contested and has been requeued.
    Requeue,
    /// Slot hit a hardware or software fault constraint.
    Fault,
    /// Unknown raw wire value.
    Unknown(u32),
}

impl RingStatus {
    #[must_use]
    pub(super) fn from_raw(raw: u32) -> Self {
        match raw {
            slot::EMPTY => Self::Empty,
            slot::PUBLISHED => Self::Published,
            slot::CLAIMED => Self::Claimed,
            slot::DONE => Self::Done,
            slot::WAIT_IO => Self::WaitIo,
            slot::YIELD => Self::Yield,
            slot::REQUEUE => Self::Requeue,
            slot::FAULT => Self::Fault,
            other => Self::Unknown(other),
        }
    }

    /// Raw wire discriminant for sketching, replay, and compact telemetry.
    #[must_use]
    pub const fn raw(self) -> u32 {
        match self {
            Self::Empty => slot::EMPTY,
            Self::Published => slot::PUBLISHED,
            Self::Claimed => slot::CLAIMED,
            Self::Done => slot::DONE,
            Self::WaitIo => slot::WAIT_IO,
            Self::Yield => slot::YIELD,
            Self::Requeue => slot::REQUEUE,
            Self::Fault => slot::FAULT,
            Self::Unknown(raw) => raw,
        }
    }

    /// Whether this status still represents in-flight work rather than a
    /// terminal slot outcome.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::Published | Self::Claimed | Self::WaitIo | Self::Yield | Self::Requeue
        )
    }
}

/// Snapshot of one ring slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingSlotSnapshot {
    /// Zero-based slot index.
    pub slot_idx: u32,
    /// Current state.
    pub status: RingStatus,
    /// Tenant id assigned to the slot.
    pub tenant_id: u32,
    /// Top-level opcode currently stored in the slot.
    pub opcode: u32,
    /// First three argument words, useful for quick debugging.
    pub args_prefix: [u32; 3],
}

/// Aggregated telemetry for one ticketed route window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTelemetry {
    /// Stable ticket id encoded in `arg0`.
    pub ticket: u32,
    /// Tenant id shared by all emitted slots in this window.
    pub tenant_id: u32,
    /// Opcode shared by the window payload slots.
    pub opcode: u32,
    /// Number of required slots in the window.
    pub required_slots: u32,
    /// Number of lookahead slots in the window.
    pub lookahead_slots: u32,
    /// Number of slots currently published.
    pub published: u32,
    /// Number of slots currently claimed.
    pub claimed: u32,
    /// Number of slots completed.
    pub done: u32,
    /// Number of slots waiting for I/O.
    pub wait_io: u32,
    /// Number of yielded slots.
    pub yield_count: u32,
    /// Number of requeued slots.
    pub requeue: u32,
    /// Number of faulted slots.
    pub fault: u32,
}

impl WindowTelemetry {
    /// Whether this ticket still has unfinished work in the ring.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.published > 0
            || self.claimed > 0
            || self.wait_io > 0
            || self.yield_count > 0
            || self.requeue > 0
    }
}

/// Slot occupancy counts across the ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RingOccupancy {
    /// Number of empty slots.
    pub empty: u32,
    /// Number of published slots.
    pub published: u32,
    /// Number of claimed slots.
    pub claimed: u32,
    /// Number of done slots.
    pub done: u32,
    /// Number of slots waiting for IO.
    pub wait_io: u32,
    /// Number of slots yielded.
    pub yield_count: u32,
    /// Number of requeued slots.
    pub requeue: u32,
    /// Number of faulted slots.
    pub fault: u32,
    /// Number of slots with unrecognized raw status values.
    pub unknown: u32,
}

impl RingOccupancy {
    /// Total slots represented by this occupancy snapshot.
    #[must_use]
    pub fn total_slots(&self) -> u32 {
        checked_status_sum(
            [
                self.empty,
                self.published,
                self.claimed,
                self.done,
                self.wait_io,
                self.yield_count,
                self.requeue,
                self.fault,
                self.unknown,
            ],
            "total ring slots",
        )
    }

    /// Host-visible active queue depth: all non-empty slots that are not done.
    #[must_use]
    pub fn queue_depth(&self) -> u32 {
        checked_status_sum(
            [
                self.published,
                self.claimed,
                self.wait_io,
                self.yield_count,
                self.requeue,
                self.fault,
                self.unknown,
            ],
            "ring queue depth",
        )
    }
}

/// Schema version for IO/runtime evidence emitted from megakernel telemetry.
pub const RUNTIME_IO_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Required metric families for runtime IO/residency evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEvidenceMetricFamily {
    /// Ring occupancy metrics are present.
    Ring,
    /// Control-buffer decode metrics are present.
    Control,
    /// Host/device copy accounting metrics are present.
    Copy,
    /// Resident device-byte metrics are present.
    Residency,
}

impl RuntimeEvidenceMetricFamily {
    /// Stable evidence-family token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ring => "ring",
            Self::Control => "control",
            Self::Copy => "copy",
            Self::Residency => "residency",
        }
    }
}

/// Coverage bits for the required runtime evidence metric families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeEvidenceMetricCoverage {
    /// Ring occupancy metrics are present.
    pub ring: bool,
    /// Control-buffer decode metrics are present.
    pub control: bool,
    /// Host/device copy accounting metrics are present.
    pub copy: bool,
    /// Resident device-byte metrics are present.
    pub residency: bool,
}

impl RuntimeEvidenceMetricCoverage {
    /// Coverage with every runtime evidence family present.
    #[must_use]
    pub const fn complete() -> Self {
        Self {
            ring: true,
            control: true,
            copy: true,
            residency: true,
        }
    }

    /// Missing required metric families.
    #[must_use]
    pub fn missing_families(self) -> Vec<RuntimeEvidenceMetricFamily> {
        let mut missing = Vec::new();
        if !self.ring {
            missing.push(RuntimeEvidenceMetricFamily::Ring);
        }
        if !self.control {
            missing.push(RuntimeEvidenceMetricFamily::Control);
        }
        if !self.copy {
            missing.push(RuntimeEvidenceMetricFamily::Copy);
        }
        if !self.residency {
            missing.push(RuntimeEvidenceMetricFamily::Residency);
        }
        missing
    }
}

/// Runtime-owned IO/residency evidence envelope for release and benchmark artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MegakernelRuntimeEvidence {
    /// Runtime evidence schema version.
    pub schema_version: u32,
    /// Device-resident bytes retained by the dispatch family.
    pub resident_device_bytes: u64,
    /// Host-visible copy bytes still required for this evidence sample.
    pub host_copy_bytes: u64,
    /// Host copy bytes avoided by resident handles or device-side IO.
    pub host_copy_avoided_bytes: u64,
    /// Ring occupancy snapshot.
    pub ring_occupancy: RingOccupancy,
    /// Control-buffer decode cost in nanoseconds.
    pub control_decode_ns: u64,
    /// Ring decode cost in nanoseconds.
    pub ring_decode_ns: u64,
    /// Required metric-family coverage.
    pub coverage: RuntimeEvidenceMetricCoverage,
}

impl MegakernelRuntimeEvidence {
    /// Construct a complete runtime evidence envelope.
    #[must_use]
    pub const fn complete(
        resident_device_bytes: u64,
        host_copy_bytes: u64,
        host_copy_avoided_bytes: u64,
        ring_occupancy: RingOccupancy,
        control_decode_ns: u64,
        ring_decode_ns: u64,
    ) -> Self {
        Self {
            schema_version: RUNTIME_IO_EVIDENCE_SCHEMA_VERSION,
            resident_device_bytes,
            host_copy_bytes,
            host_copy_avoided_bytes,
            ring_occupancy,
            control_decode_ns,
            ring_decode_ns,
            coverage: RuntimeEvidenceMetricCoverage::complete(),
        }
    }

    /// Metric families missing from this evidence envelope.
    #[must_use]
    pub fn missing_metric_families(&self) -> Vec<RuntimeEvidenceMetricFamily> {
        self.coverage.missing_families()
    }

    /// Whether all required runtime evidence families are present.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.schema_version == RUNTIME_IO_EVIDENCE_SCHEMA_VERSION
            && self.missing_metric_families().is_empty()
    }

    /// Avoided host-copy bytes in basis points of total relevant copy volume.
    #[must_use]
    pub fn host_copy_avoidance_bps(&self) -> u16 {
        let total = u128::from(self.host_copy_bytes)
            .saturating_add(u128::from(self.host_copy_avoided_bytes));
        if total == 0 {
            return 0;
        }
        let bps = u128::from(self.host_copy_avoided_bytes)
            .saturating_mul(10_000)
            / total;
        bps.min(10_000) as u16
    }
}

fn checked_status_sum<const N: usize>(values: [u32; N], label: &'static str) -> u32 {
    let _ = label;
    values
        .into_iter()
        .fold(0_u32, |acc, value| acc.saturating_add(value))
}

#[cfg(test)]
mod evidence_tests {
    use super::*;

    #[test]
    fn runtime_evidence_reports_missing_metric_families() {
        let evidence = MegakernelRuntimeEvidence {
            schema_version: RUNTIME_IO_EVIDENCE_SCHEMA_VERSION,
            resident_device_bytes: 0,
            host_copy_bytes: 0,
            host_copy_avoided_bytes: 0,
            ring_occupancy: RingOccupancy::default(),
            control_decode_ns: 0,
            ring_decode_ns: 0,
            coverage: RuntimeEvidenceMetricCoverage {
                ring: true,
                control: false,
                copy: true,
                residency: false,
            },
        };

        let missing = evidence
            .missing_metric_families()
            .into_iter()
            .map(RuntimeEvidenceMetricFamily::as_str)
            .collect::<Vec<_>>();

        assert_eq!(missing, vec!["control", "residency"]);
        assert!(!evidence.is_complete());
    }

    #[test]
    fn runtime_evidence_records_copy_avoidance_and_occupancy() {
        let evidence = MegakernelRuntimeEvidence::complete(
            4096,
            1024,
            3072,
            RingOccupancy {
                empty: 1,
                published: 2,
                claimed: 3,
                done: 4,
                wait_io: 5,
                yield_count: 6,
                requeue: 7,
                fault: 8,
                unknown: 9,
            },
            11,
            13,
        );

        assert!(evidence.is_complete());
        assert_eq!(evidence.resident_device_bytes, 4096);
        assert_eq!(evidence.ring_occupancy.total_slots(), 45);
        assert_eq!(evidence.ring_occupancy.queue_depth(), 40);
        assert_eq!(evidence.host_copy_avoidance_bps(), 7500);
    }
}

/// Structured view of the control buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControlSnapshot {
    /// Shutdown flag.
    pub shutdown: bool,
    /// Total drained slots.
    pub done_count: u32,
    /// Epoch value (batch fences).
    pub epoch: u32,
    /// Non-zero opcode metrics.
    pub metrics: Vec<(u32, u32)>,
    /// Per-tenant fairness counters (cumulative).
    pub tenant_fairness: Vec<u32>,
    /// Per-priority fairness counters (cumulative).
    pub priority_fairness: Vec<u32>,
}

/// Aggregated runtime performance counters derived from one telemetry snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelRuntimeCounters {
    /// Total ring slots represented by the snapshot.
    pub total_slots: u32,
    /// Active queue depth: published/claimed/waiting/requeued/fault/unknown slots.
    pub queue_depth: u32,
    /// Empty ring slots, used as the host-visible idle-capacity signal.
    pub gpu_idle_slots: u32,
    /// Idle slots in parts per million of the ring size.
    pub gpu_idle_ppm: u32,
    /// Active frontier density in basis points of the ring size.
    pub frontier_density_bps: u16,
    /// Occupancy proxy in basis points: non-idle slots divided by total slots.
    pub occupancy_proxy_bps: u16,
    /// Total slots the GPU has drained according to the control buffer.
    pub drained_slots: u32,
    /// Done slots visible in the ring snapshot and pending reclaim.
    pub unreclaimed_done_slots: u32,
    /// Sum of tenant fairness counters.
    pub tenant_fairness_total: u64,
    /// Max minus min non-zero tenant fairness counter.
    pub tenant_fairness_skew: u32,
    /// Sum of priority fairness counters.
    pub priority_fairness_total: u64,
    /// Requeued slots visible in the ring.
    pub requeue_slots: u32,
    /// Faulted slots visible in the ring.
    pub fault_slots: u32,
}

/// Watchdog view computed from two host-visible telemetry snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegakernelWatchdogSnapshot {
    /// Increase in drained slots between the previous and current snapshot.
    pub done_delta: u32,
    /// Current active queue depth.
    pub queue_depth: u32,
    /// Current faulted slots.
    pub fault_slots: u32,
    /// Current requeued slots.
    pub requeue_slots: u32,
    /// Current idle slots in parts per million.
    pub gpu_idle_ppm: u32,
    /// True when work remains queued but no drain progress was observed.
    pub suspected_stall: bool,
}

/// Combined host-visible telemetry for a megakernel run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RingTelemetry {
    /// Decoded control-buffer snapshot.
    pub control: ControlSnapshot,
    /// Occupancy summary.
    pub occupancy: RingOccupancy,
    /// All decoded slots.
    pub slots: Vec<RingSlotSnapshot>,
    /// Decoded ticketed windows for any caller-specified window opcodes.
    pub windows: Vec<WindowTelemetry>,
}

/// Schema version for telemetry decode capacity evidence.
pub const TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION: u32 = 1;

/// Evidence that a telemetry decode used caller-owned output and scratch buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryDecodeCapacityEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Number of decoded ring slots in the output snapshot.
    pub decoded_slot_count: usize,
    /// Capacity of the caller-owned ring-slot output buffer.
    pub slot_output_capacity: usize,
    /// Number of decoded route-window rows in the output snapshot.
    pub decoded_window_count: usize,
    /// Capacity of the caller-owned route-window output buffer.
    pub window_output_capacity: usize,
    /// Capacity retained for sorted window-opcode scratch.
    pub window_opcode_scratch_capacity: usize,
    /// Capacity retained for route-window accumulator scratch.
    pub window_accumulator_scratch_capacity: usize,
    /// True when evidence was produced from caller-owned scratch.
    pub uses_caller_owned_scratch: bool,
}

impl TelemetryDecodeCapacityEvidence {
    /// Return true when output and scratch capacities cover the decoded rows.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.schema_version == TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION
            && self.uses_caller_owned_scratch
            && self.slot_output_capacity >= self.decoded_slot_count
            && self.window_output_capacity >= self.decoded_window_count
            && self.window_accumulator_scratch_capacity >= self.decoded_window_count
    }
}

impl RingTelemetry {
    /// Build capacity evidence for a strict caller-owned telemetry decode.
    #[must_use]
    pub fn decode_capacity_evidence(
        &self,
        scratch: &TelemetryDecodeScratch,
    ) -> TelemetryDecodeCapacityEvidence {
        TelemetryDecodeCapacityEvidence {
            schema_version: TELEMETRY_DECODE_CAPACITY_SCHEMA_VERSION,
            decoded_slot_count: self.slots.len(),
            slot_output_capacity: self.slots.capacity(),
            decoded_window_count: self.windows.len(),
            window_output_capacity: self.windows.capacity(),
            window_opcode_scratch_capacity: scratch.window_opcodes.capacity(),
            window_accumulator_scratch_capacity: scratch.windows.capacity(),
            uses_caller_owned_scratch: true,
        }
    }
}

/// Caller-owned scratch for repeated megakernel telemetry decodes.
///
/// Long-running supervisors poll telemetry at high frequency. Reusing this
/// scratch keeps each sample to straight-line buffer rewrites rather than
/// per-poll map allocation.
#[derive(Debug, Default)]
pub struct TelemetryDecodeScratch {
    pub(super) window_opcodes: Vec<u32>,
    pub(super) windows: FxHashMap<(u32, u32), WindowAccumulator>,
}

impl TelemetryDecodeScratch {
    /// Construct empty decode scratch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            window_opcodes: Vec::new(),
            windows: FxHashMap::default(),
        }
    }

    /// Clear retained decode rows without releasing allocated scratch capacity.
    pub fn clear(&mut self) {
        self.window_opcodes.clear();
        self.windows.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct WindowAccumulator {
    pub(super) tenant_id: u32,
    pub(super) opcode: u32,
    pub(super) required_slots: u32,
    pub(super) lookahead_slots: u32,
    pub(super) published: u32,
    pub(super) claimed: u32,
    pub(super) done: u32,
    pub(super) wait_io: u32,
    pub(super) yield_count: u32,
    pub(super) requeue: u32,
    pub(super) fault: u32,
}
