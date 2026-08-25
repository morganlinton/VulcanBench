//! Substrate-neutral execution planning for performance and accuracy.

use std::ops::Range;

use crate::ir::{BufferAccess, DataType, MemoryKind, Program};
use crate::optimizer::AdapterCaps;
use crate::program_caps::{self, RequiredCapabilities};
use crate::validate::{validate_with_options, ValidationOptions};

pub mod fusion;
pub mod memory_budget;
mod policy;
mod strategy;
pub use memory_budget::{DeviceMemoryBudget, MemoryBudgetReport};
pub use policy::{PolicyRoute, SchedulingPolicy};
pub use strategy::{
    AccuracyStrategy, AutotuneStrategy, DispatchStrategy, FusionStrategy, LayoutStrategy,
    ProvenanceStrategy, ReadbackStrategy, StrategyPlan,
};

/// Concerns that vyre treats as first-class planning concerns.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum InnovationTrack {
    /// Fuse compatible top-level regions into one dispatch.
    WholeProgramFusion,
    /// Keep execution state GPU-resident across repeated dispatches.
    PersistentExecution,
    /// Run a shadow reference path when precision risk is high.
    DifferentialAccuracy,
    /// Measure shape variants before choosing a dispatch shape.
    ConformanceGuidedAutotune,
    /// Preserve provenance data on the GPU until the caller asks for it.
    GpuResidentProvenance,
    /// Compile buffer layout choices from Program metadata.
    DataLayoutCompiler,
    /// Avoid host readback for buffers the caller cannot observe.
    ReadbackMinimization,
}

/// One track's current recommendation for a program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackDecision {
    /// Planning concern being evaluated.
    pub track: InnovationTrack,
    /// Whether the track should be enabled for this Program.
    pub active: bool,
    /// Short stable explanation for the decision.
    pub reason: &'static str,
}

/// Complete execution plan extracted from a Program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    /// BLAKE3 hash of the canonical VIR wire encoding.
    pub program_fingerprint: [u8; 32],
    /// Capabilities required by this Program's nodes and expressions.
    pub required_capabilities: RequiredCapabilities,
    /// Fusion-related planning facts.
    pub fusion: FusionPlan,
    /// Buffer and readback planning facts.
    pub memory: MemoryPlan,
    /// Region/provenance planning facts.
    pub provenance: ProvenancePlan,
    /// Accuracy and shadow-reference planning facts.
    pub accuracy: AccuracyPlan,
    /// Autotuning planning facts.
    pub autotune: AutotunePlan,
    /// Concrete execution strategies derived from the plan facts.
    pub strategy: StrategyPlan,
    /// Per-track decisions used by dashboards and diagnostics.
    pub tracks: Vec<TrackDecision>,
}

impl ExecutionPlan {
    /// Return whether `track` is active in this plan.
    #[must_use]
    pub fn track_active(&self, track: InnovationTrack) -> bool {
        self.tracks
            .iter()
            .any(|decision| decision.track == track && decision.active)
    }
}

