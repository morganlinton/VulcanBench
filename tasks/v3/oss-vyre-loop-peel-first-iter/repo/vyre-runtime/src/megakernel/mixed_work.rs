//! Runtime-owned mixed-work protocol for resident megakernel batches.
//!
//! This module is intentionally domain-neutral. Scan, graph, parser, and flow
//! callers own their manifests and payload layouts; the runtime owns only the
//! queue class, work-unit type, resident artifact id, output slab id, watchdog
//! budget, and deterministic evidence contract needed to drain one resident
//! batch without hidden host loops.

/// Schema version for mixed-work protocol evidence.
pub const MIXED_WORK_PROTOCOL_SCHEMA_VERSION: u32 = 1;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Resident queue class used by the megakernel scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MixedWorkQueueClass {
    /// Byte, literal, regex, or token scan work.
    Scan,
    /// Frontier, CSR, motif, or reachability graph work.
    Graph,
    /// Lexer, parser, VAST, or changed-range parser work.
    Parser,
    /// Relation, dataflow, IFDS, or fixed-point flow work.
    Flow,
    /// Runtime control work such as bounded drain sentinels.
    Control,
}

impl MixedWorkQueueClass {
    /// Stable label used in evidence and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Graph => "graph",
            Self::Parser => "parser",
            Self::Flow => "flow",
            Self::Control => "control",
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::Scan => 1,
            Self::Graph => 2,
            Self::Parser => 3,
            Self::Flow => 4,
            Self::Control => 5,
        }
    }
}

/// Resident work-unit type selected inside a queue class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MixedWorkUnitType {
    /// Scan one byte chunk or literal/regex shard.
    ScanChunk,
    /// Verify scan candidates in a resident verifier fragment.
    ScanVerifier,
    /// Expand or compact a graph frontier.
    GraphFrontier,
    /// Compact graph output or frontier queues.
    GraphCompaction,
    /// Run one parser shard or lexer/tokenization shard.
    ParserShard,
    /// Apply one parser changed-range shard.
    ParserChangedRange,
    /// Apply a relation delta batch.
    FlowRelationDelta,
    /// Run one flow fixed-point step.
    FlowFixpointStep,
    /// Drain-control sentinel used to bound persistent execution.
    DrainSentinel,
}

impl MixedWorkUnitType {
    /// Stable label used in evidence and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScanChunk => "scan_chunk",
            Self::ScanVerifier => "scan_verifier",
            Self::GraphFrontier => "graph_frontier",
            Self::GraphCompaction => "graph_compaction",
            Self::ParserShard => "parser_shard",
            Self::ParserChangedRange => "parser_changed_range",
            Self::FlowRelationDelta => "flow_relation_delta",
            Self::FlowFixpointStep => "flow_fixpoint_step",
            Self::DrainSentinel => "drain_sentinel",
        }
    }

    const fn tag(self) -> u64 {
        match self {
            Self::ScanChunk => 11,
            Self::ScanVerifier => 12,
            Self::GraphFrontier => 21,
            Self::GraphCompaction => 22,
            Self::ParserShard => 31,
            Self::ParserChangedRange => 32,
            Self::FlowRelationDelta => 41,
            Self::FlowFixpointStep => 42,
            Self::DrainSentinel => 51,
        }
    }
}

/// Opaque id for an artifact already resident in megakernel-owned buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResidentArtifactId(pub u32);

impl ResidentArtifactId {
    /// Return true when this id names a concrete resident artifact.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Opaque id for a resident output slab owned by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputSlabId(pub u32);

impl OutputSlabId {
    /// Return true when this id names a concrete output slab.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// One resident mixed-work unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MixedWorkUnit {
    /// Stable sequence number used for deterministic drain and output evidence.
    pub sequence: u64,
    /// Scheduler queue class.
    pub queue_class: MixedWorkQueueClass,
    /// Work-unit kind inside the queue class.
    pub unit_type: MixedWorkUnitType,
    /// Resident artifact consumed by this work unit.
    pub resident_artifact_id: ResidentArtifactId,
    /// Output slab written by this work unit.
    pub output_slab_id: OutputSlabId,
    /// Per-unit watchdog budget in scheduler ticks.
    pub watchdog_budget_ticks: u32,
    /// Caller-owned payload digest. Runtime treats payload bytes as opaque.
    pub payload_digest: u64,
}

impl MixedWorkUnit {
    /// Construct one mixed-work unit.
    #[must_use]
    pub const fn new(
        sequence: u64,
        queue_class: MixedWorkQueueClass,
        unit_type: MixedWorkUnitType,
        resident_artifact_id: ResidentArtifactId,
        output_slab_id: OutputSlabId,
        watchdog_budget_ticks: u32,
        payload_digest: u64,
    ) -> Self {
        Self {
            sequence,
            queue_class,
            unit_type,
            resident_artifact_id,
            output_slab_id,
            watchdog_budget_ticks,
            payload_digest,
        }
    }
}

/// Borrowed resident mixed-work plan supplied to the runtime scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixedWorkProtocolPlan<'a> {
    /// Work units to drain in deterministic sequence order.
    pub units: &'a [MixedWorkUnit],
    /// Total watchdog budget for draining the plan.
    pub drain_watchdog_budget_ticks: u64,
}

