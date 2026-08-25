//! Concurrency schedule contracts test suite.

const SCHEDULES: &str = include_str!("../../docs/optimization/CONCURRENCY_SCHEDULE_CONTRACTS.toml");

#[test]
fn concurrency_schedule_contracts_record_shared_state_memory_model_and_forbidden_outcomes() {
    for required in [
        "schedule_id",
        "owning_crate",
        "shared_state",
        "operations",
        "memory_model",
        "preemption_points",
        "state_bound",
        "forbidden_outcome",
        "counterexample_digest",
    ] {
        assert!(
            SCHEDULES.contains(required),
            "concurrency schedule contract must include {required}"
        );
    }
}