/// Errors that prevent building a trustworthy execution plan.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// Program validation or canonical wire encoding failed.
    #[error("non-canonical program: {source}")]
    NonCanonicalProgram {
        /// Original validation or serialization error.
        source: crate::error::Error,
    },
    /// An output buffer advertises a byte range outside its full allocation.
    #[error(
        "invalid output range for buffer {name}: {start}..{end} exceeds full size {full_size}. Fix: keep output byte ranges ordered and inside the declared buffer size."
    )]
    InvalidOutputRange {
        /// Buffer name.
        name: String,
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
        /// Full buffer size in bytes.
        full_size: u64,
    },
    /// A buffer uses a runtime-sized element type that cannot be represented in
    /// static byte planning.
    #[error(
        "runtime-sized buffer {name} with element type {element:?} cannot be statically planned. Fix: lower the buffer to a concrete fixed-width ABI type before execution planning."
    )]
    RuntimeSizedBuffer {
        /// Buffer name.
        name: String,
        /// Runtime-sized data type.
        element: DataType,
    },
    /// One declared buffer exceeds the selected adapter's per-buffer limit.
    #[error(
        "device memory budget exceeded for buffer {name}: {size_bytes} bytes exceeds per-buffer budget {budget_bytes} on backend {backend}. Fix: shard the buffer, compact the layout, or select a backend with a larger storage-buffer limit."
    )]
    BufferBudgetExceeded {
        /// Backend identifier.
        backend: &'static str,
        /// Buffer name.
        name: String,
        /// Planned static byte size.
        size_bytes: u64,
        /// Per-buffer byte budget.
        budget_bytes: u64,
    },
    /// The full Program exceeds the selected adapter's peak static memory budget.
    #[error(
        "device peak memory budget exceeded: planned {planned_bytes} static bytes exceeds budget {budget_bytes} on backend {backend}. Fix: split the Program, enable resident reuse, or lower the graph in shards before dispatch."
    )]
    PeakBudgetExceeded {
        /// Backend identifier.
        backend: &'static str,
        /// Planned peak static bytes.
        planned_bytes: u64,
        /// Peak static byte budget.
        budget_bytes: u64,
    },
}

/// Build a backend-neutral execution plan with default validation options.
///
/// # Errors
///
/// Returns [`PlanError`] when validation, static memory planning, output-range
/// planning, or canonical fingerprint construction fails for `program`.
pub fn plan(program: &Program) -> Result<ExecutionPlan, PlanError> {
    plan_for_adapter(program, &AdapterCaps::conservative())
}

/// Build a backend-neutral execution plan parameterized by adapter facts.
///
/// # Errors
///
/// Returns [`PlanError`] when `program` cannot be validated or planned for the
/// supplied `adapter_caps`.
pub fn plan_for_adapter(
    program: &Program,
    adapter_caps: &AdapterCaps,
) -> Result<ExecutionPlan, PlanError> {
    plan_with_options_for_adapter(program, ValidationOptions::default(), adapter_caps)
}

/// Build a backend-neutral execution plan after validating with `options`.
///
/// # Errors
///
/// Returns [`PlanError`] when `program` violates `options` or cannot be lowered
/// into a backend-neutral execution plan.
pub fn plan_with_options(
    program: &Program,
    options: ValidationOptions<'_>,
) -> Result<ExecutionPlan, PlanError> {
    plan_with_options_for_adapter(program, options, &AdapterCaps::conservative())
}

/// Build a backend-neutral execution plan after validating with `options` and
/// adapter facts.
///
/// # Errors
///
/// Returns [`PlanError`] when validation, capability scanning, memory planning,
/// provenance planning, or fingerprint construction fails.
pub fn plan_with_options_for_adapter(
    program: &Program,
    options: ValidationOptions<'_>,
    adapter_caps: &AdapterCaps,
) -> Result<ExecutionPlan, PlanError> {
    validate_program_for_plan(program, options)?;
    let required_capabilities = program_caps::scan(program);
    let fusion = fusion_plan(program);
    let memory = memory_plan(program, adapter_caps)?;
    let program_fingerprint = canonical_program_fingerprint(program)?;
    let provenance = provenance_plan(program, &fusion);
    let accuracy = accuracy_plan(&required_capabilities, &provenance);
    let autotune = autotune_plan(program, &required_capabilities, &fusion, adapter_caps);

    let strategy = StrategyPlan::from_parts(&fusion, &memory, &provenance, &accuracy, &autotune);
    let tracks = track_decisions(&fusion, &memory, &provenance, &accuracy, &autotune);

    Ok(ExecutionPlan {
        program_fingerprint,
        required_capabilities,
        fusion,
        memory,
        provenance,
        accuracy,
        autotune,
        strategy,
        tracks,
    })
}

