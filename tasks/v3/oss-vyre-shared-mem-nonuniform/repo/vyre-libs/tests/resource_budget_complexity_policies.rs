//! Resource budget complexity policies test suite.

const BUDGETS: &str = include_str!("../../docs/optimization/RESOURCE_BUDGET_POLICY.toml");
const COMPLEXITY: &str =
    include_str!("../../docs/optimization/ALGORITHMIC_COMPLEXITY_DOS_GUARDS.toml");

#[test]
fn resource_budget_policy_records_limits_units_effects_diagnostics_and_metrics() {
    for required in [
        "budget_id",
        "resource_class",
        "tier_a_config",
        "default_limit",
        "hard_limit",
        "measurement_unit",
        "operator_visible_effect",
        "exhaustion_diagnostic",
        "metrics_link",
    ] {
        assert!(
            BUDGETS.contains(required),
            "resource budget policy must include {required}"
        );
    }
}

#[test]
fn algorithmic_complexity_guards_record_claims_triggers_budgets_fallbacks_and_diagnostics() {
    for required in [
        "guard_id",
        "surface",
        "complexity_claim",
        "worst_case_trigger",
        "budget_policy",
        "fallback_policy",
        "proof_artifact",
        "diagnostic",
        "nested ambiguous repetition",
    ] {
        assert!(
            COMPLEXITY.contains(required),
            "algorithmic complexity guard must include {required}"
        );
    }
}
