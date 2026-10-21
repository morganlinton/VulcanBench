//! Research supply chain governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const SOURCE_GATES: &str =
    include_str!("../../../docs/optimization/RESEARCH_SOURCE_INTEGRITY_GATES.toml");
const SUPPLY_CHAIN: &str =
    include_str!("../../../docs/optimization/PUBLIC_RELEASE_SUPPLY_CHAIN_PROVENANCE.toml");
const LINKAGE: &str =
    include_str!("../../../docs/optimization/PLAN_RESEARCH_LINKAGE_AUDIT.toml");
const SEAMS: &str =
    include_str!("../../../docs/optimization/DEDUP_SEAM_COMPLETION_AUDIT.toml");

#[test]
fn research_source_ledger_contains_supply_chain_primary_sources() {
    for key in [
        "SLSA_PROVENANCE",
        "SPDX_SBOM",
        "SIGSTORE_REKOR",
        "OPENSSF_SCORECARD",
    ] {
        assert!(
            LEDGER.contains(key),
            "research source ledger must include {key}"
        );
    }

    for required in [
        "official-supply-chain-specification",
        "official-sbom-specification",
        "official-transparency-log-documentation",
        "official-open-source-security-health-tool",
    ] {
        assert!(
            LEDGER.contains(required),
            "research source ledger must classify {required}"
        );
    }
}

#[test]
fn research_source_integrity_gates_require_canonical_urls_and_digest_material() {
    for required in [
        "required_ledger_fields",
        "digest_material",
        "VYRE_RESEARCH_SOURCE_KEY_MISSING",
        "VYRE_RESEARCH_SOURCE_DIGEST_MISMATCH",
        "VYRE_RESEARCH_SOURCE_NOT_PRIMARY",
    ] {
        assert!(
            SOURCE_GATES.contains(required),
            "research source gates must include {required}"
        );
    }
}

#[test]
fn public_release_supply_chain_provenance_separates_public_vyre_from_private_evidence() {
    for required in [
        "provenance_predicate",
        "sbom_document",
        "transparency_entry",
        "scorecard_profile",
        "dependency_risk_record",
        "license_record",
        "signing_identity",
        "public-vyre-only",
        "blocked-from-publication",
    ] {
        assert!(
            SUPPLY_CHAIN.contains(required),
            "supply-chain provenance must include {required}"
        );
    }
}

#[test]
fn plan_research_linkage_connects_vx_rows_to_sources_and_capsules() {
    for required in [
        "vx_id",
        "evidence_token",
        "source_ledger_key",
        "citation_snapshot",
        "provenance_link",
        "private_path_redaction",
        "benchmark_capsule",
    ] {
        assert!(
            LINKAGE.contains(required),
            "plan research linkage audit must include {required}"
        );
    }
}

#[test]
fn dedup_seam_completion_audit_names_owner_registry_and_proof_gate() {
    for required in [
        "seam_id",
        "owning_crate",
        "shared_registry",
        "duplicate_risk",
        "source_impact_edge",
        "proof_gate",
        "completion_evidence",
        "VX-701..VX-720",
    ] {
        assert!(
            SEAMS.contains(required),
            "dedup seam completion audit must include {required}"
        );
    }
}