fn validate_program_for_plan(
    program: &Program,
    options: ValidationOptions<'_>,
) -> Result<(), PlanError> {
    if options.backend.is_none()
        && options.backend_capabilities.is_none()
        && program.is_structurally_validated()
    {
        return Ok(());
    }
    let report = validate_with_options(program, options);
    if report.errors.is_empty() {
        return Ok(());
    }
    let message_len = report
        .errors
        .iter()
        .map(|error| error.message().len())
        .sum::<usize>()
        + report.errors.len().saturating_sub(1) * 2;
    let mut messages = String::with_capacity(message_len);
    for (index, error) in report.errors.iter().enumerate() {
        if index != 0 {
            messages.push_str("; ");
        }
        messages.push_str(error.message());
    }
    Err(PlanError::NonCanonicalProgram {
        source: crate::error::Error::WireFormatValidation {
            message: format!(
                "canonical execution plan validation failed: {messages}. Fix: repair the Program before planning."
            ),
        },
    })
}

fn fusion_plan(program: &Program) -> FusionPlan {
    let stats = program.stats();
    FusionPlan {
        entry_op_id: program.entry_op_id().map(ToOwned::to_owned),
        top_level_regions: stats.top_level_regions as usize,
        node_count: stats.node_count,
        batch_fusion_candidate: !program.is_non_composable_with_self()
            && program.is_top_level_region_wrapped(),
    }
}

fn canonical_program_fingerprint(program: &Program) -> Result<[u8; 32], PlanError> {
    let mut wire = Vec::with_capacity(4096);
    program
        .to_wire_into(&mut wire)
        .map_err(|source| PlanError::NonCanonicalProgram { source })?;
    Ok(*blake3::hash(&wire).as_bytes())
}

fn memory_plan(program: &Program, adapter_caps: &AdapterCaps) -> Result<MemoryPlan, PlanError> {
    let mut static_bytes = 0u64;
    let mut visible_readback_bytes = 0u64;
    let mut avoided_readback_bytes = 0u64;
    // Pre-size to the exact buffer count  -  every buffer becomes one
    // BufferPlan entry  -  so the inner push loop never reallocates the
    // backing storage. memory_plan is called once per
    // execution_plan::plan(), which itself is on every backend's
    // pre-dispatch path.
    let mut buffers = Vec::with_capacity(program.buffers().len());
    let mut dynamic_buffers = 0usize;
    for buffer in program.buffers() {
        let count = buffer.count();
        let elem_size =
            buffer
                .element()
                .size_bytes()
                .ok_or_else(|| PlanError::RuntimeSizedBuffer {
                    name: buffer.name().to_string(),
                    element: buffer.element(),
                })? as u64;
        let size = if count > 0 {
            Some(u64::from(count).checked_mul(elem_size).ok_or_else(|| {
                PlanError::NonCanonicalProgram {
                    source: crate::error::Error::WireFormatValidation {
                        message: format!(
                            "canonical execution plan buffer `{}` byte size overflows u64. Fix: split the buffer before planning.",
                            buffer.name()
                        ),
                    },
                }
            })?)
        } else {
            None
        };
        if let Some(s) = size {
            static_bytes = static_bytes.checked_add(s).ok_or_else(|| {
                PlanError::NonCanonicalProgram {
                    source: crate::error::Error::WireFormatValidation {
                        message: format!(
                            "canonical execution plan static memory total overflows u64 while adding `{}`. Fix: split the Program before planning.",
                            buffer.name()
                        ),
                    },
                }
            })?;
        }
        let output_range = buffer.output_byte_range();
        if buffer.is_output() {
            let full_size = size.unwrap_or(0);
            if full_size == 0 {
                return Err(PlanError::NonCanonicalProgram {
                    source: crate::error::Error::WireFormatValidation {
                        message: format!(
                            "canonical execution plan requires static output buffer `{}` size. Fix: set BufferDecl::output(...).with_count(n) before planning.",
                            buffer.name()
                        ),
                    },
                });
            }
            let visible = if let Some(range) = output_range.clone() {
                if range.start > range.end || range.end as u64 > full_size {
                    return Err(PlanError::InvalidOutputRange {
                        name: buffer.name().to_string(),
                        start: range.start,
                        end: range.end,
                        full_size,
                    });
                }
                (range.end - range.start) as u64
            } else {
                full_size
            };
            visible_readback_bytes += visible;
            avoided_readback_bytes += full_size.saturating_sub(visible);
        }
        if buffer.count() == 0 {
            dynamic_buffers += 1;
        }
        buffers.push(BufferPlan {
            name: buffer.name().to_string(),
            binding: buffer.binding(),
            access: buffer.access(),
            kind: buffer.kind(),
            element: buffer.element(),
            count: buffer.count(),
            static_size_bytes: size,
            output_range,
        });
    }
    let plan = MemoryPlan {
        buffers,
        static_bytes,
        dynamic_buffers,
        visible_readback_bytes,
        avoided_readback_bytes,
    };
    DeviceMemoryBudget::from_adapter(adapter_caps).validate(&plan)?;
    Ok(plan)
}

