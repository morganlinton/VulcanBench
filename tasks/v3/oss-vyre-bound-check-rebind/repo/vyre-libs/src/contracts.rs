//! Shared operation-contract presets for the standard catalog.

use vyre_spec::{CostHint, DeterminismClass, OperationContract, SideEffectClass};

/// Pure deterministic scalar operation with a cheap cost profile.
pub const PURE_DETERMINISTIC_CHEAP: OperationContract = OperationContract {
    capability_requirements: None,
    determinism: Some(DeterminismClass::Deterministic),
    side_effect: Some(SideEffectClass::Pure),
    cost_hint: Some(CostHint::Cheap),
};

/// Pure deterministic rule predicate with a cheap cost profile.
pub const RULE_PREDICATE_CHEAP: OperationContract = OperationContract {
    capability_requirements: None,
    determinism: Some(DeterminismClass::Deterministic),
    side_effect: Some(SideEffectClass::Pure),
    cost_hint: Some(CostHint::Cheap),
};
