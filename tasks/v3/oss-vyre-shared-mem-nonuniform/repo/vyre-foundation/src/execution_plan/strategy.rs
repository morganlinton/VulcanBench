use super::{AccuracyPlan, AutotunePlan, FusionPlan, MemoryPlan, ProvenancePlan, SchedulingPolicy};

/// Strategy for whole-program fusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FusionStrategy {
    /// Program is a candidate for fusion with upstream/downstream neighbors.
    Candidate,
    /// Program must execute as an isolated dispatch.
    Isolated,
}

/// Strategy for kernel dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchStrategy {
    /// Execute as a standard one-shot compiled pipeline.
    CompiledPipeline,
    /// Execute as a work-item in a persistent megakernel runtime.
    PersistentRuntime,
}

/// Strategy for accuracy verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccuracyStrategy {
    /// Execute directly without shadow checks.
    Direct,
    /// Run a shadow reference interpreter for high-risk transcendental ops.
    ShadowReference,
}

/// Strategy for hardware-aware autotuning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutotuneStrategy {
    /// Use the declared workgroup size / sharding policy.
    DeclaredShape,
    /// Measure multiple workgroup size variants before choosing a target.
    MeasureVariants,
}

/// Strategy for execution provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvenanceStrategy {
    /// Track minimal required metadata.
    Minimal,
    /// Generate a detailed GPU execution trace for every opcode.
    GpuTrace,
}

/// Strategy for buffer layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStrategy {
    /// Program has no declared buffers.
    Empty,
    /// All buffer sizes are statically declared.
    Static,
    /// At least one buffer size comes from runtime input data.
    Dynamic,
}

/// Strategy for host readback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadbackStrategy {
    /// Read back the full visible output size.
    Full {
        /// Number of bytes read.
        bytes: u64,
    },
    /// Read back only the caller-visible byte range.
    Trimmed {
        /// Bytes copied to the caller.
        visible_bytes: u64,
        /// Bytes skipped by trimming.
        avoided_bytes: u64,
    },
}

impl ReadbackStrategy {
    /// Number of bytes the host will observe after applying this readback strategy.
    #[must_use]
    pub fn visible_bytes(&self) -> u64 {
        match self {
            Self::Full { bytes } => *bytes,
            Self::Trimmed { visible_bytes, .. } => *visible_bytes,
        }
    }
}

/// Concrete strategy selections derived from an execution plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyPlan {
    /// Fusion strategy.
    pub fusion: FusionStrategy,
    /// Dispatch strategy.
    pub dispatch: DispatchStrategy,
    /// Accuracy strategy.
    pub accuracy: AccuracyStrategy,
    /// Autotune strategy.
    pub autotune: AutotuneStrategy,
    /// Provenance strategy.
    pub provenance: ProvenanceStrategy,
    /// Layout strategy.
    pub layout: LayoutStrategy,
    /// Readback strategy.
    pub readback: ReadbackStrategy,
}

