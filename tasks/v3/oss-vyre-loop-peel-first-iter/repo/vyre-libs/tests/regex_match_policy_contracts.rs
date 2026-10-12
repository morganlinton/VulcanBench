//! Regex match policy contracts test suite.

const CONTRACTS: &str =
    include_str!("../../docs/optimization/REGEX_MATCH_POLICY_CONTRACTS.toml");

const REQUIRED_POLICIES: &[&str] = &[
    "leftmost_first",
    "leftmost_longest",
    "all_matches",
    "overlapping",
    "start_of_match",
    "vectored_haystack",
];

#[test]
fn regex_match_policy_contracts_cover_required_policies() {
    for policy in REQUIRED_POLICIES {
        assert!(
            CONTRACTS.contains(&format!("policy_id = \"{policy}\"")),
            "Fix: regex match policy contracts must include `{policy}`"
        );
    }
}

#[test]
fn regex_match_policy_contracts_record_offset_and_backend_requirements() {
    for required in [
        "first-accepted-alternative",
        "longest-match-at-leftmost-start",
        "every-non-overlapping-match",
        "every-overlapping-match",
        "start-and-end-offsets",
        "absolute-offsets-over-concatenated-vector",
        "backend-must-support-overlap-or-use-verifier",
        "backend-must-carry-start-offset-state",
    ] {
        assert!(
            CONTRACTS.contains(required),
            "Fix: regex match policy contracts must include `{required}`"
        );
    }
}

#[test]
fn regex_match_policy_contracts_gate_overlap_and_start_of_match() {
    assert!(
        CONTRACTS.contains("policy_id = \"overlapping\"")
            && CONTRACTS.contains("requires_overlap_scan = true")
            && CONTRACTS.contains("VYRE_SCAN_POLICY_OVERLAP_REQUIRES_VERIFIER"),
        "Fix: overlap policy must require overlap support or verifier"
    );
    assert!(
        CONTRACTS.contains("policy_id = \"start_of_match\"")
            && CONTRACTS.contains("requires_start_of_match = true")
            && CONTRACTS.contains("policy_id = \"vectored_haystack\"")
            && CONTRACTS.contains("VYRE_SCAN_POLICY_VECTORED_STATE_REQUIRED"),
        "Fix: start-of-match and vectored policies must require start-offset state"
    );
    let policy_rows = CONTRACTS.matches("[[policy]]").count();
    assert_eq!(
        CONTRACTS
            .matches("evidence_path = \"vyre-libs/tests/regex_match_policy_contracts.rs\"")
            .count(),
        policy_rows,
        "Fix: every match policy row must point at this proof gate"
    );
}