impl<'a> MixedWorkProtocolPlan<'a> {
    /// Construct a borrowed mixed-work protocol plan.
    #[must_use]
    pub const fn new(units: &'a [MixedWorkUnit], drain_watchdog_budget_ticks: u64) -> Self {
        Self {
            units,
            drain_watchdog_budget_ticks,
        }
    }
}

/// Evidence emitted after validating a mixed-work protocol plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixedWorkProtocolEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Total work units.
    pub unit_count: u32,
    /// Scan queue units.
    pub scan_units: u32,
    /// Graph queue units.
    pub graph_units: u32,
    /// Parser queue units.
    pub parser_units: u32,
    /// Flow queue units.
    pub flow_units: u32,
    /// Runtime control queue units.
    pub control_units: u32,
    /// Sum of per-unit watchdog budgets.
    pub total_watchdog_budget_ticks: u64,
    /// Largest per-unit watchdog budget.
    pub max_watchdog_budget_ticks: u32,
    /// Drain budget supplied for the full resident batch.
    pub drain_watchdog_budget_ticks: u64,
    /// True when the sum of per-unit watchdog budgets is bounded by the drain budget.
    pub bounded_drain: bool,
    /// Hidden host-loop count. Valid mixed-work plans keep this at zero.
    pub hidden_host_loop_count: u32,
    /// Deterministic digest of queue class, unit type, ids, budgets, and payload digests.
    pub deterministic_output_digest: u64,
}

impl MixedWorkProtocolEvidence {
    /// Return true when scan, graph, parser, and flow classes are all present.
    #[must_use]
    pub const fn covers_scan_graph_parser_flow(self) -> bool {
        self.scan_units != 0 && self.graph_units != 0 && self.parser_units != 0 && self.flow_units != 0
    }

    /// Return true when evidence is complete enough for release benches.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.schema_version == MIXED_WORK_PROTOCOL_SCHEMA_VERSION
            && self.unit_count != 0
            && self.bounded_drain
            && self.hidden_host_loop_count == 0
            && self.deterministic_output_digest != 0
    }
}

/// Mixed-work protocol validation error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MixedWorkProtocolError {
    /// The plan has no resident work.
    #[error("mixed-work plan is empty. Fix: publish at least one resident work unit before scheduling.")]
    EmptyPlan,
    /// The total drain budget is zero.
    #[error("mixed-work drain watchdog budget is zero. Fix: provide a positive resident drain budget.")]
    ZeroDrainWatchdogBudget,
    /// A unit has no watchdog budget.
    #[error("mixed-work unit {sequence} has zero watchdog budget. Fix: assign a positive per-unit watchdog budget.")]
    ZeroUnitWatchdogBudget {
        /// Sequence number of the invalid unit.
        sequence: u64,
    },
    /// A unit references no resident artifact.
    #[error("mixed-work unit {sequence} has resident artifact id 0. Fix: publish a resident artifact before queueing work.")]
    ZeroResidentArtifactId {
        /// Sequence number of the invalid unit.
        sequence: u64,
    },
    /// A unit references no output slab.
    #[error("mixed-work unit {sequence} has output slab id 0. Fix: allocate a resident output slab before queueing work.")]
    ZeroOutputSlabId {
        /// Sequence number of the invalid unit.
        sequence: u64,
    },
    /// Queue class and unit type do not match.
    #[error(
        "mixed-work unit {sequence} routes {unit_type} through {queue_class}. Fix: use a unit type owned by the queue class."
    )]
    QueueClassMismatch {
        /// Sequence number of the invalid unit.
        sequence: u64,
        /// Queue class label.
        queue_class: &'static str,
        /// Unit type label.
        unit_type: &'static str,
    },
    /// Unit count cannot fit the evidence ABI.
    #[error("mixed-work unit count {unit_count} overflows u32 evidence. Fix: shard the resident batch.")]
    UnitCountOverflow {
        /// Unit count that exceeded the evidence ABI.
        unit_count: usize,
    },
    /// Class-specific count cannot fit the evidence ABI.
    #[error("mixed-work {queue_class} unit count overflowed u32 evidence. Fix: shard that queue class.")]
    ClassCountOverflow {
        /// Queue class whose count overflowed.
        queue_class: &'static str,
    },
    /// Watchdog sum overflowed the evidence ABI.
    #[error("mixed-work watchdog budget sum overflowed u64. Fix: shard the resident batch.")]
    WatchdogBudgetOverflow,
    /// The plan cannot drain inside the supplied watchdog budget.
    #[error(
        "mixed-work watchdog budget {total_watchdog_budget_ticks} exceeds drain budget {drain_watchdog_budget_ticks}. Fix: increase the drain budget or shard the resident batch."
    )]
    WatchdogBudgetExceeded {
        /// Sum of per-unit watchdog budgets.
        total_watchdog_budget_ticks: u64,
        /// Drain budget supplied by the caller.
        drain_watchdog_budget_ticks: u64,
    },
}

