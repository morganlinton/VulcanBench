//! Regex unicode profiles test suite.

const PROFILES: &str = include_str!("../../docs/optimization/REGEX_UNICODE_PROFILES.toml");

const REQUIRED_PROFILES: &[&str] = &[
    "byte",
    "utf8_scalar",
    "unicode_class",
    "simple_fold",
    "full_fold",
    "normalization_sensitive",
];

#[test]
fn regex_unicode_profiles_cover_required_semantic_modes() {
    for profile in REQUIRED_PROFILES {
        assert!(
            PROFILES.contains(&format!("profile_id = \"{profile}\"")),
            "Fix: regex Unicode profiles must include `{profile}`"
        );
    }
}

#[test]
fn regex_unicode_profiles_separate_byte_gpu_from_unicode_verifier_modes() {
    assert!(
        PROFILES.contains("profile_id = \"byte\"")
            && PROFILES.contains("encoding_contract = \"raw-bytes\"")
            && PROFILES.contains("gpu_eligible = true")
            && PROFILES.contains("verifier_required = false"),
        "Fix: byte Unicode profile must remain raw-byte GPU eligible without verifier"
    );
    assert!(
        PROFILES.matches("verifier_required = true").count() >= 5
            && PROFILES.matches("gpu_eligible = false").count() >= 5,
        "Fix: Unicode scalar/class/fold/normalization profiles must require verifier and reject direct GPU eligibility"
    );
}

#[test]
fn regex_unicode_profiles_record_fold_normalization_and_diagnostics() {
    for required in [
        "case_fold = \"simple\"",
        "case_fold = \"full\"",
        "normalization = \"caller-owned-normalization\"",
        "class_semantics = \"unicode-general-category\"",
        "VYRE_SCAN_UNSUPPORTED_UNICODE_MODE_GPU",
        "VYRE_SCAN_UNSUPPORTED_UNICODE_NORMALIZATION",
    ] {
        assert!(
            PROFILES.contains(required),
            "Fix: regex Unicode profiles must include `{required}`"
        );
    }
    let profile_rows = PROFILES.matches("[[profile]]").count();
    assert_eq!(
        PROFILES
            .matches("evidence_path = \"vyre-libs/tests/regex_unicode_profiles.rs\"")
            .count(),
        profile_rows,
        "Fix: every regex Unicode profile row must point at this proof gate"
    );
}
