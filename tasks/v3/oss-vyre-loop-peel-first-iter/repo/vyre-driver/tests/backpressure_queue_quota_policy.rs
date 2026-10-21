//! Backpressure queue quota policy test suite.

const QUOTAS: &str =
    include_str!("../../docs/optimization/BACKPRESSURE_QUEUE_QUOTA_POLICY.toml");

#[test]
fn backpressure_queue_quota_policy_records_inflight_limits_actions_shedding_fairness_and_container_links() {
    for required in [
        "quota_id",
        "queue_class",
        "max_inflight",
        "backpressure_action",
        "load_shedding_policy",
        "fairness_policy",
        "container_limit_link",
        "operator_diagnostic",
        "pause-input-reader",
    ] {
        assert!(
            QUOTAS.contains(required),
            "backpressure queue quota policy must include {required}"
        );
    }
}
