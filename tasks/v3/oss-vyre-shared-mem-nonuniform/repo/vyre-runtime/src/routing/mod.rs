//! High-level execution routing engine.
//!
//! Substrate-neutral policies that consume [`vyre_foundation::execution_plan::ExecutionPlan`]
//! facts and map them to concrete backend strategies.

use vyre_foundation::execution_plan::ExecutionPlan;

/// Target backend category chosen by the router.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RoutingDecision {
    /// Legacy explicit reference route.
    ///
    /// The standard runtime policy does not select this automatically; callers
    /// that require GPU execution should treat this as an opt-in diagnostic
    /// route, never as an implicit fallback.
    CpuSimd,
    /// Use the default GPU pipeline.
    GpuPipeline,
    /// Use the persistent megakernel.
    PersistentMegakernel,
}

/// Operator-visible routing evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoutingExplanation {
    /// Policy name that made the decision.
    pub policy: &'static str,
    /// Final route selected by the runtime.
    pub decision: RoutingDecision,
    /// Stable short reason for the selected route.
    pub reason: &'static str,
}

/// Pluggable routing policy.
pub trait RoutingPolicy: Send + Sync {
    /// Name of the policy for diagnostics.
    fn name(&self) -> &'static str;

    /// Decide which backend route to take for a given plan.
    fn route(&self, plan: &ExecutionPlan) -> RoutingDecision;

    /// Decide which backend route to take and explain the decision.
    fn route_with_explanation(&self, plan: &ExecutionPlan) -> RoutingExplanation {
        RoutingExplanation {
            policy: self.name(),
            decision: self.route(plan),
            reason: "policy returned route without extended evidence",
        }
    }
}

/// The standard routing engine.
pub struct RoutingEngine {
    policy: Box<dyn RoutingPolicy>,
}

impl RoutingEngine {
    /// Create a new engine with the given policy.
    pub fn new(policy: impl RoutingPolicy + 'static) -> Self {
        Self {
            policy: Box::new(policy),
        }
    }

    /// Route a program to a backend.
    pub fn route(&self, plan: &ExecutionPlan) -> RoutingDecision {
        self.policy.route(plan)
    }

    /// Route a program and return operator-visible evidence.
    pub fn route_with_explanation(&self, plan: &ExecutionPlan) -> RoutingExplanation {
        self.policy.route_with_explanation(plan)
    }
}
pub mod standard_policy;