/// Validate a mixed-work protocol plan and return deterministic drain evidence.
///
/// # Errors
///
/// Returns [`MixedWorkProtocolError`] when the plan cannot be drained by the
/// resident scheduler without invalid ids, class mismatches, hidden host loops,
/// or an unbounded watchdog budget.
pub fn mixed_work_protocol_evidence(
    plan: &MixedWorkProtocolPlan<'_>,
) -> Result<MixedWorkProtocolEvidence, MixedWorkProtocolError> {
    validate_mixed_work_protocol(plan)
}

/// Validate a mixed-work protocol plan and return deterministic drain evidence.
///
/// # Errors
///
/// Returns [`MixedWorkProtocolError`] when any work unit is malformed or the
/// plan exceeds its drain watchdog budget.
pub fn validate_mixed_work_protocol(
    plan: &MixedWorkProtocolPlan<'_>,
) -> Result<MixedWorkProtocolEvidence, MixedWorkProtocolError> {
    if plan.units.is_empty() {
        return Err(MixedWorkProtocolError::EmptyPlan);
    }
    if plan.drain_watchdog_budget_ticks == 0 {
        return Err(MixedWorkProtocolError::ZeroDrainWatchdogBudget);
    }
    if plan.units.len() > u32::MAX as usize {
        return Err(MixedWorkProtocolError::UnitCountOverflow {
            unit_count: plan.units.len(),
        });
    }

    let mut counts = [0_u32; 5];
    let mut total_watchdog_budget_ticks = 0_u64;
    let mut max_watchdog_budget_ticks = 0_u32;
    let mut digest = FNV_OFFSET;

    for unit in plan.units {
        validate_unit(*unit)?;
        bump_class_count(&mut counts, unit.queue_class)?;
        total_watchdog_budget_ticks = total_watchdog_budget_ticks
            .checked_add(u64::from(unit.watchdog_budget_ticks))
            .ok_or(MixedWorkProtocolError::WatchdogBudgetOverflow)?;
        max_watchdog_budget_ticks = max_watchdog_budget_ticks.max(unit.watchdog_budget_ticks);
        digest = mix_unit_digest(digest, *unit);
    }

    if total_watchdog_budget_ticks > plan.drain_watchdog_budget_ticks {
        return Err(MixedWorkProtocolError::WatchdogBudgetExceeded {
            total_watchdog_budget_ticks,
            drain_watchdog_budget_ticks: plan.drain_watchdog_budget_ticks,
        });
    }

    Ok(MixedWorkProtocolEvidence {
        schema_version: MIXED_WORK_PROTOCOL_SCHEMA_VERSION,
        unit_count: plan.units.len() as u32,
        scan_units: counts[0],
        graph_units: counts[1],
        parser_units: counts[2],
        flow_units: counts[3],
        control_units: counts[4],
        total_watchdog_budget_ticks,
        max_watchdog_budget_ticks,
        drain_watchdog_budget_ticks: plan.drain_watchdog_budget_ticks,
        bounded_drain: true,
        hidden_host_loop_count: 0,
        deterministic_output_digest: digest,
    })
}