fn provenance_plan(program: &Program, _fusion: &FusionPlan) -> ProvenancePlan {
    ProvenancePlan {
        top_level_region_wrapped: program.is_top_level_region_wrapped(),
        region_count: program.stats().region_count as usize,
        emit_region_trace: program.is_top_level_region_wrapped(),
    }
}

fn accuracy_plan(caps: &RequiredCapabilities, _provenance: &ProvenancePlan) -> AccuracyPlan {
    AccuracyPlan {
        shadow_reference_recommended: caps.subgroup_ops,
        reason: if caps.subgroup_ops {
            "subgroup semantics"
        } else {
            "baseline"
        },
    }
}

fn autotune_plan(
    program: &Program,
    _caps: &RequiredCapabilities,
    _fusion: &FusionPlan,
    adapter_caps: &AdapterCaps,
) -> AutotunePlan {
    let node_count = program.stats().node_count;
    let policy = SchedulingPolicy::standard();
    let problem_size = infer_static_problem_size(program);
    let recommended_workgroup_size = [
        policy.select_workgroup_x(
            program.parallel_region_size()[0],
            problem_size,
            adapter_caps,
        ),
        1,
        1,
    ];
    let recommended_tile =
        policy.select_workgroup_tile(program.parallel_region_size(), problem_size, adapter_caps);
    let recommended_vector_pack_bits = policy.select_vector_pack_bits(32, adapter_caps);
    let recommended_unroll_depth = policy.select_unroll_depth(None, adapter_caps);
    let profile_driven = adapter_caps.ideal_unroll_depth > 0
        || adapter_caps.ideal_vector_pack_bits > 0
        || !adapter_caps.ideal_workgroup_tile.contains(&0);
    let large_program = policy.recommend_autotune(node_count);
    AutotunePlan {
        // Only flag autotuning as recommended when there's a real
        // signal: a large enough program OR a device profile that
        // declares preferred shapes. The previous
        // `recommended_workgroup_size != parallel_region_size` check
        // fired spuriously for tiny declared shapes (e.g. [1, 1, 1])
        // because the policy's min_workgroup_x floor is 32  -  the
        // selector always returns a different number, so every small
        // program got marked autotune-recommended even when it has no
        // measurement variants worth exploring.
        recommended: large_program || profile_driven,
        parallel_region_size: program.parallel_region_size(),
        recommended_workgroup_size,
        recommended_tile,
        recommended_vector_pack_bits,
        recommended_unroll_depth,
        reason: if profile_driven {
            "device profile"
        } else if large_program {
            "large program"
        } else {
            "none"
        },
    }
}

fn infer_static_problem_size(program: &Program) -> Option<u32> {
    program
        .buffers()
        .iter()
        .filter(|buffer| buffer.count() > 0 && !matches!(buffer.kind(), MemoryKind::Shared))
        .map(crate::ir_inner::model::program::BufferDecl::count)
        .min()
}

