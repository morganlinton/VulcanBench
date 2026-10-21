//! C conditional range policy test suite.

const POLICY: &str = include_str!("../../docs/optimization/C_CONDITIONAL_RANGE_POLICY.toml");

#[test]
fn c_conditional_range_policy_blocks_inactive_active_findings() {
    for state in ["active", "inactive", "unknown", "macro_dependent", "error"] {
        assert!(
            POLICY.contains(state),
            "C conditional range policy must define state {state}"
        );
    }

    assert!(POLICY.contains("skip-unless-inactive-code-requested"));
    assert!(POLICY.contains("fail-closed-for-active-findings"));
    assert!(POLICY.contains("active_finding_allowed = false"));
}
