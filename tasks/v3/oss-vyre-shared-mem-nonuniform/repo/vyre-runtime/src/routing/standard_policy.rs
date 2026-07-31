//! Standard routing policies for common compute workloads.

use super::{RoutingDecision, RoutingExplanation, RoutingPolicy};
use vyre_foundation::execution_plan::{ExecutionPlan, PolicyRoute, SchedulingPolicy};

/// Default megakernel-first release policy.
pub struct StandardPolicy;

impl RoutingPolicy for StandardPolicy {
    fn name(&self) -> &'static str {
        "standard-megakernel-first"
    }

    fn route(&self, plan: &ExecutionPlan) -> RoutingDecision {
        self.route_with_explanation(plan).decision
    }

    fn route_with_explanation(&self, plan: &ExecutionPlan) -> RoutingExplanation {
        match SchedulingPolicy::standard().route(plan.fusion.node_count, plan.memory.static_bytes) {
            PolicyRoute::CpuSimd => RoutingExplanation {
                policy: self.name(),
                decision: RoutingDecision::PersistentMegakernel,
                reason: "standard policy overrides CPU SIMD suggestion to persistent megakernel for release execution",
            },
            PolicyRoute::GpuPipeline => RoutingExplanation {
                policy: self.name(),
                decision: RoutingDecision::PersistentMegakernel,
                reason: "standard policy promotes GPU pipeline suggestion to persistent megakernel for resident execution",
            },
            PolicyRoute::PersistentMegakernel => RoutingExplanation {
                policy: self.name(),
                decision: RoutingDecision::PersistentMegakernel,
                reason: "scheduling policy selected persistent megakernel directly",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::execution_plan::{
        AccuracyPlan, AccuracyStrategy, AutotunePlan, AutotuneStrategy, DispatchStrategy,
        ExecutionPlan, FusionPlan, FusionStrategy, LayoutStrategy, MemoryPlan, ProvenancePlan,
        ProvenanceStrategy, ReadbackStrategy, StrategyPlan,
    };
    use vyre_foundation::program_caps::RequiredCapabilities;

    fn plan(node_count: usize, static_bytes: u64) -> ExecutionPlan {
        ExecutionPlan {
            program_fingerprint: [0; 32],
            required_capabilities: RequiredCapabilities::default(),
            fusion: FusionPlan {
                entry_op_id: None,
                top_level_regions: 1,
                node_count,
                batch_fusion_candidate: false,
            },
            memory: MemoryPlan {
                buffers: Vec::new(),
                static_bytes,
                dynamic_buffers: 0,
                visible_readback_bytes: 0,
                avoided_readback_bytes: 0,
            },
            provenance: ProvenancePlan {
                top_level_region_wrapped: true,
                region_count: 1,
                emit_region_trace: false,
            },
            accuracy: AccuracyPlan {
                shadow_reference_recommended: false,
                reason: "test fixture",
            },
            autotune: AutotunePlan {
                recommended: false,
                parallel_region_size: [1, 1, 1],
                recommended_workgroup_size: [1, 1, 1],
                recommended_tile: [1, 1, 1],
                recommended_vector_pack_bits: 32,
                recommended_unroll_depth: 1,
                reason: "test fixture",
            },
            strategy: StrategyPlan {
                fusion: FusionStrategy::Isolated,
                dispatch: DispatchStrategy::PersistentRuntime,
                accuracy: AccuracyStrategy::Direct,
                autotune: AutotuneStrategy::DeclaredShape,
                provenance: ProvenanceStrategy::Minimal,
                layout: LayoutStrategy::Empty,
                readback: ReadbackStrategy::Full { bytes: 0 },
            },
            tracks: Vec::new(),
        }
    }

    #[test]
    fn standard_policy_explains_persistent_megakernel_override() {
        let policy = StandardPolicy;
        let explanation = policy.route_with_explanation(&plan(1, 1));

        assert_eq!(explanation.policy, "standard-megakernel-first");
        assert_eq!(explanation.decision, RoutingDecision::PersistentMegakernel);
        assert!(
            explanation.reason.contains("persistent megakernel"),
            "Fix: routing explanation must expose why persistent execution was selected: {explanation:?}"
        );
    }

    #[test]
    fn routing_engine_exposes_policy_explanation() {
        let engine = crate::routing::RoutingEngine::new(StandardPolicy);
        let explanation = engine.route_with_explanation(&plan(128, 1 << 20));

        assert_eq!(explanation.policy, "standard-megakernel-first");
        assert_eq!(explanation.decision, RoutingDecision::PersistentMegakernel);
        assert!(!explanation.reason.is_empty());
    }
}