fn track_decisions(
    fusion: &FusionPlan,
    memory: &MemoryPlan,
    _provenance: &ProvenancePlan,
    accuracy: &AccuracyPlan,
    autotune: &AutotunePlan,
) -> Vec<TrackDecision> {
    vec![
        track_decision(
            InnovationTrack::WholeProgramFusion,
            fusion.batch_fusion_candidate,
            "fusion",
        ),
        track_decision(
            InnovationTrack::PersistentExecution,
            SchedulingPolicy::standard().use_persistent_runtime(fusion.node_count),
            "persistent",
        ),
        track_decision(
            InnovationTrack::DifferentialAccuracy,
            accuracy.shadow_reference_recommended,
            accuracy.reason,
        ),
        track_decision(
            InnovationTrack::ConformanceGuidedAutotune,
            autotune.recommended,
            autotune.reason,
        ),
        track_decision(InnovationTrack::GpuResidentProvenance, false, "none"),
        track_decision(
            InnovationTrack::DataLayoutCompiler,
            memory.static_bytes > 0,
            "layout",
        ),
        track_decision(
            InnovationTrack::ReadbackMinimization,
            memory.avoided_readback_bytes > 0,
            "trimmed readback",
        ),
    ]
}

fn track_decision(track: InnovationTrack, active: bool, reason: &'static str) -> TrackDecision {
    TrackDecision {
        track,
        active,
        reason,
    }
}

/// Region-fusion facts extracted from the Program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FusionPlan {
    /// Optional stable op id for the entry region.
    pub entry_op_id: Option<String>,
    /// Number of top-level regions.
    pub top_level_regions: usize,
    /// Total statement-node count.
    pub node_count: usize,
    /// Whether the Program is eligible for batch fusion.
    pub batch_fusion_candidate: bool,
}

/// Memory allocation and readback facts extracted from buffers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryPlan {
    /// Per-buffer planning facts.
    pub buffers: Vec<BufferPlan>,
    /// Sum of statically declared buffer bytes.
    pub static_bytes: u64,
    /// Number of buffers whose size is known only from runtime inputs.
    pub dynamic_buffers: usize,
    /// Bytes that must be visible to the host after dispatch.
    pub visible_readback_bytes: u64,
    /// Bytes avoided by trimming readback ranges.
    pub avoided_readback_bytes: u64,
}

/// Planning facts for one declared buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BufferPlan {
    /// Buffer name.
    pub name: String,
    /// Binding number.
    pub binding: u32,
    /// Declared access mode.
    pub access: BufferAccess,
    /// Memory address space.
    pub kind: MemoryKind,
    /// Element type.
    pub element: DataType,
    /// Declared element count, or zero for runtime-sized buffers.
    pub count: u32,
    /// Static byte size when `count` is nonzero.
    pub static_size_bytes: Option<u64>,
    /// Caller-visible output byte range, when trimmed.
    pub output_range: Option<Range<usize>>,
}

/// Region/provenance facts used to decide trace strategy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenancePlan {
    /// Whether the entry is wrapped in canonical top-level regions.
    pub top_level_region_wrapped: bool,
    /// Total region count in the Program.
    pub region_count: usize,
    /// Whether the backend should emit a GPU-resident region trace.
    pub emit_region_trace: bool,
}

/// Accuracy strategy facts used for shadow-reference selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccuracyPlan {
    /// Whether a shadow reference pass is recommended.
    pub shadow_reference_recommended: bool,
    /// Stable reason for the recommendation.
    pub reason: &'static str,
}