impl StrategyPlan {
    pub(super) fn from_parts(
        fusion: &FusionPlan,
        memory: &MemoryPlan,
        provenance: &ProvenancePlan,
        accuracy: &AccuracyPlan,
        autotune: &AutotunePlan,
    ) -> Self {
        let policy = SchedulingPolicy::standard();
        Self {
            fusion: if fusion.batch_fusion_candidate {
                FusionStrategy::Candidate
            } else {
                FusionStrategy::Isolated
            },
            dispatch: if policy.use_persistent_runtime(fusion.node_count) {
                DispatchStrategy::PersistentRuntime
            } else {
                DispatchStrategy::CompiledPipeline
            },
            accuracy: if accuracy.shadow_reference_recommended {
                AccuracyStrategy::ShadowReference
            } else {
                AccuracyStrategy::Direct
            },
            autotune: if autotune.recommended {
                AutotuneStrategy::MeasureVariants
            } else {
                AutotuneStrategy::DeclaredShape
            },
            provenance: if provenance.emit_region_trace {
                ProvenanceStrategy::GpuTrace
            } else {
                ProvenanceStrategy::Minimal
            },
            layout: if memory.dynamic_buffers > 0 {
                LayoutStrategy::Dynamic
            } else if memory.static_bytes > 0 {
                LayoutStrategy::Static
            } else {
                LayoutStrategy::Empty
            },
            readback: if memory.avoided_readback_bytes > 0 {
                ReadbackStrategy::Trimmed {
                    visible_bytes: memory.visible_readback_bytes,
                    avoided_bytes: memory.avoided_readback_bytes,
                }
            } else {
                ReadbackStrategy::Full {
                    bytes: memory.visible_readback_bytes,
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_fusion() -> FusionPlan {
        FusionPlan {
            entry_op_id: None,
            top_level_regions: 0,
            node_count: 10,
            batch_fusion_candidate: false,
        }
    }

    fn baseline_memory() -> MemoryPlan {
        MemoryPlan {
            buffers: Vec::new(),
            static_bytes: 256,
            dynamic_buffers: 0,
            visible_readback_bytes: 256,
            avoided_readback_bytes: 0,
        }
    }

    fn baseline_provenance() -> ProvenancePlan {
        ProvenancePlan {
            top_level_region_wrapped: false,
            region_count: 0,
            emit_region_trace: false,
        }
    }

    fn baseline_accuracy() -> AccuracyPlan {
        AccuracyPlan {
            shadow_reference_recommended: false,
            reason: "baseline",
        }
    }

    fn baseline_autotune() -> AutotunePlan {
        AutotunePlan {
            recommended: false,
            parallel_region_size: [1, 1, 1],
            recommended_workgroup_size: [1, 1, 1],
            recommended_tile: [1, 1, 1],
            recommended_vector_pack_bits: 32,
            recommended_unroll_depth: 1,
            reason: "none",
        }
    }

    fn baseline_strategy() -> StrategyPlan {
        StrategyPlan::from_parts(
            &baseline_fusion(),
            &baseline_memory(),
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        )
    }

    #[test]
    fn baseline_uses_persistent_dispatch() {
        assert_eq!(
            baseline_strategy().dispatch,
            DispatchStrategy::PersistentRuntime
        );
    }

    #[test]
    fn large_program_uses_persistent_dispatch() {
        let mut fusion = baseline_fusion();
        fusion.node_count = 200;
        let s = StrategyPlan::from_parts(
            &fusion,
            &baseline_memory(),
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert_eq!(s.dispatch, DispatchStrategy::PersistentRuntime);
    }

    #[test]
    fn fusion_candidate_enables_fusion_strategy() {
        let mut fusion = baseline_fusion();
        fusion.batch_fusion_candidate = true;
        let s = StrategyPlan::from_parts(
            &fusion,
            &baseline_memory(),
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert_eq!(s.fusion, FusionStrategy::Candidate);
    }

    #[test]
    fn shadow_reference_triggers_accuracy_strategy() {
        let mut accuracy = baseline_accuracy();
        accuracy.shadow_reference_recommended = true;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &baseline_memory(),
            &baseline_provenance(),
            &accuracy,
            &baseline_autotune(),
        );
        assert_eq!(s.accuracy, AccuracyStrategy::ShadowReference);
    }

    #[test]
    fn autotune_recommended_triggers_measure_variants() {
        let mut autotune = baseline_autotune();
        autotune.recommended = true;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &baseline_memory(),
            &baseline_provenance(),
            &baseline_accuracy(),
            &autotune,
        );
        assert_eq!(s.autotune, AutotuneStrategy::MeasureVariants);
    }

    #[test]
    fn region_trace_triggers_gpu_provenance() {
        let mut provenance = baseline_provenance();
        provenance.emit_region_trace = true;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &baseline_memory(),
            &provenance,
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert_eq!(s.provenance, ProvenanceStrategy::GpuTrace);
    }

    #[test]
    fn dynamic_buffers_set_dynamic_layout() {
        let mut memory = baseline_memory();
        memory.dynamic_buffers = 1;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &memory,
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert_eq!(s.layout, LayoutStrategy::Dynamic);
    }

    #[test]
    fn zero_bytes_sets_empty_layout() {
        let mut memory = baseline_memory();
        memory.static_bytes = 0;
        memory.dynamic_buffers = 0;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &memory,
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert_eq!(s.layout, LayoutStrategy::Empty);
    }

    #[test]
    fn trimmed_readback_activates_when_bytes_avoided() {
        let mut memory = baseline_memory();
        memory.avoided_readback_bytes = 100;
        memory.visible_readback_bytes = 156;
        let s = StrategyPlan::from_parts(
            &baseline_fusion(),
            &memory,
            &baseline_provenance(),
            &baseline_accuracy(),
            &baseline_autotune(),
        );
        assert!(matches!(
            s.readback,
            ReadbackStrategy::Trimmed {
                visible_bytes: 156,
                avoided_bytes: 100,
            }
        ));
    }

    #[test]
    fn readback_visible_bytes_works_for_both_variants() {
        let full = ReadbackStrategy::Full { bytes: 1024 };
        assert_eq!(full.visible_bytes(), 1024);

        let trimmed = ReadbackStrategy::Trimmed {
            visible_bytes: 512,
            avoided_bytes: 512,
        };
        assert_eq!(trimmed.visible_bytes(), 512);
    }
}