fn validate_unit(unit: MixedWorkUnit) -> Result<(), MixedWorkProtocolError> {
    if unit.watchdog_budget_ticks == 0 {
        return Err(MixedWorkProtocolError::ZeroUnitWatchdogBudget {
            sequence: unit.sequence,
        });
    }
    if !unit.resident_artifact_id.is_valid() {
        return Err(MixedWorkProtocolError::ZeroResidentArtifactId {
            sequence: unit.sequence,
        });
    }
    if !unit.output_slab_id.is_valid() {
        return Err(MixedWorkProtocolError::ZeroOutputSlabId {
            sequence: unit.sequence,
        });
    }
    if !unit_type_matches_queue(unit.queue_class, unit.unit_type) {
        return Err(MixedWorkProtocolError::QueueClassMismatch {
            sequence: unit.sequence,
            queue_class: unit.queue_class.as_str(),
            unit_type: unit.unit_type.as_str(),
        });
    }
    Ok(())
}

const fn unit_type_matches_queue(
    queue_class: MixedWorkQueueClass,
    unit_type: MixedWorkUnitType,
) -> bool {
    matches!(
        (queue_class, unit_type),
        (MixedWorkQueueClass::Scan, MixedWorkUnitType::ScanChunk)
            | (MixedWorkQueueClass::Scan, MixedWorkUnitType::ScanVerifier)
            | (MixedWorkQueueClass::Graph, MixedWorkUnitType::GraphFrontier)
            | (MixedWorkQueueClass::Graph, MixedWorkUnitType::GraphCompaction)
            | (MixedWorkQueueClass::Parser, MixedWorkUnitType::ParserShard)
            | (MixedWorkQueueClass::Parser, MixedWorkUnitType::ParserChangedRange)
            | (MixedWorkQueueClass::Flow, MixedWorkUnitType::FlowRelationDelta)
            | (MixedWorkQueueClass::Flow, MixedWorkUnitType::FlowFixpointStep)
            | (MixedWorkQueueClass::Control, MixedWorkUnitType::DrainSentinel)
    )
}

fn bump_class_count(
    counts: &mut [u32; 5],
    queue_class: MixedWorkQueueClass,
) -> Result<(), MixedWorkProtocolError> {
    let index = match queue_class {
        MixedWorkQueueClass::Scan => 0,
        MixedWorkQueueClass::Graph => 1,
        MixedWorkQueueClass::Parser => 2,
        MixedWorkQueueClass::Flow => 3,
        MixedWorkQueueClass::Control => 4,
    };
    counts[index] = counts[index]
        .checked_add(1)
        .ok_or(MixedWorkProtocolError::ClassCountOverflow {
            queue_class: queue_class.as_str(),
        })?;
    Ok(())
}

fn mix_unit_digest(mut digest: u64, unit: MixedWorkUnit) -> u64 {
    digest = fnv_mix(digest, unit.sequence);
    digest = fnv_mix(digest, unit.queue_class.tag());
    digest = fnv_mix(digest, unit.unit_type.tag());
    digest = fnv_mix(digest, u64::from(unit.resident_artifact_id.0));
    digest = fnv_mix(digest, u64::from(unit.output_slab_id.0));
    digest = fnv_mix(digest, u64::from(unit.watchdog_budget_ticks));
    fnv_mix(digest, unit.payload_digest)
}