/// Autotuning facts used for dispatch shape selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutotunePlan {
    /// Whether the backend should measure variants.
    pub recommended: bool,
    /// Declared parallel region size.
    pub parallel_region_size: [u32; 3],
    /// Adapter/profile-selected workgroup size.
    pub recommended_workgroup_size: [u32; 3],
    /// Adapter/profile-selected tile shape for tiled lowering.
    pub recommended_tile: [u32; 3],
    /// Adapter/profile-selected vector pack width in bits.
    pub recommended_vector_pack_bits: u32,
    /// Adapter/profile-selected unroll depth.
    pub recommended_unroll_depth: u32,
    /// Stable reason for the recommendation.
    pub reason: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

    fn trivial_program() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(4),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::load("input", Expr::u32(0)),
            )],
        )
    }

    #[test]
    fn plan_succeeds_on_trivial_program() {
        let p = trivial_program();
        let exec_plan = plan(&p).expect("Fix: plan should succeed on trivial program; restore this invariant before continuing.");
        assert!(exec_plan.memory.static_bytes > 0);
        assert_eq!(exec_plan.memory.dynamic_buffers, 0);
    }

    #[test]
    fn plan_fingerprint_is_deterministic() {
        let p = trivial_program();
        let plan1 = plan(&p).unwrap();
        let plan2 = plan(&p).unwrap();
        assert_eq!(plan1.program_fingerprint, plan2.program_fingerprint);
    }

    #[test]
    fn track_active_returns_false_for_inactive() {
        let p = trivial_program();
        let exec_plan = plan(&p).unwrap();
        // GpuResidentProvenance is always inactive in current implementation
        assert!(!exec_plan.track_active(InnovationTrack::GpuResidentProvenance));
    }

    #[test]
    fn plan_tiny_program_uses_persistent_dispatch() {
        let p = trivial_program();
        let exec_plan = plan(&p).unwrap();
        assert_eq!(
            exec_plan.strategy.dispatch,
            DispatchStrategy::PersistentRuntime
        );
    }

    #[test]
    fn plan_rejects_buffer_over_adapter_budget_before_dispatch() {
        let p = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(8)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        );
        let caps = AdapterCaps {
            backend: "tiny-test-gpu",
            max_storage_buffer_binding_size: 16,
            ..AdapterCaps::conservative()
        };
        let error = plan_for_adapter(&p, &caps)
            .expect_err("oversized buffers must fail during planning, before backend allocation");
        assert!(matches!(
            error,
            PlanError::BufferBudgetExceeded {
                backend: "tiny-test-gpu",
                name,
                size_bytes: 32,
                budget_bytes: 16,
            } if name == "out"
        ));
    }

    #[test]
    fn plan_rejects_peak_static_memory_over_adapter_budget() {
        let names = [
            "b00", "b01", "b02", "b03", "b04", "b05", "b06", "b07", "b08", "b09", "b10", "b11",
            "b12", "b13", "b14", "b15", "b16",
        ];
        let buffers = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                BufferDecl::read_write(*name, index as u32, DataType::U32).with_count(4)
            })
            .collect::<Vec<_>>();
        let p = Program::wrapped(buffers, [1, 1, 1], vec![Node::Return]);
        let caps = AdapterCaps {
            backend: "tiny-test-gpu",
            max_storage_buffer_binding_size: 16,
            ..AdapterCaps::conservative()
        };
        let error = plan_for_adapter(&p, &caps)
            .expect_err("aggregate static memory must fail during planning");
        assert!(matches!(
            error,
            PlanError::PeakBudgetExceeded {
                backend: "tiny-test-gpu",
                planned_bytes: 272,
                budget_bytes: 256,
            }
        ));
    }

    #[test]
    fn device_profile_changes_autotune_recommendations() {
        let p = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [1, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );
        let compact = AdapterCaps {
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            subgroup_size: 32,
            ideal_unroll_depth: 4,
            ideal_vector_pack_bits: 64,
            ideal_workgroup_tile: [8, 8, 1],
            ..AdapterCaps::conservative()
        };
        let wide = AdapterCaps {
            ideal_unroll_depth: 8,
            ideal_vector_pack_bits: 128,
            ideal_workgroup_tile: [16, 16, 1],
            ..compact
        };

        let compact_plan = plan_for_adapter(&p, &compact).unwrap();
        let wide_plan = plan_for_adapter(&p, &wide).unwrap();

        assert_eq!(compact_plan.autotune.recommended_workgroup_size, [64, 1, 1]);
        assert_eq!(wide_plan.autotune.recommended_workgroup_size, [256, 1, 1]);
        assert_eq!(compact_plan.autotune.recommended_tile, [8, 8, 1]);
        assert_eq!(wide_plan.autotune.recommended_tile, [16, 16, 1]);
        assert_eq!(compact_plan.autotune.recommended_vector_pack_bits, 64);
        assert_eq!(wide_plan.autotune.recommended_vector_pack_bits, 128);
        assert_eq!(compact_plan.autotune.recommended_unroll_depth, 4);
        assert_eq!(wide_plan.autotune.recommended_unroll_depth, 8);
    }
}
