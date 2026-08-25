//! Crypto secret governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/CRYPTO_SECRET_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn crypto_secret_primary_sources_are_registered() {
    for key in [
        "NIST_SP_800_57",
        "NIST_SP_800_90A",
        "OWASP_SECRETS_MANAGEMENT",
        "OWASP_CRYPTO_STORAGE",
        "RFC_9106_ARGON2",
        "RFC_8446_TLS13",
        "OWASP_ASVS_CRYPTO",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn crypto_secret_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-881..VX-900",
        "secret_material_policy",
        "crypto_rng_key_lifecycle",
        "constant_time_contracts",
        "password_hashing_derivation_policy",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "crypto secret governance tranche coverage must include {required}"
        );
    }
}