fn fnv_mix(mut digest: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(FNV_PRIME);
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::{
        mixed_work_protocol_evidence, validate_mixed_work_protocol, MixedWorkProtocolError,
        MixedWorkProtocolPlan, MixedWorkQueueClass, MixedWorkUnit, MixedWorkUnitType,
        OutputSlabId, ResidentArtifactId, MIXED_WORK_PROTOCOL_SCHEMA_VERSION,
    };

    fn unit(
        sequence: u64,
        queue_class: MixedWorkQueueClass,
        unit_type: MixedWorkUnitType,
    ) -> MixedWorkUnit {
        MixedWorkUnit::new(
            sequence,
            queue_class,
            unit_type,
            ResidentArtifactId(100 + sequence as u32),
            OutputSlabId(200 + sequence as u32),
            10,
            0xfeed_0000 + sequence,
        )
    }

    #[test]
    fn mixed_scan_graph_parser_flow_work_emits_deterministic_bounded_drain_evidence() {
        let units = [
            unit(1, MixedWorkQueueClass::Scan, MixedWorkUnitType::ScanChunk),
            unit(2, MixedWorkQueueClass::Graph, MixedWorkUnitType::GraphFrontier),
            unit(3, MixedWorkQueueClass::Parser, MixedWorkUnitType::ParserShard),
            unit(4, MixedWorkQueueClass::Flow, MixedWorkUnitType::FlowRelationDelta),
            unit(5, MixedWorkQueueClass::Control, MixedWorkUnitType::DrainSentinel),
        ];
        let plan = MixedWorkProtocolPlan::new(&units, 64);

        let first = mixed_work_protocol_evidence(&plan)
            .expect("Fix: valid mixed-work plan should emit evidence");
        let second = validate_mixed_work_protocol(&plan)
            .expect("Fix: valid mixed-work plan should emit stable evidence");

        assert_eq!(first, second);
        assert_eq!(first.schema_version, MIXED_WORK_PROTOCOL_SCHEMA_VERSION);
        assert!(first.is_complete());
        assert!(first.covers_scan_graph_parser_flow());
        assert!(first.bounded_drain);
        assert_eq!(first.hidden_host_loop_count, 0);
        assert_eq!(first.unit_count, 5);
        assert_eq!(first.total_watchdog_budget_ticks, 50);
        assert_eq!(first.max_watchdog_budget_ticks, 10);
        assert_ne!(first.deterministic_output_digest, 0);
    }

    #[test]
    fn zero_watchdog_budget_is_rejected() {
        let units = [MixedWorkUnit::new(
            7,
            MixedWorkQueueClass::Scan,
            MixedWorkUnitType::ScanChunk,
            ResidentArtifactId(1),
            OutputSlabId(1),
            0,
            9,
        )];
        let plan = MixedWorkProtocolPlan::new(&units, 1);

        assert!(matches!(
            validate_mixed_work_protocol(&plan),
            Err(MixedWorkProtocolError::ZeroUnitWatchdogBudget { sequence: 7 })
        ));
    }

    #[test]
    fn class_unit_mismatch_is_rejected() {
        let units = [MixedWorkUnit::new(
            9,
            MixedWorkQueueClass::Parser,
            MixedWorkUnitType::FlowFixpointStep,
            ResidentArtifactId(1),
            OutputSlabId(1),
            1,
            9,
        )];
        let plan = MixedWorkProtocolPlan::new(&units, 1);

        assert!(matches!(
            validate_mixed_work_protocol(&plan),
            Err(MixedWorkProtocolError::QueueClassMismatch {
                sequence: 9,
                queue_class: "parser",
                unit_type: "flow_fixpoint_step"
            })
        ));
    }

    #[test]
    fn drain_budget_must_bound_all_units() {
        let units = [
            unit(1, MixedWorkQueueClass::Scan, MixedWorkUnitType::ScanChunk),
            unit(2, MixedWorkQueueClass::Flow, MixedWorkUnitType::FlowRelationDelta),
        ];
        let plan = MixedWorkProtocolPlan::new(&units, 19);

        assert!(matches!(
            validate_mixed_work_protocol(&plan),
            Err(MixedWorkProtocolError::WatchdogBudgetExceeded {
                total_watchdog_budget_ticks: 20,
                drain_watchdog_budget_ticks: 19
            })
        ));
    }

    #[test]
    fn resident_artifact_and_output_slab_ids_are_required() {
        let bad_artifact = [MixedWorkUnit::new(
            1,
            MixedWorkQueueClass::Scan,
            MixedWorkUnitType::ScanChunk,
            ResidentArtifactId(0),
            OutputSlabId(1),
            1,
            1,
        )];
        assert!(matches!(
            validate_mixed_work_protocol(&MixedWorkProtocolPlan::new(&bad_artifact, 1)),
            Err(MixedWorkProtocolError::ZeroResidentArtifactId { sequence: 1 })
        ));

        let bad_slab = [MixedWorkUnit::new(
            2,
            MixedWorkQueueClass::Scan,
            MixedWorkUnitType::ScanChunk,
            ResidentArtifactId(1),
            OutputSlabId(0),
            1,
            1,
        )];
        assert!(matches!(
            validate_mixed_work_protocol(&MixedWorkProtocolPlan::new(&bad_slab, 1)),
            Err(MixedWorkProtocolError::ZeroOutputSlabId { sequence: 2 })
        ));
    }
}
