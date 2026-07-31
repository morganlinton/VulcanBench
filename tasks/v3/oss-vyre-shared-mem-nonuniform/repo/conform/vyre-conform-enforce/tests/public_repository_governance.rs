//! Public repository governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const IDENTITY: &str =
    include_str!("../../../docs/optimization/PUBLIC_REPOSITORY_IDENTITY_GOVERNANCE.toml");
const RULESETS: &str =
    include_str!("../../../docs/optimization/REPOSITORY_RULESET_RELEASE_PROTECTION.toml");
const SECURITY: &str =
    include_str!("../../../docs/optimization/REPOSITORY_SECURITY_DISCLOSURE_POLICY.toml");
const RELEASES: &str =
    include_str!("../../../docs/optimization/RELEASE_NOTES_CHANGELOG_GOVERNANCE.toml");
const HEALTH: &str =
    include_str!("../../../docs/optimization/PUBLIC_REPOSITORY_HEALTH_GOVERNANCE.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_PUBLIC_REPOSITORY_TRANCHE_COVERAGE.toml");

#[test]
fn public_repository_sources_are_registered() {
    for key in [
        "GITHUB_REPOSITORY_RULESETS",
        "GITHUB_SECURITY_POLICY",
        "GITHUB_REPOSITORY_SECURITY_ADVISORIES",
        "GITHUB_RELEASES",
        "GITHUB_REPOSITORY_TOPICS",
        "KEEP_A_CHANGELOG",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn repository_identity_governance_names_one_public_vyre_repo_and_excludes_santh_private_sync() {
    for required in [
        "repository_id",
        "canonical_public_repository",
        "santhsecurity/vyre",
        "private_repository_policy",
        "backup_repository_policy",
        "outer-santhsecurity/Santh-is-backup-only-not-release-or-sync-authority",
        "visibility_policy",
        "topic_policy",
        "readme_link_policy",
        "metadata_policy",
    ] {
        assert!(
            IDENTITY.contains(required),
            "public repository identity governance must include {required}"
        );
    }
}

#[test]
fn repository_rulesets_protect_main_release_tags_and_public_push_boundary() {
    for required in [
        "ruleset_id",
        "target",
        "enforcement",
        "protected_ref_policy",
        "required_gate_policy",
        "bypass_policy",
        "private_path_policy",
        "release_tag_policy",
        "public-main-branch",
        "public-release-tags",
        "public-push-boundary",
    ] {
        assert!(
            RULESETS.contains(required),
            "repository ruleset release protection must include {required}"
        );
    }
}

#[test]
fn repository_security_disclosure_policy_links_security_md_supported_versions_advisories_and_redaction() {
    for required in [
        "policy_id",
        "security_file_policy",
        "supported_versions_policy",
        "reporting_channel_policy",
        "private_report_policy",
        "advisory_policy",
        "affectedness_policy",
        "public_disclosure_boundary",
        "public-vyre-security-md",
        "private-santh-report-exclusion",
    ] {
        assert!(
            SECURITY.contains(required),
            "repository security disclosure policy must include {required}"
        );
    }
}

#[test]
fn release_notes_changelog_governance_links_versions_tags_notes_changelog_advisories_and_artifacts() {
    for required in [
        "release_record_id",
        "version_policy",
        "tag_policy",
        "release_notes_policy",
        "changelog_policy",
        "security_advisory_link_policy",
        "artifact_link_policy",
        "private_boundary_policy",
        "public-vyre-release",
        "private-local-evidence-summary",
    ] {
        assert!(
            RELEASES.contains(required),
            "release notes changelog governance must include {required}"
        );
    }
}

#[test]
fn public_repository_health_links_readme_topics_security_release_supply_chain_and_boundary_checks() {
    for required in [
        "health_id",
        "readme_policy",
        "topic_policy",
        "security_policy_link",
        "release_policy_link",
        "supply_chain_policy_link",
        "issue_pr_policy",
        "repo_boundary_check",
        "public-vyre-repository-health",
        "private-santh-boundary-health",
    ] {
        assert!(
            HEALTH.contains(required),
            "public repository health governance must include {required}"
        );
    }
}

#[test]
fn plan_contains_public_repository_rows() {
    for row in [
        "VX-1081",
        "VX-1082",
        "VX-1083",
        "VX-1084",
        "VX-1085",
        "VX-1086",
        "VX-1087",
        "VX-1088",
        "VX-1089",
        "VX-1090",
        "VX-1091",
        "VX-1092",
        "VX-1093",
        "VX-1094",
        "VX-1095",
        "VX-1096",
        "VX-1097",
        "VX-1098",
        "VX-1099",
        "VX-1100",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn public_repository_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-1081..VX-1100",
        "public_repository_identity_governance",
        "repository_ruleset_release_protection",
        "repository_security_disclosure_policy",
        "release_notes_changelog_governance",
        "public_repository_health_governance",
        "repo_boundary_publication_checks",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "public repository tranche coverage must include {required}"
        );
    }
}
